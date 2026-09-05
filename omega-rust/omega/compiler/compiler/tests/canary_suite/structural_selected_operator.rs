use super::*;

#[path = "../fixture_rosters/structural_selected_operator.rs"]
pub(super) mod fixture_roster;

#[test]
fn specialized_structural_fixed_operator_hosted_native_canary_compiles() {
    let canary = pass_canary(fixture_roster::SPECIALIZED_STRUCTURAL_FIXED_OPERATOR_HOSTED_NATIVE);
    for target in ["linux_x86_64", "linux_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "hosted structural selection should realize natively for {target}: {diagnostics:#?}"
                )
            });
        let native = report.retained_native_artifact().unwrap_or_else(|| {
            panic!("hosted structural selection should retain {target} native custody")
        });
        native.validate().unwrap_or_else(|error| {
            panic!("hosted structural selection {target} should replay: {error}")
        });
        let physical = report
            .require_package_native_physical_evidence()
            .unwrap_or_else(|error| {
                panic!("hosted structural selection {target} should earn D32: {error}")
            });
        assert!(std::ptr::eq(
            physical,
            native.physical_evidence().unwrap_or_else(|| panic!(
                "hosted structural selection should retain {target} D32"
            )),
        ));
    }
}

#[test]
fn specialized_structural_fixed_operator_hosted_local_transfer_is_exact() {
    let canary = pass_canary(fixture_roster::SPECIALIZED_STRUCTURAL_FIXED_OPERATOR_HOSTED_NATIVE);
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("hosted structural selection should check");
    let main = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("hosted entry machine");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == main.symbol)
        .expect("checked hosted entry plan");
    assert_eq!(
        plan.trivial_affine_locals
            .iter()
            .map(|local| local.declaration_ordinal)
            .collect::<Vec<_>>(),
        [0, 1]
    );
    let [
        checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
            declaration_ordinal: 0,
            ..
        },
        checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
            declaration_ordinal: 1,
            ..
        },
        checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        },
        checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_local_discard_ordinals,
            ..
        },
    ] = plan.operations.as_slice()
    else {
        panic!("two establishments, one consuming call, and one Unit return")
    };
    assert_eq!(
        structural_arguments
            .iter()
            .map(|argument| argument.source_local_declaration_ordinal())
            .collect::<Vec<_>>(),
        [Some(0), Some(1)]
    );
    assert!(structural_arguments.iter().all(|argument| {
        argument.path.is_empty() && argument.access == checked_trees::CheckedStructuralAccess::Owned
    }));
    assert!(trivial_affine_local_discard_ordinals.is_empty());

    let state = checked
        .typed
        .machine_states(main)
        .first()
        .expect("hosted entry state");
    let local_symbols = checked
        .typed
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            typed_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(local_symbols.len(), 2);
    let mut exact_call_target = None;
    for (statement_index, local_symbol) in local_symbols.into_iter().enumerate() {
        let events = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter_map(|(_, event)| {
                (event.machine_symbol == main.symbol
                    && event.state_symbol == state.symbol
                    && event.root == facts::PlaceRoot::Symbol(local_symbol))
                .then_some(event)
            })
            .collect::<Vec<_>>();
        let [establishment, transfer] = events.as_slice() else {
            panic!(
                "one establishment and one transfer for hosted local {statement_index}: {events:#?}"
            )
        };
        let establishment_source =
            language_semantics::PermissionEventSource::Statement { statement_index };
        let provenance = language_semantics::PermissionProvenance::Established {
            machine_symbol: main.symbol,
            state_symbol: state.symbol,
            source: establishment_source,
        };
        assert_eq!(establishment.source, establishment_source);
        assert_eq!(
            establishment.kind,
            language_semantics::PermissionEventKind::Establish
        );
        let language_semantics::PermissionEventSource::Call {
            statement_index: 2,
            call_ordinal: 0,
            target_symbol,
        } = transfer.source
        else {
            panic!("hosted local must transfer at the exact consuming call")
        };
        if let Some(expected) = exact_call_target {
            assert_eq!(target_symbol, expected);
        } else {
            exact_call_target = Some(target_symbol);
        }
        assert_eq!(
            transfer.kind,
            language_semantics::PermissionEventKind::Transfer
        );
        for event in events {
            assert_eq!(event.multiplicity, language_semantics::Multiplicity::Affine);
            assert_eq!(event.access, language_semantics::PermissionAccess::Owned);
            assert_eq!(
                event.claim_identity,
                language_semantics::PermissionClaimIdentity::Unknown
            );
            assert_eq!(event.provenance, provenance);
            assert!(!event.obligation_live);
            assert!(
                checked
                    .facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(event.segments)
                    .is_empty()
            );
        }
    }

    let _ =
        checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &checked,
            "Main::main",
        )
        .expect("checked local transfers should publish as exact Terminal custody");
}

