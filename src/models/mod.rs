#[derive(Clone, Debug)]
pub struct SmartAttribute {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub name: String,
    #[allow(dead_code)]
    pub current: String,
    #[allow(dead_code)]
    pub worst: String,
    #[allow(dead_code)]
    pub threshold: String,
    #[allow(dead_code)]
    pub raw_value: String,
    #[allow(dead_code)]
    pub status: AttributeStatus,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AttributeStatus {
    Good,
    Warning,
    Critical,
}

#[derive(Clone, Debug)]
pub struct PartitionInfo {
    pub mount_point: String,
    pub fs_type: String,
    pub total_gb: f64,
    pub used_gb: f64,
    pub free_gb: f64,
    pub used_percent: f64,
}

#[derive(Clone, Debug)]
pub struct DiskInfo {
    pub dev: String,
    pub kind: String,
    pub model: Option<String>,
    pub serial: Option<String>,
    pub firmware: Option<String>,
    pub capacity: Option<f64>,
    pub capacity_str: Option<String>,
    pub health_percent: Option<u8>,
    pub temp_c: Option<i32>,
    pub data_written_tb: Option<f64>,
    pub data_read_tb: Option<f64>,
    pub power_on_hours: Option<u64>,
    pub power_cycles: Option<u64>,
    pub unsafe_shutdowns: Option<u64>,
    pub rotation_rpm: Option<u64>,
    pub protocol: Option<String>,
    pub device_type: Option<String>,
    pub smart_attributes: Vec<SmartAttribute>,
    pub partitions: Vec<PartitionInfo>,
}

impl DiskInfo {
    pub fn empty(dev: impl Into<String>) -> Self {
        Self {
            dev: dev.into(),
            kind: String::from("Unknown"),
            model: None,
            serial: None,
            firmware: None,
            capacity: None,
            capacity_str: None,
            health_percent: None,
            temp_c: None,
            data_written_tb: None,
            data_read_tb: None,
            power_on_hours: None,
            power_cycles: None,
            unsafe_shutdowns: None,
            rotation_rpm: None,
            protocol: None,
            device_type: None,
            smart_attributes: vec![],
            partitions: vec![],
        }
    }
}