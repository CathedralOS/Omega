use super::{
    CheckedScalarBranchDestination, CheckedScalarMachineGraph, CheckedScalarStateTerminator,
    CheckedStructuralRankedArgumentPlan, CheckedStructuralRankedGuardPlan, CheckedTrees,
    LoweringError, ScalarLoopEntry, prepare, source_custody, validate_rank,
};
use crate::scalar_graph::PrimitiveType;

#[test]
fn mutable_receiver_keeps_its_invocation_place_across_a_scalar_backedge() {
    let checked = crate::front_end::checked_program(
        r#"
        data Counter {}
        machine Counter::walk(&mut self, remaining: u64)
        requires remaining <= 5
        terminates by remaining -> Nat::Descending in 0..6;
        -> u64 {
            transition remaining > 0 {
                true -> walk(remaining - 1)
                false -> 42
            }
        }
    "#,
    );
    let lowered = crate::lower_machine(
        &checked,
        crate::TerminalMachineSelection::Name("Counter::walk"),
    )
    .expect("a borrowed receiver is retained, not rebound, by the loop");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("the original exclusive receiver and rank independently verify");
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let receiver = &machine.structural_parameters[0];
    assert_eq!(
        receiver.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    assert!(
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.structural_parameters)
            .all(|parameter| parameter.place != receiver.place),
        "the receiver remains invocation-scoped, never a loop-carried formal"
    );
    for access in [
        checked_trees::CheckedStructuralAccess::SharedBorrow,
        checked_trees::CheckedStructuralAccess::Owned,
    ] {
        let mut changed = checked.clone();
        changed.facts.flow.terminal_scalar_graphs.machines[0].states[0].structural_parameters[0]
            .access = access;
        assert!(
            crate::lower_machine(
                &changed,
                crate::TerminalMachineSelection::Name("Counter::walk"),
            )
            .is_err(),
            "a retained {access:?} cannot replace the authored exclusive loan"
        );
    }
}

fn fixture(
    predicate: &str,
    recursive_truth: bool,
    first_truth: bool,
    ranked: bool,
) -> CheckedTrees {
    let ranking = if ranked {
        "terminates by remaining -> Nat::Descending in 0..6;"
    } else {
        ""
    };
    fixture_with_ranking(predicate, recursive_truth, first_truth, ranking)
}

fn fixture_with_ranking(
    predicate: &str,
    recursive_truth: bool,
    first_truth: bool,
    ranking: &str,
) -> CheckedTrees {
    let target = |truth| {
        if truth == recursive_truth {
            "walk(limits, remaining - 1, marker)"
        } else {
            "marker"
        }
    };
    let source = format!(
        "data Limits {{ limit: u64; divisor: u64 [3..=5]; }}
         machine walk(limits: Limits, remaining: u64 [0..=5], marker: u64)
         {ranking} -> u64 {{
             transition {predicate} {{
                 {first_truth} -> {}
                 {} -> {}
             }}
         }}",
        target(first_truth),
        !first_truth,
        target(!first_truth),
    );
    crate::front_end::checked_program(&source)
}

fn graph(checked: &CheckedTrees) -> &CheckedScalarMachineGraph {
    named_graph(checked, "walk")
}

fn validate(
    checked: &CheckedTrees,
    graph: &CheckedScalarMachineGraph,
) -> Result<(), LoweringError> {
    let state = &graph.states[0];
    let (machine, source) = source_custody::authored_state(checked, state.state)?;
    validate_rank(
        checked,
        machine,
        source,
        graph,
        graph.ranked_scc.as_ref().expect("rank"),
    )
}

