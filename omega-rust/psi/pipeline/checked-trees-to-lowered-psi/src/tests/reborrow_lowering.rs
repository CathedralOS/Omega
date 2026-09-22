use super::{
    checked_scalar_suspension_fixture, lower_reborrow_rows, multihop_reborrow_source,
    reborrow_restored_call_source, reborrow_source, scalar_fixture_call_coordinate,
    shared_reborrow_restored_call_source, terminal_module_with_reborrow,
    three_shared_reborrow_restored_call_source, two_shared_reborrow_restored_call_source,
    two_shared_reborrow_restored_call_source_with_observations,
};
use crate::TerminalMachineSelection;
use crate::lower_machine;
use crate::lowering_error::LoweringError;
use checked_trees::CheckedUnitEffectOperationPlan;

#[test]
fn inline_scalar_call_computation_rejects_a_missing_source_occurrence() {
    let mut checked = checked_scalar_suspension_fixture();
    let (_, state, statement, _, _) = scalar_fixture_call_coordinate(&checked);
    let computations = &mut checked.facts.values.scalar_computations;
    let root = computations
        .root_at(
            state,
            u32::try_from(statement).unwrap(),
            checked_trees::CheckedScalarExpressionRole::LocalInitializer { binding_ordinal: 1 },
        )
        .expect("wait initializer computation")
        .root;
    let checked_trees::CheckedScalarComputationKind::Call { source_call, .. } =
        &mut computations.nodes.get_mut(root).kind
    else {
        panic!("wait initializer call")
    };
    *source_call = Default::default();
    assert!(
        lower_machine(&checked, TerminalMachineSelection::Name("root")).is_err(),
        "an inline call must retain its exact checked occurrence"
    );
}

#[test]
fn receiver_free_scalar_suspension_plan_rejoins_parameter_local_and_argument_frontier() {
    let mut checked = checked_scalar_suspension_fixture();
    let root = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "root")
        .expect("root machine");
    let state = checked.machine_states(root).first().expect("root state");
    let parameter = checked
        .state_parameters(state)
        .first()
        .expect("scalar parameter");
    let parameter_symbol = parameter.symbol;
    let parameter_type = parameter.type_reference;
    let typed_trees::statement::StatementNode::LocalData(local) =
        &checked.statement_table.statements(state.statement_nodes)[0]
    else {
        panic!("first statement is the scalar local")
    };
    let local_symbol = local.symbol;
    let local_type = local.type_reference;
    let (root_symbol, state_symbol, statement_index, call_ordinal, target) =
        scalar_fixture_call_coordinate(&checked);
    assert_eq!((statement_index, call_ordinal), (1, 0));
    let live = |storage, origin| checked_trees::SuspensionCrossingLiveValueFact {
        type_reference: local_type,
        storage,
        origin,
        claims: Vec::new(),
        effective: language_semantics::CarryPolicy::PERMISSIVE,
    };
    checked
        .facts
        .carry
        .suspension_crossings
        .push(checked_trees::SuspensionCrossingCarryFact {
            machine: root_symbol,
            state: state_symbol,
            statement_index,
            call_ordinal,
            target,
            receiver: None,
            effective: language_semantics::CarryPolicy::PERMISSIVE,
            live_values: vec![
                checked_trees::SuspensionCrossingLiveValueFact {
                    type_reference: parameter_type,
                    storage: checked_trees::SuspensionCrossingStorage::Parameter,
                    origin: checked_trees::SuspensionCrossingValueOrigin::Parameter {
                        symbol: parameter_symbol,
                        position: 0,
                    },
                    claims: Vec::new(),
                    effective: language_semantics::CarryPolicy::PERMISSIVE,
                },
                live(
                    checked_trees::SuspensionCrossingStorage::Local,
                    checked_trees::SuspensionCrossingValueOrigin::Local {
                        symbol: local_symbol,
                        statement_index: 0,
                        environment_position: 1,
                    },
                ),
                live(
                    checked_trees::SuspensionCrossingStorage::CallArgument,
                    checked_trees::SuspensionCrossingValueOrigin::CallArgument { position: 0 },
                ),
            ],
        });
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("root"))
        .expect("bounded scalar suspension lowers");
    let [site] = lowered.semantic_module.suspension_call_sites.as_slice() else {
        panic!("one exact suspension call site")
    };
    let [plan] = lowered.semantic_module.suspension_call_plans.as_slice() else {
        panic!("one exact suspension call plan")
    };
    assert_eq!(site.operation, plan.operation);
    assert_eq!(site.crossing, plan.crossing);
    assert_eq!(
        site.frontier_commitment,
        terminal_psi::suspension_frontier_commitment(plan)
    );
    for storage in [
        terminal_psi::TerminalSuspensionStorage::Parameter,
        terminal_psi::TerminalSuspensionStorage::Local,
        terminal_psi::TerminalSuspensionStorage::CallArgument,
    ] {
        assert!(plan.live_values.iter().any(|live| live.storage == storage));
    }
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("exact suspension plan verifies");
    let bytes =
        terminal_codec::encode_module(&lowered.semantic_module).expect("suspension plan encodes");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("suspension plan decodes"),
        lowered.semantic_module
    );
}

