use crate::models::{AttributeStatus, DiskInfo, PartitionInfo, SmartAttribute};
use regex::Regex;
use std::process::Command;
use sysinfo::Disks;

pub fn scan_disks() -> Result<Vec<DiskInfo>, String> {
    use std::fs;
    let mut out = Vec::new();

    let dev_entries = fs::read_dir("/dev").map_err(|e| format!("failed to read /dev: {}", e))?;
    for entry in dev_entries {
        if let Ok(e) = entry {
            let name = e.file_name().into_string().unwrap_or_default();

            if name.starts_with("nvme") && !name.contains('p') {
                let dev_path = format!("/dev/{}", name);
                if let Ok(mut di) = probe_smart(&dev_path, "NVMe") {
                    get_partitions(&name, &mut di);
                    out.push(di);
                }
            }

            if name.starts_with("sd") && name.len() == 3 {
                let dev_path = format!("/dev/{}", name);
                let kind = if is_ssd(&name) { "SATA" } else { "HDD" };
                if let Ok(mut di) = probe_smart(&dev_path, kind) {
                    get_partitions(&name, &mut di);
                    out.push(di);
                }
            }
        }
    }

    out.sort_by(|a, b| a.dev.cmp(&b.dev));
    Ok(out)
}

fn get_partitions(dev_name: &str, di: &mut DiskInfo) {
    let disks = Disks::new_with_refreshed_list();

    for disk in disks.iter() {
        let disk_name = disk.name().to_string_lossy();
        if disk_name.contains(dev_name) {
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

fn is_ssd(dev_name: &str) -> bool {
    let path = format!("/sys/block/{}/queue/rotational", dev_name);
    if let Ok(s) = std::fs::read_to_string(path) {
        s.trim() == "0"
    } else {
        false
    }
}

fn probe_smart(dev: &str, hint_kind: &str) -> Result<DiskInfo, String> {
    let output = Command::new("smartctl")
        .args(["-a", dev])
        .output()
        .map_err(|e| format!("failed to run smartctl on {}: {}", dev, e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut di = DiskInfo::empty(dev.to_string());
    di.kind = hint_kind.to_string();

    let re = |pat: &str| Regex::new(pat).unwrap();

    extract_into(&stdout, r"Model Number:\s+(.+)", &mut di.model);
    extract_into(&stdout, r"Device Model:\s+(.+)", &mut di.model);
    extract_into(&stdout, r"Serial Number:\s+(.+)", &mut di.serial);
    extract_into(&stdout, r"Firmware Version:\s+(.+)", &mut di.firmware);

    di.protocol = Some(if hint_kind == "NVMe" {
        "NVMe".to_string()
    } else {
        "ATA".to_string()
    });
    di.device_type = Some(if hint_kind == "HDD" {
        "HDD".to_string()
    } else {
        "SSD".to_string()
    });

    if let Some(cap) =
        re(r"(?:Total NVM Capacity|Namespace 1 Size/Capacity|User Capacity):\s+([\d,]+)\s+\[.*?(\d+(?:\.\d+)?)\s+(GB|TB)")
            .captures(&stdout)
    {
        if let Ok(bytes) = cap[1].replace(",", "").parse::<f64>() {
            di.capacity = Some(bytes);
            di.capacity_str = Some(format!("{} {}", &cap[2], &cap[3]));
        }
    }

    if let Some(cap) = re(r"Percentage Used:\s+(\d+)%").captures(&stdout) {
        if let Ok(used) = cap[1].parse::<u8>() {
            di.health_percent = Some(100u8.saturating_sub(used));
        }
    }

    if let Some(cap) = re(r"Temperature:\s+(\d+)\s+Celsius").captures(&stdout) {
        if let Ok(t) = cap[1].parse::<i32>() {
            di.temp_c = Some(t);
        }
    } else if let Some(cap) = re(r"Temperature_Celsius.*?(\d+)(?:\s+\(|$)").captures(&stdout) {
        if let Ok(t) = cap[1].parse::<i32>() {
            di.temp_c = Some(t);
        }
    }

    if let Some(cap) = re(r"Data Units Written:\s+([\d,]+)").captures(&stdout) {
        if let Ok(units) = cap[1].replace(",", "").parse::<f64>() {
            di.data_written_tb = Some(nvme_units_to_tb(units));
        }
    }
    if let Some(cap) = re(r"Data Units Read:\s+([\d,]+)").captures(&stdout) {
        if let Ok(units) = cap[1].replace(",", "").parse::<f64>() {
            di.data_read_tb = Some(nvme_units_to_tb(units));
        }
    }

    if let Some(cap) = re(r"Total_LBAs_Written\s+\S+\s+\S+\s+\S+\s+([\d,]+)").captures(&stdout) {
        if let Ok(lbas) = cap[1].replace(",", "").parse::<f64>() {
            di.data_written_tb = Some(lbas_to_tb(lbas));
        }
    }
    if let Some(cap) = re(r"Total_LBAs_Read\s+\S+\s+\S+\s+\S+\s+([\d,]+)").captures(&stdout) {
        if let Ok(lbas) = cap[1].replace(",", "").parse::<f64>() {
            di.data_read_tb = Some(lbas_to_tb(lbas));
        }
    }

    if let Some(cap) = re(r"Power Cycles:\s+([\d,]+)").captures(&stdout) {
        if let Ok(v) = cap[1].replace(",", "").parse::<u64>() {
            di.power_cycles = Some(v);
        }
    } else if let Some(cap) = re(r"Power_Cycle_Count.*?(\d+)").captures(&stdout) {
        if let Ok(v) = cap[1].parse::<u64>() {
            di.power_cycles = Some(v);
        }
    }

    if let Some(cap) = re(r"Power On Hours:\s+([\d,]+)").captures(&stdout) {
        if let Ok(v) = cap[1].replace(",", "").parse::<u64>() {
            di.power_on_hours = Some(v);
        }
    } else if let Some(cap) = re(r"Power_On_Hours.*?(\d+)").captures(&stdout) {
        if let Ok(v) = cap[1].parse::<u64>() {
            di.power_on_hours = Some(v);
        }
    }

    if let Some(cap) = re(r"Unsafe Shutdowns:\s+([\d,]+)").captures(&stdout) {
        if let Ok(v) = cap[1].replace(",", "").parse::<u64>() {
            di.unsafe_shutdowns = Some(v);
        }
    }

    if let Some(cap) = re(r"Rotation Rate:\s+(\d+)\s+rpm").captures(&stdout) {
        if let Ok(rpm) = cap[1].parse::<u64>() {
            di.rotation_rpm = Some(rpm);
        }
    }

    parse_smart_attributes(&stdout, &mut di);

    Ok(di)
}

fn parse_smart_attributes(stdout: &str, di: &mut DiskInfo) {
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

            let status = if threshold_val > 0 && current_val <= threshold_val {
                AttributeStatus::Critical
            } else if threshold_val > 0 && current_val <= threshold_val + 10 {
                AttributeStatus::Warning
            } else {
                AttributeStatus::Good
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

fn extract_into(src: &str, pat: &str, out: &mut Option<String>) {
    let re = Regex::new(pat).unwrap();
    if let Some(c) = re.captures(src) {
        *out = Some(c[1].trim().to_string());
    }
}

fn nvme_units_to_tb(units: f64) -> f64 {
    units * 512_000.0 / 1_000_000_000_000.0
}

fn lbas_to_tb(lbas: f64) -> f64 {
    lbas * 512.0 / 1_000_000_000_000.0
}