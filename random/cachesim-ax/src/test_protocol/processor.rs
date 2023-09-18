use crate::simulator::interface::*;
use super::signals::*;


pub struct TestProcessor {
    pub instr: Insts,
}

impl Processor<SignalTypes> for TestProcessor {
    // clock
    fn on_tick(&mut self, send_cache_q: &mut SignalQ<ProcToCacheSig>) {
        println!("on_tick from TestProcessor");
        if let Some(inst) = self.instr.pop() {match inst {
            Instr::Read(addr) => send_cache_q.push_back((ProcToCacheSig::Req(addr), 0)),
            _ => (),
        }} else { panic!("where are my instructions :< I need at least 1"); }
    }
    fn on_post_tick<F: Fn() -> ()>(&mut self, done: F) {
        println!("on_post_tick from TestProcessor");
        if self.instr.len() == 0 { done(); }
    }
    // cache communication
    fn on_cache_sig(&mut self, sig: CacheToProcSig, send_cache_q: &mut SignalQ<ProcToCacheSig>) {
        println!("on_cache_sig from TestProcessor");
    }
}