#[test]
fn specialized_structural_fixed_operator_hosted_local_transfer_rejects_drift() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        UnknownLocal,
        DuplicateLocal,
        ParameterSubstitution,
        ProjectedPath,
        BorrowedAccess,
        RestoredDiscard,
    }

    let canary = pass_canary(fixture_roster::SPECIALIZED_STRUCTURAL_FIXED_OPERATOR_HOSTED_NATIVE);
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("hosted structural selection should check");
    let main = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("hosted entry machine");

    for drift in [
        Drift::UnknownLocal,
        Drift::DuplicateLocal,
        Drift::ParameterSubstitution,
        Drift::ProjectedPath,
        Drift::BorrowedAccess,
        Drift::RestoredDiscard,
    ] {
        let mut drifted = checked.clone();
        let plan = drifted
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == main.symbol)
            .expect("checked hosted entry plan");
        let call_index = plan
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation,
                    checked_trees::CheckedUnitEffectOperationPlan::CallUnit { .. }
                )
            })
            .expect("hosted consuming call");
        match drift {
            Drift::RestoredDiscard => {
                let Some(checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
                    trivial_affine_local_discard_ordinals,
                    ..
                }) = plan.operations.last_mut()
                else {
                    panic!("checked hosted return")
                };
                trivial_affine_local_discard_ordinals.push(0);
            }
            drift => {
                let checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } = &mut plan.operations[call_index]
                else {
                    unreachable!()
                };
                match drift {
                    Drift::UnknownLocal => {
                        structural_arguments[0].source =
                            checked_trees::CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal {
                                declaration_ordinal: 2,
                            };
                    }
                    Drift::DuplicateLocal => {
                        structural_arguments[0].source = structural_arguments[1].source.clone();
                    }
                    Drift::ParameterSubstitution => {
                        structural_arguments[0].source =
                            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                parameter_index: 0,
                            };
                    }
                    Drift::ProjectedPath => structural_arguments[0].path.push(
                        checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0),
                    ),
                    Drift::BorrowedAccess => {
                        structural_arguments[0].access =
                            checked_trees::CheckedStructuralAccess::SharedBorrow;
                    }
                    Drift::RestoredDiscard => unreachable!(),
                }
            }
        }
        assert!(
            checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
                &drifted,
                "Main::main",
            )
            .is_err(),
            "{drift:?} must reject before Terminal publication"
        );
    }
}

#[test]
fn specialized_structural_fixed_operator_unit_terminal_custody_canary_compiles() {
    let canary =
        pass_canary(fixture_roster::SPECIALIZED_STRUCTURAL_FIXED_OPERATOR_UNIT_TERMINAL_CUSTODY);
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("structural fixed-token Unit custody canary should check");
    let consume = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("structural Unit consumer");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == consume.symbol)
        .expect("checked structural Unit plan");
    let [
        checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
            coordinate,
            result,
            structural_arguments,
            ..
        },
        checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_discards,
            ..
        },
    ] = plan.operations.as_slice()
    else {
        panic!("exact structural-scalar call and Unit return")
    };
    assert_eq!(coordinate.statement_index, 0);
    assert_eq!(coordinate.call_ordinal, 0);
    assert_eq!(result.binding_ordinal, 0);
    assert_eq!(
        result.primitive_type,
        typed_trees::types::PrimitiveType::Bool
    );
    assert_eq!(structural_arguments.len(), 2);
    assert!(structural_arguments.iter().all(|argument| {
        argument.path.is_empty()
            && argument.byte_sequence_literal().is_none()
            && argument.access == checked_trees::CheckedStructuralAccess::Owned
    }));
    assert!(trivial_affine_discards.is_empty());
    selected_dispatch::validate_selected_operator_terminal_custody(
        &checked,
        checked.selected_provider_plans(),
    )
    .expect("structural Unit call must retain its exact selected ProviderPlan");

    let produced =
        checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &checked, "consume",
        )
        .expect("structural fixed-token Unit application should reach Terminal");
    produced
        .boundary_operator_scope()
        .validate_for_artifact(produced.artifact())
        .expect("checked Unit D29 scope should bind the exact Terminal artifact");
    let [application] = produced.boundary_operator_scope().applications() else {
        panic!("one exact structural fixed-token Unit application")
    };
    let [occurrence] = produced.boundary_operator_scope().occurrences() else {
        panic!("one exact structural fixed-token Unit occurrence")
    };
    let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("structural fixed-token Unit Terminal semantics should decode");
    let calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| operation.id == occurrence.terminal_operation())
        .collect::<Vec<_>>();
    let [call] = calls.as_slice() else {
        panic!("one exact D29-bound structural-scalar Terminal call")
    };
    let terminal_psi::OperationKind::CallStructuralScalar {
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = &call.kind
    else {
        panic!("Unit selection must lower as CallStructuralScalar")
    };
    assert_eq!(structural_arguments.len(), 2);
    assert!(structural_arguments.iter().all(|argument| {
        argument.path.is_empty() && argument.access == terminal_psi::StructuralAccess::Owned
    }));
    assert!(claim_transfers.is_empty());
    assert!(requirement_obligations.is_empty());
    assert!(crash_continuations.is_empty());

    let realizations =
        selected_dispatch::derive_checked_specialized_operator_application_realizations(
            &checked,
            checked.selected_provider_plans(),
        )
        .expect("structural Unit application should rejoin its specialization");
    let [realization] = realizations.as_slice() else {
        panic!("one exact specialized structural Unit realization")
    };
    assert_eq!(realization.application_arguments, application.arguments);
}

