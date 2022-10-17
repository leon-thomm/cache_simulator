/// A cache coherence protocol simulator for MESI and the Dragon protocol.

struct Processor {
    // instructions
    // cache
    // state
}
impl Processor {
    // new
    // tick
}

struct Cache {
    // state
    // data
    // bus reference
}
impl Cache {
    // new
    // proc_signal
    // bus_signal
    // on_bus_ready  // callback invoked once the bus executes a queued request from this cache
}

struct Bus {
    // state
    // cache references
    // requests
}
impl Bus {
    // new

    // tick
    fn tick(&mut self) {
        self.state = match self.state {
            BusState::Idle => {
                // check for queued requests
                if self.requests.len() > 0 {
                    let r = requests.pop();
                    // send request to cache, returns the busy time for the bus
                    BusState::Busy(r.cache.on_bus_ready(r))
                } else {
                    BusState::Idle
                }
            },
            BusState::Busy(busy_time) => {
                if busy_time > 0 {
                    BusState::Busy(busy_time - 1)
                } else {
                    BusState::Idle
                }
            }
        }
    }

    // cache_signal / enqueue_request
}


fn main() {
    println!("Hello, world!");
}
