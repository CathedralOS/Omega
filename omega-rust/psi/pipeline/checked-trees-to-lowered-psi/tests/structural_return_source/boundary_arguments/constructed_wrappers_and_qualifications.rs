use super::{
    ObserveSettlement, artifact, assert_unsettled_helper_crash, checked,
    constructed_wrapper_source, pause_before_crashing_helper, record_field_computation, start,
    unit_wrapper_artifact, unit_wrapper_source, unsigned,
};
use crate::structural_return_source::{
    AdmissionProfile, RESULT_BOUNDARY_CUSTODY_SOURCE, ResultBoundaryHandler,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter, TerminalInterpretError,
    TerminalScalarValue, Terminator, decode_module,
};
use checked_trees::CheckedUnitEffectOperationPlan;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

#[test]
fn unit_wrapper_constructor_source_and_permission_mutations_reject() {
    let original = checked(&constructed_wrapper_source("value: i64;", "value: 7i64"));
    for mutation in 0..3 {
        let mut changed = original.clone();
        let root = changed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Root::enter")
            .unwrap();
        let root_symbol = root.symbol;
        let state = &changed.machine_states(root)[0];
        let StatementNode::LocalData(local) =
            &changed.statement_table.statements(state.statement_nodes)[0]
        else {
            unreachable!()
        };
        let local_symbol = local.symbol;
        let ExpressionNode::StructLiteral(literal) =
            changed.expression_table.expression(local.initial_value)
        else {
            unreachable!()
        };
        let field = &changed.expression_table.struct_fields(literal.fields)[0];
        let expression = field.value;
        let field_symbol = field.field_symbol;
        if mutation == 0 {
            let ExpressionNode::Integer(value) =
                changed.typed.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            *value = value.with_landing(numerics::literals::IntegerLanding {
                landed_type: numerics::literals::LandedIntegerType::U64,
                domain: numerics::arithmetic::ArithmeticDomain::Exact,
            });
        } else if mutation == 1 {
            let handle = changed.typed.data_members.iter().find_map(|(handle, member)| {
                matches!(member, typed_trees::data::DataMember::Field(field) if field.symbol == field_symbol).then_some(handle)
            }).unwrap();
            let typed_trees::data::DataMember::Field(field) =
                changed.typed.data_members.get_mut(handle)
            else {
                unreachable!()
            };
            field.relevance = language_core::BindingRelevance::Erased;
        } else {
            let event = changed
                .facts
                .flow
                .ownership
                .permissions
                .iter()
                .find_map(|(_, event)| {
                    (event.machine_symbol == root_symbol
                        && event.root == facts::PlaceRoot::Symbol(local_symbol))
                    .then_some(event.clone())
                })
                .unwrap();
            changed.facts.flow.ownership.permissions.insert(event);
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err(),
            "constructor source custody mutation {mutation}"
        );
    }
}

#[test]
fn unit_wrapper_constructor_value_cannot_drift_from_source() {
    let mut original = checked(&constructed_wrapper_source("value: i64;", "value: 7i64"));
    let replacement = checked(&constructed_wrapper_source("value: i64;", "value: 8i64"));
    let value = replacement
        .facts
        .values
        .scalar_computations
        .nodes
        .get(record_field_computation(&replacement))
        .kind
        .clone();
    let handle = record_field_computation(&original);
    let retained = &mut original
        .facts
        .values
        .scalar_computations
        .nodes
        .get_mut(handle)
        .kind;
    assert_ne!(*retained, value);
    *retained = value;
    assert!(checked_trees_to_lowered_psi::lower_machine(&original, "Root::enter").is_err());
}

