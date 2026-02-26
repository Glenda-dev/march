#![no_std]
#![no_main]
#![allow(dead_code)]

#[macro_use]
extern crate glenda;
extern crate alloc;

mod layout;
mod march;

use glenda::cap::{
    CSPACE_CAP, CapType, ENDPOINT_CAP, ENDPOINT_SLOT, MONITOR_CAP, RECV_SLOT, REPLY_SLOT,
};
use glenda::client::{DeviceClient, InitClient, ResourceClient};
use glenda::interface::SystemService;
use glenda::interface::resource::ResourceService;
use glenda::ipc::Badge;
use glenda::protocol::resource::{self, ResourceType};
use glenda::utils::manager::CSpaceManager;
use layout::{DEVICE_CAP, DEVICE_SLOT, INIT_CAP, INIT_SLOT, KERNEL_CAP, KERNEL_SLOT};
use march::MarchService;

#[unsafe(no_mangle)]
fn main() -> usize {
    glenda::console::init_logging("March");
    log!("Starting High Precision Time Service...");

    // 1. Setup resource clients
    let mut res_client = ResourceClient::new(MONITOR_CAP);
    let mut cspace_mgr = CSpaceManager::new(CSPACE_CAP, 0x1000);

    // Create endpoint for march service
    res_client
        .alloc(Badge::null(), CapType::Endpoint, 0, ENDPOINT_SLOT)
        .expect("Failed to alloc endpoint");

    res_client
        .get_cap(Badge::null(), ResourceType::Kernel, 0, KERNEL_SLOT)
        .expect("Failed to get kernel cap");

    res_client
        .get_cap(Badge::null(), ResourceType::Endpoint, resource::DEVICE_ENDPOINT, DEVICE_SLOT)
        .expect("Failed to get device cap");
    let mut dev_client = DeviceClient::new(DEVICE_CAP);

    res_client
        .get_cap(Badge::null(), ResourceType::Endpoint, resource::INIT_ENDPOINT, INIT_SLOT)
        .expect("Failed to get init cap");
    let mut init_client = InitClient::new(INIT_CAP);

    // 2. Start Service logic
    let mut march = MarchService::new(
        &mut res_client,
        &mut cspace_mgr,
        &mut dev_client,
        &mut init_client,
        KERNEL_CAP,
    );
    march.listen(ENDPOINT_CAP, REPLY_SLOT, RECV_SLOT).expect("Failed to listen");

    march.init().expect("Failed to init march");
    march.run().expect("Service crashed");

    0
}
