/// This module provides types and traits for easily building components, which are
/// asynchronix models with a few extra features:
/// 
/// - a tick input function which is called every cycle
/// - an optional post-tick input function which is called after all components have ticked
/// - *delayed* outputs for sending messages to other components with a discrete delay
/// 
/// Notice that all these input functions are sync. Rust does not currently support
/// async traits, but asynchronix works with sync inputs as well.
/// 
/// Components can be written like this:
/// 
/// ```rust
/// struct MyComponent {
///     out: <MyComponent as Component>::OutputTypes,
/// }
/// 
/// impl Model for MyComponent {}
/// 
/// impl Component for MyComponent {
///     type OutputTypes = (DelayedOutput<String>, DelayedOutput<i32>);
///     fn on_tick(&mut self) {
///         self.out.0.send("hello".into(), 1);     // send "hello" msg on output 0 in the next cycle
///         self.out.1.send(42, 2);                 // send 42 on output 1 in two cycles
///     }
///     fn on_post_tick(&mut self) {}                // no post-tick cleanup needed
///     fn get_outputs(&mut self) -> Option<&mut Self::OutputTypes> { Some(&mut self.out) }
/// }
/// ```

use std::collections::VecDeque;
use asynchronix::model::{Model, Output};
use futures::executor;

// a trait for types that can be sent as messages; automatically implemented for all types
// for which this is the case
#[allow(non_camel_case_types)]
pub trait AX_OUT: Clone + Send + 'static {}
impl<T: Clone + Send + 'static> AX_OUT for T {}

struct TimedMsg<MsgT> {
    msg: MsgT,
    time: u64,
}

// a wrapper for Output which allows sending messages with a delay
// this requires a queue of messages to be sent.
// I like this slightly more than the self-scheduling approach
#[derive(Default)]
pub struct DelayedOutput<MsgT: AX_OUT> {
    pub output: Output<MsgT>,
    queue: VecDeque<TimedMsg<MsgT>>,
    time: u64,
}

impl<MsgT: AX_OUT> DelayedOutput<MsgT> {
    pub fn send(&mut self, msg: MsgT, delay: u64) {
        self.queue.push_back(TimedMsg {
            msg,
            time: self.time + delay,
        });
    }
    fn release(&mut self) {
        while let Some(TimedMsg { msg, time }) = self.queue.front() {
            if *time > self.time { break; }
            let TimedMsg { msg, .. } = self.queue.pop_front().unwrap();
            executor::block_on(self.output.send(msg));
        }
        self.time += 1;
    }
}

pub trait Component: Model {
    type OutputTypes: ComponentOutputs;
    fn on_tick(&mut self);
    fn on_post_tick(&mut self) {}
    fn get_outputs(&mut self) -> Option<&mut Self::OutputTypes>;
    fn release_outputs(&mut self) {
        if let Some(outputs) = self.get_outputs() {
            outputs.release();
        }
    }
}

// ================ implement ComponentOutputs for tuples of DelayedOutput types: ================

pub trait ComponentOutputs {
    fn release(&mut self);
}

macro_rules! impl_component_outputs {
    ($($T:ident => $i:tt),*) => {
        impl<$($T: AX_OUT),*> ComponentOutputs for ($(DelayedOutput<$T>,)*) {
            fn release(&mut self) {
                $(self.$i.release();)*
            }
        }
    };
}

/*
    The macro simply converts e.g.

        impl_component_outputs!(T0 => 0, T1 => 1, T2 => 2);
    
    into
    
        impl<T0: AX_OUT, T1: AX_OUT, T2: AX_OUT> ComponentOutputs for (DelayedOutput<T0>, DelayedOutput<T1>, DelayedOutput<T2>) {
            fn release(&mut self) {
                self.0.release();
                self.1.release();
                self.2.release();
            }
        }
    
    So it implements ComponentOutputs for tuples (of manually specified length) of DelayedOutput types.
    This allows heterogeneous output types, i.e. a component can have e.g. one output of message type u32 and another of type String, etc.
    This trait also implements a function to push all output messages that are due.
 */

impl_component_outputs!();
impl_component_outputs!(T0 => 0);
impl_component_outputs!(T0 => 0, T1 => 1);
impl_component_outputs!(T0 => 0, T1 => 1, T2 => 2);
impl_component_outputs!(T0 => 0, T1 => 1, T2 => 2, T3 => 3);
impl_component_outputs!(T0 => 0, T1 => 1, T2 => 2, T3 => 3, T4 => 4);
impl_component_outputs!(T0 => 0, T1 => 1, T2 => 2, T3 => 3, T4 => 4, T5 => 5);
impl_component_outputs!(T0 => 0, T1 => 1, T2 => 2, T3 => 3, T4 => 4, T5 => 5, T6 => 6);
impl_component_outputs!(T0 => 0, T1 => 1, T2 => 2, T3 => 3, T4 => 4, T5 => 5, T6 => 6, T7 => 7);
impl_component_outputs!(T0 => 0, T1 => 1, T2 => 2, T3 => 3, T4 => 4, T5 => 5, T6 => 6, T7 => 7, T8 => 8);
impl_component_outputs!(T0 => 0, T1 => 1, T2 => 2, T3 => 3, T4 => 4, T5 => 5, T6 => 6, T7 => 7, T8 => 8, T9 => 9);
