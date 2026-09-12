//! Dispatch must retain the exact owner of an operation result or state parameter.
use super::*;
use legalized_operations::{LegalizedScalarTerminator, LegalizedStructuralCaseSource};
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralTypeId};

#[test]
fn local_record_getter_replay_rejects_substituted_receiver_custody() {
    let source = "data Pair [copy] { left: u64; right: u64; }
        machine Pair::get_right(&self) -> u64 { self.right }
        machine observe(left: u64, right: u64) -> u64 {
            let original: Pair = Pair { left: left, right: right };
            let other: Pair = Pair { left: right, right: left };
            original.get_right()
        }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked =
        typed_trees_to_checked_trees::lower_typed_trees(typed).expect("valid local getter source");
    let _artifact = terminal_production::produce_terminal_artifact(&checked, "observe")
        .expect("unchanged receiver custody independently replays");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "observe")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let locals = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| {
            if let checked_trees::statement::StatementNode::LocalData(local) = statement {
                Some(local.symbol)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 2);
    let receiver = checked.facts.values.scalar_computations.structural_arguments.iter().find_map(|(handle, argument)| {
        if let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) = argument
            && matches!(argument.source, checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } if symbol == locals[0])
        { Some(handle) } else { None }
    }).expect("retained local receiver operand");
    for mutation in 0..5 {
        let mut changed = checked.clone();
        let checked_trees::CheckedScalarComputationStructuralArgument::Place(argument) = changed
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get_mut(receiver)
        else {
            panic!("receiver")
        };
        match mutation {
            0 => {
                argument.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol: locals[1],
                    }
            }
            1 => {
                argument.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                        symbol: symbols::SymbolHandle::invalid(),
                    }
            }
            2 => argument.access = checked_trees::CheckedStructuralAccess::MutableBorrow,
            3 => argument.type_identity = "OtherRecord".into(),
            4 => {
                argument.source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 0,
                    }
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::produce_terminal_artifact(&changed, "observe").is_err(),
            "receiver mutation {mutation}"
        );
    }
}

#[test]
fn case_dispatch_rejects_substituted_result_and_state_parameter_sources() {
    let owned_source = concat!(
        include_str!("choose.omg"),
        "\n",
        include_str!("owned_state.omg")
    );
    for (entry, source, expects_parameter) in [
        ("collect", include_str!("borrowed.omg"), false),
        ("collect_owned", owned_source, true),
    ] {
        let artifact = produce_source(entry, source);
        for target in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
        ] {
            let selections = OptimizationSelections::new([]).unwrap();
            let optimized = optimize_artifact_sections(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &AdmissionProfile::default(),
                compiler_baseline_request_v1(&selections),
            )
            .unwrap();
            let compiled =
                abstract_operations_to_target_operations::lower_optimized_to_target_operations(
                    optimized, target,
                )
                .unwrap();
            let legalized = target_operations_to_selected_instructions::legalize_target_operations(
                compiled.target_operations(),
                compiled.optimized().plan(),
                compiled.optimized(),
            )
            .unwrap();
            let validate = |plan| {
                target_operations_to_selected_instructions::validate_legalized_operations(
                    compiled.target_operations(),
                    compiled.optimized().plan(),
                    compiled.optimized(),
                    plan,
                )
            };
            assert_eq!(validate(legalized.plan().clone()).unwrap(), legalized);
            let mut dispatches = 0;
            for (function_index, function) in legalized.plan().scalar_functions.iter().enumerate() {
                for (block_index, block) in function.blocks.iter().enumerate() {
                    let LegalizedScalarTerminator::StructuralCase { source, .. } =
                        &block.terminator
                    else {
                        continue;
                    };
                    assert_eq!(
                        matches!(source, LegalizedStructuralCaseSource::BlockParameter { .. }),
                        expects_parameter
                    );
                    dispatches += 1;
                    for mutation in 0..3 {
                        let mut changed = legalized.plan().clone();
                        let LegalizedScalarTerminator::StructuralCase { source, .. } =
                            &mut changed.scalar_functions[function_index].blocks[block_index]
                                .terminator
                        else {
                            unreachable!()
                        };
                        match source {
                            LegalizedStructuralCaseSource::OperationResult {
                                operation,
                                result,
                            } => match mutation {
                                0 => *operation = OperationId::new(999_999).unwrap(),
                                1 => result.place = PlaceId::new(999_999).unwrap(),
                                _ => {
                                    result.structural_type = StructuralTypeId::new(999_999).unwrap()
                                }
                            },
                            LegalizedStructuralCaseSource::BlockParameter {
                                block,
                                declaration,
                            } => match mutation {
                                0 => *block = BlockId::new(999_999).unwrap(),
                                1 => declaration.place = PlaceId::new(999_999).unwrap(),
                                _ => {
                                    declaration.structural_type =
                                        StructuralTypeId::new(999_999).unwrap()
                                }
                            },
                        }
                        assert!(
                            validate(changed).is_err(),
                            "{entry} mutation {mutation} on {target:?}"
                        );
                    }
                }
            }
            assert!(
                dispatches > 0,
                "the fixture must exercise its source variant"
            );
        }
    }
}
