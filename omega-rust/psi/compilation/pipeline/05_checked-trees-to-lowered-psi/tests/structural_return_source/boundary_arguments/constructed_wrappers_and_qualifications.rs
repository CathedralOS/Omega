use super::{
    ObserveSettlement, constructed_wrapper_source, start, unit_wrapper_artifact,
    unit_wrapper_source,
};
use crate::structural_return_source::{
    AdmissionProfile, TerminalExecutionResult, TerminalExecutionStatus, TerminalFuelMeter,
    TerminalInterpretError, decode_module,
};
use checked_trees::CheckedUnitEffectOperationPlan;
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

#[test]
fn unit_wrapper_constructor_source_and_permission_mutations_reject() {
    let original = crate::front_end::checked_program(&constructed_wrapper_source(
        "value: i64;",
        "value: 7i64",
    ));
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
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter")
            )
            .is_err(),
            "constructor source custody mutation {mutation}"
        );
    }
}

#[test]
fn unit_wrapper_cannot_substitute_a_same_typed_local_and_its_cleanup() {
    let source = constructed_wrapper_source("", "").replace(
        "let receipt: Receipt =",
        "let spare: Receipt = Receipt {}; let receipt: Receipt =",
    );
    let mut original = crate::front_end::checked_program(&source);
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
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &original,
            TerminalMachineSelection::Name("Root::enter")
        )
        .is_err()
    );
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
    let artifact = unit_wrapper_artifact(&crate::front_end::checked_program(&source));
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
    let original = crate::front_end::checked_program(&unit_wrapper_source());
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
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("Root::enter")
        )
        .is_err()
    );
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