#[test]
fn unsupported_receiver_suspension_frontier_fails_closed() {
    let mut checked = checked_scalar_suspension_fixture();
    let (root_symbol, state_symbol, statement_index, call_ordinal, target) =
        scalar_fixture_call_coordinate(&checked);
    checked
        .facts
        .carry
        .suspension_crossings
        .push(checked_trees::SuspensionCrossingCarryFact {
            machine: root_symbol,
            state: state_symbol,
            statement_index,
            call_ordinal,
            target,
            receiver: Some(root_symbol),
            effective: language_semantics::CarryPolicy::PERMISSIVE,
            live_values: Vec::new(),
        });
    assert!(matches!(
        lower_machine(&checked, TerminalMachineSelection::Name("root")),
        Err(LoweringError::Unsupported(
            "receiver-bearing suspension frontier lacks an exact Terminal receiver place join"
        ))
    ));
}

#[test]
fn unsupported_staged_local_suspension_frontier_fails_closed() {
    let mut checked = crate::front_end::checked_program(
        r#"
            machine wait(value: bool) -> bool
            requires true == true
            ensures true == true
            { value }

            machine root(parameter: bool) -> bool
            requires true == true
            ensures true == true
            {
                let local: bool = true;
                let parked: bool = wait(local && parameter);
                local
            }
        "#,
    );
    let (root, state, statement_index, call_ordinal, target) =
        scalar_fixture_call_coordinate(&checked);
    let root_state = checked
        .machines()
        .iter()
        .find(|machine| machine.symbol == root)
        .and_then(|machine| {
            checked
                .machine_states(machine)
                .iter()
                .find(|candidate| candidate.symbol == state)
        })
        .expect("root state");
    let typed_trees::statement::StatementNode::LocalData(local) = &checked
        .statement_table
        .statements(root_state.statement_nodes)[0]
    else {
        panic!("first statement is local data")
    };
    let local_symbol = local.symbol;
    let local_type = local.type_reference;
    checked
        .facts
        .carry
        .suspension_crossings
        .push(checked_trees::SuspensionCrossingCarryFact {
            machine: root,
            state,
            statement_index,
            call_ordinal,
            target,
            receiver: None,
            effective: language_semantics::CarryPolicy::PERMISSIVE,
            live_values: vec![checked_trees::SuspensionCrossingLiveValueFact {
                type_reference: local_type,
                storage: checked_trees::SuspensionCrossingStorage::Local,
                origin: checked_trees::SuspensionCrossingValueOrigin::Local {
                    symbol: local_symbol,
                    statement_index: 0,
                    environment_position: 0,
                },
                claims: Vec::new(),
                effective: language_semantics::CarryPolicy::PERMISSIVE,
            }],
        });
    let result = lower_machine(&checked, TerminalMachineSelection::Name("root"));
    assert!(
        matches!(
            result,
            Err(LoweringError::Unsupported(
                "suspension frontier source value origin is inexact"
            ))
        ),
        "unexpected staged-local fence result: {result:?}"
    );
}

