#![allow(non_camel_case_types, non_snake_case, dead_code)]

use crate::Contact;
use objc2_core_foundation::{CFArray, CFDictionary, CFMutableDictionary, CFString};
use std::ffi::{c_char, c_long, c_void};
use std::ptr::NonNull;

pub type MTDeviceRef = *mut c_void;
pub type MTActuatorRef = *mut c_void;
pub type MTActuationRef = *const c_void;

pub type kern_return_t = i32;
pub type IOReturn = kern_return_t;
pub type OSStatus = i32;
pub type mach_port_t = u32;
pub type io_object_t = u32;
pub type io_iterator_t = io_object_t;
pub type io_service_t = io_object_t;
pub type io_connect_t = io_object_t;
pub type natural_t = u32;
pub type IONotificationPortRef = *mut c_void;
pub type dispatch_queue_t = *mut c_void;

pub const KERN_SUCCESS: kern_return_t = 0;
pub const K_IO_RETURN_SUCCESS: IOReturn = 0;
pub const K_IO_MAIN_PORT_DEFAULT: mach_port_t = 0;

const SYS_IOKIT: u32 = 0xE000_0000;
pub const K_IO_MESSAGE_CAN_SYSTEM_SLEEP: u32 = SYS_IOKIT | 0x270;
pub const K_IO_MESSAGE_SYSTEM_WILL_SLEEP: u32 = SYS_IOKIT | 0x280;
pub const K_IO_MESSAGE_SYSTEM_HAS_POWERED_ON: u32 = SYS_IOKIT | 0x300;

pub type ContactCallback = unsafe extern "C" fn(
    MTDeviceRef,
    *mut Contact,
    i32,
    f64,
    i32,
) -> i32;

pub type PathCallback = unsafe extern "C" fn(MTDeviceRef, isize, isize, *mut Contact);

pub type IOServiceMatchingCallback = unsafe extern "C" fn(*mut c_void, io_iterator_t);
pub type IOServiceInterestCallback = unsafe extern "C" fn(
    *mut c_void,
    io_service_t,
    natural_t,
    *mut c_void,
);

