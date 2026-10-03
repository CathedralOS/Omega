use super::check_program;

const SUPPORT: &str = "
    data Packet { value: &u64; }
    machine install(out: &mut Packet, source: &u64) { out.value = source; }
    machine observe(packet: &Packet) -> u64 { packet.value }
";

#[test]
fn stored_call_loan_protects_backing_until_destination_last_use() {
    for (body, accepted) in [
        (
            "install(&mut packet, source); _ = observe(&packet); source = 9;",
            true,
        ),
        (
            "install(&mut packet, source); source = 9; _ = observe(&packet);",
            false,
        ),
    ] {
        let source = format!(
            "{SUPPORT}
            machine caller(old: &u64, source: &mut u64) {{
                let packet: Packet = Packet {{ value: old }};
                {body}
            }}"
        );
        let checked = check_program(&source);
        assert_eq!(checked.is_ok(), accepted, "{body}: {checked:?}");
    }
}

#[test]
fn stored_call_loan_definite_replacement_releases_only_old_slot() {
    check_program(&format!(
        "{SUPPORT}
        machine caller(old: &mut u64, source: &u64) {{
            let packet: Packet = Packet {{ value: old }};
            install(&mut packet, source);
            old = 9;
            _ = observe(&packet);
        }}"
    ))
    .expect("the overwritten source is no longer carried");
}

#[test]
fn stored_call_loan_rejects_local_backing_escape() {
    check_program(
        "
        data Packet { value: &u64; }
        machine install(out: &mut Packet) {
            let local: u64 = 7;
            out.value = &local;
        }
        machine caller(out: &mut Packet) { install(out); }
    ",
    )
    .expect_err("an output slot cannot retain the callee's local backing");
}

#[test]
fn stored_call_loan_survives_named_state_transport() {
    check_program(
        "
        data Packet { value: &u64; }
        machine install(out: &mut Packet, source: &u64) {
            out.value = source;
            transition { _ -> done(out) }
            state done(out: &mut Packet) { let seen: u64 = out.value; }
        }
        machine caller(out: &mut Packet, source: &u64) { install(out, source); }
    ",
    )
    .expect("stored shared backing survives an ordinary named edge");

    check_program(
        "
        data Packet { value: &u64; }
        machine install(out: &mut Packet, source: &mut u64) {
            out.value = source;
            transition { _ -> corrupt(out, source) }
            state corrupt(out: &mut Packet, source: &mut u64) {
                source = 9;
                let seen: u64 = out.value;
            }
        }
    ",
    )
    .expect_err("a named edge cannot lend mutable backing beside a live stored loan");
}

#[test]
fn stored_call_loan_retains_local_backing_until_successor_replacement() {
    check_program(
        "
        data Packet { value: &u64; }
        machine install(out: &mut Packet, replacement: &u64) {
            let local: u64 = 7;
            out.value = &local;
            transition { _ -> replace(out, replacement) }
            state replace(out: &mut Packet, replacement: &u64) { out.value = replacement; }
        }
    ",
    )
    .expect("the local home stays live until the successor replaces its stored loan");
}

#[test]
fn stored_call_loan_rejects_reestablishing_live_local_backing() {
    check_program(
        "
        data Packet { value: &u64; }
        machine install(out: &mut Packet, replacement: &u64, repeat: bool) {
            transition { _ -> fill(out, replacement, repeat) }
            state fill(out: &mut Packet, replacement: &u64, repeat: bool) {
                let local: u64 = 7;
                out.value = &local;
                transition repeat {
                    true -> fill(out, replacement, false)
                    _ -> replace(out, replacement)
                }
            }
            state replace(out: &mut Packet, replacement: &u64) { out.value = replacement; }
        }
    ",
    )
    .expect_err("a later replacement cannot excuse reinitializing live borrowed backing");
}

#[test]
fn stored_call_loan_keeps_copied_slot_source_after_replacement() {
    for (capture, accepted) in [("out.value", false), ("replacement", true)] {
        let source = format!(
            "
            data Packet {{ value: &u64; }}
            machine install(out: &mut Packet, replacement: &u64) {{
                let local: u64 = 7;
                out.value = &local;
                transition {{ _ -> save(out, replacement) }}
                state save(out: &mut Packet, replacement: &u64) {{
                    let saved: &u64 = {capture};
                    out.value = replacement;
                    transition {{ _ -> publish(out, saved) }}
                }}
                state publish(out: &mut Packet, saved: &u64) {{ out.value = saved; }}
            }}
        "
        );
        let result = check_program(&source);
        assert_eq!(result.is_ok(), accepted, "captured {capture}: {result:?}");
    }
}

#[test]
fn stored_call_loan_static_replacement_releases_only_the_replaced_carrier() {
    for (copy, use_copy, accepted) in [
        ("", "", true),
        (
            "let saved: &[u8] = packet.bytes;",
            "let old_length: u64 = saved.len();",
            false,
        ),
    ] {
        let source = format!(
            r#"
            data Packet {{ bytes: &[u8]; }}
            machine assign(out: &mut Packet, bytes: &[u8]) {{ out.bytes = bytes; }}
            machine forward(out: &mut Packet) {{ assign(out, "look"); }}
            machine caller(old: &mut [u8; 4]) {{
                let packet: Packet = Packet {{ bytes: old[0..4] }};
                {copy}
                forward(&mut packet);
                old[0] = 9;
                let length: u64 = packet.bytes.len();
                {use_copy}
            }}
        "#
        );
        let result = check_program(&source);
        assert_eq!(result.is_ok(), accepted, "copy={copy}: {result:?}");
    }
}
