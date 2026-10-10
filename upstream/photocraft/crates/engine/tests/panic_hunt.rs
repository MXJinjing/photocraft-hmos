//! Fuzz every command with adversarial params; a command must return `Err`, never panic or hang.
//! (Rule 9 in AGENTS.md.) Each command runs on its own thread with a timeout, so one bad command
//! can't wedge the run — panics are caught and hangs are reported. Opt-in (slow):
//! `cargo test -p photocraft-engine --test panic_hunt -- --ignored`.
use photocraft_engine::{Session, command_specs};
use serde_json::{Value, json};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc;
use std::time::Duration;

fn fresh() -> Session {
    let mut s = Session::new();
    s.execute("file.new", json!({"width": 24, "height": 16})).unwrap();
    s.execute("layer.new.layer", json!({})).unwrap();
    s.execute("select.rect", json!({"x": 1, "y": 1, "width": 6, "height": 5})).unwrap();
    s
}

/// Run `id` with `p` on a fresh session, on a worker thread. Returns `Err` with a reason on panic
/// or timeout, `Ok` otherwise (whether the command returned Ok or a graceful Err).
fn run_guarded(id: &str, p: &Value) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    let (id, p) = (id.to_string(), test_params(id, p));
    std::thread::spawn(move || {
        let r = catch_unwind(AssertUnwindSafe(|| {
            let mut s = fresh();
            let _ = s.execute(&id, p);
        }));
        let _ = tx.send(r.is_ok());
    });
    match rx.recv_timeout(Duration::from_secs(4)) {
        Ok(true) => Ok(()),
        Ok(false) => Err("panicked".into()),
        Err(_) => Err("hung (>4s)".into()),
    }
}

// Registry fuzzing must never send documents to the contributor's default printer.
// Render the PDF and exercise layout validation, but suppress every spooler call.
fn test_params(id: &str, p: &Value) -> Value {
    let mut p = p.clone();
    if matches!(id, "file.print" | "file.printOneCopy")
        && let Some(fields) = p.as_object_mut()
    {
        fields.insert("dryRun".into(), json!(true));
        fields.insert("send".into(), json!(false));
    }
    p
}

#[test]
fn print_fuzzing_always_renders_pdf_without_spooling() {
    for id in ["file.print", "file.printOneCopy"] {
        for params in [json!({}), json!({"send": true, "dryRun": false})] {
            let safe = test_params(id, &params);
            assert_eq!(safe["send"], false);
            assert_eq!(safe["dryRun"], true);
        }
    }
    assert_eq!(test_params("file.new", &json!({})), json!({}));
}

#[test]
#[ignore = "slow full-registry fuzz; run with --ignored"]
fn no_command_panics_or_hangs_on_adversarial_params() {
    let params: Vec<Value> = vec![
        json!({}),
        json!({"x": -9, "y": -9, "radius": 0, "amount": 0, "angle": 0, "opacity": 0, "scale": 0, "levels": 0, "tolerance": 0, "gamma": 0, "layer": 999999, "channel": 999, "index": 999999, "count": 0}),
        json!({"x": 1e5, "y": -1e5, "radius": 300, "amount": 300, "angle": 1e5, "opacity": -100, "scale": -5, "gamma": -1}),
        json!({"points": [], "from": [0, 0], "to": [0, 0], "stops": [], "matrix": [0, 0, 0, 0, 0, 0], "colors": [], "name": "", "mode": "", "style": ""}),
    ];
    let mut bad: Vec<String> = Vec::new();
    for spec in command_specs() {
        for p in &params {
            if let Err(why) = run_guarded(spec.id, p) {
                bad.push(format!("{} [{why}] <- {p}", spec.id));
                break;
            }
        }
    }
    assert!(bad.is_empty(), "{} commands panicked or hung:\n{}", bad.len(), bad.join("\n"));
}