#[test]
fn specialized_structural_fixed_operator_unit_terminal_custody_rejects_drift() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        DuplicateSource,
        Reordered,
        ProjectedPath,
        BorrowedAccess,
        MissingRealization,
        MissingProviderPlan,
        RestoredDiscard,
    }

    let canary =
        pass_canary(fixture_roster::SPECIALIZED_STRUCTURAL_FIXED_OPERATOR_UNIT_TERMINAL_CUSTODY);
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("structural fixed-token Unit custody canary should check");
    let consume = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("structural Unit consumer");

    for drift in [
        Drift::DuplicateSource,
        Drift::Reordered,
        Drift::ProjectedPath,
        Drift::BorrowedAccess,
        Drift::MissingRealization,
        Drift::MissingProviderPlan,
        Drift::RestoredDiscard,
    ] {
        let mut drifted = checked.clone();
        let plan = drifted
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == consume.symbol)
            .expect("checked structural Unit plan");
        match drift {
            Drift::RestoredDiscard => {
                let Some(checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
                    trivial_affine_discards,
                    ..
                }) = plan.operations.last_mut()
                else {
                    panic!("checked Unit return")
                };
                trivial_affine_discards.push(0);
            }
            drift => {
                let Some(
                    checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                        realization_machine,
                        provider_plan_commitment,
                        structural_arguments,
                        ..
                    },
                ) = plan.operations.first_mut()
                else {
                    panic!("checked structural-scalar Unit call")
                };
                match drift {
                    Drift::DuplicateSource => {
                        structural_arguments[1].source = structural_arguments[0].source.clone();
                    }
                    Drift::Reordered => structural_arguments.swap(0, 1),
                    Drift::ProjectedPath => structural_arguments[0].path.push(
                        checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0),
                    ),
                    Drift::BorrowedAccess => {
                        structural_arguments[0].access =
                            checked_trees::CheckedStructuralAccess::SharedBorrow;
                    }
                    Drift::MissingRealization => {
                        *realization_machine = symbols::SymbolHandle::invalid();
                    }
                    Drift::MissingProviderPlan => {
                        *provider_plan_commitment =
                            checked_trees::CheckedProviderPlanCommitment::default();
                    }
                    Drift::RestoredDiscard => unreachable!(),
                }
            }
        }
        if checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &drifted,
            "consume",
        )
        .is_ok()
        {
            panic!("{drift:?} must reject before Terminal publication");
        }
    }
}

#[test]
fn specialized_mixed_structural_fixed_operator_hosted_native_reaches_d32() {
    let canary =
        pass_canary(fixture_roster::SPECIALIZED_MIXED_STRUCTURAL_FIXED_OPERATOR_HOSTED_NATIVE);
    for target in ["linux_x86_64", "linux_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "hosted mixed structural/fixed-integer selection should realize natively for {target}: {diagnostics:#?}"
                )
            });
        let native = report.retained_native_artifact().unwrap_or_else(|| {
            panic!("hosted mixed structural/fixed-integer selection should retain {target} native custody")
        });
        native.validate().unwrap_or_else(|error| {
            panic!(
                "hosted mixed structural/fixed-integer selection {target} should replay: {error}"
            )
        });
        let physical = report
            .require_package_native_physical_evidence()
            .unwrap_or_else(|error| {
                panic!(
                    "hosted mixed structural/fixed-integer selection {target} should earn D32: {error}"
                )
            });
        assert!(std::ptr::eq(
            physical,
            native.physical_evidence().unwrap_or_else(|| panic!(
                "hosted mixed structural/fixed-integer selection should retain {target} D32"
            )),
        ));
    }
}