#[test]
fn unit_wrapper_cannot_substitute_a_same_typed_local_and_its_cleanup() {
    let source = constructed_wrapper_source("", "").replace(
        "let receipt: Receipt =",
        "let spare: Receipt = Receipt {}; let receipt: Receipt =",
    );
    let mut original = checked(&source);
    unit_wrapper_artifact(&original);
    let root = original
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.trivial_affine_locals.len() == 2)
        .unwrap();
    let mut changed_call = false;
    let mut changed_cleanup = false;
    for operation in &mut root.operations {
        match operation {
            CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            } => {
                assert_eq!(
                    structural_arguments[0].source_local_declaration_ordinal(),
                    Some(1)
                );
                structural_arguments[0].source =
                    checked_trees::CheckedUnitStructuralArgumentSourcePlan::TrivialAffineLocal {
                        declaration_ordinal: 0,
                    };
                changed_call = true;
            }
            CheckedUnitEffectOperationPlan::Complete {
                trivial_affine_local_discard_ordinals,
                ..
            } => {
                assert_eq!(*trivial_affine_local_discard_ordinals, [0]);
                *trivial_affine_local_discard_ordinals = vec![1];
                changed_cleanup = true;
            }
            _ => {}
        }
    }
    assert!(changed_call && changed_cleanup);
    assert!(checked_trees_to_lowered_psi::lower_machine(&original, "Root::enter").is_err());
}

#[test]
fn unit_wrapper_forwards_shared_parameter_without_manufacturing_claims() {
    let source = unit_wrapper_source()
        .replace("Receipt [linear]", "Receipt")
        .replace("Receipt::settle(self,", "Receipt::settle(&self,")
        .replace("receipt: Receipt", "receipt: &Receipt");
    let artifact = unit_wrapper_artifact(&checked(&source));
    let module = decode_module(&artifact.0).unwrap();
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    assert!(root.entry_claims.is_empty());
    assert!(root.blocks.iter().flat_map(|block| &block.operations).any(|operation| {
        matches!(&operation.kind, terminal_psi::OperationKind::CallStructuralScalar { structural_arguments, claim_transfers, .. }
            if structural_arguments.len() == 1
                && structural_arguments[0].access == terminal_psi::StructuralAccess::SharedBorrow
                && claim_transfers.is_empty())
    }));
    // The wrapper receives the borrow, but this boundary's reference-self is
    // attachment metadata under the existing boundary signature convention.
    assert!(module.boundary_machines[0].structural_parameters.is_empty());
    let mut execution = start(&artifact);
    assert_eq!(execution.live_claim_frontier().count(), 0);
    let mut observer = ObserveSettlement {
        erased_reference_self: true,
        ..ObserveSettlement::default()
    };
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observer.calls, [vec![unsigned(70), unsigned(70)]]);
    assert_eq!(execution.live_claim_frontier().count(), 0);
    assert_eq!(execution.effects().len(), 1);
}

