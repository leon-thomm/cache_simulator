// implements a message queue with discrete message delays, based on std::sync::mpsc

use std::cmp::Ordering;
use std::sync::mpsc;

// delayed message type

#[derive(Clone)]
pub struct DelayedMsg<MsgType> {
    pub t: i32,
    pub msg: MsgType,
}

pub type DelQSender<MsgType> = mpsc::Sender<DelayedMsg<MsgType>>;

// timed message type

/*
    to avoid having to mutate every element in the queue, which would either require
    unsafe code or rebuilding the whole heap, the queue does not decrease a delay,
    but instead increases its timestamp. the queue uses TimedMsg where t denotes the
    timestamp at which the message should be sent (and the monotonically increased
    ord defines the order of different time messages within one timestamp),
    whereas in DelayedMsg t denotes the remaining delay.
 */

struct TimedMsg<MsgType> {
    t: i32,
    ord: i32,
    msg: MsgType,
}

impl<MsgType> Eq for TimedMsg<MsgType> {}

impl<MsgType> PartialEq for TimedMsg<MsgType> {
    fn eq(&self, other: &Self) -> bool {
        (self.t, self.ord) == (other.t, other.ord)
    }
}

impl<MsgType> Ord for TimedMsg<MsgType> {
    fn cmp(&self, other: &Self) -> Ordering {
        // lexicographically
        (other.t, other.ord).cmp(&(self.t, self.ord))
    }
}

impl<MsgType> PartialOrd for TimedMsg<MsgType> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// delayed message queue

pub struct DelayedQ<MsgType> {
    q: std::collections::BinaryHeap<TimedMsg<MsgType>>,
    rx: mpsc::Receiver<DelayedMsg<MsgType>>,
    time: i32,
    ord_ctr: i32,
}

impl<MsgType> DelayedQ<MsgType> {
    pub fn new() -> (Self, DelQSender<MsgType>) {
        let (tx, rx) = mpsc::channel();
        (DelayedQ {
            q: std::collections::BinaryHeap::new(),
            rx,
            time: 0,
            ord_ctr: 0
        }, tx)
    }
    pub fn update_time(&mut self, new_time: i32) {
        self.time = new_time;
    }
    pub fn update_q(&mut self) {
        while let Ok(DelayedMsg{ t: d, msg: m}) = self.rx.try_recv() {
            // transform delay into timestamp
            self.q.push(TimedMsg{ t: self.time + d, msg: m, ord: self.ord_ctr });
            self.ord_ctr += 1;
        }
    }
    pub fn is_empty(&mut self) -> bool {
        // self.update_q();
        self.q.is_empty()
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