use glenda::arch::time::get_time;
use glenda::cap::{CapPtr, Endpoint, Kernel, Reply};
use glenda::client::{DeviceClient, InitClient, ResourceClient};
use glenda::error::Error;
use glenda::interface::device::DeviceService;
use glenda::ipc::{Badge, MsgTag, UTCB};
use glenda::protocol::device::{DeviceQuery, LogicDeviceType};
use glenda::utils::manager::{CSpaceManager, CSpaceService};
use glenda_drivers::client::timer::TimerClient;
use glenda_drivers::interface::TimerDriver;
use heap::TimerHeap;

pub mod heap;
pub mod server;
pub mod time;

pub struct TimerSource {
    pub name: alloc::string::String,
    pub freq: u64,
    pub client: TimerClient,
}

pub struct MarchService<'a> {
    pub kernel_cap: Kernel,
    pub initial_ns: u64,
    pub initial_ticks: u64,
    pub freq: u64,
    pub drift_ppb: i64,
    pub running: bool,
    pub endpoint: Endpoint,
    pub reply: Reply,
    pub recv: CapPtr,
    pub dev_client: &'a mut DeviceClient,
    pub res_client: &'a mut ResourceClient,
    pub cspace_mgr: &'a mut CSpaceManager,
    pub heap: TimerHeap,
    pub init_client: &'a mut InitClient,
    pub timer_sources: alloc::vec::Vec<TimerSource>,
    pub reference_index: Option<usize>,
}

impl<'a> MarchService<'a> {
    pub fn new(
        res_client: &'a mut ResourceClient,
        cspace_mgr: &'a mut CSpaceManager,
        dev_client: &'a mut DeviceClient,
        init_client: &'a mut InitClient,
        kernel_cap: Kernel,
    ) -> Self {
        Self {
            kernel_cap,
            initial_ns: 0,
            initial_ticks: 0,
            freq: 10_000_000,
            drift_ppb: 0,
            running: false,
            endpoint: Endpoint::from(CapPtr::null()),
            reply: Reply::from(CapPtr::null()),
            recv: CapPtr::null(),
            dev_client,
            res_client,
            cspace_mgr,
            heap: TimerHeap::new(),
            init_client,
            timer_sources: alloc::vec::Vec::new(),
            reference_index: None,
        }
    }

    pub fn update_time_base(&mut self, rtc_ns: u64, ticks: u64) {
        self.initial_ns = rtc_ns;
        self.initial_ticks = ticks;
        log!("Time base updated: {} ns", self.initial_ns);
    }

    pub fn get_wall_time_ns(&self) -> u64 {
        let current_ticks = get_time();
        let elapsed_ticks = current_ticks.wrapping_sub(self.initial_ticks);
        let mut elapsed_ns = (elapsed_ticks as u128 * 1_000_000_000 / self.freq as u128) as u64;
        if self.drift_ppb != 0 {
            let adj = (elapsed_ns as i128 * self.drift_ppb as i128 / 1_000_000_000) as i64;
            elapsed_ns = (elapsed_ns as i64 + adj) as u64;
        }
        self.initial_ns + elapsed_ns
    }

    pub fn get_mono_time_ns(&self) -> u64 {
        let current_ticks = get_time();
        (current_ticks as u128 * 1_000_000_000 / self.freq as u128) as u64
    }

    pub fn rescan_devices(&mut self) -> Result<(), Error> {
        let query = DeviceQuery { name: None, compatible: alloc::vec![], dev_type: Some(11) };
        if let Ok(names) = self.dev_client.query(Badge::null(), query) {
            for name in names {
                // Check if already discovered
                if self.timer_sources.iter().any(|s| s.name == name) {
                    continue;
                }

                if let Ok((_, desc)) = self.dev_client.get_logic_desc(Badge::null(), &name) {
                    if let LogicDeviceType::Timer(freq) = desc.dev_type {
                        log!("Discovered timer: {} with freq={} Hz", name, freq);
                        let slot = self.cspace_mgr.alloc(self.res_client)?;
                        let ep = self.dev_client.alloc_logic(Badge::null(), 11, &name, slot)?;
                        let tc = TimerClient::new(ep);

                        self.timer_sources.push(TimerSource {
                            name: name.clone(),
                            freq,
                            client: tc,
                        });
                    }
                }
            }
        }

        // Find best reference timer (highest frequency)
        let mut best_freq = 0;
        let mut best_idx = None;
        for (i, source) in self.timer_sources.iter().enumerate() {
            if source.freq > best_freq {
                best_freq = source.freq;
                best_idx = Some(i);
            }
        }

        if let Some(idx) = best_idx {
            if self.reference_index != Some(idx) {
                let name = self.timer_sources[idx].name.clone();
                log!("Selecting reference timer: {} (freq={})", name, best_freq);
                self.reference_index = Some(idx);
                let rtc_time = self.timer_sources[idx].client.get_time();
                let ticks = get_time();
                self.update_time_base(rtc_time, ticks);
            }
        }
        Ok(())
    }

    pub fn update_alarm(&mut self) -> Result<(), Error> {
        if let Some(deadline) = self.heap.peek_deadline() {
            let delta_ns = if deadline > self.initial_ns { deadline - self.initial_ns } else { 0 };
            let ticks =
                self.initial_ticks + (delta_ns as u128 * self.freq as u128 / 1_000_000_000) as u64;
            let kernel = self.kernel_cap;
            kernel.set_alarm(ticks as usize, self.endpoint.cap())?;
        }
        Ok(())
    }

    pub fn check_timers(&mut self) -> Result<(), Error> {
        let now = self.get_wall_time_ns();
        while let Some(slot) = self.heap.pop_expired(now) {
            let mut utcb = unsafe { UTCB::new() };
            utcb.clear();
            utcb.set_msg_tag(MsgTag::ok());
            let _ = Reply::from(slot).reply(&mut utcb);
            let _ = self.cspace_mgr.free(slot);
        }
        Ok(())
    }
}
