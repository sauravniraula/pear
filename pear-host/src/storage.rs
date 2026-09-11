use std::{
    fs::{self, OpenOptions},
    path::Path,
};

use objc2::AnyThread;
use objc2_foundation::{NSArray, NSURL};
use objc2_virtualization::{
    VZDiskImageStorageDeviceAttachment, VZVirtioBlockDeviceConfiguration,
    VZVirtualMachineConfiguration,
};

use crate::config::VMSettings;

const GIB: u64 = 1024 * 1024 * 1024;
const BLOCK_SIZE: u64 = 512;

pub fn attach(config: &VZVirtualMachineConfiguration, settings: &VMSettings) {
    unsafe {
        ensure_disk(&settings.disk, settings.disk_gib).unwrap();

        let disk_url = NSURL::from_file_path(&settings.disk).unwrap();
        let attachment = VZDiskImageStorageDeviceAttachment::initWithURL_readOnly_error(
            VZDiskImageStorageDeviceAttachment::alloc(),
            &disk_url,
            false,
        )
        .unwrap();

        let block_device = VZVirtioBlockDeviceConfiguration::initWithAttachment(
            VZVirtioBlockDeviceConfiguration::alloc(),
            &attachment,
        )
        .into_super();
        let storage_devices = NSArray::from_retained_slice(&[block_device]);
        config.setStorageDevices(&storage_devices);
    }
}

pub fn ensure_disk(path: &Path, size_gib: u64) -> Result<(), String> {
    if path.exists() {
        let metadata = fs::metadata(path)
            .map_err(|error| format!("Failed to inspect disk image {}: {error}", path.display()))?;

        if !metadata.is_file() {
            return Err(format!(
                "Disk path is not a regular file: {}",
                path.display()
            ));
        }

        if metadata.len() == 0 {
            return Err(format!("Disk image is empty: {}", path.display()));
        }

        if metadata.len() % BLOCK_SIZE != 0 {
            return Err(format!(
                "Disk image size must be a multiple of {BLOCK_SIZE} bytes: {}",
                path.display()
            ));
        }

        return Ok(());
    }

    let size_bytes = size_gib
        .checked_mul(GIB)
        .ok_or_else(|| format!("Disk size too large: {size_gib} GiB"))?;

    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Failed to create disk directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let disk = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("Failed to create disk image {}: {error}", path.display()))?;

    if let Err(error) = disk.set_len(size_bytes) {
        let _ = fs::remove_file(path);

        return Err(format!(
            "Failed to resize disk image {}: {error}",
            path.display()
        ));
    }

    println!("Created {} GiB sparse disk at {}", size_gib, path.display());

    Ok(())
}
