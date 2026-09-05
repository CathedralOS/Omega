//! Ordered boundary-settlement source derived from installed-provider claim custody.

use super::installed_provider::installed_provider_legalization_fixture;
use super::structural_call::qualified_fixture_unit;
use abstract_operations::{AbstractOperation, AbstractOperationPlan};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{FuelScheduleIdentity, OperationId};
use target_operations::TargetOperationPlan;
use terminal_psi::{CompletionReceipt, StructuralMultiplicity};

pub(in crate::tests) fn claim_completion_settlement_fixture() -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut abstract_plan, mut target, _) = installed_provider_legalization_fixture();
    abstract_plan.provider_candidates.clear();
    abstract_plan.boundary_machines[0]
        .structural_parameters
        .truncate(1);
    abstract_plan.boundary_machines[0].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Linear;
    for parameter in &mut abstract_plan.functions[0].structural_parameters {
        parameter.multiplicity = StructuralMultiplicity::Linear;
    }
    let AbstractOperation::BoundaryCall {
        boundary,
        structural_arguments,
        completion_claim_sources,
        completion_receipts,
        ..
    } = abstract_plan.functions[0].operations[0].clone()
    else {
        panic!("boundary-call fixture");
    };
    let return_operation = abstract_plan.functions[0].operations[1].clone();
    let second_operation = OperationId::new(2).unwrap();
    abstract_plan.functions[0].operations = vec![
        AbstractOperation::BoundaryCall {
            psi_operation: OperationId::new(1).unwrap(),
            result: abstract_operations::AbstractBoundaryResult::Unit,
            boundary,
            arguments: Vec::new(),
            structural_arguments: vec![structural_arguments[0].clone()],
            completion_claim_sources: completion_claim_sources.clone(),
            completion_receipts: vec![completion_receipts[0]],
        },
        AbstractOperation::BoundaryCall {
            psi_operation: second_operation,
            result: abstract_operations::AbstractBoundaryResult::Unit,
            boundary,
            arguments: Vec::new(),
            structural_arguments: vec![structural_arguments[1].clone()],
            completion_claim_sources: completion_claim_sources.clone(),
            completion_receipts: vec![CompletionReceipt {
                claim: completion_receipts[1].claim,
                argument_index: 0,
            }],
        },
        return_operation,
    ];
    let target_operations::TargetOperation::UnitBody(body) = &mut target.functions[0].operation
    else {
        panic!("caller Unit body");
    };
    for parameter in &mut body.parameters {
        parameter.multiplicity = StructuralMultiplicity::Linear;
    }
    let return_operation = body.operations[1].clone();
    let settlement = |psi_operation, argument, sources, receipts, seed| {
        target_operations::TargetUnitOperation::BoundarySettlement {
            result: target_operations::TargetBoundaryResult::Unit,
            psi_operation,
            boundary,
            execution: target_operations::ProviderExecutionBinding::from_execution_record(
                target_operations::ProviderPlanReportIdentity::new(seed).unwrap(),
                seed + 1,
                seed + 2,
                seed + 3,
                seed + 4,
            )
            .unwrap()
            .into(),
            realization: target_operations::ClaimCompletionOnlyRealization.into(),
            scalar_arguments: Vec::new(),
            runtime_scalar_arguments: Vec::new(),
            arguments: vec![argument],
            byte_sequence_arguments: Vec::new(),
            completion_claim_sources: sources,
            completion_receipts: receipts,
        }
    };
    body.operations = vec![
        settlement(
            OperationId::new(1).unwrap(),
            structural_arguments[0].clone(),
            completion_claim_sources.clone(),
            vec![completion_receipts[0]],
            7,
        ),
        settlement(
            second_operation,
            structural_arguments[1].clone(),
            completion_claim_sources.clone(),
            vec![CompletionReceipt {
                claim: completion_receipts[1].claim,
                argument_index: 0,
            }],
            17,
        ),
        return_operation,
    ];
    target.functions[0].provenance.operations =
        vec![OperationId::new(1).unwrap(), second_operation];
    let unit = qualified_fixture_unit(
        optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstract_plan,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .expect("claim-completion settlement optimization seed"),
        abstract_plan.structural_types[0].id,
    );
    (abstract_plan, target, unit)
}