#[test]
fn source_rank_custody_preserves_boolean_wrappers_reversal_and_arm_order() {
    for (predicate, recursive_truth) in [
        ("remaining > 0", true),
        ("remaining == 0", false),
        ("remaining < 1", false),
        ("remaining <= 0", false),
        ("0 < remaining", true),
        ("!(remaining > 0)", false),
    ] {
        for first_truth in [false, true] {
            let checked = fixture(predicate, recursive_truth, first_truth, true);
            let graph = graph(&checked);
            assert!(
                validate(&checked, graph).is_ok(),
                "{predicate}: first {first_truth}"
            );
            let rank = graph.ranked_scc.as_ref().unwrap();
            assert_eq!(rank.rank_scalar_parameter_index, 0);
            assert_eq!(
                rank.rank_upper_bound,
                u128::from(u64::MAX),
                "carrier bound, not source range ceiling 6"
            );
            let [edge] = rank.covered_cyclic_edges.as_slice() else {
                panic!("one source backedge");
            };
            assert_eq!(
                edge.statement_ordinal,
                u32::from(first_truth != recursive_truth)
            );
            let CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne {
                argument_ordinal,
                ..
            } = edge.successor_argument;
            assert_eq!(
                argument_ordinal, 1,
                "owned input precedes the rank in the authored telescope"
            );
        }
    }
}

#[test]
fn source_rank_custody_rejects_post_check_witness_metadata_mutations() {
    for ranking in [
        "terminates by remaining -> Nat::Descending in 0..6;",
        "terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);",
    ] {
        let original = fixture_with_ranking("remaining > 0", true, true, ranking);
        assert!(validate(&original, graph(&original)).is_ok());
        for mutation in 0..12 {
            let mut changed = original.clone();
            let machine = changed
                .typed
                .machines_mut()
                .iter_mut()
                .find(|machine| machine.name.as_str() == "walk")
                .unwrap();
            let witness = machine
                .termination_plan
                .implementation_witness
                .as_mut()
                .unwrap();
            match mutation {
                0 => witness.ranking_view = language_semantics::RankingViewId::NAT_INCREASING_TO,
                1 => witness.ranking_view = language_semantics::RankingViewId::NULL,
                2 => witness.view_path = "Nat::IncreasingTo".into(),
                3 => witness.view_path.clear(),
                4 => witness.view_arguments.push("marker".into()),
                5 => witness.subjects[0] = "marker".into(),
                6 => witness.subjects.clear(),
                7 => witness.subjects.push("remaining".into()),
                8 => witness.rank_range = None,
                9 => witness.rank_range.as_mut().unwrap().floor = "1".into(),
                10 => witness.rank_range.as_mut().unwrap().ceiling = "marker".into(),
                _ => witness.rank_range.as_mut().unwrap().ceiling_inclusive = true,
            }
            assert!(
                validate(&changed, graph(&changed)).is_err(),
                "witness mutation {mutation}: {ranking}"
            );
        }
    }
}

#[test]
fn source_rank_custody_rejects_each_retained_coordinate_descriptor_and_bound_mutation() {
    let checked = fixture("remaining > 0", true, true, true);
    let original = graph(&checked);
    assert!(validate(&checked, original).is_ok());
    for mutation in 0..16 {
        let mut graph = original.clone();
        let rank = graph.ranked_scc.as_mut().unwrap();
        match mutation {
            0 => rank.header_state = symbols::SymbolHandle::invalid(),
            1 => rank.rank_scalar_parameter_index = 1,
            2 => rank.rank_primitive_type = PrimitiveType::U32,
            3 => rank.rank_lower_bound = 1,
            4 => rank.rank_upper_bound = 6,
            5 => rank.covered_cyclic_edges.clear(),
            6 => rank.covered_cyclic_edges.push(rank.covered_cyclic_edges[0]),
            mutation => {
                let edge = &mut rank.covered_cyclic_edges[0];
                match mutation {
                    7 => edge.statement_ordinal += 1,
                    8 => edge.source_state = symbols::SymbolHandle::invalid(),
                    9 => edge.target_state = symbols::SymbolHandle::invalid(),
                    10 => {
                        let CheckedStructuralRankedGuardPlan::UnsignedParameterPositive {
                            scalar_parameter_index,
                            ..
                        } = &mut edge.guard;
                        *scalar_parameter_index = 1;
                    }
                    11 => {
                        let CheckedStructuralRankedGuardPlan::UnsignedParameterPositive {
                            primitive_type,
                            ..
                        } = &mut edge.guard;
                        *primitive_type = PrimitiveType::U32;
                    }
                    mutation => {
                        let CheckedStructuralRankedArgumentPlan::UnsignedParameterMinusOne {
                            argument_ordinal,
                            source_scalar_parameter_index,
                            target_scalar_parameter_index,
                            primitive_type,
                        } = &mut edge.successor_argument;
                        match mutation {
                            12 => *argument_ordinal = 0,
                            13 => *source_scalar_parameter_index = 1,
                            14 => *target_scalar_parameter_index = 1,
                            _ => *primitive_type = PrimitiveType::U32,
                        }
                    }
                }
            }
        }
        assert!(
            validate(&checked, &graph).is_err(),
            "rank mutation {mutation}"
        );
    }
}

