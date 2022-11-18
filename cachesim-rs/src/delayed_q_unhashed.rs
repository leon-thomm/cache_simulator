// implements a message queue with discrete message delays

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::fmt::Error;
use std::rc::Rc;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use crate::Msg;

// delayed message type

#[derive(Clone)]
pub struct DelayedMsg<MsgType> {
    pub t: i32,
    pub msg: MsgType,
}

#[derive(Clone)]
pub struct DelQSender<MsgType>{
    q: Rc<RefCell<DelayedQ<MsgType>>>
}
impl<MsgType> DelQSender<MsgType> {
    pub fn send(&self, msg: DelayedMsg<MsgType>) -> Result<(), Err> {
        let mut queue = self.q.borrow_mut();
        let timed_msg: TimedMsg<MsgType> = TimedMsg { t: queue.time + msg.t, msg: msg.msg };
        let t = timed_msg.t;
        if t >= queue.key_last.unwrap_or(0) {
            queue.q.push_back(timed_msg);
        } else if t < queue.key_first.unwrap_or(i32::MAX) {
            queue.q.push_front(timed_msg);
        } else {
            let mut i = 0;
            for m in queue.q.iter() {
                if m.t > t {
                    break;
                }
                i += 1;
            }
            queue.q.insert(i, timed_msg);
        }
        queue.update_keys();
        Ok(())
    }
}
#[derive(Debug)]
pub enum Err{}

// timed message type

/*
    In TimesMsg, `t` stands for the timestamp at which the message should be made available,
    whereas `t` in DelayedMsg stands for the delay of the message from the time of issue.
 */

struct TimedMsg<MsgType> {
    t: i32,
    msg: MsgType,
}

impl<MsgType> Eq for TimedMsg<MsgType> {}

impl<MsgType> PartialEq for TimedMsg<MsgType> {
    fn eq(&self, other: &Self) -> bool {
        self.t == other.t
    }
}

impl<MsgType> Ord for TimedMsg<MsgType> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.t.cmp(&self.t)
    }
}

impl<MsgType> PartialOrd for TimedMsg<MsgType> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// delayed message queue

pub struct DelayedQ<MsgType> {
    time: i32,
    q: VecDeque<TimedMsg<MsgType>>,
    key_first: Option<i32>,
    key_last: Option<i32>,
}

impl<MsgType> DelayedQ<MsgType> {
    pub fn new() -> (DelayedQWrapper<MsgType>, DelQSender<MsgType>) {
        let self_ = Rc::new(RefCell::new(DelayedQ {
            time: 0,
            q: VecDeque::new(),
            key_first: None,
            key_last: None,
        }));
        let sender = DelQSender {
            q: self_.clone()
        };
        (DelayedQWrapper{ q: self_ }, sender)
    }
    pub fn try_fetch(&mut self) -> Option<MsgType> {
        if let Some(m) = self.q.front() {
            if m.t != self.time { return None; }
        }
        let msg = self.q.pop_front().map(|dm| dm.msg);
        self.update_keys();
        msg
    }
    fn update_keys(&mut self) {
        self.key_first = self.q.front().map(|m| m.t);
        self.key_last = self.q.back().map(|m| m.t);
    }
    pub fn update_time(&mut self, new_time: i32) {
        self.time = new_time;
    }
}

// a simple wrapper to have a similar interface to delayed_q::DelayedQ
pub struct DelayedQWrapper<MsgType> {
    q: Rc<RefCell<DelayedQ<MsgType>>>,
}
impl<MsgType> DelayedQWrapper<MsgType> {
    pub fn try_fetch(&mut self) -> Option<MsgType> {
        return self.q.borrow_mut().try_fetch()
    }
    pub fn update_time(&mut self, new_time: i32) { self.q.borrow_mut().update_time(new_time); }
    pub fn update_q(&self) {}
    pub fn is_empty(&self) -> bool {
        self.q.borrow().q.is_empty()
    }
    pub fn msg_available(&self) -> bool {
        self.q.borrow().q.len() > 0
    }
}