#[test]
fn ordinary_scalar_lowering_keeps_suspension_catalogs_empty() {
    let checked = crate::front_end::checked_program(
        r#"
            machine identity(value: bool) -> bool
            requires true == true
            ensures true == true
            { value }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("identity"))
        .expect("ordinary scalar lowering");
    assert_eq!(lowered.semantic_module.suspension_call_plan_count, 0);
    assert!(lowered.semantic_module.suspension_call_sites.is_empty());
    assert!(lowered.semantic_module.suspension_call_plans.is_empty());
}

#[test]
fn terminal_write_only_root_handoff_preserves_exclusive_access() {
    let checked = crate::front_end::checked_program(
        r#"
        data Cell { value: i32; }
        machine exercise(value: &write Cell) {
            let parent: &write Cell = &write value;
            let child: &write Cell = &write parent;
            child.value = 1;
        }
    "#,
    );
    let rows = lower_reborrow_rows(&checked).expect("write-only root retains its handoff");
    let [row] = rows.as_slice() else {
        panic!("one complete root handoff");
    };
    assert_eq!(
        row.direct_root_access,
        terminal_psi::StructuralAccess::WriteOnlyBorrow
    );
    assert_eq!(row.lineage.len(), 1);
    assert_eq!(
        row.lineage[0].child_access,
        terminal_psi::StructuralAccess::WriteOnlyBorrow
    );
    // This helper exercises handoff metadata shape, not source/native admission.
    let module = terminal_module_with_reborrow(&checked);
    terminal_verifier::validate_module(&module).expect("write-only handoff verifies");
    let bytes = terminal_codec::encode_module(&module).expect("write-only handoff encodes");
    assert_eq!(terminal_codec::decode_module(&bytes).unwrap(), module);
    for forbidden in [
        terminal_psi::StructuralAccess::SharedBorrow,
        terminal_psi::StructuralAccess::MutableBorrow,
        terminal_psi::StructuralAccess::Owned,
    ] {
        let mut changed = module.clone();
        changed.reborrow_root_handoffs[0].lineage[0].child_access = forbidden;
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "child access {forbidden:?}"
        );
    }
    for forbidden in [
        terminal_psi::StructuralAccess::SharedBorrow,
        terminal_psi::StructuralAccess::Owned,
    ] {
        let mut changed = module.clone();
        changed.reborrow_root_handoffs[0].direct_root_access = forbidden;
        assert!(
            terminal_verifier::validate_module(&changed).is_err(),
            "root access {forbidden:?}"
        );
    }
}

#[test]
fn terminal_reborrow_root_handoff_lowers_mutable_and_write_only_children() {
    for child_access in ["&mut", "&write"] {
        let checked = reborrow_source(child_access);
        let rows = lower_reborrow_rows(&checked)
            .expect("one-hop state-exit reborrow publishes root custody");
        let [row] = rows.as_slice() else {
            panic!("one exact root handoff")
        };
        assert_eq!(
            row.direct_root_access,
            terminal_psi::StructuralAccess::MutableBorrow
        );
        let [step] = row.lineage.as_slice() else {
            panic!("one exact child edge")
        };
        assert_eq!(
            step.child_access,
            if child_access == "&mut" {
                terminal_psi::StructuralAccess::MutableBorrow
            } else {
                terminal_psi::StructuralAccess::WriteOnlyBorrow
            }
        );
        assert_eq!(step.formation_boundary, step.child_activation);
        assert_eq!(
            row.direct_root_lifetime_identity,
            row.direct_root_place.root_identity
        );
    }
}