#[test]
fn source_rank_custody_rejoins_successors_instead_of_matching_two_retained_rosters() {
    let checked = fixture("remaining > 0", true, true, true);
    let original = graph(&checked);
    assert!(validate(&checked, original).is_ok());
    let mut changed = original.clone();
    let CheckedScalarStateTerminator::Conditional {
        when_true: CheckedScalarBranchDestination::Jump(successor),
        ..
    } = &mut changed.states[0].terminator
    else {
        panic!("recursive true arm");
    };
    successor.statement_ordinal += 1;
    changed.ranked_scc.as_mut().unwrap().covered_cyclic_edges[0].statement_ordinal += 1;
    assert!(
        validate(&checked, &changed).is_err(),
        "matching forged plan coordinates are not source authority"
    );

    let mut changed = original.clone();
    changed.states[0].terminator = CheckedScalarStateTerminator::Return {
        statement_ordinal: 1,
    };
    assert!(
        validate(&checked, &changed).is_err(),
        "missing executable edge cannot be covered by a retained rank row"
    );
    assert!(prepare(&checked, &changed, &[], &[], &mut 10).is_err());
}

fn named_graph<'a>(checked: &'a CheckedTrees, name: &str) -> &'a CheckedScalarMachineGraph {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("machine");
    checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine.symbol)
        .expect("scalar graph")
}

/// Square-and-multiply re-enters `step`, never the entry; the nested count
/// re-enters both `row` and the self-transferring `column`.
const INTERIOR_HEADER_LOOPS: [(&str, &str); 2] = [
    (
        "modular_power",
        "machine modular_power(base: i32, exponent: i32) -> i32 {
             transition exponent < 0 {
                 true -> (0)
                 _ -> step(1, base % 100, exponent)
             }
             state step(result: i32, base: i32, exponent: i32) -> i32 {
                 transition exponent == 0 {
                     true -> (result)
                     _ -> classify(result, base, exponent)
                 }
             }
             state classify(result: i32, base: i32, exponent: i32) -> i32 {
                 let bit: i32 = exponent % 2;
                 transition bit == 1 {
                     true -> accumulate(result, base, exponent)
                     _ -> square(result, base, exponent)
                 }
             }
             state accumulate(result: i32, base: i32, exponent: i32) -> i32 {
                 let next_result: i32 = ((result % 100) * (base % 100)) % 100;
                 transition {
                     _ -> square(next_result, base, exponent)
                 }
             }
             state square(result: i32, base: i32, exponent: i32) -> i32 {
                 let next_base: i32 = ((base % 100) * (base % 100)) % 100;
                 transition {
                     _ -> step(result, next_base, exponent / 2)
                 }
             }
         }",
    ),
    (
        "grid_count",
        "machine grid_count(rows: i32, columns: i32) -> i32 {
             transition {
                 _ -> row(0, rows % 8, columns % 8)
             }
             state row(total: i32, remaining_rows: i32, columns: i32) -> i32 {
                 transition remaining_rows <= 0 {
                     true -> (total)
                     _ -> column(total, remaining_rows - 1, columns, columns)
                 }
             }
             state column(total: i32, remaining_rows: i32, columns: i32, remaining_columns: i32) -> i32 {
                 transition remaining_columns <= 0 {
                     true -> row(total, remaining_rows, columns)
                     _ -> column((total % 100) + 1, remaining_rows, columns, remaining_columns - 1)
                 }
             }
         }",
    ),
];

