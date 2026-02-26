use glenda::cap::{CapPtr, Endpoint, Kernel};

pub const TIMER_DEV_SLOT: CapPtr = CapPtr::from(9);
pub const DEVICE_SLOT: CapPtr = CapPtr::from(10);
pub const KERNEL_SLOT: CapPtr = CapPtr::from(11);
pub const INIT_SLOT: CapPtr = CapPtr::from(12);

pub const TIMER_DEV_CAP: Endpoint = Endpoint::from(TIMER_DEV_SLOT);
pub const DEVICE_CAP: Endpoint = Endpoint::from(DEVICE_SLOT);
pub const KERNEL_CAP: Kernel = Kernel::from(KERNEL_SLOT);
pub const INIT_CAP: Endpoint = Endpoint::from(INIT_SLOT);
