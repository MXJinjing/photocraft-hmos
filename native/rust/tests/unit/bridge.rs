use super::*;
#[test]
fn cancellation_wakes_waiters_and_rejects_new_requests() {
    let (tx, rx) = mpsc::channel();
    pending().lock().unwrap().insert(42, tx);
    cancel();
    assert_eq!(rx.recv().unwrap(), Err("cancelled".into()));
    assert_eq!(request("save", "x.psd", &[]), Err("cancelled".into()));
    reply(42, Ok(("late".into(), vec![]))); // Late picker completions are ignored.
    resume();
}
