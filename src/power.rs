use crate::device::DeviceInner;
use crate::ffi::*;
use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

struct PowerRegistration {
    #[allow(dead_code)]
    port: IONotificationPortRef,
    #[allow(dead_code)]
    notifier: io_object_t,
    #[allow(dead_code)]
    connection: io_connect_t,
}

unsafe impl Send for PowerRegistration {}
unsafe impl Sync for PowerRegistration {}

static REGISTRATION: OnceLock<Option<PowerRegistration>> = OnceLock::new();
static DEVICES: OnceLock<Mutex<Vec<Weak<DeviceInner>>>> = OnceLock::new();
static POWER_CONNECTION: AtomicU32 = AtomicU32::new(0);

fn devices() -> &'static Mutex<Vec<Weak<DeviceInner>>> {
    DEVICES.get_or_init(|| Mutex::new(Vec::new()))
}

fn registration() -> Option<&'static PowerRegistration> {
    REGISTRATION
        .get_or_init(|| unsafe {
            let mut port: IONotificationPortRef = ptr::null_mut();
            let mut notifier = 0;
            let connection = IORegisterForSystemPower(
                ptr::null_mut(),
                &mut port,
                Some(power_callback),
                &mut notifier,
            );
            if connection == 0 || port.is_null() {
                return None;
            }
            POWER_CONNECTION.store(connection, Ordering::Release);
            // QOS_CLASS_USER_INITIATED = 0x19.
            let queue = dispatch_get_global_queue(0x19, 0);
            IONotificationPortSetDispatchQueue(port, queue);
            Some(PowerRegistration { port, notifier, connection })
        })
        .as_ref()
}

pub(crate) fn watch(device: &Arc<DeviceInner>) {
    if registration().is_none() {
        return;
    }
    let ptr = Arc::as_ptr(device) as usize;
    let mut list = devices().lock().unwrap_or_else(|e| e.into_inner());
    list.retain(|weak| weak.strong_count() > 0);
    if list.iter().any(|weak| weak.as_ptr() as usize == ptr) {
        return;
    }
    list.push(Arc::downgrade(device));
}

pub(crate) fn unwatch(device: &Arc<DeviceInner>) {
    let ptr = Arc::as_ptr(device) as usize;
    devices()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|weak| weak.strong_count() > 0 && weak.as_ptr() as usize != ptr);
}

unsafe extern "C" fn power_callback(
    _refcon: *mut c_void,
    _service: io_service_t,
    message_type: natural_t,
    message_argument: *mut c_void,
) {
    match message_type {
        K_IO_MESSAGE_CAN_SYSTEM_SLEEP | K_IO_MESSAGE_SYSTEM_WILL_SLEEP => {
            let connection = POWER_CONNECTION.load(Ordering::Acquire);
            if connection != 0 {
                let notification_id = message_argument as isize as std::ffi::c_long;
                unsafe {
                    let _ = IOAllowPowerChange(connection, notification_id);
                }
            }
        }
        K_IO_MESSAGE_SYSTEM_HAS_POWERED_ON => {
            let live: Vec<_> = {
                let mut list = devices().lock().unwrap_or_else(|e| e.into_inner());
                let live: Vec<_> = list.iter().filter_map(Weak::upgrade).collect();
                list.retain(|weak| weak.strong_count() > 0);
                live
            };
            for device in live {
                std::thread::spawn(move || device.raw_restart_after_wake());
            }
        }
        _ => {}
    }
}
