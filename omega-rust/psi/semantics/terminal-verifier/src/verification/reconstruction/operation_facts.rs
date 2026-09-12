//! Ordered reconstruction of one terminal operation's facts and obligation.

use std::collections::BTreeMap;

use proof_admission::{Obligation, ObligationClass};
use semantic_vocabulary::{MachineId, Proposition, ScalarType, ValueId};
use terminal_psi::{Operation, OperationKind, TerminalMachine, TerminalModule};
use terminal_semantics::{
    GoalFreeScalarLeafSemantics, goal_free_scalar_leaf_semantics,
    proof_bearing_scalar_leaf_semantics, structural_effect_leaf_observation,
};

use crate::ModuleError;

use super::super::call_composition::compose_call_operation;
use super::{ReconstructedOperationObligation, ReconstructedTerminalObligationOwner};

mod boolean_polarity;
mod byte_extent;
mod scalar_case;
mod scalar_record;

#[derive(Clone, Copy)]
pub(super) enum OperationFactPurpose {
    ProofObligations,
    PrivateCrashPredicates,
}

fn append_scalar_facts(
    semantics: &GoalFreeScalarLeafSemantics,
    value_types: &BTreeMap<ValueId, ScalarType>,
    purpose: OperationFactPurpose,
    axioms: &mut Vec<Proposition>,
) -> Result<(), ModuleError> {
    let implications = match purpose {
        OperationFactPurpose::ProofObligations => {
            boolean_polarity::implications(semantics, value_types)?
        }
        // Private crash checking already converts the original denotation
        // equation. Rebinding its redundant implication projections at every
        // path edge would multiply facts without adding crash-proof authority.
        OperationFactPurpose::PrivateCrashPredicates => Vec::new(),
    };
    axioms.push(semantics.result_equation().clone());
    axioms.extend(implications);
    Ok(())
}

pub(super) fn append_operation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    value_types: &BTreeMap<ValueId, ScalarType>,
    purpose: OperationFactPurpose,
    axioms: &mut Vec<Proposition>,
    operation_obligations: &mut Vec<ReconstructedOperationObligation>,
) -> Result<(), ModuleError> {
    if matches!(operation.kind, OperationKind::EstablishScalarRecord { .. }) {
        return scalar_record::append(module, machine, operation, axioms);
    }
    if matches!(operation.kind, OperationKind::EstablishScalarCase { .. }) {
        return scalar_case::append(module, machine, operation, axioms, operation_obligations);
    }
    if matches!(operation.kind, OperationKind::EstablishScalarArray { .. }) {
        crate::validation::scalar_array::shape(module, machine, operation)?;
        // Type and complete initialization are total validation judgments.
        // No scalar equality or extra proof authority is asserted here.
        return Ok(());
    }
    if let OperationKind::EstablishPrimitiveLocal { .. } = &operation.kind
        && let Some(result) = operation.result.structural()
    {
        axioms.retain(|proposition| {
            !crate::validation::proposition_observes_places(proposition, &[result.place])
        });
    }
    if let OperationKind::WriteOnlyPrimitiveStore { destination, .. }
    | OperationKind::StructuralScalarFieldStore { destination, .. }
    | OperationKind::StructuralByteSequenceFieldStore { destination, .. }
    | OperationKind::StructuralByteSequenceFieldByteStore { destination, .. } = &operation.kind
    {
        axioms.retain(|proposition| {
            !crate::validation::proposition_observes_places(proposition, &[*destination])
        });
    }
    if let Some(equation) = crate::validation::structural_byte_sequence_field_length_equation(
        module, machine, operation,
    )? {
        axioms.push(equation);
    }
    if let OperationKind::StructuralByteSequenceFieldStore {
        length, obligation, ..
    } = &operation.kind
    {
        let capacity =
            crate::validation::structural_byte_sequence_store_capacity(module, machine, operation)?;
        let integer_type =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid");
        let bound = semantic_vocabulary::ScalarTerm::integer(
            integer_type,
            semantic_vocabulary::IntegerValue::Unsigned(u128::from(capacity)),
        )
        .map_err(|error| {
            ModuleError::OperationSemanticSchema(
                terminal_semantics::OperationSemanticError::InvalidProposition(error),
            )
        })?;
        operation_obligations.push(ReconstructedOperationObligation {
            owner: ReconstructedTerminalObligationOwner::Operation {
                machine: machine.id,
                operation: operation.id,
            },
            obligation: Obligation {
                id: *obligation,
                proposition: Proposition::LessOrEqual(
                    semantic_vocabulary::ScalarTerm::value(
                        *length,
                        ScalarType::Integer(integer_type),
                    ),
                    bound,
                ),
                class: ObligationClass::Derivable,
            },
            semantic_axioms: axioms.clone(),
            canonical_certificate: true,
        });
        return Ok(());
    }
    if let Some(semantics) = goal_free_scalar_leaf_semantics(operation, value_types)
        .map_err(ModuleError::OperationSemanticSchema)?
    {
        return append_scalar_facts(&semantics, value_types, purpose, axioms);
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
        if let Some((id, proposition)) = observation.canonical_obligation() {
            operation_obligations.push(ReconstructedOperationObligation {
                owner: ReconstructedTerminalObligationOwner::Operation {
                    machine: machine.id,
                    operation: operation.id,
                },
                obligation: Obligation {
                    id,
                    proposition,
                    class: ObligationClass::Derivable,
                },
                semantic_axioms: axioms.clone(),
                canonical_certificate: true,
            });
        }
        if let Some(equation) = observation.local_equation() {
            axioms.push(equation.clone());
        }
        if let Some(equation) =
            byte_extent::length_equation(module, machine, operation, &observation)?
        {
            axioms.push(equation);
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
        OperationKind::IeeeFloatConstant { .. }
        | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. }
        | OperationKind::IeeeFloatCompare { .. }
        | OperationKind::StoreDynamicDescriptor { .. } => Ok(()),
        OperationKind::WriteOnlyPrimitiveStore { .. }
        | OperationKind::EstablishPrimitiveLocal { .. }
        | OperationKind::PrimitiveScalarRead { .. }
        | OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::StructuralByteSequenceFieldStore { .. }
        | OperationKind::StructuralByteSequenceFieldLength { .. }
        | OperationKind::StructuralByteSequenceFieldByteStore { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::ByteSequenceLength { .. }
        | OperationKind::ByteSequenceRead { .. }
        | OperationKind::ByteSequenceWrite { .. }
        | OperationKind::ByteSequenceSubslice { .. }
        | OperationKind::EstablishScalarCase { .. }
        | OperationKind::EstablishScalarArray { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. }
        | OperationKind::EstablishScalarRecord { .. }
        | OperationKind::PortWrite { .. }
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::StructuralCaseMembership { .. }
        | OperationKind::IntegerStructuralField { .. } => {
            unreachable!("structural/effect rows return before specialized reconstruction")
        }
        OperationKind::Call { .. }
        | OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterUnit { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. }
        | OperationKind::BoundaryCall { .. } => {
            unreachable!("call rows return before specialized reconstruction")
        }
    }
}
