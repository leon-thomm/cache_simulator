use cachesim_rs::delayed_q_unhashed::{DelayedQ, DelayedMsg};

#[test]
fn test_delayed_queue() {
    // test delayed queue

    let (mut dq, tx) = DelayedQ::<i32>::new();

    tx.send(DelayedMsg {
        t: 0,
        msg: 42,
    }).unwrap();

    tx.send(DelayedMsg {
        t: 0,
        msg: 43,
    }).unwrap();

    tx.send(DelayedMsg {
        t: 1,
        msg: 44,
    }).unwrap();

    dq.update_q();
    let mut c = 0;
    let mut x = false;
    while dq.msg_available() {
        println!("messages after {} cycles:", c);
        while let Some(msg) = dq.try_fetch() {
            println!("msg: {}", msg);
            if !x {
                tx.send(DelayedMsg { t: 0, msg: 100 }).unwrap();
                dq.update_q();
                x = true;
                println!("appended another message in cycle {}", c);
            }
        }
        c += 1;
        dq.update_time(c);
    }

    println!("done, cycles: {}", c);
}