#[test]
fn terminal_reborrow_restored_call_use_lowers_exclusive_and_sole_shared_children() {
    for (label, checked, machine_name, expected_access) in [
        (
            "mutable",
            reborrow_restored_call_source("&mut"),
            "Harness::exercise",
            terminal_psi::StructuralAccess::MutableBorrow,
        ),
        (
            "write-only",
            reborrow_restored_call_source("&write"),
            "Harness::exercise",
            terminal_psi::StructuralAccess::WriteOnlyBorrow,
        ),
        (
            "sole-shared",
            shared_reborrow_restored_call_source(),
            "Harness::exercise",
            terminal_psi::StructuralAccess::SharedBorrow,
        ),
    ] {
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name(machine_name))
            .unwrap_or_else(|error| {
                panic!("{label} restored-parent mutating call lowers to Terminal Psi: {error:?}")
            });
        let [use_row] = lowered
            .semantic_module
            .reborrow_restored_call_uses
            .as_slice()
        else {
            panic!("one exact restored-parent call use")
        };
        assert_eq!(use_row.machine, lowered.semantic_module.entry);
        assert_eq!(use_row.child_access, expected_access);
        assert_eq!(
            use_row.restoration_class,
            if expected_access == terminal_psi::StructuralAccess::SharedBorrow {
                terminal_psi::TerminalReborrowRestorationClass::SharedFreezeRestoration
            } else {
                terminal_psi::TerminalReborrowRestorationClass::ExclusiveReactivation
            }
        );
        if expected_access == terminal_psi::StructuralAccess::SharedBorrow {
            let [member] = use_row.shared_cohort.as_slice() else {
                panic!("sole shared restoration retains one cohort member")
            };
            assert_eq!(member.child_owner_identity, use_row.child_owner_identity);
            assert_eq!(member.child_owner_path, use_row.child_owner_path);
            assert_eq!(member.child_place, use_row.child_place);
            assert_eq!(member.child_access, use_row.child_access);
            assert_eq!(member.child_activation, use_row.child_activation);
            assert_eq!(member.child_weakening, use_row.child_weakening);
        } else {
            assert!(use_row.shared_cohort.is_empty());
        }
        let terminal_psi::TerminalBorrowBoundarySource::Call {
            statement_index,
            call_ordinal,
            target_identity,
        } = &use_row.call_boundary
        else {
            panic!("restored use retains one exact source call coordinate")
        };
        assert_eq!(*call_ordinal, 0);
        assert!(!target_identity.is_empty());
        let caller = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == use_row.machine)
            .expect("restored-use caller");
        let operation = caller
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| operation.id == use_row.operation)
            .expect("restored-use operation");
        let terminal_psi::OperationKind::CallUnit { callee, .. } = operation.kind else {
            panic!("restored-use operation is CallUnit")
        };
        assert_eq!(use_row.call_target_machine, callee);
        let certificate = checked
            .facts
            .borrow
            .reborrow_restored_call_use_certificates
            .iter()
            .next()
            .expect("checked restored use")
            .1;
        let checked_trees::FlowInvalidationSource::Statement {
            statement_index: child_end,
        } = checked
            .facts
            .borrow
            .reborrow_loan_resources
            .get(certificate.child_resource)
            .weakening_source
        else {
            panic!("restored child weakens at one statement")
        };
        assert_eq!(
            *statement_index,
            u64::try_from(child_end).expect("statement range")
        );
        assert_eq!(
            use_row.direct_root_lifetime_identity,
            use_row.direct_root_place.root_identity
        );
        assert_eq!(use_row.formation_boundary, use_row.child_activation);
        assert!(lowered.source_call_occurrences.iter().any(|occurrence| {
            occurrence.terminal_operation == use_row.operation
                && occurrence.source_state
                    == checked
                        .facts
                        .borrow
                        .reborrow_restored_call_use_certificates
                        .iter()
                        .next()
                        .expect("checked restored use")
                        .1
                        .state_symbol
        }));
        terminal_verifier::validate_module(&lowered.semantic_module)
            .expect("restored call use verifies");
        let encoded = terminal_codec::encode_module(&lowered.semantic_module)
            .expect("restored call use encodes");
        assert_eq!(
            terminal_codec::decode_module(&encoded).expect("restored call use decodes"),
            lowered.semantic_module
        );
    }
}

