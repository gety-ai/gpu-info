use windows::{
    core::Interface,
    Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter1, IDXGIDevice, IDXGIFactory1, DXGI_ADAPTER_DESC1,
        DXGI_ADAPTER_FLAG_REMOTE, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND,
    },
};

use crate::{vendor_name, GPUKind, GPU};

const MIB: u64 = 1024 * 1024;

/// Adapters with less dedicated memory than this are treated as integrated.
/// DXGI has no integrated/discrete flag; integrated GPUs only reserve a small
/// carve-out (typically 128-512 MiB) while discrete GPUs have at least 2 GiB.
const INTEGRATED_MAX_DEDICATED_MEMORY: u64 = 1024 * MIB;

const NVIDIA_VENDOR_ID: u32 = 0x10DE;

#[derive(Debug, thiserror::Error)]
pub enum DxgiError {
    #[error("Failed to perform DXGI operation: {0}")]
    OperationFailed(#[from] windows::core::Error),
}

/// Enumerates hardware adapters through DXGI.
///
/// DXGI is used instead of Vulkan on Windows because creating a Vulkan instance
/// loads the vendor ICD and every implicit layer installed on the system
/// (overlays, capture hooks, ...), none of which can be unloaded afterwards.
pub fn retrieve_gpu_info_via_dxgi() -> Result<Vec<GPU>, DxgiError> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }?;

    let mut gpus = Vec::new();
    for index in 0.. {
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(adapter) => adapter,
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(e) => return Err(e.into()),
        };
        let desc = unsafe { adapter.GetDesc1() }?;
        if is_software_adapter(desc.Flags) {
            continue;
        }
        gpus.push(gpu_from_desc(&desc, umd_version(&adapter)));
    }

    Ok(gpus)
}

fn gpu_from_desc(desc: &DXGI_ADAPTER_DESC1, umd_version: Option<u64>) -> GPU {
    let name_len = desc
        .Description
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(desc.Description.len());
    let dedicated_memory = desc.DedicatedVideoMemory as u64;

    GPU {
        kind: adapter_kind(desc.Flags, dedicated_memory),
        name: String::from_utf16_lossy(&desc.Description[..name_len]),
        vendor: vendor_name(desc.VendorId).to_string(),
        driver_version: umd_version
            .map(|version| format_driver_version(desc.VendorId, version))
            .unwrap_or_default(),
        vram: dedicated_memory / MIB,
        clock_speed: None,
        temperature: None,
    }
}

fn is_software_adapter(flags: u32) -> bool {
    flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0
}

fn adapter_kind(flags: u32, dedicated_memory: u64) -> GPUKind {
    if flags & DXGI_ADAPTER_FLAG_REMOTE.0 as u32 != 0 {
        GPUKind::Virtual
    } else if dedicated_memory < INTEGRATED_MAX_DEDICATED_MEMORY {
        GPUKind::Integrated
    } else {
        GPUKind::Discrete
    }
}

/// Returns the user-mode driver version packed as four 16-bit components.
fn umd_version(adapter: &IDXGIAdapter1) -> Option<u64> {
    unsafe { adapter.CheckInterfaceSupport(&IDXGIDevice::IID) }
        .ok()
        .map(|version| version as u64)
}

/// Formats the packed UMD version, e.g. `32.0.16.1742`.
///
/// NVIDIA publishes its drivers under a shorter number made of the last five
/// digits of the UMD version, so `32.0.16.1742` becomes `617.42`.
fn format_driver_version(vendor_id: u32, version: u64) -> String {
    let [major, minor, build, revision] = [48, 32, 16, 0].map(|shift| (version >> shift) & 0xFFFF);
    if vendor_id == NVIDIA_VENDOR_ID {
        let release = (build % 10) * 10_000 + revision;
        format!("{}.{:02}", release / 100, release % 100)
    } else {
        format!("{major}.{minor}.{build}.{revision}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    fn pack(components: [u64; 4]) -> u64 {
        components[0] << 48 | components[1] << 32 | components[2] << 16 | components[3]
    }

    fn desc(name: &str, vendor_id: u32, flags: u32, dedicated_memory: u64) -> DXGI_ADAPTER_DESC1 {
        let mut desc = DXGI_ADAPTER_DESC1 {
            VendorId: vendor_id,
            Flags: flags,
            DedicatedVideoMemory: dedicated_memory as usize,
            ..Default::default()
        };
        for (dst, src) in desc.Description.iter_mut().zip(name.encode_utf16()) {
            *dst = src;
        }
        desc
    }

    #[test]
    fn test_format_nvidia_driver_version() {
        assert_eq!(
            format_driver_version(0x10DE, pack([32, 0, 16, 1742])),
            "617.42"
        );
        assert_eq!(
            format_driver_version(0x10DE, pack([31, 0, 15, 5222])),
            "552.22"
        );
        assert_eq!(
            format_driver_version(0x10DE, pack([32, 0, 15, 6094])),
            "560.94"
        );
        assert_eq!(
            format_driver_version(0x10DE, pack([32, 0, 16, 42])),
            "600.42"
        );
    }

    #[test]
    fn test_format_other_driver_version() {
        assert_eq!(
            format_driver_version(0x8086, pack([32, 0, 101, 6129])),
            "32.0.101.6129"
        );
        assert_eq!(
            format_driver_version(0x1002, pack([31, 0, 24033, 1003])),
            "31.0.24033.1003"
        );
    }

    #[test]
    fn test_software_adapter_is_skipped() {
        assert!(is_software_adapter(DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32));
        assert!(!is_software_adapter(0));
        assert!(!is_software_adapter(DXGI_ADAPTER_FLAG_REMOTE.0 as u32));
    }

    #[test]
    fn test_gpu_from_desc() {
        let gpu = gpu_from_desc(
            &desc("NVIDIA GeForce RTX 4090", 0x10DE, 0, 24_138 * MIB),
            Some(pack([32, 0, 16, 1742])),
        );
        assert_eq!(gpu.name, "NVIDIA GeForce RTX 4090");
        assert_eq!(gpu.vendor, "NVIDIA");
        assert_eq!(gpu.driver_version, "617.42");
        assert_eq!(gpu.vram, 24_138);
        assert_eq!(gpu.kind, GPUKind::Discrete);

        let gpu = gpu_from_desc(&desc("Intel(R) UHD Graphics", 0x8086, 0, 128 * MIB), None);
        assert_eq!(gpu.vendor, "Intel");
        assert_eq!(gpu.driver_version, "");
        assert_eq!(gpu.kind, GPUKind::Integrated);

        let gpu = gpu_from_desc(
            &desc("Remote", 0x1234, DXGI_ADAPTER_FLAG_REMOTE.0 as u32, 0),
            None,
        );
        assert_eq!(gpu.vendor, "Unknown");
        assert_eq!(gpu.kind, GPUKind::Virtual);
    }

    #[test]
    fn test_retrieve_gpu_info_via_dxgi() {
        let gpus = retrieve_gpu_info_via_dxgi().unwrap();
        eprintln!("{gpus:#?}");
        assert!(gpus
            .iter()
            .all(|gpu| gpu.name != "Microsoft Basic Render Driver"));
    }
}
