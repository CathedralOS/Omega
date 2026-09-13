use super::*;

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
    let tokens = source_files_to_tokens::Lexer::new(&source)
        .tokenize()
        .expect("tokens");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolved");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("typed");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|diagnostics| panic!("checked: {diagnostics:#?}\n{source}"))
}

fn graph(checked: &CheckedTrees) -> &CheckedScalarMachineGraph {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "walk")
        .expect("walk");
    checked
        .facts
        .flow
        .terminal_scalar_graphs
        .for_machine(machine.symbol)
        .expect("scalar graph")
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
        state,
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
fn source_rank_custody_preserves_absent_static_and_dynamic_ranges() {
    for ranking in [
        "terminates by remaining;",
        "terminates by remaining -> Nat::Descending in 0..6;",
        "terminates by remaining -> Nat::Descending in 0..=6;",
        "terminates by remaining -> Nat::Descending in 0..(limits.limit % limits.divisor + 6);",
    ] {
        let checked = fixture_with_ranking("remaining > 0", true, true, ranking);
        assert!(validate(&checked, graph(&checked)).is_ok(), "{ranking}");
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
fn source_rank_custody_rejects_added_range_and_unmatched_typed_rosters() {
    let original = fixture_with_ranking("remaining > 0", true, true, "terminates by remaining;");
    assert!(validate(&original, graph(&original)).is_ok());
    let mut changed = original.clone();
    changed
        .typed
        .machines_mut()
        .iter_mut()
        .find(|machine| machine.name.as_str() == "walk")
        .unwrap()
        .termination_plan
        .implementation_witness
        .as_mut()
        .unwrap()
        .rank_range = Some(language_semantics::RankRange {
        floor: "0".into(),
        ceiling: "6".into(),
        ceiling_inclusive: false,
    });
    assert!(validate(&changed, graph(&changed)).is_err());

    for added_range in [false, true] {
        let mut changed = original.clone();
        let machine = graph(&changed).machine;
        let custody = changed
            .typed
            .ranking_expression_custody
            .iter_mut()
            .find(|custody| custody.machine == machine)
            .unwrap();
        if added_range {
            custody.rank_range = Some(custody.subjects[0]);
        } else {
            custody.view_arguments.push(custody.subjects[0]);
        }
        assert!(validate(&changed, graph(&changed)).is_err());
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
    assert!(prepare(&checked, &changed, &[], &mut 10).is_err());
}

#[test]
fn loop_descriptor_positions_are_dense_without_changing_invocation_positions() {
    let checked = fixture("remaining > 0", true, true, false);
    let graph = graph(&checked);
    // Exercise the descriptor namespace independently from source signature
    // validation: preparation clones declarations already validated by its caller.
    let parameter = |place, position| StructuralParameterDeclaration {
        place: PlaceId::new(place).unwrap(),
        position,
        is_self: false,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let invocation = vec![parameter(1, 1), parameter(2, 3)];
    let mut next_place = 10;
    let plan = prepare(&checked, graph, &invocation, &mut next_place)
        .unwrap()
        .unwrap();
    assert!(plan.rank.is_none());
    assert_eq!(
        invocation
            .iter()
            .map(|parameter| parameter.position)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(
        plan.parameters
            .iter()
            .map(|parameter| parameter.position)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    assert_eq!(
        invocation
            .iter()
            .map(|parameter| parameter.place)
            .collect::<Vec<_>>(),
        [place_id(1), place_id(2)]
    );
    assert_eq!(
        plan.parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<Vec<_>>(),
        [place_id(10), place_id(11)]
    );
    assert_eq!(next_place, 12);
}