#[test]
fn specialized_mixed_structural_fixed_operator_arguments_are_exact() {
    let canary =
        pass_canary(fixture_roster::SPECIALIZED_MIXED_STRUCTURAL_FIXED_OPERATOR_HOSTED_NATIVE);
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("hosted mixed structural/fixed-integer selection should check");
    let consume = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("mixed structural/fixed-integer Unit helper");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == consume.symbol)
        .expect("checked mixed structural/fixed-integer helper plan");
    let [
        checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
            result,
            realization_machine,
            realization_state,
            scalar_arguments,
            structural_arguments,
            ..
        },
        checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_discards,
            ..
        },
    ] = plan.operations.as_slice()
    else {
        panic!("one mixed selected call and one Unit return")
    };
    assert_eq!(
        result.primitive_type,
        typed_trees::types::PrimitiveType::I32
    );
    assert!(matches!(
        scalar_arguments.as_slice(),
        [checked_trees::CheckedScalarExpression::IntegerLiteral { literal }]
            if literal.value_i64() == Some(11)
                && literal.landing().is_some_and(|landing|
                    landing.landed_type
                        == numerics::literals::LandedIntegerType::I32)
    ));
    assert!(matches!(
        structural_arguments.as_slice(),
        [argument]
            if argument.source_parameter_index() == Some(0)
                && argument.path.is_empty()
                && argument.access == checked_trees::CheckedStructuralAccess::Owned
    ));
    assert!(trivial_affine_discards.is_empty());

    let realization = checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .iter()
        .find(|realization| {
            realization.machine == *realization_machine && realization.state == *realization_state
        })
        .expect("exact mixed structural/fixed-integer realization plan");
    assert!(matches!(
        realization.structural_parameters.as_slice(),
        [parameter]
            if parameter.position == 0
                && parameter.multiplicity == language_semantics::Multiplicity::Affine
                && parameter.access == checked_trees::CheckedStructuralAccess::Owned
    ));
    assert!(matches!(
        realization.scalar_parameters.as_slice(),
        [parameter]
            if parameter.source_position == 1
                && parameter.primitive_type == typed_trees::types::PrimitiveType::I32
    ));

    let main = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("hosted mixed entry machine");
    let main_plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == main.symbol)
        .expect("checked hosted mixed entry plan");
    assert!(matches!(
        main_plan.operations.as_slice(),
        [
            checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                declaration_ordinal: 0,
                ..
            },
            checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
                structural_arguments,
                ..
            },
            checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
                trivial_affine_local_discard_ordinals,
                ..
            },
        ] if matches!(structural_arguments.as_slice(), [argument]
            if argument.source_local_declaration_ordinal() == Some(0)
                && argument.path.is_empty()
                && argument.access == checked_trees::CheckedStructuralAccess::Owned)
            && trivial_affine_local_discard_ordinals.is_empty()
    ));

    let produced =
        checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &checked, "consume",
        )
        .expect("mixed structural/fixed-integer selection should reach Terminal");
    let [occurrence] = produced.boundary_operator_scope().occurrences() else {
        panic!("one exact mixed structural/fixed-integer occurrence")
    };
    let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("mixed structural/fixed-integer Terminal semantics should decode");
    let matching_calls = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(move |operation| operation.id == occurrence.terminal_operation())
                .map(move |operation| (machine, operation))
        })
        .collect::<Vec<_>>();
    let [(terminal_consume, call)] = matching_calls.as_slice() else {
        panic!("one exact D29-bound mixed Terminal call")
    };
    let terminal_psi::OperationKind::CallStructuralScalar {
        arguments,
        structural_arguments,
        claim_transfers,
        requirement_obligations,
        crash_continuations,
        ..
    } = &call.kind
    else {
        panic!("mixed Unit selection must lower as CallStructuralScalar")
    };
    let [scalar_argument] = arguments.as_slice() else {
        panic!("one exact Terminal scalar argument")
    };
    let [structural_argument] = structural_arguments.as_slice() else {
        panic!("one exact Terminal structural argument")
    };
    let [structural_parameter] = terminal_consume.structural_parameters.as_slice() else {
        panic!("one exact Terminal structural parameter")
    };
    assert_eq!(structural_argument.place, structural_parameter.place);
    assert!(structural_argument.path.is_empty());
    assert_eq!(
        structural_argument.access,
        terminal_psi::StructuralAccess::Owned
    );
    assert!(claim_transfers.is_empty());
    assert!(requirement_obligations.is_empty());
    assert!(crash_continuations.is_empty());

    let scalar_producers = terminal_consume
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            operation
                .result
                .scalar()
                .is_some_and(|result| result.id == *scalar_argument)
        })
        .collect::<Vec<_>>();
    let [scalar_producer] = scalar_producers.as_slice() else {
        panic!("one exact producer for the Terminal scalar argument")
    };
    let terminal_psi::OperationKind::IntegerConstant { value } = &scalar_producer.kind else {
        panic!("mixed scalar argument must remain an integer constant")
    };
    assert_eq!(*value, semantic_vocabulary::IntegerValue::Signed(11));
    let i32_type = semantic_vocabulary::ScalarType::Integer(
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
            .expect("i32"),
    );
    assert_eq!(
        scalar_producer
            .result
            .scalar()
            .expect("integer constant result")
            .scalar_type,
        i32_type
    );
    assert_eq!(
        call.result
            .scalar()
            .expect("mixed call scalar result")
            .scalar_type,
        i32_type
    );
}

