// Disk discovery and SMART data collection using smartctl

// Import data models for disk information
use crate::models::{AttributeStatus, DiskInfo, PartitionInfo, SmartAttribute};
// Regex for parsing smartctl output
use regex::Regex;
// Command execution for calling smartctl
use std::process::Command;
// Disk and partition enumeration
use sysinfo::Disks;

/// Scans /dev for NVMe and SATA/HDD drives and collects SMART data.
/// Returns a vector of DiskInfo structures sorted by device path.
///
/// # Errors
/// Returns an error string if /dev cannot be read or if no drives are found.
pub fn scan_disks() -> Result<Vec<DiskInfo>, String> {
    use std::fs;
    let mut out = Vec::new();

    // Read entries from /dev directory
    let dev_entries = fs::read_dir("/dev").map_err(|e| format!("failed to read /dev: {}", e))?;
    
    for entry in dev_entries {
        if let Ok(e) = entry {
            let name = e.file_name().into_string().unwrap_or_default();

            // Detect NVMe drives (nvme0n1, nvme1n1, etc.)
            // Filter out partitions which contain 'p' (nvme0n1p1, nvme0n1p2)
            if name.starts_with("nvme") && !name.contains('p') {
                let dev_path = format!("/dev/{}", name);
                if let Ok(mut di) = probe_smart(&dev_path, "NVMe") {
                    get_partitions(&name, &mut di);
                    out.push(di);
                }
            }

            // Detect SATA drives (sda, sdb, sdc, etc.)
            // Only 3-character names to avoid partitions like sda1
            if name.starts_with("sd") && name.len() == 3 {
                let dev_path = format!("/dev/{}", name);
                // Check if it's an SSD or HDD by reading rotational flag
                let kind = if is_ssd(&name) { "SATA" } else { "HDD" };
                if let Ok(mut di) = probe_smart(&dev_path, kind) {
                    get_partitions(&name, &mut di);
                    out.push(di);
                }
            }
        }
    }

    // Sort drives alphabetically by device path
    out.sort_by(|a, b| a.dev.cmp(&b.dev));
    Ok(out)
}

/// Populates partition information for a given drive.
/// Uses sysinfo to enumerate mounted partitions and collect usage statistics.
///
/// # Arguments
/// * `dev_name` - Base device name (e.g., "nvme0n1", "sda")
/// * `di` - DiskInfo structure to populate with partition data
fn get_partitions(dev_name: &str, di: &mut DiskInfo) {
    // Refresh the list of mounted disks
    let disks = Disks::new_with_refreshed_list();

    for disk in disks.iter() {
        let disk_name = disk.name().to_string_lossy();
        
        // Match partitions belonging to this device
        if disk_name.contains(dev_name) {
            // Calculate space metrics in gigabytes
            let total = disk.total_space() as f64 / 1_000_000_000.0;
            let available = disk.available_space() as f64 / 1_000_000_000.0;
            let used = total - available;
            let used_percent = if total > 0.0 {
                (used / total) * 100.0
            } else {
                0.0
            };

            di.partitions.push(PartitionInfo {
                mount_point: disk.mount_point().display().to_string(),
                fs_type: disk.file_system().to_string_lossy().into_owned(),
                total_gb: total,
                used_gb: used,
                free_gb: available,
                used_percent,
            });
        }
    }
}

/// Determines if a drive is an SSD by checking the rotational flag.
/// SSDs have rotational=0, HDDs have rotational=1.
///
/// # Arguments
/// * `dev_name` - Device name (e.g., "sda")
///
/// # Returns
/// True if the device is an SSD, false if it's an HDD or the flag cannot be read.
fn is_ssd(dev_name: &str) -> bool {
    let path = format!("/sys/block/{}/queue/rotational", dev_name);
    if let Ok(s) = std::fs::read_to_string(path) {
        s.trim() == "0"
    } else {
        false
    }
}

