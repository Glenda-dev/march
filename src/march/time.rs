use super::MarchService;
use glenda::arch::time::get_time;
use glenda::error::Error;
use glenda::interface::TimeService;
use glenda::ipc::Badge;
use glenda::utils::manager::CSpaceService;
use glenda_drivers::interface::TimerDriver;

impl<'a> TimeService for MarchService<'a> {
    fn time_now(&mut self, _badge: Badge) -> Result<u64, Error> {
        Ok(self.get_wall_time_ns())
    }
    fn mono_now(&mut self, _badge: Badge) -> Result<u64, Error> {
        Ok(self.get_mono_time_ns())
    }
    fn sleep(&mut self, _badge: Badge, ms: usize) -> Result<(), Error> {
        let now = self.get_wall_time_ns();
        let deadline = now + (ms as u64) * 1_000_000;
        let slot = self.cspace_mgr.alloc(self.res_client)?;
        self.cspace_mgr.root().move_cap(self.reply.cap(), slot)?;
        self.heap.push(deadline, slot);
        let _ = self.update_alarm();
        Ok(())
    }
    fn adj_time(&mut self, _badge: Badge, absolute_ns: u64, drift_ppb: i64) -> Result<(), Error> {
        if absolute_ns != 0 {
            if let Some(idx) = self.reference_index {
                let _ = self.timer_sources[idx].client.set_time(absolute_ns);
            }
            self.initial_ns = absolute_ns;
            self.initial_ticks = get_time();
        }
        if drift_ppb != 0 {
            self.drift_ppb = drift_ppb;
        }
        Ok(())
    }
}