#[test]
fn specialized_mixed_structural_result_operator_has_exact_checked_custody() {
    let canary =
        pass_canary(fixture_roster::SPECIALIZED_MIXED_STRUCTURAL_RESULT_OPERATOR_HOSTED_NATIVE);
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("hosted mixed structural-result selection should check");
    let consume = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("mixed structural-result Unit helper");
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(consume.symbol)
        .expect("checked mixed structural-result helper plan");
    let [
        checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
            result,
            realization_machine,
            realization_state,
            scalar_arguments,
            structural_arguments,
            discard_result_on_return,
            ..
        },
        checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
            trivial_affine_discards,
            ..
        },
    ] = plan.operations.as_slice()
    else {
        panic!("one selected structural-result call and one Unit return")
    };
    assert_eq!(result.statement_index, 0);
    assert_eq!(result.binding_ordinal, 0);
    assert_eq!(
        result.multiplicity,
        language_semantics::Multiplicity::Affine
    );
    assert!(*discard_result_on_return);
    assert!(trivial_affine_discards.is_empty());
    assert!(matches!(
        scalar_arguments.as_slice(),
        [checked_trees::CheckedScalarExpression::IntegerLiteral { literal }]
            if literal.value_i64() == Some(11)
                && literal.landing().is_some_and(|landing|
                    landing.landed_type
                        == numerics::literals::LandedIntegerType::I32)
    ));
    assert!(matches!(
        structural_arguments.as_slice(),
        [argument]
            if argument.path.is_empty()
                && argument.access == checked_trees::CheckedStructuralAccess::Owned
                && argument.source_parameter_index() == Some(0)
    ));

    let realization = checked
        .facts
        .flow
        .terminal_structural_returns
        .claim_free_affine_machines
        .iter()
        .find(|candidate| {
            candidate.machine == *realization_machine && candidate.state == *realization_state
        })
        .expect("selected claim-free affine structural realization");
    assert_eq!(realization.structural_parameter.position, 0);
    assert_eq!(
        realization.structural_parameter.multiplicity,
        language_semantics::Multiplicity::Affine
    );
    assert_eq!(
        realization.structural_parameter.access,
        checked_trees::CheckedStructuralAccess::Owned
    );
    assert!(realization.structural_parameter.qualifications.is_empty());
    assert_eq!(realization.scalar_parameters.len(), 1);
    assert_eq!(realization.scalar_parameters[0].source_position, 1);
    assert_eq!(
        realization.scalar_parameters[0].primitive_type,
        typed_trees::types::PrimitiveType::I32
    );
    assert_eq!(realization.result.type_identity, result.type_identity);
    assert_eq!(
        realization.result.multiplicity,
        language_semantics::Multiplicity::Affine
    );
    assert!(realization.result.qualifications.is_empty());
}

#[test]
fn specialized_mixed_structural_result_operator_hosted_native_reaches_d32() {
    let canary =
        pass_canary(fixture_roster::SPECIALIZED_MIXED_STRUCTURAL_RESULT_OPERATOR_HOSTED_NATIVE);
    for target in ["linux_x86_64", "linux_arm64"] {
        let report = compile_rooted_backend_canary_without_output_for_target(&canary, target)
            .unwrap_or_else(|diagnostics| {
                panic!(
                    "hosted mixed structural-result selection should realize natively for {target}: {diagnostics:#?}"
                )
            });
        let native = report.retained_native_artifact().unwrap_or_else(|| {
            panic!("hosted mixed structural-result selection should retain {target} native custody")
        });
        native.validate().unwrap_or_else(|error| {
            panic!("hosted mixed structural-result selection should replay for {target}: {error}")
        });
        let physical = report
            .require_package_native_physical_evidence()
            .unwrap_or_else(|error| {
                panic!("hosted mixed structural-result selection should earn {target} D32: {error}")
            });
        assert!(std::ptr::eq(
            physical,
            native.physical_evidence().unwrap_or_else(|| panic!(
                "hosted mixed structural-result selection should retain {target} D32"
            )),
        ));
    }
}

