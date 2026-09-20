use crate::CallFrameResolver;
use crate::machine_calls::calls::write_frames::CYCLE_EQUATIONS;
use crate::machine_calls::calls::write_frames::PREFIX_WALKS;
use crate::machine_calls::calls::write_frames::transition_topology::reachable_cycle_edges_can_permute_write_parameters;
use typed_trees::TypedTrees;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("symbols");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("types")
}

#[test]
fn cycle_statement_calls_reuse_complete_callee_frames() {
    let program = typed(
        r#"
        data Output { count: u64; untouched: u64; }
        machine write(output: &mut Output) { output.count = 1; }
        machine swap(left: &mut Output, right: &mut Output) {
            transition { _ -> cycle(left, right) }
            state cycle(left: &mut Output, right: &mut Output) {
                write(left);
                write(right);
                transition { _ -> cycle(right, left) }
            }
        }
        "#,
    );
    let helper = &program.machines()[0];
    let cycle_machine = &program.machines()[1];
    let cycle = &program.machine_states(cycle_machine)[1];
    let resolver = CallFrameResolver::new(&program).expect("resolver");
    let helper_frames = resolver.inferred_machine_state_write_frames(helper);
    assert_eq!(
        helper_frames[0].complete_paths(),
        Some(["$P0.count".to_owned()].as_slice())
    );
    PREFIX_WALKS.with(|walks| walks.set(0));
    let frame = resolver.inferred_state_write_frame(cycle_machine, cycle);
    let walks = PREFIX_WALKS.with(|walks| walks.get());
    assert_eq!(
        frame.complete_paths(),
        Some(["$P0.count".to_owned(), "$P1.count".to_owned()].as_slice()),
        "permuted roots retain exact fields without invalidating untouched storage"
    );
    assert_eq!(
        walks, 0,
        "the already complete helper needs no further walk"
    );
}

#[test]
fn callback_through_named_transition_never_caches_a_truncated_call_frame() {
    for backedge in ["", "transition { _ -> cycle(output) }"] {
        let program = typed(
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
/// drop it on the way back. A `&mut u64` is a write-capable root, so no edge
/// can be an exact permutation.
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

/// The same cycle carrying a copy enum instead. `Kind` contains no exclusive
/// reference anywhere in its structure, so it is the state's own storage,
/// not a write-capable root: every edge permutes the one `&mut Output` and
/// the solver recovers the complete frame.
const KIND_CYCLE: &str = r#"
data Kind [copy] { case Next; case Stop; }
data Output { accepted: bool; count: u64; }
machine classify(output: &mut Output) {
    transition { _ -> load(output) }
    state load(output: &mut Output) {
        output.count = 1;
        transition { _ -> dispatch(output, Kind::Next) }
    }
    state dispatch(output: &mut Output, kind: Kind) {
        transition kind {
            Kind::Next -> load(output)
            _ -> done(output)
        }
    }
    state done(output: &mut Output) {
        output.accepted = true;
    }
}
"#;

#[test]
fn count_mismatched_cycle_declines_before_building_equations() {
    let program = typed(REFERENCE_CYCLE);
    let machine = &program.machines()[0];
    let resolver = CallFrameResolver::new(&program).expect("resolver");
    CYCLE_EQUATIONS.with(|equations| equations.set(0));
    let frames = resolver.inferred_machine_state_write_frames(machine);
    let equations = CYCLE_EQUATIONS.with(|equations| equations.get());
    let outcomes = frames
        .iter()
        .map(|frame| frame.complete_paths().map(<[String]>::to_vec))
        .collect::<Vec<_>>();
    assert_eq!(
        outcomes,
        vec![None, None, None, Some(vec!["$P0.accepted".to_owned()])],
        "the prefix walk keeps the cycle opaque and the acyclic tail complete"
    );
    assert_eq!(
        equations, 0,
        "a topology that cannot permute by count never builds a frame equation"
    );
}

#[test]
fn topology_precheck_ignores_mismatched_edges_outside_cycles() {
    let program = typed(
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
    let states = program.machine_states(machine);
    assert!(
        reachable_cycle_edges_can_permute_write_parameters(&program, machine, &states[0]),
        "the entry edge drops `kind` but lies on no cycle"
    );
    assert!(reachable_cycle_edges_can_permute_write_parameters(
        &program, machine, &states[1]
    ));
    let resolver = CallFrameResolver::new(&program).expect("resolver");
    let frames = resolver.inferred_machine_state_write_frames(machine);
    assert_eq!(
        frames[1].complete_paths(),
        Some(["$P0".to_owned(), "$P1".to_owned()].as_slice()),
        "the swapped cycle still solves to both roots"
    );

    let program = typed(REFERENCE_CYCLE);
    let machine = &program.machines()[0];
    for state in program.machine_states(machine).iter().take(3) {
        assert!(
            !reachable_cycle_edges_can_permute_write_parameters(&program, machine, state),
            "{} reaches the reference-dropping edge",
            state.name.as_str()
        );
    }
    assert!(reachable_cycle_edges_can_permute_write_parameters(
        &program,
        machine,
        &program.machine_states(machine)[3]
    ));
}

#[test]
fn copy_value_cycle_edges_permute_and_solve_to_complete_frames() {
    let program = typed(KIND_CYCLE);
    let machine = &program.machines()[0];
    for state in program.machine_states(machine) {
        assert!(
            reachable_cycle_edges_can_permute_write_parameters(&program, machine, state),
            "{} carries only the `&mut Output` root around the cycle",
            state.name.as_str()
        );
    }
    let resolver = CallFrameResolver::new(&program).expect("resolver");
    let frames = resolver.inferred_machine_state_write_frames(machine);
    let outcomes = frames
        .iter()
        .map(|frame| frame.complete_paths().map(<[String]>::to_vec))
        .collect::<Vec<_>>();
    let cycle = Some(vec!["$P0.accepted".to_owned(), "$P0.count".to_owned()]);
    assert_eq!(
        outcomes,
        vec![
            cycle.clone(),
            cycle.clone(),
            cycle,
            Some(vec!["$P0.accepted".to_owned()])
        ],
        "a copy enum crossing a cycle edge no longer keeps the cycle opaque"
    );
}