#[test]
fn terminal_reborrow_restored_call_use_lowers_exact_two_member_shared_cohort() {
    let checked = two_shared_reborrow_restored_call_source();
    let exercise = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| checked.typed.symbols.name(plan.machine) == "Harness::exercise")
        .expect("the exact alias-erased exercise plan");
    let [
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: observation_coordinate,
            structural_arguments: observation_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: mutation_coordinate,
            structural_arguments: mutation_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = exercise.operations.as_slice()
    else {
        panic!("one observation call, one restored mutation, and Unit return")
    };
    assert_eq!(observation_coordinate.statement_index, 3);
    assert_eq!(mutation_coordinate.statement_index, 4);
    assert_eq!(observation_arguments.len(), 2);
    assert!(observation_arguments.iter().all(|argument| {
        argument.source_parameter_index() == Some(0)
            && argument.path.is_empty()
            && argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow
    }));
    let [mutation_argument] = mutation_arguments.as_slice() else {
        panic!("one restored whole-parent mutation argument")
    };
    assert_eq!(mutation_argument.source_parameter_index(), Some(0));
    assert!(mutation_argument.path.is_empty());
    assert_eq!(
        mutation_argument.access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );
    let lowered = lower_machine(
        &checked,
        TerminalMachineSelection::Name("Harness::exercise"),
    )
    .expect("two-member shared cohort lowers to Terminal Psi");
    let [row] = lowered
        .semantic_module
        .reborrow_restored_call_uses
        .as_slice()
    else {
        panic!("one restored-parent call publication")
    };
    assert_eq!(
        row.restoration_class,
        terminal_psi::TerminalReborrowRestorationClass::SharedFreezeRestoration
    );
    let [left, right] = row.shared_cohort.as_slice() else {
        panic!("the exact two-member shared-freeze roster")
    };
    assert_ne!(left, right);
    for member in [left, right] {
        assert_eq!(
            member.child_access,
            terminal_psi::StructuralAccess::SharedBorrow
        );
        assert_eq!(member.child_weakening, row.child_weakening);
    }
    assert!(row.shared_cohort.iter().any(|member| {
        member.child_owner_identity == row.child_owner_identity
            && member.child_owner_path == row.child_owner_path
            && member.child_place == row.child_place
            && member.child_activation == row.child_activation
    }));
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("two-member restored call use verifies");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("two-member restored call use encodes");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("two-member restored call use decodes"),
        lowered.semantic_module
    );
}