#[test]
fn specialized_mixed_structural_result_operator_has_exact_terminal_custody() {
    let canary =
        pass_canary(fixture_roster::SPECIALIZED_MIXED_STRUCTURAL_RESULT_OPERATOR_HOSTED_NATIVE);
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("hosted mixed structural-result selection should check");
    let produced =
        checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &checked, "consume",
        )
        .expect("mixed structural-result selection should reach Terminal");
    produced
        .boundary_operator_scope()
        .validate_for_artifact(produced.artifact())
        .expect("mixed structural-result D29 scope should bind the Terminal artifact");
    let [occurrence] = produced.boundary_operator_scope().occurrences() else {
        panic!("one exact mixed structural-result occurrence")
    };
    let module = terminal_codec::decode_module(produced.artifact().semantic_bytes())
        .expect("mixed structural-result Terminal semantics should decode");
    let matching_calls = module
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(move |operation| operation.id == occurrence.terminal_operation())
                .map(move |operation| (machine, operation))
        })
        .collect::<Vec<_>>();
    let [(terminal_consume, call)] = matching_calls.as_slice() else {
        panic!("one exact D29-bound mixed structural-result Terminal call")
    };
    let terminal_psi::OperationKind::CallStructuralWithScalarArguments {
        callee,
        arguments,
        structural_arguments,
        claim_transfers,
        returned_claim_transfers,
        requirement_obligations,
        crash_continuations,
    } = &call.kind
    else {
        panic!("mixed structural-result selection must retain its distinct Terminal role")
    };
    assert_eq!(arguments.len(), 1);
    assert_eq!(structural_arguments.len(), 1);
    assert!(claim_transfers.is_empty());
    assert!(returned_claim_transfers.is_empty());
    assert!(requirement_obligations.is_empty());
    assert!(crash_continuations.is_empty());
    let result = call
        .result
        .structural()
        .expect("structural operation result");
    assert_eq!(
        result.multiplicity,
        terminal_psi::StructuralMultiplicity::Affine
    );
    assert!(result.qualifications.is_empty());
    assert!(result.projected_qualifications.is_empty());
    assert!(result.claims.is_empty());
    assert!(matches!(
        terminal_consume.blocks.as_slice(),
        [block] if matches!(
            &block.terminator,
            terminal_psi::Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } if trivial_affine_discards.as_slice() == [result.place]
        )
    ));
    let provider = module
        .machines
        .iter()
        .find(|machine| machine.id == *callee)
        .expect("mixed structural-result provider realization");
    assert_eq!(provider.parameters.len(), 1);
    assert_eq!(provider.structural_parameters.len(), 1);
    assert!(provider.entry_claims.is_empty());
    assert!(provider.content_entry_claims.is_empty());
    assert!(matches!(
        provider.blocks.as_slice(),
        [block] if block.operations.is_empty()
            && matches!(
                &block.terminator,
                terminal_psi::Terminator::ReturnStructural {
                    source,
                    returned_claims,
                    trivial_affine_discards,
                    ..
                } if *source == provider.structural_parameters[0].place
                    && returned_claims.is_empty()
                    && trivial_affine_discards.is_empty()
            )
    ));
}

