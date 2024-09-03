use nvml_wrapper::enum_wrappers::device::TemperatureSensor;
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::Nvml;

#[derive(Copy, Clone, Debug)]
pub struct Sensor {
    device_index: u32,
    kind: SensorKind
}

// TODO add more sensor types, like power usage, fan speed, temperature limit, memory stats, and the label
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SensorKind {
    Temperature
}

impl Sensor {
    
    pub fn new(device_index: u32, kind: SensorKind) -> Sensor {
        Sensor {
            device_index,
            kind
        }
    }
    
    pub fn get_value(&self, nvml: &Nvml) -> Result<String, NvmlError> {
        let device = nvml.device_by_index(self.device_index)?;
        
        match self.kind {
            SensorKind::Temperature => {
                let temperature = device.temperature(TemperatureSensor::Gpu)? * 1000;
                Ok(temperature.to_string())
            }
        }
    }
}