#[test]
fn interior_loop_headers_leave_the_entry_as_a_once_run_prologue() {
    for (name, source) in INTERIOR_HEADER_LOOPS {
        let checked = crate::front_end::checked_program(source);
        let plan = prepare(&checked, named_graph(&checked, name), &[], &[], &mut 10)
            .expect("a dominating interior header admits the cycle")
            .expect("the cycle yields a loop plan");
        assert!(
            matches!(plan.entry, ScalarLoopEntry::Prologue) && plan.rank.is_none(),
            "{name}: the entry binds only on the machine signature, and no rank is invented"
        );
        let lowered = crate::lower_machine(&checked, crate::TerminalMachineSelection::Name(name))
            .unwrap_or_else(|error| panic!("{name}: {error:?}"));
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .unwrap_or_else(|error| panic!("{name}: the unranked interior loop verifies: {error:?}"));
        let machine = &lowered.semantic_module.machines[0];
        let entry = machine
            .blocks
            .iter()
            .find(|block| block.id == machine.entry)
            .expect("entry block");
        assert!(
            entry.parameters.is_empty() && entry.structural_parameters.is_empty(),
            "{name}: the entry is the machine's own block, not a loop header"
        );
        let components = terminal_verifier::control_cycle_members(machine).unwrap();
        assert!(
            !components.is_empty()
                && components
                    .iter()
                    .all(|members| !members.contains(&machine.entry)),
            "{name}: every cycle stays behind the prologue: {components:?}"
        );
        assert!(machine.ranked_scc.is_none(), "{name}: no authored witness");
    }
}

#[test]
fn a_cycle_without_a_dominating_header_declines() {
    // Either state can be entered first, so neither dominates the other and
    // the cycle has no single header to carry its values.
    let checked = crate::front_end::checked_program(
        "machine ping_pong(start: i32, count: i32) -> i32 {
             transition start > 0 {
                 true -> ping(count % 16, 0)
                 _ -> pong(count % 16, 0)
             }
             state ping(remaining: i32, total: i32) -> i32 {
                 transition remaining <= 0 {
                     true -> (total)
                     _ -> pong(remaining - 1, (total % 100) + 1)
                 }
             }
             state pong(remaining: i32, total: i32) -> i32 {
                 transition remaining <= 0 {
                     true -> (total)
                     _ -> ping(remaining - 1, (total % 100) + 2)
                 }
             }
         }",
    );
    assert!(matches!(
        prepare(
            &checked,
            named_graph(&checked, "ping_pong"),
            &[],
            &[],
            &mut 10
        ),
        Err(LoweringError::Unsupported(
            "scalar graph cycle has no dominating loop header"
        ))
    ));
}

#[test]
fn an_interior_header_keeps_its_owners_authored_ranking_obligation() {
    // A Nat countdown witness on the header's own machine without a retained
    // ranked plan is a lost judgment, not an unranked loop.
    let ranked = fixture("remaining > 0", true, true, true);
    let witness = ranked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .and_then(|machine| machine.termination_plan.implementation_witness.clone())
        .expect("authored Nat witness");
    let (name, source) = INTERIOR_HEADER_LOOPS[0];
    let mut changed = crate::front_end::checked_program(source);
    changed
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == name)
        .unwrap()
        .termination_plan
        .implementation_witness = Some(witness);
    assert!(matches!(
        prepare(&changed, named_graph(&changed, name), &[], &[], &mut 10),
        Err(LoweringError::Unsupported(
            "scalar loop lost or substituted its authored ranking witness"
        ))
    ));

    // A fused member heading its own loop carries a witness the checker never
    // judged for the dispatch machine's graph.
    let fused = crate::front_end::checked_program(
        "data Main {}
         machine Main::evaluate(&mut self) -> u64 {
             transition {
                 _ -> self.walk(4, 0)
             }
         }
         machine Main::walk(&mut self, limit: u64, index: u64)
         terminates by (index, limit) -> Nat::BoundedDistance;
         -> u64
         {
             transition index < limit {
                 true -> walk(limit, index + 1)
                 false -> index
             }
         }",
    );
    let evaluate = fused
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::evaluate")
        .expect("evaluate");
    let graph = fused
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(evaluate.symbol)
        .expect("fused scalar graph");
    assert!(graph.states.len() > 1, "walk fuses into evaluate's graph");
    assert!(matches!(
        prepare(&fused, graph, &[], &[], &mut 10),
        Err(LoweringError::Unsupported(
            "scalar loop header's ranking witness has no graph judgment"
        ))
    ));
}