#[test]
fn unit_wrapper_consumes_established_affine_result_without_duplicate_cleanup() {
    let source = unit_wrapper_source()
        .replace("Receipt [linear]", "Receipt")
        .replace(
            "let accepted: u16 = Wrapper::measure(receipt, 70u16);",
            "let moved: Receipt = forward(receipt); let accepted: u16 = Wrapper::measure(moved, 70u16);",
        );
    let source = format!("{source}\nmachine forward(receipt: Receipt) -> Receipt {{ receipt }}");
    let original = checked(&source);
    let root_source = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Root::enter")
        .unwrap()
        .symbol;
    let plan = original
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(root_source)
        .expect("ordinary Unit sequence retains the structural producer and scalar consumer");
    assert!(matches!(
        &plan.operations[0],
        CheckedUnitEffectOperationPlan::StructuralCall {
            discard_result_on_return: false,
            result,
            ..
        } if result.binding_ordinal == 0
    ));
    let CheckedUnitEffectOperationPlan::ScalarCall {
        structural_arguments,
        claim_transfers,
        ..
    } = &plan.operations[1]
    else {
        panic!("the next initializer consumes the established affine result");
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(
        structural_arguments[0].source_structural_result_binding_ordinal(),
        Some(0)
    );
    assert!(claim_transfers.is_empty());

    let artifact = unit_wrapper_artifact(&original);
    let module = decode_module(&artifact.0).unwrap();
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let produced = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.result {
            terminal_psi::OperationResult::Structural(result) => Some(result.place),
            _ => None,
        })
        .expect("the ordinary identity call produces one structural place");
    assert_ne!(produced, root.structural_parameters[0].place);
    let arguments = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallStructuralScalar {
                structural_arguments,
                claim_transfers,
                ..
            } => {
                assert!(claim_transfers.is_empty());
                Some(structural_arguments)
            }
            _ => None,
        })
        .expect("the wrapper is called through its structural scalar signature");
    assert_eq!(arguments.len(), 1);
    assert_eq!(arguments[0].place, produced);
    assert!(arguments[0].path.is_empty());
    for block in &root.blocks {
        if let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &block.terminator
        {
            assert!(trivial_affine_discards.is_empty());
        }
    }
    let mut execution = start(&artifact);
    let mut observer = ObserveSettlement::default();
    assert_eq!(execution.live_claim_frontier().count(), 0);
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observer.calls, [vec![unsigned(70), unsigned(70)]]);
    assert_eq!(execution.live_claim_frontier().count(), 0);

    for mutation in 0..2 {
        let mut changed = module.clone();
        let root = changed
            .machines
            .iter_mut()
            .find(|machine| machine.id == changed.entry)
            .unwrap();
        if mutation == 0 {
            let place = root
                .structural_places
                .iter_mut()
                .find(|place| place.id == produced)
                .unwrap();
            let semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } =
                &mut place.kind
            else {
                unreachable!()
            };
            *producer = semantic_vocabulary::OperationId::new(u64::MAX).unwrap();
        } else {
            let terminator = root
                .blocks
                .iter_mut()
                .find_map(|block| {
                    if let Terminator::ReturnUnit {
                        trivial_affine_discards,
                        ..
                    } = &mut block.terminator
                    {
                        Some(trivial_affine_discards)
                    } else {
                        None
                    }
                })
                .unwrap();
            terminator.push(produced);
        }
        assert!(
            terminal_verifier::verify_module(
                &changed,
                &terminal_codec::decode_proof_bundle(&artifact.1).unwrap(),
                &AdmissionProfile::default(),
            )
            .is_err(),
            "independent affine result producer/cleanup mutation {mutation}"
        );
    }

    for mutation in 0..2 {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == root_source)
            .unwrap();
        if mutation == 0 {
            let CheckedUnitEffectOperationPlan::ScalarCall {
                structural_arguments,
                ..
            } = &mut plan.operations[1]
            else {
                unreachable!()
            };
            structural_arguments[0].source =
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: 0,
                };
        } else {
            let CheckedUnitEffectOperationPlan::StructuralCall {
                discard_result_on_return,
                ..
            } = &mut plan.operations[0]
            else {
                unreachable!()
            };
            *discard_result_on_return = true;
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Root::enter").is_err(),
            "established affine result custody mutation {mutation}"
        );
    }
}

#[test]
fn unit_wrapper_qualifications_and_range_proofs_survive_provider_rejection() {
    let source = unit_wrapper_source()
        .replace(
            "pub data Receipt [linear] { value: u64; }",
            "pub data Receipt [linear] { value: u64; }\ndomain Receipt::Ready;",
        )
        .replace("receipt: Receipt", "receipt: Receipt in Ready")
        .replace("value: u16)", "value: u16 [1..=100])");
    let artifact = unit_wrapper_artifact(&checked(&source));
    let module = decode_module(&artifact.0).unwrap();
    assert_eq!(module.structural_domains.len(), 1);
    let wrapper = module
        .machines
        .iter()
        .find(|machine| machine.id != module.entry)
        .unwrap();
    assert_eq!(wrapper.contract.requires.len(), 1);
    assert_eq!(wrapper.structural_parameters[0].qualifications.len(), 1);
    let mut execution = start(&artifact);
    let mut observer = ObserveSettlement {
        reject: true,
        ..ObserveSettlement::default()
    };
    assert!(matches!(
        execution.resume(&mut TerminalFuelMeter::unbounded(), &mut observer),
        Err(TerminalInterpretError::EffectRejected { .. })
    ));
    assert_eq!(execution.live_claim_frontier().count(), 1);
    assert!(execution.effects().is_empty());
    observer.reject = false;
    assert_eq!(
        execution
            .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(observer.receipts[0], observer.receipts[1]);
    assert_eq!(execution.live_claim_frontier().count(), 0);
    assert_eq!(execution.effects().len(), 1);
}

#[test]
fn unit_wrapper_rejects_missing_checked_and_terminal_claim_transfers() {
    let original = checked(&unit_wrapper_source());
    let artifact = unit_wrapper_artifact(&original);
    let mut checked = original.clone();
    let operation = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.operations)
        .find(|operation| matches!(operation, CheckedUnitEffectOperationPlan::ScalarCall { .. }))
        .unwrap();
    let CheckedUnitEffectOperationPlan::ScalarCall {
        claim_transfers, ..
    } = operation
    else {
        unreachable!()
    };
    assert_eq!(claim_transfers.len(), 1);
    claim_transfers.clear();
    assert!(checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").is_err());
    let mut module = decode_module(&artifact.0).unwrap();
    let operation = module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::CallStructuralScalar { .. }
            )
        })
        .unwrap();
    let terminal_psi::OperationKind::CallStructuralScalar {
        claim_transfers, ..
    } = &mut operation.kind
    else {
        unreachable!()
    };
    assert_eq!(claim_transfers.len(), 1);
    claim_transfers.clear();
    assert!(
        terminal_verifier::verify_module(
            &module,
            &terminal_codec::decode_proof_bundle(&artifact.1).unwrap(),
            &AdmissionProfile::default()
        )
        .is_err()
    );
}

