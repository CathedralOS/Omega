use crate::CallFrameResolver;
use crate::machine_calls::calls::write_frames::CYCLE_EQUATIONS;

#[test]
fn callback_through_named_transition_never_caches_a_truncated_call_frame() {
    for backedge in ["", "transition { _ -> cycle(output) }"] {
        let program = crate::front_end::typed_program(
            &r#"
        data Output { count: u64; }
        data Carrier { output: &mut Output; }
        machine outer(output: &mut Output) {
            transition { _ -> cycle(output) }
            state cycle(output: &mut Output) {
                callback(Carrier { output: output });
                output.count = 1;
                BACKEDGE
            }
        }
        machine callback(carrier: Carrier) { outer(carrier.output); }
        "#
            .replace("BACKEDGE", backedge),
        );
        let outer = &program.machines()[0];
        let callback = &program.machines()[1];
        let callback_entry = &program.machine_states(callback)[0];
        let cycle = &program.machine_states(outer)[1];
        for warm_cycle in [false, true] {
            let resolver = CallFrameResolver::new(&program).expect("resolver");
            if warm_cycle {
                resolver.inferred_state_write_frame(outer, cycle);
            }
            let frame = resolver.inferred_state_write_frame(callback, callback_entry);
            if let Some(paths) = frame.complete_paths() {
                assert!(
                    paths.iter().any(|path| path == "$P0"
                        || path == "$P0.output"
                        || path == "$P0.output.count"),
                    "a complete callback frame must cover the transitive write, warm_cycle={warm_cycle}, backedge={backedge}: {frame:?}"
                );
            }
        }
    }
}

/// A named cycle whose edges carry an exclusive reference into one state and
/// drop it on the way back. Its effects are finite even without a bijection.
/// This tests effect inference, not admission of the overlapping arguments.
const REFERENCE_CYCLE: &str = r#"
data Output { accepted: bool; count: u64; }
machine classify(output: &mut Output) {
    transition { _ -> load(output) }
    state load(output: &mut Output) {
        output.count = 1;
        transition { _ -> dispatch(output, &mut output.count) }
    }
    state dispatch(output: &mut Output, count: &mut u64) {
        count = 2;
        transition output.accepted {
            false -> load(output)
            _ -> done(output)
        }
    }
    state done(output: &mut Output) {
        output.accepted = true;
    }
}
"#;

#[test]
fn reference_roots_can_enter_and_leave_a_cycle() {
    let program = crate::front_end::typed_program(REFERENCE_CYCLE);
    let machine = &program.machines()[0];
    let resolver = CallFrameResolver::new(&program).expect("resolver");
    CYCLE_EQUATIONS.with(|equations| equations.set(0));
    let frames = resolver.inferred_machine_state_write_frames(machine);
    for (position, frame) in frames.iter().enumerate() {
        let paths = frame.complete_paths().expect("finite root substitutions");
        assert!(paths.iter().any(|path| path == "$P0.accepted"), "{frame:?}");
        if position < 3 {
            assert!(paths.iter().any(|path| path == "$P0.count"), "{frame:?}");
        }
        if position == 2 {
            assert!(paths.iter().any(|path| path == "$P1"), "{frame:?}");
        }
    }
    assert!(CYCLE_EQUATIONS.with(|equations| equations.get()) > 0);
}

#[test]
fn cycle_substitution_retains_swapped_write_roots() {
    let program = crate::front_end::typed_program(
        r#"
        data Kind [copy] { case Next; case Stop; }
        machine swap(left: &mut u64, right: &mut u64, kind: Kind) {
            transition { _ -> cycle(left, right) }
            state cycle(left: &mut u64, right: &mut u64) {
                left = 1;
                transition { _ -> cycle(right, left) }
            }
        }
        "#,
    );
    let machine = &program.machines()[0];
    let resolver = CallFrameResolver::new(&program).expect("resolver");
    let frames = resolver.inferred_machine_state_write_frames(machine);
    assert_eq!(
        frames[1].complete_paths(),
        Some(["$P0".to_owned(), "$P1".to_owned()].as_slice()),
        "the swapped cycle still solves to both roots"
    );
}
