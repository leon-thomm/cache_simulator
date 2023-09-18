use asynchronix::model::Model;
use asynchronix::simulation::{Mailbox, SimInit, Simulation};
use asynchronix::time::MonotonicTime;

mod components;
use components::*;

type MsgType1 = u32;
type MsgType2 = String;

#[derive(Default)]
struct Comp0 {
    out: <Comp0 as Component>::OutputTypes,
}

impl Model for Comp0 {}

impl Component for Comp0 {
    type OutputTypes = (DelayedOutput<MsgType1>, DelayedOutput<MsgType2>);
    fn on_tick(&mut self) {
        println!("Comp1 tick");
        self.out.0.send(1, 1);
        self.out.1.send("hello".into(), 1);
    }
    fn on_post_tick(&mut self) {}
    fn get_outputs(&mut self) -> Option<&mut Self::OutputTypes> { Some(&mut self.out) }
}

#[derive(Default)]
struct Comp1 {}

impl Model for Comp1 {}

impl Component for Comp1 {
    type OutputTypes = ();
    fn on_tick(&mut self) {}
    fn on_post_tick(&mut self) {}
    fn get_outputs(&mut self) -> Option<&mut Self::OutputTypes> { None }
}

impl Comp1 {
    fn recv_msg1(&mut self, msg: MsgType1) {
        println!("received message: {} in Comp2", msg);
    }
}

#[derive(Default)]
struct Comp2 {}

impl Model for Comp2 {}

impl Component for Comp2 {
    type OutputTypes = ();
    fn on_tick(&mut self) {}
    fn on_post_tick(&mut self) {}
    fn get_outputs(&mut self) -> Option<&mut Self::OutputTypes> { None }
}

impl Comp2 {
    fn recv_msg2(&mut self, msg: MsgType2) {
        println!("received message: {} in Comp3", msg);
    }
}

/*
                COMPONENT 0
                 /       \ 
                |         |
           u32  v         v  str
        COMPONENT 1     COMPONENT 2
*/

fn main() {
    let mut c0 = Comp0::default();
    let mut c1 = Comp1::default();
    let mut c2 = Comp2::default();

    let mut c0_mbox = Mailbox::<Comp0>::new();
    let mut c1_mbox = Mailbox::<Comp1>::new();
    let mut c2_mbox = Mailbox::<Comp2>::new();

    let mut c0_addr = c0_mbox.address();
    let mut c1_addr = c1_mbox.address();
    let mut c2_addr = c2_mbox.address();
    
    c0.out.0.output.connect(Comp1::recv_msg1, &c1_addr);
    c0.out.1.output.connect(Comp2::recv_msg2, &c2_addr);

    let mut sim = SimInit::new()
        .add_model(c0, c0_mbox)
        .add_model(c1, c1_mbox)
        .add_model(c2, c2_mbox)
        .init(MonotonicTime::EPOCH);

    
    let tick_comps = |sim: &mut Simulation| {
        sim.send_event(Comp0::on_tick, (), &c0_addr);
        sim.send_event(Comp1::on_tick, (), &c1_addr);
        sim.send_event(Comp2::on_tick, (), &c2_addr);
        sim.step();
    };

    let post_tick_comps = |sim: &mut Simulation| {
        sim.send_event(Comp0::on_post_tick, (), &c0_addr);
        sim.send_event(Comp1::on_post_tick, (), &c1_addr);
        sim.send_event(Comp2::on_post_tick, (), &c2_addr);
        sim.step();
    };

    let release_outputs = |sim: &mut Simulation| {
        sim.send_event(Component::release_outputs, (), &c0_addr);
        sim.send_event(Component::release_outputs, (), &c1_addr);
        sim.send_event(Component::release_outputs, (), &c2_addr);
        sim.step();
    };

    for i in 0..2 {
        println!("tick {}", i);
        tick_comps(&mut sim);
        release_outputs(&mut sim);
        post_tick_comps(&mut sim);
    }
    println!("done");

}