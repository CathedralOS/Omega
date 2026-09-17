use crate::CallFrameResolver;
use crate::machine_calls::calls::write_frames::CYCLE_EQUATIONS;
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

/// A named cycle whose edges carry a value-typed `Kind` into one state and
/// drop it on the way back. `Kind` is a named type, so the capability law
/// counts it as write-capable and no edge can be an exact permutation.
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
    let program = typed(KIND_CYCLE);
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

    let program = typed(KIND_CYCLE);
    let machine = &program.machines()[0];
    for state in program.machine_states(machine).iter().take(3) {
        assert!(
            !reachable_cycle_edges_can_permute_write_parameters(&program, machine, state),
            "{} reaches the `Kind` edge",
            state.name.as_str()
        );
    }
    assert!(reachable_cycle_edges_can_permute_write_parameters(
        &program,
        machine,
        &program.machine_states(machine)[3]
    ));
}