unsafe extern "C" {
    // MultitouchSupport.framework
    pub fn MTAbsoluteTimeGetCurrent() -> f64;
    pub fn MTDeviceIsAvailable() -> bool;
    pub fn MTDeviceCreateDefault() -> MTDeviceRef;
    pub fn MTDeviceCreateList() -> Option<NonNull<CFArray>>;
    pub fn MTDeviceCreateFromDeviceID(device_id: u64) -> MTDeviceRef;
    pub fn MTDeviceCreateFromService(service: io_service_t) -> MTDeviceRef;
    pub fn MTDeviceRelease(device: MTDeviceRef);
    pub fn MTDeviceStart(device: MTDeviceRef, mode: i32) -> OSStatus;
    pub fn MTDeviceStop(device: MTDeviceRef) -> OSStatus;

    pub fn MTDeviceIsRunning(device: MTDeviceRef) -> bool;
    pub fn MTDeviceIsBuiltIn(device: MTDeviceRef) -> bool;
    pub fn MTDeviceIsOpaqueSurface(device: MTDeviceRef) -> bool;
    pub fn MTDeviceIsAlive(device: MTDeviceRef) -> bool;
    pub fn MTDeviceIsMTHIDDevice(device: MTDeviceRef) -> bool;
    pub fn MTDeviceSupportsForce(device: MTDeviceRef) -> bool;
    pub fn MTDeviceSupportsActuation(device: MTDeviceRef) -> bool;
    pub fn MTDeviceDriverIsReady(device: MTDeviceRef) -> bool;
    pub fn MTDevicePowerControlSupported(device: MTDeviceRef) -> bool;

    pub fn MTDeviceGetService(device: MTDeviceRef) -> io_service_t;
    pub fn MTDeviceGetSensorSurfaceDimensions(device: MTDeviceRef, width: *mut i32, height: *mut i32) -> OSStatus;
    pub fn MTDeviceGetSensorDimensions(device: MTDeviceRef, rows: *mut i32, columns: *mut i32) -> OSStatus;
    pub fn MTDeviceGetFamilyID(device: MTDeviceRef, family_id: *mut i32) -> OSStatus;
    pub fn MTDeviceGetDeviceID(device: MTDeviceRef, device_id: *mut u64) -> OSStatus;
    pub fn MTDeviceGetVersion(device: MTDeviceRef, version: *mut i32) -> OSStatus;
    pub fn MTDeviceGetDriverType(device: MTDeviceRef, driver_type: *mut i32) -> OSStatus;
    pub fn MTDeviceGetTransportMethod(device: MTDeviceRef, transport: *mut i32) -> OSStatus;
    pub fn MTDeviceGetSerialNumber(device: MTDeviceRef, serial: *mut *const CFString) -> OSStatus;

    pub fn MTDeviceGetSystemForceResponseEnabled(device: MTDeviceRef) -> bool;
    pub fn MTDeviceSetSystemForceResponseEnabled(device: MTDeviceRef, enabled: bool);
    pub fn MTDeviceSupportsSilentClick(device: MTDeviceRef, supported: *mut bool) -> OSStatus;
    pub fn MTDevicePowerSetEnabled(device: MTDeviceRef, enabled: bool) -> OSStatus;
    pub fn MTDevicePowerGetEnabled(device: MTDeviceRef, enabled: *mut bool);

    pub fn MTRegisterContactFrameCallback(device: MTDeviceRef, callback: Option<ContactCallback>);
    pub fn MTUnregisterContactFrameCallback(device: MTDeviceRef, callback: Option<ContactCallback>);
    pub fn MTRegisterPathCallback(device: MTDeviceRef, callback: Option<PathCallback>);
    pub fn MTUnregisterPathCallback(device: MTDeviceRef, callback: Option<PathCallback>);

    pub fn MTDeviceGetMTActuator(device: MTDeviceRef) -> MTActuatorRef;
    pub fn MTActuatorGetSystemActuationsEnabled(actuator: MTActuatorRef) -> bool;
    pub fn MTActuatorSetSystemActuationsEnabled(actuator: MTActuatorRef, enabled: bool) -> OSStatus;
    pub fn MTActuatorCreateFromDeviceID(device_id: u64) -> MTActuatorRef;
    pub fn MTActuatorOpen(actuator: MTActuatorRef) -> IOReturn;
    pub fn MTActuatorClose(actuator: MTActuatorRef) -> IOReturn;
    pub fn MTActuatorActuate(actuator: MTActuatorRef, pattern: i32, flags: u32, intensity: f32, scale: f32) -> IOReturn;
    pub fn MTActuatorIsOpen(actuator: MTActuatorRef) -> bool;
    pub fn MTActuationActuate(actuation: MTActuationRef, actuator: MTActuatorRef, flags: u32) -> IOReturn;
    pub fn MTActuationCreateFromDictionary(dictionary: &CFDictionary, actuator: MTActuatorRef) -> MTActuationRef;

    // IOKit.framework
    pub fn IOServiceMatching(name: *const c_char) -> *mut CFMutableDictionary;
    pub fn IONotificationPortCreate(master_port: mach_port_t) -> IONotificationPortRef;
    pub fn IONotificationPortDestroy(port: IONotificationPortRef);
    pub fn IONotificationPortSetDispatchQueue(port: IONotificationPortRef, queue: dispatch_queue_t);
    pub fn IOServiceAddMatchingNotification(
        port: IONotificationPortRef,
        notification_type: *const c_char,
        matching: *const CFMutableDictionary,
        callback: Option<IOServiceMatchingCallback>,
        refcon: *mut c_void,
        iterator: *mut io_iterator_t,
    ) -> kern_return_t;
    pub fn IOIteratorNext(iterator: io_iterator_t) -> io_object_t;
    pub fn IOObjectRetain(object: io_object_t) -> kern_return_t;
    pub fn IOObjectRelease(object: io_object_t) -> kern_return_t;
    pub fn IORegisterForSystemPower(
        refcon: *mut c_void,
        port: *mut IONotificationPortRef,
        callback: Option<IOServiceInterestCallback>,
        notifier: *mut io_object_t,
    ) -> io_connect_t;
    pub fn IOAllowPowerChange(root_port: io_connect_t, notification_id: c_long) -> IOReturn;

    // libSystem / libdispatch
    pub fn dispatch_get_global_queue(identifier: isize, flags: usize) -> dispatch_queue_t;
}