#[test]
fn unit_wrapper_rejects_same_typed_structural_argument_substitution() {
    let source = unit_wrapper_source().replace(
        "Root::enter(receipt: Receipt) reaches PortIo { let accepted: u16 = Wrapper::measure(receipt, 70u16); }",
        "Root::enter(first: Receipt, second: Receipt) reaches PortIo { let accepted: u16 = Wrapper::measure(first, 70u16); let another: u16 = Wrapper::measure(second, 7u16); }",
    );
    let mut checked = checked(&source);
    unit_wrapper_artifact(&checked);
    let operation = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.operations)
        .find(|operation| matches!(operation, CheckedUnitEffectOperationPlan::ScalarCall { .. }))
        .unwrap();
    let CheckedUnitEffectOperationPlan::ScalarCall {
        structural_arguments,
        ..
    } = operation
    else {
        unreachable!()
    };
    assert_eq!(structural_arguments[0].source_parameter_index(), Some(0));
    structural_arguments[0].source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    assert!(checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter").is_err());
}

#[test]
fn unit_wrapper_operand_crash_retains_the_transferred_linear_claim() {
    let source = format!(
        "machine abort() -> u16 crashes Abort {{ crash Abort; }}\n{}",
        unit_wrapper_source()
            .replace(
                "receipt.settle(value, value)",
                "receipt.settle(abort(), value)"
            )
            .replace("reaches PortIo", "reaches PortIo\ncrashes Abort")
    );
    let artifact = unit_wrapper_artifact(&checked(&source));
    let mut execution = start(&artifact);
    let mut observer = ObserveSettlement::default();
    let claims = execution.live_claim_frontier().collect::<Vec<_>>();
    assert_eq!(claims.len(), 1);
    pause_before_crashing_helper(
        &artifact,
        &mut execution,
        &mut observer,
        &claims,
        terminal_psi::CrashCause::Abort,
    );
    let status = execution
        .resume(&mut TerminalFuelMeter::unbounded(), &mut observer)
        .unwrap();
    assert_unsettled_helper_crash(
        &artifact,
        &mut execution,
        &mut observer,
        &claims,
        terminal_psi::CrashCause::Abort,
        status,
    );
}

#[test]
fn returned_boundary_scalar_accepts_computed_argument_before_linear_settlement() {
    let source = format!(
        "machine identity(value: bool) -> bool {{ value }}\n{}",
        RESULT_BOUNDARY_CUSTODY_SOURCE
    )
    .replace(
        "Receipt::settle(self)",
        "Receipt::settle(self, value: bool)",
    )
    .replace("receipt.settle()", "receipt.settle(identity(true))");
    let checked = checked(&source);
    let artifact = artifact(&checked);
    let mut execution = start(&artifact);
    assert_eq!(execution.live_claim_frontier().count(), 1);
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::unbounded(),
                &mut ResultBoundaryHandler { reject: false }
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
    assert_eq!(execution.effects().len(), 1);
    assert_eq!(execution.live_claim_frontier().count(), 0);
}
