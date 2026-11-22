use super::Device;
use crate::ffi::*;
use objc2_core_foundation::CFString;
use std::ptr;
use std::sync::atomic::Ordering;

/// The type of multitouch device reported by MultitouchSupport.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceKind {
    Trackpad,
    MagicMouse,
    /// A built-in display that reports a placeholder 1×1 sensor grid.
    /// Its touches arrive as HID events rather than MultitouchSupport frames.
    Touchscreen,
    Other,
}

impl DeviceKind {
    fn from_family(family: Option<i32>, device: &Device) -> Self {
        match family {
            Some(98..=105 | 108 | 109 | 128..=130 | 137) => Self::Trackpad,
            Some(113) if device.is_built_in() => Self::Trackpad,
            Some(112 | 113) => Self::MagicMouse,
            Some(2) if device.is_built_in() => Self::Touchscreen,
            _ => Self::Other,
        }
    }
}

impl Device {
    pub fn is_running(&self) -> bool {
        unsafe { MTDeviceIsRunning(self.inner.raw) }
    }
    pub fn is_built_in(&self) -> bool {
        unsafe { MTDeviceIsBuiltIn(self.inner.raw) }
    }
    pub fn is_opaque_surface(&self) -> bool {
        unsafe { MTDeviceIsOpaqueSurface(self.inner.raw) }
    }
    pub fn is_alive(&self) -> bool {
        unsafe { MTDeviceIsAlive(self.inner.raw) }
    }
    pub fn is_hid_device(&self) -> bool {
        unsafe { MTDeviceIsMTHIDDevice(self.inner.raw) }
    }
    pub fn supports_force(&self) -> bool {
        unsafe { MTDeviceSupportsForce(self.inner.raw) }
    }
    pub fn supports_actuation(&self) -> bool {
        unsafe { MTDeviceSupportsActuation(self.inner.raw) }
    }
    pub fn is_driver_ready(&self) -> bool {
        unsafe { MTDeviceDriverIsReady(self.inner.raw) }
    }
    pub fn supports_power_control(&self) -> bool {
        unsafe { MTDevicePowerControlSupported(self.inner.raw) }
    }

    /// The IOKit service backing this device, if the framework reports one.
    pub fn service(&self) -> Option<u32> {
        let service = unsafe { MTDeviceGetService(self.inner.raw) };
        (service != 0).then_some(service)
    }

    pub fn sensor_surface_dimensions(&self) -> Option<(i32, i32)> {
        let (mut width, mut height) = (0, 0);
        (unsafe { MTDeviceGetSensorSurfaceDimensions(self.inner.raw, &mut width, &mut height) }
            == 0)
            .then_some((width, height))
    }

    pub fn sensor_dimensions(&self) -> Option<(i32, i32)> {
        let (mut rows, mut columns) = (0, 0);
        (unsafe { MTDeviceGetSensorDimensions(self.inner.raw, &mut rows, &mut columns) } == 0)
            .then_some((rows, columns))
    }

    fn get_i32(&self, f: unsafe extern "C" fn(MTDeviceRef, *mut i32) -> OSStatus) -> Option<i32> {
        let mut value = 0;
        (unsafe { f(self.inner.raw, &mut value) } == 0).then_some(value)
    }

    pub fn family_id(&self) -> Option<i32> {
        self.get_i32(MTDeviceGetFamilyID)
    }
    pub fn version(&self) -> Option<i32> {
        self.get_i32(MTDeviceGetVersion)
    }
    pub fn driver_type(&self) -> Option<i32> {
        self.get_i32(MTDeviceGetDriverType)
    }
    pub fn transport_method(&self) -> Option<i32> {
        self.get_i32(MTDeviceGetTransportMethod)
    }

    pub fn device_id(&self) -> Option<u64> {
        let mut value = 0;
        (unsafe { MTDeviceGetDeviceID(self.inner.raw, &mut value) } == 0).then_some(value)
    }

    pub fn serial_number(&self) -> Option<String> {
        let mut value: *const CFString = ptr::null();
        if unsafe { MTDeviceGetSerialNumber(self.inner.raw, &mut value) } != 0 {
            return None;
        }
        // Get rule: the string is borrowed from the device.
        unsafe { value.as_ref() }.map(ToString::to_string)
    }

    pub fn kind(&self) -> DeviceKind {
        DeviceKind::from_family(self.family_id(), self)
    }

    pub fn name(&self) -> String {
        let family = self.family_id();
        match DeviceKind::from_family(family, self) {
            DeviceKind::Trackpad if matches!(family, Some(128..=130)) => "Magic Trackpad".into(),
            DeviceKind::Trackpad => "MacBook Trackpad".into(),
            DeviceKind::MagicMouse => "Magic Mouse".into(),
            DeviceKind::Touchscreen => "MacBook Pro Display".into(),
            DeviceKind::Other if family == Some(176) => "Touch Bar".into(),
            DeviceKind::Other => {
                let id = family
                    .map(|v| v.to_string())
                    .unwrap_or_else(|| "unknown".into());
                if self
                    .sensor_surface_dimensions()
                    .is_some_and(|(w, h)| w > 1000 && h < 100)
                {
                    format!("Unknown Touch Bar (family ID: {id})")
                } else {
                    format!("Unknown Device (family ID: {id})")
                }
            }
        }
    }

    pub fn system_force_response_enabled(&self) -> bool {
        unsafe { MTDeviceGetSystemForceResponseEnabled(self.inner.raw) }
    }

    pub fn set_system_force_response_enabled(&self, enabled: bool) {
        unsafe { MTDeviceSetSystemForceResponseEnabled(self.inner.raw, enabled) };
    }

    pub fn supports_silent_click(&self) -> bool {
        let mut supported = false;
        unsafe { MTDeviceSupportsSilentClick(self.inner.raw, &mut supported) == 0 && supported }
    }

    pub fn power_enabled(&self) -> bool {
        let mut enabled = false;
        unsafe { MTDevicePowerGetEnabled(self.inner.raw, &mut enabled) };
        enabled
    }

    pub fn set_power_enabled(&self, enabled: bool) -> bool {
        unsafe { MTDevicePowerSetEnabled(self.inner.raw, enabled) == 0 }
    }

    pub fn auto_restart_on_wake(&self) -> bool {
        self.inner.auto_restart_on_wake.load(Ordering::Acquire)
    }

    pub fn set_auto_restart_on_wake(&self, enabled: bool) {
        self.inner
            .auto_restart_on_wake
            .store(enabled, Ordering::Release);
    }

    pub fn system_actuations_enabled(&self) -> Option<bool> {
        let actuator = unsafe { MTDeviceGetMTActuator(self.inner.raw) };
        (!actuator.is_null()).then(|| unsafe { MTActuatorGetSystemActuationsEnabled(actuator) })
    }

    pub fn set_system_actuations_enabled(&self, enabled: bool) -> bool {
        let actuator = unsafe { MTDeviceGetMTActuator(self.inner.raw) };
        !actuator.is_null()
            && unsafe { MTActuatorSetSystemActuationsEnabled(actuator, enabled) == 0 }
    }
}
