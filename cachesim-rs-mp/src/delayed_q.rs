// implements a message queue with discrete message delays, based on std::sync::mpsc

use std::cmp::Ordering;
use std::sync::mpsc;

// delayed message type

#[derive(Clone)]
pub struct DelayedMsg<MsgType> {
    pub t: i32,
    pub msg: MsgType,
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

pub type DelQSender<MsgType> = mpsc::Sender<DelayedMsg<MsgType>>;

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
    pub tx: mpsc::Sender<DelayedMsg<MsgType>>,
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
    pub fn update_time(&mut self, new_time: i32) {
        self.time = new_time;
    }
    pub fn update_q(&mut self) {
        while let Ok(DelayedMsg{ t: d, msg: m}) = self.rx.try_recv() {
            // transform delay into timestamp
            self.q.push(TimedMsg{ t: self.time + d, msg: m });
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