#[test]
fn terminal_reborrow_restored_call_use_lowers_exact_three_member_shared_cohort() {
    let checked = three_shared_reborrow_restored_call_source();
    let exercise = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| checked.typed.symbols.name(plan.machine) == "Harness::exercise")
        .expect("the exact alias-erased exercise plan");
    let [
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: observation_coordinate,
            structural_arguments: observation_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::CallUnit {
            coordinate: mutation_coordinate,
            structural_arguments: mutation_arguments,
            ..
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = exercise.operations.as_slice()
    else {
        panic!("one observation call, one restored mutation, and Unit return")
    };
    assert_eq!(observation_coordinate.statement_index, 4);
    assert_eq!(mutation_coordinate.statement_index, 5);
    assert_eq!(observation_arguments.len(), 3);
    assert!(observation_arguments.iter().all(|argument| {
        argument.source_parameter_index() == Some(0)
            && argument.path.is_empty()
            && argument.access == checked_trees::CheckedStructuralAccess::SharedBorrow
    }));
    let [mutation_argument] = mutation_arguments.as_slice() else {
        panic!("one restored whole-parent mutation argument")
    };
    assert_eq!(mutation_argument.source_parameter_index(), Some(0));
    assert!(mutation_argument.path.is_empty());
    assert_eq!(
        mutation_argument.access,
        checked_trees::CheckedStructuralAccess::MutableBorrow
    );

    let lowered = lower_machine(
        &checked,
        TerminalMachineSelection::Name("Harness::exercise"),
    )
    .expect("three-member shared cohort lowers to Terminal Psi");
    let [row] = lowered
        .semantic_module
        .reborrow_restored_call_uses
        .as_slice()
    else {
        panic!("one restored-parent call publication")
    };
    let [left, middle, right] = row.shared_cohort.as_slice() else {
        panic!("the exact three-member shared-freeze roster")
    };
    assert_ne!(left, middle);
    assert_ne!(left, right);
    assert_ne!(middle, right);
    for member in [left, middle, right] {
        assert_eq!(
            member.child_access,
            terminal_psi::StructuralAccess::SharedBorrow
        );
        assert_eq!(member.child_weakening, row.child_weakening);
    }
    terminal_verifier::validate_module(&lowered.semantic_module)
        .expect("three-member restored call use verifies");
    let bytes = terminal_codec::encode_module(&lowered.semantic_module)
        .expect("three-member restored call use encodes");
    assert_eq!(
        terminal_codec::decode_module(&bytes).expect("three-member restored call use decodes"),
        lowered.semantic_module
    );

    let certificate = checked
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .next()
        .expect("three-member checked restored use")
        .1
        .clone();
    let mut duplicate = checked;
    let disposition = duplicate
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(certificate.disposition);
    disposition.shared_cohort[2] = disposition.shared_cohort[0];
    assert!(
        lower_machine(
            &duplicate,
            TerminalMachineSelection::Name("Harness::exercise")
        )
        .is_err(),
        "nonadjacent duplicate cohort members must fail checked-to-Terminal replay"
    );
}

#[test]
fn terminal_two_shared_restored_call_aliasing_fences_reordered_and_extra_observations() {
    for observations in [
        "Sink::observe(right, left);",
        "Sink::observe(left, right); Sink::observe(left, right);",
    ] {
        let checked = two_shared_reborrow_restored_call_source_with_observations(observations);
        assert!(
            checked
                .facts
                .borrow
                .reborrow_restored_call_use_certificates
                .is_empty(),
            "unsupported shared observation layout must not gain checked authority"
        );
        assert!(
            lower_machine(
                &checked,
                TerminalMachineSelection::Name("Harness::exercise")
            )
            .is_err(),
            "unsupported shared observation layout must remain outside the Unit plan"
        );
    }
}

#[test]
fn terminal_shared_restored_call_use_rejects_checked_cohort_drift() {
    let baseline = shared_reborrow_restored_call_source();
    let certificate = baseline
        .facts
        .borrow
        .reborrow_restored_call_use_certificates
        .iter()
        .next()
        .expect("shared restored use")
        .1
        .clone();

    let mut missing = baseline.clone();
    missing
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(certificate.disposition)
        .shared_cohort
        .clear();
    assert!(
        lower_machine(
            &missing,
            TerminalMachineSelection::Name("Harness::exercise")
        )
        .is_err()
    );

    let mut wrong_disposition = baseline.clone();
    wrong_disposition
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(certificate.disposition)
        .disposition = checked_trees::CheckedReborrowResourceDisposition::Reactivate;
    assert!(
        lower_machine(
            &wrong_disposition,
            TerminalMachineSelection::Name("Harness::exercise")
        )
        .is_err()
    );

    let mut wrong_containment = baseline;
    wrong_containment
        .facts
        .borrow
        .reborrow_containment_certificates
        .get_mut(certificate.containment)
        .containment = checked_trees::CheckedReborrowContainmentKind::ExclusiveSuspension;
    assert!(
        lower_machine(
            &wrong_containment,
            TerminalMachineSelection::Name("Harness::exercise")
        )
        .is_err()
    );

    let mut wrong_parent_status = shared_reborrow_restored_call_source();
    wrong_parent_status
        .facts
        .borrow
        .reborrow_loan_resources
        .get_mut(certificate.child_resource)
        .parent_end_status
        .status = checked_trees::ParentLexicalStatusAtChildEnd::RetiredWithChild;
    assert!(
        lower_machine(
            &wrong_parent_status,
            TerminalMachineSelection::Name("Harness::exercise")
        )
        .is_err()
    );

    let mut wrong_parent_weakening = shared_reborrow_restored_call_source();
    wrong_parent_weakening
        .facts
        .borrow
        .reborrow_containment_certificates
        .get_mut(certificate.containment)
        .parent_weakening = certificate.child_weakening;
    assert!(
        lower_machine(
            &wrong_parent_weakening,
            TerminalMachineSelection::Name("Harness::exercise")
        )
        .is_err()
    );

    let mut invalid_formation_constraint = shared_reborrow_restored_call_source();
    invalid_formation_constraint
        .facts
        .borrow
        .reborrow_loan_resources
        .get_mut(certificate.child_resource)
        .parent_suspension
        .parent_entry_constraint = arena::Handle::invalid();
    assert!(
        lower_machine(
            &invalid_formation_constraint,
            TerminalMachineSelection::Name("Harness::exercise")
        )
        .is_err()
    );
}

#[test]
fn terminal_reborrow_root_handoff_lowers_finite_linear_exclusive_lineages() {
    for (middle_access, leaf_access, expected) in [
        (
            "&mut",
            "&mut",
            [
                terminal_psi::StructuralAccess::MutableBorrow,
                terminal_psi::StructuralAccess::MutableBorrow,
            ],
        ),
        (
            "&mut",
            "&write",
            [
                terminal_psi::StructuralAccess::MutableBorrow,
                terminal_psi::StructuralAccess::WriteOnlyBorrow,
            ],
        ),
        (
            "&write",
            "&write",
            [
                terminal_psi::StructuralAccess::WriteOnlyBorrow,
                terminal_psi::StructuralAccess::WriteOnlyBorrow,
            ],
        ),
    ] {
        let checked = multihop_reborrow_source(middle_access, leaf_access);
        let rows = lower_reborrow_rows(&checked)
            .expect("a finite linear exclusive lineage publishes root custody");
        let [row] = rows.as_slice() else {
            panic!("one exact multihop root handoff")
        };
        assert_eq!(
            row.direct_root_access,
            terminal_psi::StructuralAccess::MutableBorrow
        );
        assert_eq!(
            row.lineage
                .iter()
                .map(|step| step.child_access)
                .collect::<Vec<_>>(),
            expected,
        );
        assert!(row.lineage.iter().all(|step| {
            step.formation_boundary == step.child_activation
                && step.child_place.root_identity == row.direct_root_place.root_identity
        }));
        assert_eq!(
            row.direct_root_lifetime_identity,
            row.direct_root_place.root_identity
        );
    }
}

#[test]
fn terminal_reborrow_root_handoff_fences_shared_and_branched_lineages() {
    let shared = reborrow_source("&");
    assert!(lower_reborrow_rows(&shared).is_err());

    let branched = crate::front_end::checked_program(
        r#"
            data Cell { value: i32; }
            data Main { cell: Cell; }
            machine use_mut(value: &mut Cell) { value.value = 1; }
            machine Main::exercise(&mut self) {
                let root: &mut Cell = &mut self.cell;
                let first: &mut Cell = &mut root;
                use_mut(first);
                let second: &mut Cell = &mut root;
            }
        "#,
    );
    assert!(lower_reborrow_rows(&branched).is_err());
}

#[test]
fn terminal_multihop_root_handoff_rejects_missing_or_reordered_checked_edges() {
    let baseline = multihop_reborrow_source("&mut", "&mut");
    let containment = baseline
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .next()
        .expect("first containment edge")
        .0;
    let mut missing = baseline.clone();
    let retained = missing
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .filter(|(handle, _)| *handle != containment)
        .map(|(_, row)| row.clone())
        .collect::<Vec<_>>();
    missing
        .facts
        .borrow
        .reborrow_containment_certificates
        .reset_retain_capacity();
    for row in retained {
        missing
            .facts
            .borrow
            .reborrow_containment_certificates
            .insert(row);
    }
    assert!(lower_reborrow_rows(&missing).is_err());

    let mut reordered = baseline;
    let event = reordered
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .find(|(_, event)| {
            event.disposition
                == checked_trees::CheckedReborrowResourceDisposition::StateExitDirectRootHandoff
        })
        .expect("multihop root disposition")
        .0;
    reordered
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(event)
        .retired_parent_path
        .swap(0, 1);
    assert!(lower_reborrow_rows(&reordered).is_err());
}

#[test]
fn terminal_reborrow_root_handoff_rejects_tampered_checked_joins() {
    let baseline = reborrow_source("&mut");
    let event_handle = baseline
        .facts
        .borrow
        .reborrow_disposition_events
        .iter()
        .next()
        .expect("disposition")
        .0;
    let certificate_handle = baseline
        .facts
        .borrow
        .reborrow_containment_certificates
        .iter()
        .next()
        .expect("containment")
        .0;

    let mut wrong_phase = baseline.clone();
    wrong_phase
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(event_handle)
        .boundary_phase = checked_trees::CheckedBorrowResourceLifecyclePhase::Activation;
    assert!(lower_reborrow_rows(&wrong_phase).is_err());

    let mut wrong_disposition = baseline.clone();
    wrong_disposition
        .facts
        .borrow
        .reborrow_disposition_events
        .get_mut(event_handle)
        .disposition = checked_trees::CheckedReborrowResourceDisposition::Reactivate;
    assert!(lower_reborrow_rows(&wrong_disposition).is_err());

    let mut wrong_containment = baseline.clone();
    wrong_containment
        .facts
        .borrow
        .reborrow_containment_certificates
        .get_mut(certificate_handle)
        .containment = checked_trees::CheckedReborrowContainmentKind::SharedFreeze;
    assert!(lower_reborrow_rows(&wrong_containment).is_err());

    let child_handle = baseline
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .next()
        .expect("child resource")
        .0;
    let mut wrong_effect = baseline.clone();
    wrong_effect
        .facts
        .borrow
        .reborrow_loan_resources
        .get_mut(child_handle)
        .access_effect = checked_trees::CheckedReborrowAccessEffect::SharedFreeze;
    assert!(lower_reborrow_rows(&wrong_effect).is_err());

    let mut missing_event = baseline.clone();
    missing_event.facts.borrow.reborrow_disposition_events = arena::Arena::new();
    assert!(lower_reborrow_rows(&missing_event).is_err());
}

#[test]
fn terminal_reborrow_root_handoff_codec_and_verifier_reject_tampering() {
    let checked = reborrow_source("&mut");
    let module = terminal_module_with_reborrow(&checked);
    terminal_verifier::validate_module(&module).expect("exact handoff verifies");
    let encoded = terminal_codec::encode_module(&module).expect("handoff encodes");
    assert_eq!(
        terminal_codec::decode_module(&encoded).expect("handoff decodes"),
        module
    );

    let mut amplified = module.clone();
    amplified.reborrow_root_handoffs[0].direct_root_access = terminal_psi::StructuralAccess::Owned;
    assert!(terminal_verifier::validate_module(&amplified).is_err());

    let mut redirected = module.clone();
    redirected.reborrow_root_handoffs[0].lineage[0].formation_boundary =
        terminal_psi::TerminalBorrowBoundarySource::Statement {
            statement_index: u64::MAX,
        };
    assert!(terminal_verifier::validate_module(&redirected).is_err());

    let mut duplicated = module.clone();
    duplicated
        .reborrow_root_handoffs
        .push(duplicated.reborrow_root_handoffs[0].clone());
    assert!(terminal_verifier::validate_module(&duplicated).is_err());

    let mut observed_invalid_access_tag = false;
    for (index, byte) in encoded.iter().enumerate() {
        if *byte != 3 {
            continue;
        }
        let mut tampered = encoded.clone();
        tampered[index] = 0xff;
        if matches!(
            terminal_codec::decode_module(&tampered),
            Err(terminal_codec::CodecError::InvalidTag(
                "StructuralAccess",
                0xff
            ))
        ) {
            observed_invalid_access_tag = true;
            break;
        }
    }
    assert!(
        observed_invalid_access_tag,
        "codec rejects a corrupted custody access tag"
    );
}

#[test]
fn terminal_multihop_root_handoff_round_trips_and_rejects_lineage_drift() {
    let checked = multihop_reborrow_source("&mut", "&mut");
    let module = terminal_module_with_reborrow(&checked);
    assert_eq!(module.reborrow_root_handoffs[0].lineage.len(), 2);
    terminal_verifier::validate_module(&module).expect("exact multihop handoff verifies");
    let encoded = terminal_codec::encode_module(&module).expect("multihop handoff encodes");
    assert_eq!(
        terminal_codec::decode_module(&encoded).expect("multihop handoff decodes"),
        module
    );

    let mut empty = module.clone();
    empty.reborrow_root_handoffs[0].lineage.clear();
    assert!(terminal_verifier::validate_module(&empty).is_err());

    let mut amplified = module.clone();
    amplified.reborrow_root_handoffs[0].lineage[0].child_access =
        terminal_psi::StructuralAccess::WriteOnlyBorrow;
    amplified.reborrow_root_handoffs[0].lineage[1].child_access =
        terminal_psi::StructuralAccess::MutableBorrow;
    assert!(terminal_verifier::validate_module(&amplified).is_err());

    let mut retargeted = module;
    retargeted.reborrow_root_handoffs[0].lineage[1]
        .projection_remainder
        .push(terminal_psi::TerminalBorrowPlaceSegment::FixedIndex(0));
    assert!(terminal_verifier::validate_module(&retargeted).is_err());
}
