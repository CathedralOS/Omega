use super::*;

#[test]
fn specialized_structural_fixed_operator_hosted_native_canary_compiles() {
    let canary = pass_canary("providers/specialized_structural_fixed_operator_hosted_native");
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
    let canary = pass_canary("providers/specialized_structural_fixed_operator_hosted_native");
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
        psi_checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
            declaration_ordinal: 0,
            ..
        },
        psi_checked_trees::CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
            declaration_ordinal: 1,
            ..
        },
        psi_checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
            structural_arguments,
            ..
        },
        psi_checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
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
        argument.path.is_empty()
            && argument.access == psi_checked_trees::CheckedStructuralAccess::Owned
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
            psi_typed_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
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
                    && event.root == psi_facts::PlaceRoot::Symbol(local_symbol))
                .then_some(event)
            })
            .collect::<Vec<_>>();
        let [establishment, transfer] = events.as_slice() else {
            panic!(
                "one establishment and one transfer for hosted local {statement_index}: {events:#?}"
            )
        };
        let establishment_source =
            psi_language_semantics::PermissionEventSource::Statement { statement_index };
        let provenance = psi_language_semantics::PermissionProvenance::Established {
            machine_symbol: main.symbol,
            state_symbol: state.symbol,
            source: establishment_source,
        };
        assert_eq!(establishment.source, establishment_source);
        assert_eq!(
            establishment.kind,
            psi_language_semantics::PermissionEventKind::Establish
        );
        let psi_language_semantics::PermissionEventSource::Call {
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
            psi_language_semantics::PermissionEventKind::Transfer
        );
        for event in events {
            assert_eq!(
                event.multiplicity,
                psi_language_semantics::Multiplicity::Affine
            );
            assert_eq!(
                event.access,
                psi_language_semantics::PermissionAccess::Owned
            );
            assert_eq!(
                event.claim_identity,
                psi_language_semantics::PermissionClaimIdentity::Unknown
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

    let _ = psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
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

    let canary = pass_canary("providers/specialized_structural_fixed_operator_hosted_native");
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
                    psi_checked_trees::CheckedUnitEffectOperationPlan::CallUnit { .. }
                )
            })
            .expect("hosted consuming call");
        match drift {
            Drift::RestoredDiscard => {
                let Some(psi_checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
                    trivial_affine_local_discard_ordinals,
                    ..
                }) = plan.operations.last_mut()
                else {
                    panic!("checked hosted return")
                };
                trivial_affine_local_discard_ordinals.push(0);
            }
            drift => {
                let psi_checked_trees::CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } = &mut plan.operations[call_index]
                else {
                    unreachable!()
                };
                match drift {
                    Drift::UnknownLocal => {
                        structural_arguments[0].source =
                            psi_checked_trees::CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal {
                                declaration_ordinal: 2,
                            };
                    }
                    Drift::DuplicateLocal => {
                        structural_arguments[0].source = structural_arguments[1].source.clone();
                    }
                    Drift::ParameterSubstitution => {
                        structural_arguments[0].source =
                            psi_checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                parameter_index: 0,
                            };
                    }
                    Drift::ProjectedPath => structural_arguments[0]
                        .path
                        .push(psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0)),
                    Drift::BorrowedAccess => {
                        structural_arguments[0].access =
                            psi_checked_trees::CheckedStructuralAccess::SharedBorrow;
                    }
                    Drift::RestoredDiscard => unreachable!(),
                }
            }
        }
        assert!(
            psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
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
        pass_canary("providers/specialized_structural_fixed_operator_unit_terminal_custody");
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
        psi_checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
            coordinate,
            result,
            structural_arguments,
            ..
        },
        psi_checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
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
        psi_typed_trees::types::PrimitiveType::Bool
    );
    assert_eq!(structural_arguments.len(), 2);
    assert!(structural_arguments.iter().all(|argument| {
        argument.path.is_empty()
            && argument.byte_sequence_literal().is_none()
            && argument.access == psi_checked_trees::CheckedStructuralAccess::Owned
    }));
    assert!(trivial_affine_discards.is_empty());
    omega_selected_dispatch::validate_selected_operator_terminal_custody(
        &checked,
        checked.selected_provider_plans(),
    )
    .expect("structural Unit call must retain its exact selected ProviderPlan");

    let produced =
        psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &checked,
            "consume",
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
    let module = psi_terminal_codec::decode_module(produced.artifact().semantic_bytes())
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
    let psi_terminal::OperationKind::CallStructuralScalar {
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
        argument.path.is_empty() && argument.access == psi_terminal::StructuralAccess::Owned
    }));
    assert!(claim_transfers.is_empty());
    assert!(requirement_obligations.is_empty());
    assert!(crash_continuations.is_empty());

    let realizations =
        omega_selected_dispatch::derive_checked_specialized_operator_application_realizations(
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
        pass_canary("providers/specialized_structural_fixed_operator_unit_terminal_custody");
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
                let Some(psi_checked_trees::CheckedUnitEffectOperationPlan::ReturnUnit {
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
                    psi_checked_trees::CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
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
                    Drift::ProjectedPath => structural_arguments[0]
                        .path
                        .push(psi_checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(0)),
                    Drift::BorrowedAccess => {
                        structural_arguments[0].access =
                            psi_checked_trees::CheckedStructuralAccess::SharedBorrow;
                    }
                    Drift::MissingRealization => {
                        *realization_machine = psi_symbols::SymbolHandle::invalid();
                    }
                    Drift::MissingProviderPlan => {
                        *provider_plan_commitment =
                            psi_checked_trees::CheckedProviderPlanCommitment::default();
                    }
                    Drift::RestoredDiscard => unreachable!(),
                }
            }
        }
        if psi_checked_trees_to_terminal::produce_terminal_artifact_with_checked_boundary_operator_scope(
            &drifted,
            "consume",
        )
        .is_ok()
        {
            panic!("{drift:?} must reject before Terminal publication");
        }
    }
}
