Another attempt centered around the idea of providing a shim layer around asynchronix to make implementation of CPU components easier. Specifically, I defined a `Component` trait which gives asynchronix models a few extra capabilities:

- A component, by default, has `tick()` and `post_tick()` inputs.
- Further inputs are specified as custom functions on the component implementation.
- Outputs are of type `DelayedOutput` which wraps asynchronix outputs.
    - Output messages can be sent with a discrete delay.
    - This avoids the self-scheduling approach outlined in the asynchronix docs, which I think gets tedious quickly
    - A delayed component holds a queue of messages to be sent, and dispatch functionality is automatically implemented.
    - The `Component` trait already provides all necessary functionality to dispatch all outputs messages that are due. Since the particular output types are generic, this required some trait-macro magic.

I do like this a lot more than the previous iterations, this is highly independent on the actual nature of the particular component. An example would be:

```rust
#[derive(Default)]
struct MyComp {
    out: <MyComp as Component>::OutputTypes,
}

impl Model for MyComp {}

impl Component for MyComp {
    // this declares output types and does not require a hierarchy of
    // message types, which usually results in monsters like
    //  `send(Msg::Proc::ProcToCache(ProcToCacheMsg::ReadReq(1)...`
    type OutputTypes = (DelayedOutput<i32>, DelayedOutput<String>);

    fn on_tick(&mut self) {
        self.out.0.send(1, 1);
        self.out.1.send("hello".into(), 1);
    }
    fn on_post_tick(&mut self) {}
    fn get_outputs(&mut self) -> Option<&mut Self::OutputTypes> { Some(&mut self.out) }
}
```

But I also realize that

1. This shim layer is really shim and only provides some syntactic sugar as opposed to just implementing asynchronix models directly.
2. This hides asynchronix features. E.g. to support request output types I would need to specifically implement a wrapper to make that delayed.

Is it really worth throwing away possibly much of the asynchronix API just for delayed outputs?


## asynchronix issue:


<!-- When modeling a system where the physical constraints or abstracted communication aspects are causing a piece of information sent from one component to another takes a certain amount of time, I repeatedly find myself essentially implementing an output type which allows for sending messages. -->

What is the best way to implement sending messages to an output with a delay? [The docs](https://docs.rs/asynchronix/latest/asynchronix/index.html#a-model-using-the-local-scheduler) suggest a self-scheduling mechanism, e.g.:

```rust
scheduler.schedule_event(Duration::from_secs(1), Self::push_msg, msg);
```

with a corresponding handler

```rust
fn push_msg(&mut self, msg: Msg) {
    self.output.send(msg);
}
```

but I feel this gets a bit tedious and error-prone when the number of outputs (and their purposes) grows. Ideally, I would like to simply write

```rust
self.output.send_in(Duration::from_secs(1), msg);
```

Is there something like this? I found myself implementing this on top of asynchronix

```rust
// AX_OUT just describes all types that can be sent over outputs
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
```

but this has drawbacks and I would like to stick closer asynchronix.