use super::MarchService;
use glenda::arch::time::get_time;
use glenda::cap::{CSPACE_CAP, CapPtr, Endpoint, Reply};
use glenda::error::Error;
use glenda::interface::init::InitService;
use glenda::interface::resource::ResourceService;
use glenda::interface::{DeviceService, SystemService, TimeService};
use glenda::ipc::server::{handle_call, handle_notify};
use glenda::ipc::{Badge, MsgTag, UTCB};
use glenda::protocol::TIME_PROTO;
use glenda::protocol::device::NOTIFY_HOOK;
use glenda::protocol::device::{HookTarget, LogicDeviceType};
use glenda::protocol::init::ServiceState;
use glenda::protocol::resource::{ResourceType, TIME_ENDPOINT};
use glenda::protocol::time::{ADJ_TIME, MONO_NOW, SLEEP, TIME_NOW};

impl<'a> SystemService for MarchService<'a> {
    fn init(&mut self) -> Result<(), Error> {
        let kernel = self.kernel_cap;
        self.freq = match kernel.get_freq() {
            Ok(f) => f as u64,
            Err(_) => 10_000_000,
        };
        self.initial_ticks = get_time() as u64;
        log!("Scanning devices...");
        self.rescan_devices()?;
        let target = HookTarget::Type(LogicDeviceType::Timer);
        self.dev_client.hook(Badge::null(), target, self.ipc.endpoint.cap())?;
        log!("Registering Timer Service...");
        self.res_client.register_cap(
            Badge::null(),
            ResourceType::Endpoint,
            TIME_ENDPOINT,
            self.ipc.endpoint.cap(),
        )?;
        Ok(())
    }
    fn listen(&mut self, ep: Endpoint, reply: CapPtr, recv: CapPtr) -> Result<(), Error> {
        self.ipc.endpoint = ep;
        self.ipc.reply = Reply::from(reply);
        self.ipc.recv = recv;
        Ok(())
    }
    fn run(&mut self) -> Result<(), Error> {
        self.ipc.running = true;
        let mut reported_running = false;

        if self.reference_index.is_some() {
            self.init_client.report_service(Badge::null(), ServiceState::Running)?;
            reported_running = true;
        } else {
            log!("No timer source ready yet, waiting before reporting Running...");
        }

        while self.ipc.running {
            let mut utcb = unsafe { UTCB::new() };
            utcb.clear();
            utcb.set_reply_window(self.ipc.reply.cap());
            utcb.set_recv_window(self.ipc.recv);

            if let Err(e) = self.ipc.endpoint.recv(&mut utcb) {
                error!("Recv error: {:?}", e);
                continue;
            }

            match self.dispatch(&mut utcb) {
                Ok(()) => {
                    let _ = self.reply(&mut utcb);
                }
                Err(Error::Success) => {
                    // Handled notification, skip reply
                    let _ = CSPACE_CAP.delete(self.ipc.reply.cap());
                }
                Err(e) => {
                    let badge = utcb.get_badge();
                    let tag = utcb.get_msg_tag();
                    error!(
                        "Dispatch error: {:?} badge={}, proto={:#x}, label={:#x}",
                        e,
                        badge,
                        tag.proto(),
                        tag.label()
                    );
                    utcb.set_msg_tag(MsgTag::err());
                    utcb.set_mr(0, e as usize);
                    let _ = self.reply(&mut utcb);
                }
            }

            if !reported_running && self.reference_index.is_some() {
                self.init_client.report_service(Badge::null(), ServiceState::Running)?;
                reported_running = true;
                log!("Timer source ready, reported Running to init");
            }
        }
        Ok(())
    }
    fn dispatch(&mut self, utcb: &mut UTCB) -> Result<(), Error> {
        glenda::ipc_dispatch! {
            self, utcb,
            (TIME_PROTO, TIME_NOW) => |s: &mut Self, u: &mut UTCB| handle_call(u, |u| s.time_now(u.get_badge())),
            (TIME_PROTO, MONO_NOW) => |s: &mut Self, u: &mut UTCB| handle_call(u, |u| s.mono_now(u.get_badge())),
            (TIME_PROTO, SLEEP) => |s: &mut Self, u: &mut UTCB| handle_call(u, |u| s.sleep(u.get_badge(), u.get_mr(0))),
            (TIME_PROTO, ADJ_TIME) => |s: &mut Self, u: &mut UTCB| handle_call(u, |u| s.adj_time(u.get_badge(), u.get_mr(0) as u64, u.get_mr(1) as i64)),
            (glenda::protocol::KERNEL_PROTO, glenda::protocol::kernel::NOTIFY) => |s: &mut Self, u: &mut UTCB| {
                handle_notify(u, |u| {
                    let badge = u.get_badge();
                    let bits = badge.bits();
                    // Determine flags
                    let is_hook = bits & NOTIFY_HOOK != 0;

                    if is_hook {
                        if let Err(e) = s.rescan_devices() {
                            error!("Failed to rescan devices: {:?}", e);
                        }
                    }

                    // Always check timers and update alarm when notified by kernel
                    let _ = s.check_timers();
                    let _ = s.update_alarm();

                    Ok(())
                })?;
                Err(Error::Success)
            },
            (_, _) => |_, u: &mut UTCB| {
                let tag = u.get_msg_tag();
                let badge = u.get_badge();
                error!(
                    "Unhandled message (proto={:#x}, label={:#x}, badge={:?})",
                    tag.proto(),
                    tag.label(),
                    badge
                );
                Err(Error::InvalidMethod)
            }
        }
    }
    fn reply(&mut self, utcb: &mut UTCB) -> Result<(), Error> {
        self.ipc.reply.reply(utcb)
    }
    fn stop(&mut self) {
        self.ipc.running = false;
        let _ = self.init_client.report_service(Badge::null(), ServiceState::Stopped);
    }
}
