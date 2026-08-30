//! Ordered reconstruction of one terminal operation's facts and obligation.

use std::collections::BTreeMap;

use psi_core::{MachineId, Proposition, ScalarType, ValueId};
use psi_proof_admission::{Obligation, ObligationClass};
use psi_terminal::{Operation, OperationKind, TerminalMachine, TerminalModule};
use psi_terminal_semantics::{
    goal_free_scalar_leaf_equation, proof_bearing_scalar_leaf_semantics,
    structural_effect_leaf_observation,
};

use crate::ModuleError;

use super::super::call_composition::compose_call_operation;
use super::{ReconstructedOperationObligation, ReconstructedTerminalObligationOwner};

pub(super) fn append_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
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
        // Obligation identity is a total schema decision. Available facts may
        // influence the producer's explicit certificate, but cannot make
        // reconstruction search for a different sufficient proposition.
        let proposition = semantics
            .canonical_goal()
            .kernel_proposition()
            .map_err(ModuleError::OperationSemanticSchema)?;
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
            canonical_certificate: true,
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
        OperationKind::WriteOnlyPrimitiveStore { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::EstablishPayloadlessCase { .. }
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
