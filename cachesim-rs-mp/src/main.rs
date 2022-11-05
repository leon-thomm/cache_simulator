use std::sync::mpsc;

/*
    A MESI and Dragon cache coherence protocol simulator.
 */


enum Msg{}

// message with cycle delay
struct QMsg<MsgType>(i32, MsgType);


fn simulate(insts: Vec<Vec<Instruction>>) {
    let n = insts.len();

    // each component (processors, caches, bus) communicates to others by sending messages
    // to the simulator (main thread) via channels which will forward messages to the
    // intended recipient

    // implement everything single-threaded for now

    let (tx, rx) = mpsc::channel();

    fn tx_with_delay<MsgType>(tx: mpsc::Sender<QMsg<MsgType>>) -> fn(i32, MsgType) {
        | delay: i32, msg: MsgType | {
            tx.send(QMsg(delay, msg)).unwrap();
        }
    }

    let procs = (0..n).map(|i| {
        Processor::new(i, tx_with_delay(tx.clone()), insts[i].clone())
    }).collect::<Vec<_>>();

    let caches = (0..n).map(|i| {
        Cache::new(i, tx_with_delay(tx.clone()))
    }).collect::<Vec<_>>();

    let bus = Bus::new(tx_with_delay(tx.clone()));

    // simulate
    let cycle_count = 0;
    loop {
        // tick all processors
        for i in 0..n {
            tx.send(Message::Tick(i, 1)).unwrap();
        }

        while let Ok(msg) = rx.try_recv
    }
}


fn main() {
    let (tx, rx) = mpsc::channel();

    tx.send(MessageType::Msg("Hello".to_string())).unwrap();
    tx.send(MessageType::Msg("World".to_string())).unwrap();

    loop {
        match rx.recv() {
            Ok(MessageType::Msg(msg)) => println!("{}", msg),
            Ok(MessageType::Quit) => break,
            Err(_) => break,
        }
    }
}