#[test]
fn specialized_mixed_structural_result_operator_reaches_installed_native_custody() {
    let canary =
        pass_canary(fixture_roster::SPECIALIZED_MIXED_STRUCTURAL_RESULT_OPERATOR_HOSTED_NATIVE);
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("hosted mixed structural-result selection should check");
    let terminal =
        checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &checked, "consume",
        )
        .expect("mixed structural-result selection should reach Terminal");
    let abstract_plan = terminal_psi_to_abstract_operations::lower_artifact_sections(
        terminal.artifact().semantic_bytes(),
        terminal.artifact().proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("mixed structural-result selection should reach target-neutral Omega");

    for target in [
        target::NativeTarget::linux_x64(),
        target::NativeTarget::linux_arm64(),
    ] {
        let target_plan = abstract_operations_to_target_operations::lower_to_target_operations(
            &abstract_plan,
            target,
        )
        .expect("mixed structural-result selection should reach target operations");
        let assigned =
            target_operations_to_assigned_target_operations::assign_registers(&target_plan)
                .expect("mixed structural-result selection should retain physical assignment");
        let emitted = machine_emission::emit_machine_code(&assigned)
            .expect("mixed structural-result selection should reach machine emission");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("entry caller");
        let [call] = caller.internal_unit_calls.as_slice() else {
            panic!("one mixed structural-result call must survive machine emission")
        };
        assert!(call.result.is_none());
        assert!(call.semantic_result.is_none());
        assert_eq!(call.scalar_arguments.len(), 1);
        assert_eq!(call.arguments.len(), 1);
        assert!(call.claim_transfers.is_empty());
        let result = call
            .structural_result
            .as_ref()
            .expect("call retains its affine structural result");
        assert_eq!(
            result.operation_result.multiplicity,
            terminal_psi::StructuralMultiplicity::Affine
        );
        assert!(result.operation_result.claims.is_empty());
        assert!(result.returned_claim_transfers.is_empty());
        assert!(result.returned_claims.is_empty());
        assert_eq!(
            caller
                .unit_affine_cleanup
                .as_ref()
                .expect("caller retains result cleanup")
                .actions,
            [terminal_psi::TerminalAffineCleanupAction::DiscardRoot(
                result.operation_result.place,
            )]
        );
        let returned = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.target)
            .and_then(|function| function.structural_return.as_ref())
            .expect("selected provider retains structural return ABI");
        assert_eq!(returned.scalar_parameters.len(), 1);
        assert_eq!(returned.parameters.len(), 1);
        assert_eq!(returned.parameter_placements.len(), 1);
        assert_eq!(
            returned.result.multiplicity,
            terminal_psi::StructuralMultiplicity::Affine
        );
        assert!(returned.returned_claims.is_empty());

        let object = image_emission::build_object_artifact(&emitted)
            .expect("object validation should replay mixed structural-result custody");
        let image = image_emission::emit_executable_image(&object, 0)
            .expect("image should retain mixed structural-result custody");
        image_emission::validate_executable_image(&object, &image)
            .expect("image should independently validate mixed structural-result custody");
        let installation = image_emission::build_installation_record(
            &image,
            semantic_vocabulary::ProfileDecisionId::new(1).expect("profile decision"),
        )
        .expect("mixed structural result should enter installation custody");
        let bytes = image_emission::encode_installation_record(&installation)
            .expect("mixed structural-result installation should encode");
        let decoded = image_emission::decode_installation_record(&bytes)
            .expect("mixed structural-result installation should decode");
        image_emission::validate_installation_record(&decoded, &image)
            .expect("decoded installation should bind the mixed structural-result image");

        let mut missing_discard = emitted.clone();
        missing_discard
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .and_then(|function| function.unit_affine_cleanup.as_mut())
            .expect("caller cleanup")
            .actions
            .clear();
        assert!(image_emission::build_object_artifact(&missing_discard).is_err());

        let mut injected_claim = emitted.clone();
        injected_claim
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .and_then(|function| function.internal_unit_calls.first_mut())
            .and_then(|call| call.structural_result.as_mut())
            .expect("caller structural result")
            .operation_result
            .claims
            .push(terminal_psi::StructuralResultClaimBinding {
                claim: semantic_vocabulary::ClaimId::new(1).expect("claim"),
                path: Vec::new(),
            });
        assert!(image_emission::build_object_artifact(&injected_claim).is_err());

        let mut swapped_slot = emitted.clone();
        let drifted_call = swapped_slot
            .functions
            .iter_mut()
            .find(|function| function.machine == emitted.entry)
            .and_then(|function| function.internal_unit_calls.first_mut())
            .expect("caller call");
        drifted_call.scalar_arguments[0].destination =
            drifted_call.arguments[0].destination.clone();
        assert!(image_emission::build_object_artifact(&swapped_slot).is_err());
    }
}

