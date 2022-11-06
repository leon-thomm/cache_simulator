// implements a message queue with discrete message delays, based on std::sync::mpsc

use std::cmp::Ordering;
use std::sync::mpsc;

// delayed message type

#[derive(Clone)]
struct DelayedMsg<MsgType> {
    t: i32,
    msg: MsgType,
}

impl<MsgType> DelayedMsg<MsgType> {
    fn new(msg: MsgType, delay: i32) -> Self {
        DelayedMsg { msg, t: delay }
    }
    fn decrement(&mut self) {
        self.t -= 1;
    }
}

impl<MsgType> PartialEq for DelayedMsg<MsgType> {
    fn eq(&self, other: &Self) -> bool {
        self.t == other.t
    }
}

impl<MsgType> Eq for DelayedMsg<MsgType> {}

impl<MsgType> Ord for DelayedMsg<MsgType> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.t.cmp(&other.t)
    }
}

impl<MsgType> PartialOrd<Self> for DelayedMsg<MsgType> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// timed message type

type TimedMsg<MsgType> = DelayedMsg<MsgType>;
// to avoid having to mutate every element in the queue, which would either require
// unsafe code or rebuilding the whole heap, the queue does not decrease a delay,
// but instead increases its timestamp. to have a separate type for the separate
// meaning, the queue uses TimedMsg where t denotes the timestamp at which the
// message should be sent, whereas in DelayedMsg t denotes the remaining delay.

// delayed message queue

pub struct DelayedQ<MsgType> {
    q: std::collections::BinaryHeap<TimedMsg<MsgType>>,
    tx: mpsc::Sender<DelayedMsg<MsgType>>,
    rx: mpsc::Receiver<DelayedMsg<MsgType>>,
    time: i32,
}

impl<MsgType> DelayedQ<MsgType> {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        DelayedQ {
            q: std::collections::BinaryHeap::new(),
            tx,
            rx,
            time: 0,
        }
    }
    pub fn update_time(&mut self, dt: i32) {
        self.time += dt;
    }
    pub fn update_q(&mut self) {
        while let Ok(DelayedMsg{ t: d, msg: m}) = self.rx.try_recv() {
            // transform delay into timestamp
            self.q.push(TimedMsg::new(m, self.time + d));
        }
    }
    pub fn msg_available(&self) -> bool {
        if let Some(msg) = self.q.peek() {
            if      msg.t == self.time { true }
            else if msg.t > self.time { false }
            else { panic!("delayed message queue is out of sync: missed message") }
        } else { false }
    }
    pub fn try_fetch(&mut self) -> Option<MsgType> {
        // peek and check if there's a msg with delay 0
        if let Some(msg) = self.q.peek() {
            if msg.t == self.time {
                // pop and return the msg
                return Some(self.q.pop().unwrap().msg);
            }
        }
        None
    }
}

//
// #[derive(Clone)]
// struct DQSender<T> {
//     sender: mpsc::Sender<T>,
// }
//
// struct DelayedQ<T> {
//     tx: mpsc::Sender<(T, i32)>,
//     rx: mpsc::Receiver<(T, i32)>,
// }
//
// impl<T> DelayedQ<T> {
//     fn new() -> Self {
//         let (tx, rx) = mpsc::channel();
//         DelayedQ { tx, rx }
//     }
//     fn send(&self, msg: T, delay: i32) {
//         self.tx.send((msg, delay)).unwrap();
//     }
//     fn recv(&self) -> Option<T> {
//         let (msg, delay) = self.rx.recv().unwrap();
//         if delay > 0 {
//             self.send(msg, delay - 1);
//             None
//         } else {
//             Some(msg)
//         }
//     }
// }