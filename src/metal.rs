use crate::{GPUKind, GPULocation};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{MTLCopyAllDevices, MTLDevice, MTLDeviceLocation, MTLSize};

#[derive(Debug, thiserror::Error)]
pub enum MetalError {
    #[error("Metal is not supported on this platform")]
    NotSupported,
}

impl MetalError {
    pub fn is_not_supported(&self) -> bool {
        matches!(self, MetalError::NotSupported)
    }
}

#[derive(Debug, Clone)]
pub struct MetalGpu {
    pub kind: GPUKind,
    pub name: String,
    pub vendor: String,
    // pub driver_version: String,
    pub vram: u64, // MB
    pub is_removable: bool,
    pub is_headless: bool,
    pub registry_id: u64,
    pub location: GPULocation,
    pub has_unified_memory: bool,
    pub max_threads_per_threadgroup: MaxThreadsPerThreadgroup,
    pub recommended_max_working_set: u64, // bytes
}

impl From<MetalGpu> for super::GPU {
    fn from(gpu: MetalGpu) -> Self {
        Self {
            kind: gpu.kind,
            name: gpu.name,
            vendor: gpu.vendor,
            driver_version: "Unknown".to_string(),
            vram: gpu.vram,
            clock_speed: None,
            temperature: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MaxThreadsPerThreadgroup {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
}

impl From<MTLSize> for MaxThreadsPerThreadgroup {
    fn from(size: MTLSize) -> Self {
        Self {
            width: size.width as usize,
            height: size.height as usize,
            depth: size.depth as usize,
        }
    }
}

impl From<MTLDeviceLocation> for GPULocation {
    fn from(location: MTLDeviceLocation) -> Self {
        match location {
            MTLDeviceLocation::BuiltIn => GPULocation::BuiltIn,
            MTLDeviceLocation::Slot => GPULocation::Slot,
            MTLDeviceLocation::External => GPULocation::External,
            MTLDeviceLocation::Unspecified => GPULocation::Unspecified,
            _ => unreachable!("impossible kind"),
        }
    }
}

pub fn retrieve_gpu_info_via_metal() -> Result<Vec<MetalGpu>, MetalError> {
    let devices = MTLCopyAllDevices();

    if devices.is_empty() {
        return Err(MetalError::NotSupported);
    }

    let mut gpus = Vec::new();

    for device in devices {
        let gpu = extract_gpu_info(&device)?;
        gpus.push(gpu);
    }

    Ok(gpus)
}

fn extract_gpu_info(device: &ProtocolObject<dyn MTLDevice>) -> Result<MetalGpu, MetalError> {
    let name = device.name().to_string();
    let is_removable = device.isRemovable();
    let is_headless = device.isHeadless();
    let is_low_power = device.isLowPower();
    let registry_id = device.registryID();
    let location: GPULocation = device.location().into();
    let has_unified_memory = device.hasUnifiedMemory();
    let kind = if is_low_power {
        GPUKind::Integrated
    } else if is_removable {
        GPUKind::Discrete
    } else if location == GPULocation::BuiltIn {
        GPUKind::Integrated
    } else {
        GPUKind::Discrete
    };
    let vendor = detect_vendor(&name);
    let max_threads_per_threadgroup: MaxThreadsPerThreadgroup =
        device.maxThreadsPerThreadgroup().into();
    let recommended_max_working_set = device.recommendedMaxWorkingSetSize();
    let vram = calculate_vram(has_unified_memory, recommended_max_working_set, registry_id);
    // let driver_version = get_metal_version();

    Ok(MetalGpu {
        kind,
        name,
        vendor,
        // driver_version,
        vram,
        is_removable,
        is_headless,
        registry_id,
        location,
        has_unified_memory,
        max_threads_per_threadgroup,
        recommended_max_working_set,
    })
}

fn calculate_vram(
    has_unified_memory: bool,
    recommended_max_working_set: u64,
    registry_id: u64,
) -> u64 {
    if has_unified_memory {
        // To MB
        recommended_max_working_set / (1024 * 1024)
    } else {
        get_vram_via_iokit(registry_id).unwrap_or(recommended_max_working_set / (1024 * 1024))
    }
}

/// Use iokit to get VRAM size for external gpu
#[allow(deprecated)]
fn get_vram_via_iokit(registry_id: u64) -> Option<u64> {
    use objc2_core_foundation::{CFAllocator, CFDictionary, CFNumber, CFString, CFType};
    use objc2_io_kit::{
        kIOMasterPortDefault, IOObjectRelease, IORegistryEntryCreateCFProperties,
        IORegistryEntryIDMatching, IOServiceGetMatchingService,
    };

    let matching = unsafe { IORegistryEntryIDMatching(registry_id) }?;

    let matching_cast = matching
        .downcast::<CFDictionary>()
        .expect("Failed to downcast to CFDictionary");
    let entry = unsafe { IOServiceGetMatchingService(kIOMasterPortDefault, Some(matching_cast)) };
    if entry == 0 {
        return None;
    }

    scopeguard::defer! {
        IOObjectRelease(entry);
    }

    let mut properties = std::ptr::null_mut();

    let result = unsafe {
        IORegistryEntryCreateCFProperties(
            entry,
            &mut properties,
            CFAllocator::default().as_deref(),
            0,
        )
    };

    let vram = if result == 0 && !properties.is_null() {
        let dict = unsafe { Retained::from_raw(properties) }?;
        let dict_cast = unsafe { dict.cast_unchecked::<CFString, CFType>() };

        let keys = ["VRAM,totalMB", "VRAM", "VRAM,total"];
        let mut vram_value = None;

        for key in &keys {
            let cf_key = CFString::new(key);
            if let Some(value) = dict_cast.get(&cf_key) {
                if let Ok(num) = value.downcast::<CFNumber>() {
                    if let Some(mb) = num.as_i64() {
                        vram_value = Some(mb as u64);
                        break;
                    }
                }
            }
        }

        vram_value
    } else {
        None
    };

    vram
}

fn detect_vendor(name: &str) -> String {
    if name.contains("Apple")
        || name.contains("M1")
        || name.contains("M2")
        || name.contains("M3")
        || name.contains("M4")
        || name.contains("M5")
    {
        "Apple".to_string()
    } else if name.contains("Intel") {
        "Intel".to_string()
    } else if name.contains("AMD") || name.contains("Radeon") {
        "AMD".to_string()
    } else if name.contains("NVIDIA") {
        "NVIDIA".to_string()
    } else {
        "Unknown".to_string()
    }
}

// pub enum MetalVersion {
//     Version4_0,
//     Version3_2,
//     Version3_1,
//     Version3_0,
//     Version2_4,
//     Version2_3,
//     Version2_2,
//     Version2_1,
//     Version2_0,
//     Version1_2,
//     Version1_1,
//     Unknown,
// }

// fn get_metal_version(dev: &ProtocolObject<dyn MTLDevice>) -> String {
//     let candidates = [
//         MTLLanguageVersion::Version4_0,
//         MTLLanguageVersion::Version3_2,
//         MTLLanguageVersion::Version3_1,
//         MTLLanguageVersion::Version3_0,
//         MTLLanguageVersion::Version2_4,
//         MTLLanguageVersion::Version2_3,
//         MTLLanguageVersion::Version2_2,
//         MTLLanguageVersion::Version2_1,
//         MTLLanguageVersion::Version2_0,
//         MTLLanguageVersion::Version1_2,
//         MTLLanguageVersion::Version1_1,
//     ];
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrieve_gpu_info() {
        eprintln!(
            "retrieve_gpu_info_via_metal(), {:#?}",
            retrieve_gpu_info_via_metal()
        );
    }
}
