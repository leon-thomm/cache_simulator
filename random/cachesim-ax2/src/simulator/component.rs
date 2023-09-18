/*
    This module defines components. Components will later be wrapped into models, 
    which will be composed into an asynchronix simulation system.
    Components have the following properties:
    - They have a custom set of outputs, which can later be connected to inputs of other components.
    - They have two default inputs: tick and post_tick.
    - They may have more custom inputs, which can be defined by the user.
    - All inputs, except post_tick, receive a scheduler argument, which can be used to send messages to other components.
*/

use std::collections::VecDeque;

pub trait MsgType {}

pub struct Scheduler<T: MsgType> {
    pub q: VecDeque<(T, usize, u64)>,
}

// implement 'send' function for sending messages, but only accept message types that are in the Outputs list
impl Scheduler {
    pub fn send<T: MsgType>(&mut self, output: Outupt<T>, msg: T, delay: u64) {
        self.q.push_back((msg, output, delay));
    }
}

pub trait Component<OutputMessageT: MsgType> {
    fn tick(&mut self, scheduler: &mut Scheduler);
    fn post_tick(&mut self);
    fn done() -> bool;
}

pub struct Outupt<T: MsgType> {
    pub msg_t: T,
}