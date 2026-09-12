//! Dispatch must retain the exact owner of an operation result or state parameter.
use super::*;
use legalized_operations::{LegalizedScalarTerminator, LegalizedStructuralCaseSource};
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralTypeId};

#[test]
fn projected_shared_receiver_rejects_overlapping_mutable_field_actual() {
    let source = "data Inner { left: u64; right: u64; }
        data Outer { leading: u64; inner: Inner; other: Inner; }
        machine Inner::inspect(&self, destination: &mut u64) -> u64 { self.right }
        machine observe(value: &mut Outer) -> u64 {
            value.inner.inspect(&mut value.inner.right)
        }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    let diagnostics = match typed_trees_to_checked_trees::lower_typed_trees(typed) {
        Ok(_) => panic!("projected shared receiver cannot overlap an exclusive field argument"),
        Err(diagnostics) => diagnostics,
    };
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("receives shared receiver overlapping another argument in the same call")),
        "exact receiver/argument incompatibility: {diagnostics:#?}"
    );
}

#[test]
fn projected_record_getter_replay_rejects_sibling_root_path_and_endpoint_substitution() {
    use checked_trees::{
        CheckedScalarComputationStructuralArgument, CheckedStructuralAccess,
        CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
    };

    let tokens = source_files_to_tokens::Lexer::new(super::records::PROJECTED_RECORD_GETTER)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("valid projected shared getter source");
    for entry in ["distinct_roots", "projected"] {
        let _artifact = terminal_production::TerminalProductionRequest::new(&checked, entry)
            .produce_artifact()
            .expect("unchanged projected receiver custody independently publishes");
    }
    let declared_field = |owner: &str, name: &str| {
        let definition = checked
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == owner)
            .unwrap();
        let field = checked
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                checked_trees::data::DataMember::Field(field) if field.name.as_str() == name => {
                    Some(field)
                }
                _ => None,
            })
            .unwrap();
        CheckedUnitStructuralPathSegment::Field(
            field
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| field.name.as_str().to_owned()),
        )
    };
    let inner = declared_field("Outer", "inner");
    let other = declared_field("Outer", "other");
    let right = declared_field("Inner", "right");
    assert_ne!(inner, other);

    // Only distinct_roots has a second structural parameter: this selects its
    // right.inner occurrence, not another machine's coincidentally equal path.
    let mut receivers = checked
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .iter()
        .filter_map(|(handle, argument)| match argument {
            CheckedScalarComputationStructuralArgument::Place(argument)
                if argument.source
                    == CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 1,
                    }
                    && argument.path == [inner.clone()] =>
            {
                Some(handle)
            }
            _ => None,
        });
    let receiver = receivers.next().expect("exact right.inner receiver");
    assert!(receivers.next().is_none());
    for mutation in 0..7 {
        let mut changed = checked.clone();
        let CheckedScalarComputationStructuralArgument::Place(argument) = changed
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get_mut(receiver)
        else {
            panic!("receiver")
        };
        match mutation {
            0 => argument.path = vec![other.clone()],
            1 => {
                argument.source =
                    CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
            }
            2 => argument.path.clear(),
            3 => argument.path.push(right.clone()),
            4 => argument.access = CheckedStructuralAccess::MutableBorrow,
            5 => argument.access = CheckedStructuralAccess::Owned,
            _ => argument.type_identity = "Outer".into(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "distinct_roots")
                .produce_artifact()
                .is_err(),
            "projected operand mutation {mutation}"
        );
    }

    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "distinct_roots")
        .unwrap();
    let state = &checked.machine_states(machine)[0];
    let root = checked.state_parameters(state)[0].symbol;
    let calls = checked
        .facts
        .borrow
        .states
        .iter()
        .find_map(|(_, row)| {
            (row.machine_symbol == machine.symbol && row.state_symbol == state.symbol)
                .then_some(row.calls)
        })
        .expect("exact distinct_roots borrow-call roster");
    assert_eq!(calls.count(), 2);
    let receiver_call = calls.start();
    let original = checked.facts.borrow.calls.get(receiver_call);
    assert!(original.has_receiver);
    assert_ne!(
        original.receiver_symbol, root,
        "projected endpoint is not its root"
    );
    for mutation in 0..3 {
        let mut changed = checked.clone();
        let call = changed.facts.borrow.calls.get_mut(receiver_call);
        match mutation {
            0 => call.receiver_symbol = root,
            1 => call.receiver_symbol = symbols::SymbolHandle::invalid(),
            _ => call.has_receiver = false,
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "distinct_roots")
                .produce_artifact()
                .is_err(),
            "receiver endpoint mutation {mutation}"
        );
    }
}

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
    let _artifact = terminal_production::TerminalProductionRequest::new(&checked, "observe")
        .produce_artifact()
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
            terminal_production::TerminalProductionRequest::new(&changed, "observe")
                .produce_artifact()
                .is_err(),
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
