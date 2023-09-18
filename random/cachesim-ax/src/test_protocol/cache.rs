use crate::simulator::interface::*;
use super::signals::*;

#[derive(Default)]
pub struct TestCache {}

impl Cache<SignalTypes> for TestCache {
    // clock
    fn on_tick(&mut self, send_proc_q: &mut SignalQ<CacheToProcSig>, send_bus: &mut SignalQ<CacheToBusSig>) {
        println!("on_tick from TestCache");
    }
    fn on_post_tick(&mut self) {
        println!("on_post_tick from TestCache");
    }
    // processor and bus communication
    fn on_proc_sig(&mut self, sig: ProcToCacheSig, send_proc_q: &mut SignalQ<CacheToProcSig>, send_bus: &mut SignalQ<CacheToBusSig>) {
        println!("on_proc_sig from TestCache");
    }
    fn on_bus_sig(&mut self, sig: BusToCacheSig, send_proc_q: &mut SignalQ<CacheToProcSig>, send_bus: &mut SignalQ<CacheToBusSig>) {
        println!("on_bus_sig from TestCache");
    }
}