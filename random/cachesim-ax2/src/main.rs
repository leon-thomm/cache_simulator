mod simulator;
use simulator::component::*;

use std::marker::Sized;
use std::collections::VecDeque;

enum PCMsg {
    Read,
}

impl MsgType for PCMsg {}

enum PMsg {
    PCMsg(PCMsg),
}

impl MsgType for PMsg {}

struct MyComponent {
    pub outputs: Vec<Outupt<PMsg>>,
}

impl Component<PMsg> for MyComponent {
    fn tick(&mut self, scheduler: &mut Scheduler<PMsg>) {
        scheduler.send(self.outputs[0], PMsg::PCMsg(PCMsg::Read), 0);
    }
    fn post_tick(&mut self) {}
    fn done() -> bool {
        false
    }
}

impl MyComponent {
    fn new() -> MyComponent {
        MyComponent { outputs: vec![Outupt::<PCMsg>::new()] }
    }

    fn inp_cache_response(&mut self, scheduler: &mut Scheduler, msg: &CacheResponseMsg) {

    }
    fn inp_manual_interrupt(&mut self, scheduler: &mut Scheduler, msg: &ManualInterruptMsg) {

    }
}

fn main() {
    let mut c = MyComponent {};
    let mut s = Scheduler { q: VecDeque::new() };
    c.tick(&mut s);
    println!("Hello, world!");
}
