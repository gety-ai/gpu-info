use ash::vk;

pub fn is_vulkan_supported() -> bool {
    unsafe { ash::Entry::load().is_ok() }
}

pub fn retrieve_gpu_info_via_vk() -> Result<Vec<GPU>, Error> {
    let entry = unsafe { ash::Entry::load() }.map_err(|_| Error::VulkanNotSupported)?;
    let app_name = c"GPUInfoApp";
    let app_info = vk::ApplicationInfo::default()
        .application_name(app_name)
        .application_version(0)
        .engine_name(app_name)
        .engine_version(0)
        .api_version(vk::API_VERSION_1_0);

    let create_info = vk::InstanceCreateInfo::default().application_info(&app_info);
    let instance = unsafe { entry.create_instance(&create_info, None) }
        .map_err(|e| Error::VulkanOperationFailed(e.to_string()))?;

    let physical_devices = unsafe { instance.enumerate_physical_devices() }
        .map_err(|e| Error::VulkanOperationFailed(e.to_string()))?;

    if physical_devices.is_empty() {
        return Err(Error::VulkanOperationFailed(
            "No Vulkan-compatible GPUs found.".to_string(),
        ));
    }

    let mut gpus = Vec::new();

    for device in physical_devices {
        let properties = unsafe { instance.get_physical_device_properties(device) };
        let memory_properties = unsafe { instance.get_physical_device_memory_properties(device) };

        // Extract GPU properties
        let device_name = unsafe { CStr::from_ptr(properties.device_name.as_ptr()) }
            .to_str()
            .unwrap_or("Unknown")
            .to_string();

        let vendor_id = properties.vendor_id;
        let vendor_name = match vendor_id {
            0x8086 => "Intel",
            0x10DE => "NVIDIA",
            0x1002 => "AMD",
            _ => "Unknown",
        }
        .to_string();

        let driver_version = format!(
            "{}.{}.{}",
            (properties.driver_version >> 22) & 0x3FF,
            (properties.driver_version >> 12) & 0x3FF,
            properties.driver_version & 0xFFF
        );

        let device_type = match properties.device_type {
            vk::PhysicalDeviceType::INTEGRATED_GPU => GPUKind::Integrated,
            vk::PhysicalDeviceType::DISCRETE_GPU => GPUKind::Discrete,
            vk::PhysicalDeviceType::VIRTUAL_GPU => GPUKind::Virtual,
            vk::PhysicalDeviceType::CPU => GPUKind::CPU,
            _ => GPUKind::Unknown,
        };

        let vram_size = memory_properties
            .memory_heaps
            .iter()
            .take(memory_properties.memory_heap_count as usize)
            .filter(|heap| heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL))
            .map(|heap| heap.size)
            .sum::<u64>();

        // Populate GPU struct
        let gpu = GPU {
            kind: device_type,
            name: device_name,
            vendor: vendor_name,
            driver_version,
            vram: vram_size / (1024 * 1024), // Convert to MB
            clock_speed: None,               // Vulkan does not provide clock speed
            temperature: None,               // Vulkan does not provide temperature natively
        };

        gpus.push(gpu);
    }

    Ok(gpus)
}

#[cfg(test)]
mod tests {
    use super::*;
    use test_log::test;

    #[test]
    fn test_retrieve_gpu_info_via_vk() {
        let result = retrieve_gpu_info_via_vk();
        eprintln!("{:#?}", result);
        assert!(match result {
            Ok(gpus) => !gpus.is_empty(),
            Err(e) => e.is_vulkan_not_supported(),
        });
    }
}
