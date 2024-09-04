use nvml_wrapper::enum_wrappers::device::{
    Clock, PcieUtilCounter, TemperatureSensor, TemperatureThreshold,
};
use nvml_wrapper::error::NvmlError;
use nvml_wrapper::Nvml;
use tracing::error;

#[derive(Copy, Clone, Debug)]
pub struct Sensor {
    pub device_index: u32,
    pub kind: SensorKind,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SensorKind {
    Bar1MemoryFree,
    Bar1MemoryTotal,
    Bar1MemoryUsed,
    ClockGraphics,
    ClockMemory,
    ClockStreamingMultiprocessor,
    ClockVideo,
    DecoderUtilization,
    DecoderUtilizationSamplingPeriod,
    EncoderUtilization,
    EncoderUtilizationSamplingPeriod,
    EnforcedPowerLimit,
    FanSpeed(u32),
    MemoryFree,
    MemoryTotal,
    MemoryUsed,
    Name,
    PcieThroughputReceive,
    PcieThroughputSend,
    PerformanceState,
    PowerSource,
    PowerUsage,
    Temperature,
    TemperatureThreshold,
    TotalEnergyConsumption,
    UtilizationRateGpu,
    UtilizationRateMemory,
}

impl Sensor {
    pub fn new(device_index: u32, kind: SensorKind) -> Sensor {
        Sensor { device_index, kind }
    }

    pub fn create_all(nvml: &Nvml, device_index: u32) -> Vec<Sensor> {
        let device = match nvml.device_by_index(device_index) {
            Ok(dev) => dev,
            Err(err) => {
                error!("failed to create sensors for device {device_index}: {err}");
                return Vec::new();
            }
        };

        let mut all_sensor_kinds = vec![
            SensorKind::Bar1MemoryFree,
            SensorKind::Bar1MemoryTotal,
            SensorKind::Bar1MemoryUsed,
            SensorKind::ClockGraphics,
            SensorKind::ClockMemory,
            SensorKind::ClockStreamingMultiprocessor,
            SensorKind::ClockVideo,
            SensorKind::DecoderUtilization,
            SensorKind::DecoderUtilizationSamplingPeriod,
            SensorKind::EncoderUtilization,
            SensorKind::EncoderUtilizationSamplingPeriod,
            SensorKind::EnforcedPowerLimit,
            SensorKind::MemoryFree,
            SensorKind::MemoryTotal,
            SensorKind::MemoryUsed,
            SensorKind::Name,
            SensorKind::PcieThroughputReceive,
            SensorKind::PcieThroughputSend,
            SensorKind::PerformanceState,
            SensorKind::PowerSource,
            SensorKind::PowerUsage,
            SensorKind::Temperature,
            SensorKind::TemperatureThreshold,
            SensorKind::TotalEnergyConsumption,
            SensorKind::UtilizationRateGpu,
            SensorKind::UtilizationRateMemory,
        ];

        let num_fans = device.num_fans().unwrap_or_else(|err| {
            error!("failed to get the number of fans on device: {err}");
            0
        });
        for fan in 0..num_fans {
            all_sensor_kinds.push(SensorKind::FanSpeed(fan));
        }

        all_sensor_kinds
            .into_iter()
            .map(|kind| Sensor::new(device_index, kind))
            .collect()
    }

    pub fn get_value(&self, nvml: &Nvml) -> Result<String, NvmlError> {
        let device = nvml.device_by_index(self.device_index)?;

        let value = match self.kind {
            SensorKind::Bar1MemoryFree => device.bar1_memory_info()?.free.to_string(),
            SensorKind::Bar1MemoryTotal => device.bar1_memory_info()?.total.to_string(),
            SensorKind::Bar1MemoryUsed => device.bar1_memory_info()?.used.to_string(),
            SensorKind::ClockGraphics => device.clock_info(Clock::Graphics)?.to_string(),
            SensorKind::ClockMemory => device.clock_info(Clock::Memory)?.to_string(),
            SensorKind::ClockStreamingMultiprocessor => device.clock_info(Clock::SM)?.to_string(),
            SensorKind::ClockVideo => device.clock_info(Clock::Video)?.to_string(),
            SensorKind::DecoderUtilization => device.decoder_utilization()?.utilization.to_string(),
            SensorKind::DecoderUtilizationSamplingPeriod => {
                device.decoder_utilization()?.sampling_period.to_string()
            }
            SensorKind::EncoderUtilization => device.encoder_utilization()?.utilization.to_string(),
            SensorKind::EncoderUtilizationSamplingPeriod => {
                device.encoder_utilization()?.sampling_period.to_string()
            }
            SensorKind::EnforcedPowerLimit => (device.enforced_power_limit()? * 1000).to_string(),
            SensorKind::FanSpeed(idx) => device.fan_speed(idx)?.to_string(),
            SensorKind::MemoryFree => device.memory_info()?.free.to_string(),
            SensorKind::MemoryTotal => device.memory_info()?.total.to_string(),
            SensorKind::MemoryUsed => device.memory_info()?.used.to_string(),
            SensorKind::Name => device.name()?,
            SensorKind::PcieThroughputReceive => device
                .pcie_throughput(PcieUtilCounter::Receive)?
                .to_string(),
            SensorKind::PcieThroughputSend => {
                device.pcie_throughput(PcieUtilCounter::Send)?.to_string()
            }
            SensorKind::PerformanceState => device.performance_state()?.as_c().to_string(),
            SensorKind::PowerSource => device.power_source()?.as_c().to_string(),
            SensorKind::PowerUsage => (device.power_usage()? * 1000).to_string(),
            SensorKind::Temperature => {
                (device.temperature(TemperatureSensor::Gpu)? * 1000).to_string()
            }
            SensorKind::TemperatureThreshold => {
                (device.temperature_threshold(TemperatureThreshold::GpuMax)? * 1000).to_string()
            }
            SensorKind::TotalEnergyConsumption => device.total_energy_consumption()?.to_string(),
            SensorKind::UtilizationRateGpu => device.utilization_rates()?.gpu.to_string(),
            SensorKind::UtilizationRateMemory => device.utilization_rates()?.memory.to_string(),
        };

        Ok(value)
    }
}