/// Executes smartctl to retrieve SMART data for a specific drive.
/// Parses the output to extract model, serial, temperature, health, and usage metrics.
///
/// # Arguments
/// * `dev` - Device path (e.g., "/dev/nvme0n1")
/// * `hint_kind` - Type hint ("NVMe", "SATA", or "HDD")
///
/// # Returns
/// A populated DiskInfo structure on success, or an error string on failure.
fn probe_smart(dev: &str, hint_kind: &str) -> Result<DiskInfo, String> {
    // Execute smartctl with all attributes flag
    let output = Command::new("smartctl")
        .args(["-a", dev])
        .output()
        .map_err(|e| format!("failed to run smartctl on {}: {}", dev, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut di = DiskInfo::empty(dev.to_string());
    di.kind = hint_kind.to_string();

    // Helper function to create regex patterns
    let re = |pat: &str| Regex::new(pat).unwrap();

    // Extract basic drive information
    extract_into(&stdout, r"Model Number:\s+(.+)", &mut di.model);
    extract_into(&stdout, r"Device Model:\s+(.+)", &mut di.model);
    extract_into(&stdout, r"Serial Number:\s+(.+)", &mut di.serial);
    extract_into(&stdout, r"Firmware Version:\s+(.+)", &mut di.firmware);

    // Set protocol based on drive type
    di.protocol = Some(if hint_kind == "NVMe" {
        "NVMe".to_string()
    } else {
        "ATA".to_string()
    });
    
    // Set device type classification
    di.device_type = Some(if hint_kind == "HDD" {
        "HDD".to_string()
    } else {
        "SSD".to_string()
    });

    // Parse capacity from various possible formats
    if let Some(cap) =
        re(r"(?:Total NVM Capacity|Namespace 1 Size/Capacity|User Capacity):\s+([\d,]+)\s+\[.*?(\d+(?:\.\d+)?)\s+(GB|TB)")
            .captures(&stdout)
    {
        if let Ok(bytes) = cap[1].replace(",", "").parse::<f64>() {
            di.capacity = Some(bytes);
            di.capacity_str = Some(format!("{} {}", &cap[2], &cap[3]));
        }
    }

    // Parse health percentage (NVMe reports "Percentage Used", convert to health)
    if let Some(cap) = re(r"Percentage Used:\s+(\d+)%").captures(&stdout) {
        if let Ok(used) = cap[1].parse::<u8>() {
            di.health_percent = Some(100u8.saturating_sub(used));
        }
    }

    // Parse temperature from NVMe output
    if let Some(cap) = re(r"Temperature:\s+(\d+)\s+Celsius").captures(&stdout) {
        if let Ok(t) = cap[1].parse::<i32>() {
            di.temp_c = Some(t);
        }
    } 
    // Parse temperature from SATA SMART attributes
    else if let Some(cap) = re(r"Temperature_Celsius.*?(\d+)(?:\s+\(|$)").captures(&stdout) {
        if let Ok(t) = cap[1].parse::<i32>() {
            di.temp_c = Some(t);
        }
    }

    // Parse data written for NVMe drives (in 512KB units)
    if let Some(cap) = re(r"Data Units Written:\s+([\d,]+)").captures(&stdout) {
        if let Ok(units) = cap[1].replace(",", "").parse::<f64>() {
            di.data_written_tb = Some(nvme_units_to_tb(units));
        }
    }
    
    // Parse data read for NVMe drives (in 512KB units)
    if let Some(cap) = re(r"Data Units Read:\s+([\d,]+)").captures(&stdout) {
        if let Ok(units) = cap[1].replace(",", "").parse::<f64>() {
            di.data_read_tb = Some(nvme_units_to_tb(units));
        }
    }

    // Parse data written for SATA drives (in LBAs)
    if let Some(cap) = re(r"Total_LBAs_Written\s+\S+\s+\S+\s+\S+\s+([\d,]+)").captures(&stdout) {
        if let Ok(lbas) = cap[1].replace(",", "").parse::<f64>() {
            di.data_written_tb = Some(lbas_to_tb(lbas));
        }
    }
    
    // Parse data read for SATA drives (in LBAs)
    if let Some(cap) = re(r"Total_LBAs_Read\s+\S+\s+\S+\s+\S+\s+([\d,]+)").captures(&stdout) {
        if let Ok(lbas) = cap[1].replace(",", "").parse::<f64>() {
            di.data_read_tb = Some(lbas_to_tb(lbas));
        }
    }

    // Parse power cycles from NVMe or SATA output
    if let Some(cap) = re(r"Power Cycles:\s+([\d,]+)").captures(&stdout) {
        if let Ok(v) = cap[1].replace(",", "").parse::<u64>() {
            di.power_cycles = Some(v);
        }
    } else if let Some(cap) = re(r"Power_Cycle_Count.*?(\d+)").captures(&stdout) {
        if let Ok(v) = cap[1].parse::<u64>() {
            di.power_cycles = Some(v);
        }
    }

    // Parse power on hours from NVMe or SATA output
    if let Some(cap) = re(r"Power On Hours:\s+([\d,]+)").captures(&stdout) {
        if let Ok(v) = cap[1].replace(",", "").parse::<u64>() {
            di.power_on_hours = Some(v);
        }
    } else if let Some(cap) = re(r"Power_On_Hours.*?(\d+)").captures(&stdout) {
        if let Ok(v) = cap[1].parse::<u64>() {
            di.power_on_hours = Some(v);
        }
    }

    // Parse unsafe shutdown count (NVMe specific)
    if let Some(cap) = re(r"Unsafe Shutdowns:\s+([\d,]+)").captures(&stdout) {
        if let Ok(v) = cap[1].replace(",", "").parse::<u64>() {
            di.unsafe_shutdowns = Some(v);
        }
    }

    // Parse rotation speed for HDDs (SSDs will not have this)
    if let Some(cap) = re(r"Rotation Rate:\s+(\d+)\s+rpm").captures(&stdout) {
        if let Ok(rpm) = cap[1].parse::<u64>() {
            di.rotation_rpm = Some(rpm);
        }
    }

    // Parse detailed SMART attributes table
    parse_smart_attributes(&stdout, &mut di);

    Ok(di)
}

/// Parses the SMART attributes table from smartctl output.
/// Extracts attribute ID, name, current/worst/threshold values, and computes status.
///
/// # Arguments
/// * `stdout` - The full smartctl output text
/// * `di` - DiskInfo structure to populate with attributes
fn parse_smart_attributes(stdout: &str, di: &mut DiskInfo) {
    // Regex to match SMART attribute lines
    // Format: ID NAME FLAGS VALUE WORST THRESH TYPE UPDATED WHEN_FAILED RAW_VALUE
    let attr_re = Regex::new(
        r"^\s*(\d+)\s+(\S.*?)\s+(0x[0-9a-f]+)\s+(\d+)\s+(\d+)\s+(\d+)\s+\S+\s+\S+\s+\S+\s+(.+)$",
    )
    .unwrap();

    for line in stdout.lines() {
        if let Some(cap) = attr_re.captures(line) {
            let id = cap[1].to_string();
            let name = cap[2].trim().to_string();
            let current = cap[4].to_string();
            let worst = cap[5].to_string();
            let threshold = cap[6].to_string();
            let raw_value = cap[7].trim().to_string();

            let current_val = current.parse::<u32>().unwrap_or(0);
            let threshold_val = threshold.parse::<u32>().unwrap_or(0);

            // Determine attribute health status based on threshold
            let status = if threshold_val > 0 && current_val <= threshold_val {
                AttributeStatus::Critical  // Below threshold = failure
            } else if threshold_val > 0 && current_val <= threshold_val + 10 {
                AttributeStatus::Warning   // Within 10 of threshold = warning
            } else {
                AttributeStatus::Good      // Above threshold = healthy
            };

            di.smart_attributes.push(SmartAttribute {
                id,
                name,
                current,
                worst,
                threshold,
                raw_value,
                status,
            });
        }
    }
}

/// Helper function to extract a value using regex and store it in an Option<String>.
///
/// # Arguments
/// * `src` - Source text to search
/// * `pat` - Regex pattern with one capture group
/// * `out` - Output Option<String> to populate
fn extract_into(src: &str, pat: &str, out: &mut Option<String>) {
    let re = Regex::new(pat).unwrap();
    if let Some(c) = re.captures(src) {
        *out = Some(c[1].trim().to_string());
    }
}

/// Converts NVMe data units to terabytes.
/// NVMe reports data in units of 512KB (512,000 bytes).
///
/// # Arguments
/// * `units` - Number of 512KB units
///
/// # Returns
/// Equivalent value in terabytes
fn nvme_units_to_tb(units: f64) -> f64 {
    units * 512_000.0 / 1_000_000_000_000.0
}

/// Converts logical block addresses (LBAs) to terabytes.
/// Standard LBA size is 512 bytes.
///
/// # Arguments
/// * `lbas` - Number of logical blocks
///
/// # Returns
/// Equivalent value in terabytes
fn lbas_to_tb(lbas: f64) -> f64 {
    lbas * 512.0 / 1_000_000_000_000.0
}