#[test]
fn specialized_mixed_structural_result_operator_rejects_terminal_custody_drift() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        RemovedScalar,
        BorrowedStructuralArgument,
        RetainedAffineResult,
        CoordinatedParameterPartition,
    }

    let canary =
        pass_canary(fixture_roster::SPECIALIZED_MIXED_STRUCTURAL_RESULT_OPERATOR_HOSTED_NATIVE);
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("hosted mixed structural-result selection should check");
    let consume = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("mixed structural-result Unit helper");

    for drift in [
        Drift::RemovedScalar,
        Drift::BorrowedStructuralArgument,
        Drift::RetainedAffineResult,
        Drift::CoordinatedParameterPartition,
    ] {
        let mut drifted = checked.clone();
        let (realization_machine, realization_state) = {
            let plan = drifted
                .facts
                .flow
                .terminal_unit_effects
                .machines
                .iter_mut()
                .find(|plan| plan.machine == consume.symbol)
                .expect("checked mixed structural-result helper plan");
            let Some(
                checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                    realization_machine,
                    realization_state,
                    scalar_arguments,
                    structural_arguments,
                    discard_result_on_return,
                    ..
                },
            ) = plan.operations.first_mut()
            else {
                panic!("checked mixed structural-result selected call")
            };
            match drift {
                Drift::RemovedScalar => scalar_arguments.clear(),
                Drift::BorrowedStructuralArgument => {
                    structural_arguments[0].access =
                        checked_trees::CheckedStructuralAccess::SharedBorrow;
                }
                Drift::RetainedAffineResult => *discard_result_on_return = false,
                Drift::CoordinatedParameterPartition => {}
            }
            (*realization_machine, *realization_state)
        };
        if matches!(drift, Drift::CoordinatedParameterPartition) {
            let realization = drifted
                .facts
                .flow
                .terminal_structural_returns
                .claim_free_affine_machines
                .iter_mut()
                .find(|realization| {
                    realization.machine == realization_machine
                        && realization.state == realization_state
                })
                .expect("checked claim-free affine structural realization");
            realization.scalar_parameters[0].source_position = 0;
            realization.structural_parameter.position = 1;
        }
        assert!(
            checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
                &drifted,
                "consume",
            )
            .is_err(),
            "{drift:?} must reject before Terminal publication"
        );
    }
}

#[test]
fn specialized_mixed_structural_fixed_operator_rejects_argument_drift() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        CoordinatedParameterPartition,
        RemovedScalar,
        SubstitutedScalar,
        StructuralSource,
    }

    let canary =
        pass_canary(fixture_roster::SPECIALIZED_MIXED_STRUCTURAL_FIXED_OPERATOR_HOSTED_NATIVE);
    let checked = compile_to_checked(&canary.join("main.omg"), Some("linux_x86_64"))
        .expect("hosted mixed structural/fixed-integer selection should check");
    let consume = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "consume")
        .expect("mixed structural/fixed-integer Unit helper");

    for drift in [
        Drift::CoordinatedParameterPartition,
        Drift::RemovedScalar,
        Drift::SubstitutedScalar,
        Drift::StructuralSource,
    ] {
        let mut drifted = checked.clone();
        if matches!(drift, Drift::CoordinatedParameterPartition) {
            let (realization_machine, realization_state) = {
                let plan = drifted
                    .facts
                    .flow
                    .terminal_unit_effects
                    .machines
                    .iter()
                    .find(|plan| plan.machine == consume.symbol)
                    .expect("checked mixed structural/fixed-integer helper plan");
                let Some(
                    checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                        realization_machine,
                        realization_state,
                        ..
                    },
                ) = plan.operations.first()
                else {
                    panic!("checked mixed structural/fixed-integer selected call")
                };
                (*realization_machine, *realization_state)
            };
            let realization = drifted
                .facts
                .flow
                .terminal_structural_scalar_returns
                .machines
                .iter_mut()
                .find(|realization| {
                    realization.machine == realization_machine
                        && realization.state == realization_state
                })
                .expect("checked mixed structural/fixed-integer realization plan");
            realization.scalar_parameters[0].source_position = 0;
            realization.structural_parameters[0].position = 1;
            assert!(
                checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
                    &drifted,
                    "consume",
                )
                .is_err(),
                "{drift:?} must reject before Terminal publication"
            );
            continue;
        }
        let plan = drifted
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == consume.symbol)
            .expect("checked mixed structural/fixed-integer helper plan");
        let Some(
            checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                scalar_arguments,
                structural_arguments,
                ..
            },
        ) = plan.operations.first_mut()
        else {
            panic!("checked mixed structural/fixed-integer selected call")
        };
        match drift {
            Drift::CoordinatedParameterPartition => unreachable!("handled above"),
            Drift::RemovedScalar => scalar_arguments.clear(),
            Drift::SubstitutedScalar => {
                let [checked_trees::CheckedScalarExpression::IntegerLiteral { literal }] =
                    scalar_arguments.as_mut_slice()
                else {
                    panic!("one checked integer literal argument")
                };
                let landing = literal.landing().expect("landed i32 literal");
                *literal = numerics::literals::IntegerLiteral::from_value(12).with_landing(landing);
            }
            Drift::StructuralSource => {
                structural_arguments[0].source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: 1,
                    };
            }
        }
        assert!(
            checked_trees_to_terminal_psi::produce_terminal_artifact_with_checked_boundary_operator_scope(
                &drifted,
                "consume",
            )
            .is_err(),
            "{drift:?} must reject before Terminal publication"
        );
    }
}
