//! Assemble admitted structural metadata and operations into the ordinary graph.
use super::super::matchers::MatchedStructuralUnitForm;
use super::super::shared::*;
pub(super) fn assemble(
    target: &target_operations::TargetFunction,
    abstracted: &abstract_operations::AbstractFunction,
    optimized: &optimization_unit::PsiOptimizationFunction,
    matched: &MatchedStructuralUnitForm<'_>,
    parameters: Vec<legalized_operations::LegalizedCallUnitParameter>,
    operations: super::operations::DerivedStructuralOperations,
) -> legalized_operations::LegalizedScalarFunction {
    let TargetOperation::UnitBody(body) = &target.operation else {
        unreachable!()
    };
    let [optimized_block] = optimized.blocks.as_slice() else {
        unreachable!()
    };
    let TargetUnitOperation::Return { psi_edge, .. } = matched.target_return else {
        unreachable!()
    };

    let mut instructions = operations
        .boundary_settlements
        .into_iter()
        .map(
            |settlement| legalized_operations::LegalizedScalarInstruction {
                operation: settlement.operation,
                result: None,
                fuel: settlement.fuel.clone(),
                effect: settlement.effect,
                ownership: settlement.ownership.clone(),
                kind: legalized_operations::LegalizedScalarInstructionKind::BoundarySettlement(
                    settlement,
                ),
            },
        )
        .collect::<Vec<_>>();
    if let Some(call) = operations.call {
        instructions.push(call);
    }
    legalized_operations::LegalizedScalarFunction {
        machine: target.machine,
        attachment: target.attachment,
        provenance: target.provenance.clone(),
        call_plan: body.call_plan.clone(),
        parameters: Vec::new(),
        structural: Some(legalized_operations::LegalizedStructuralContract {
            result: None,
            structural_types: body.structural_types.clone(),
            parameters,
            structural_places: synthesized_parameter_places(&abstracted.structural_parameters),
            entry_claims: abstracted.entry_claims.clone(),
            published_service_ceiling: abstracted.published_service_ceiling.clone(),
        }),
        ranked: None,
        entry_block: optimized_block.id,
        blocks: vec![legalized_operations::LegalizedScalarBlock {
            id: optimized_block.id,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            instructions,
            terminator: legalized_operations::LegalizedScalarTerminator::Return(
                legalized_operations::LegalizedScalarReturn {
                    edge: *psi_edge,
                    value: legalized_operations::LegalizedScalarReturnValue::Unit,
                    fuel: matched.optimized_return.fuel.clone(),
                    effect: matched.optimized_return.effect,
                    ownership: matched.optimized_return.ownership.clone(),
                },
            ),
        }],
    }
}

fn synthesized_parameter_places(
    parameters: &[terminal_psi::StructuralParameterDeclaration],
) -> Vec<StructuralPlaceDeclaration> {
    parameters
        .iter()
        .map(|parameter| StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            },
        })
        .collect()
}
