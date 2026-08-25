//! Ordered reconstruction of one terminal operation's facts and obligation.

use std::collections::{BTreeMap, BTreeSet};

use psi_core::{MachineId, Proposition, PropositionContext, ScalarType, ValueId};
use psi_proof_kernel::{Obligation, ObligationClass};
use psi_terminal::{Operation, OperationKind, TerminalMachine, TerminalModule};
use psi_terminal_semantics::{
    OperationSemanticError, OperationSemanticTag, goal_free_scalar_leaf_equation,
    proof_bearing_scalar_leaf_semantics, structural_effect_leaf_observation,
};

use crate::ModuleError;

use super::super::call_composition::compose_call_operation;
use super::super::sufficient_reduction::reduce_proof_bearing_scalar_goal;
use super::{
    ReconstructedOperationObligation, ReconstructedTerminalObligationOwner, certificate_entry,
};

pub(super) fn append_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    proposition_context: &PropositionContext,
    machine_parameter_values: &BTreeSet<ValueId>,
    axioms: &mut Vec<Proposition>,
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
) -> Result<(), ModuleError> {
    if let Some(equation) = goal_free_scalar_leaf_equation(operation, value_types)
        .map_err(ModuleError::OperationSemanticSchema)?
    {
        axioms.push(equation);
        return Ok(());
    }
    if let Some(semantics) = proof_bearing_scalar_leaf_semantics(operation, value_types)
        .map_err(ModuleError::OperationSemanticSchema)?
    {
        let literal_aware_exact_reduction = matches!(
            semantics.tag(),
            OperationSemanticTag::ExactIntegerDivide | OperationSemanticTag::ExactIntegerRemainder
        )
        .then(|| {
            reduce_proof_bearing_scalar_goal(
                &semantics,
                axioms,
                &machine.contract.requires,
                machine_parameter_values,
            )
        });
        // Exact divide/remainder retain their trusted literal-aware reduction
        // when it already closes the site. Richer nontrivial exact obligations
        // may still select an available canonical certificate below.
        let exact_reduction_closes = literal_aware_exact_reduction
            .as_ref()
            .is_some_and(|proposition| proposition == &Proposition::Truth);
        let canonical_certificate = matches!(
            semantics.tag(),
            OperationSemanticTag::WrappingIntegerDivide
                | OperationSemanticTag::WrappingIntegerRemainder
                | OperationSemanticTag::SaturatingIntegerDivide
                | OperationSemanticTag::SaturatingIntegerRemainder
        ) || (!exact_reduction_closes
            && certificate_entry::retained(
                Some(proposition_context),
                semantics.canonical_goal(),
                axioms,
                &machine.contract.requires,
            ));
        let proposition = if canonical_certificate {
            semantics
                .canonical_goal()
                .kernel_proposition()
                .map_err(ModuleError::OperationSemanticSchema)?
                .ok_or(ModuleError::OperationSemanticSchema(
                    OperationSemanticError::ProofBearingScalarSchemaMismatch(semantics.tag()),
                ))?
        } else {
            literal_aware_exact_reduction.unwrap_or_else(|| {
                reduce_proof_bearing_scalar_goal(
                    &semantics,
                    axioms,
                    &machine.contract.requires,
                    machine_parameter_values,
                )
            })
        };
        operation_obligations.push(ReconstructedOperationObligation {
            owner: ReconstructedTerminalObligationOwner::Operation {
                machine: machine.id,
                operation: operation.id,
            },
            obligation: Obligation {
                id: semantics.obligation(),
                proposition,
                class: ObligationClass::Derivable,
            },
            semantic_axioms: axioms.clone(),
            canonical_certificate,
        });
        axioms.push(semantics.result_equation().clone());
        return Ok(());
    }
    if let Some(observation) = structural_effect_leaf_observation(operation)
        .map_err(ModuleError::OperationSemanticSchema)?
    {
        if let Some(equation) = observation.local_equation() {
            axioms.push(equation.clone());
        }
        return Ok(());
    }
    if compose_call_operation(
        module,
        machine,
        operation,
        machines,
        value_types,
        axioms,
        operation_obligations,
    )? {
        for composition in machine
            .content_partition_compositions
            .iter()
            .filter(|composition| composition.producer_operation == operation.id)
        {
            let proposition = composition.inferred_proposition();
            if !axioms.contains(&proposition) {
                axioms.push(proposition);
            }
        }
        return Ok(());
    }
    match operation.kind.clone() {
        OperationKind::IntegerExactCast { .. }
        | OperationKind::ExactIntegerShiftLeft { .. }
        | OperationKind::ExactIntegerShiftRight { .. }
        | OperationKind::ExactIntegerAdd { .. }
        | OperationKind::ExactIntegerSubtract { .. }
        | OperationKind::ExactIntegerMultiply { .. }
        | OperationKind::ExactIntegerDivide { .. }
        | OperationKind::ExactIntegerRemainder { .. }
        | OperationKind::WrappingIntegerDivide { .. }
        | OperationKind::WrappingIntegerRemainder { .. }
        | OperationKind::SaturatingIntegerDivide { .. }
        | OperationKind::SaturatingIntegerRemainder { .. } => {
            unreachable!("proof-bearing scalar rows return before legacy reduction dispatch")
        }
        OperationKind::IntegerConstant { .. }
        | OperationKind::BooleanConstant { .. }
        | OperationKind::BooleanNot { .. }
        | OperationKind::BooleanEqual { .. }
        | OperationKind::IntegerEqual { .. }
        | OperationKind::IntegerLessThan { .. }
        | OperationKind::IntegerLessOrEqual { .. }
        | OperationKind::IntegerBitwiseNot { .. }
        | OperationKind::IntegerWiden { .. }
        | OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. }
        | OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. }
        | OperationKind::WrappingIntegerAdd { .. }
        | OperationKind::SaturatingIntegerAdd { .. }
        | OperationKind::WrappingIntegerSubtract { .. }
        | OperationKind::SaturatingIntegerSubtract { .. }
        | OperationKind::WrappingIntegerMultiply { .. }
        | OperationKind::SaturatingIntegerMultiply { .. } => {
            unreachable!("goal-free scalar rows return before specialized reconstruction")
        }
        OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. }
        | OperationKind::PortWrite { .. }
        | OperationKind::BooleanStructuralField { .. } => {
            unreachable!("structural/effect rows return before specialized reconstruction")
        }
        OperationKind::Call { .. }
        | OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::BoundaryCall { .. } => {
            unreachable!("call rows return before specialized reconstruction")
        }
    }
}
