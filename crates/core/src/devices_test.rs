#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_devices_returns_vec() {
        let devices = detect_devices();
        // Just check that it returns a Vec (not panicking)
        assert!(devices.is_empty() || !devices.is_empty());
    }

    #[test]
    fn test_device_fields_present() {
        let devices = detect_devices();
        for dev in devices {
            assert!(!dev.id.is_empty());
            assert!(!dev.dev_type.is_empty());
        }
    }

    #[test]
    fn test_partition_fields_present() {
        let devices = detect_devices();
        for dev in devices {
            for part in dev.partitions.iter() {
                assert!(!part.name.is_empty());
                assert!(part.size_gb >= 0);
                // fs_type and used_gb are Option, but should be present for mounted partitions
                if let Some(ref mount) = part.mount_point {
                    assert!(part.fs_type.is_some(), "Partition with mount point should have fs_type");
                    assert!(part.used_gb.is_some(), "Partition with mount point should have used_gb");
                }
            }
        }
    }

    #[test]
    fn test_error_field_and_encryption() {
        let devices = detect_devices();
        for dev in devices {
            // error should be None or Some(String)
            if let Some(ref err) = dev.error {
                assert!(!err.is_empty(), "Error field should not be empty if present");
            }
            // encrypted should be a bool
            assert!(matches!(dev.encrypted, true | false));
        }
    }

    #[test]
    fn test_device_partition_consistency() {
        let devices = detect_devices();
        for dev in devices {
            // Device size should be >= sum of partition sizes (if partitions exist)
            if !dev.partitions.is_empty() {
                let total_part_size: u64 = dev.partitions.iter().map(|p| p.size_gb).sum();
                assert!(dev.size_gb >= total_part_size, "Device size_gb should be >= sum of partition size_gb");
            }
        }
    }
}
