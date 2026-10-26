//! Exhaustive Terminal operation-kind dispatch to exact lowering families.

use std::collections::BTreeMap;

use abstract_operations::AbstractOperation;
use terminal_psi::{OperationKind, TerminalMachine};

use super::{
    LoweredAffineLocal, ScalarType, StructuralLiteral, arithmetic, atomic_events, boolean,
    borrowed_windows, calls, effects, ieee_float, integer_bitwise, integer_constants_and_relations,
    integer_conversion, shifts, structural_establishment, structural_leaf_copy,
    structural_scalar_fields, trapping_integer,
};
use crate::lowering::LoweringError;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &terminal_psi::Operation,
    block: &terminal_psi::Block,
    machine: &TerminalMachine,
    module: &terminal_psi::TerminalModule,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    placed_view_plans: &[terminal_psi::TerminalPlacedViewPlan],
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
    byte_sequence_literals: &[StructuralLiteral<'_>],
    unit_affine_locals: &[StructuralLiteral<'_>],
    lowered_unit_affine_locals: &mut Vec<LoweredAffineLocal>,
) -> Result<AbstractOperation, LoweringError> {
    match &operation.kind {
        OperationKind::EstablishActivationClaim {
            bound,
            committed,
            backing_type,
            established_case,
            rejected_case,
        } => {
            terminal_semantics::activation_storage::activation_claim_shape(module, operation)
                .map_err(|_| LoweringError::InvalidActivationStorage(operation.id))?;
            let result = operation
                .result
                .structural()
                .cloned()
                .ok_or(LoweringError::InvalidActivationStorage(operation.id))?;
            Ok(AbstractOperation::EstablishActivationClaim {
                psi_operation: operation.id,
                result,
                bound: *bound,
                committed: *committed,
                backing_type: *backing_type,
                established_case: *established_case,
                rejected_case: *rejected_case,
            })
        }
        OperationKind::ReleaseActivationClaim { source, claim } => {
            Ok(AbstractOperation::ReleaseActivationClaim {
                psi_operation: operation.id,
                source: source.clone(),
                claim: *claim,
            })
        }
        OperationKind::SplitActivationClaim {
            source,
            claim,
            bound_at,
            committed_at,
            covered_case,
            rejected_case,
        } => {
            let result = operation
                .result
                .structural()
                .cloned()
                .ok_or(LoweringError::InvalidActivationStorage(operation.id))?;
            Ok(AbstractOperation::SplitActivationClaim {
                psi_operation: operation.id,
                result,
                source: source.clone(),
                claim: *claim,
                bound_at: *bound_at,
                committed_at: *committed_at,
                covered_case: *covered_case,
                rejected_case: *rejected_case,
            })
        }
        OperationKind::MergeActivationClaim {
            lower,
            upper,
            lower_claim,
            upper_claim,
            covered_case,
            rejected_case,
        } => {
            let result = operation
                .result
                .structural()
                .cloned()
                .ok_or(LoweringError::InvalidActivationStorage(operation.id))?;
            Ok(AbstractOperation::MergeActivationClaim {
                psi_operation: operation.id,
                result,
                lower: lower.clone(),
                upper: upper.clone(),
                lower_claim: *lower_claim,
                upper_claim: *upper_claim,
                covered_case: *covered_case,
                rejected_case: *rejected_case,
            })
        }
        OperationKind::EstablishPlacedReference { source, resident } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
            };
            let Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) =
                structural_types
                    .iter()
                    .find(|row| row.id == result.structural_type)
                    .map(|row| &row.shape)
            else {
                return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
            };
            let mut selected = placed_view_plans
                .iter()
                .filter(|plan| plan.carrier == *referent);
            let (Some(plan), None) = (selected.next(), selected.next()) else {
                return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
            };
            let correspondence = if let Some(roles) = resident {
                let root = super::primitive_storage::parameter(machine, source.place)
                    .map(|parameter| parameter.structural_type)
                    .or_else(|| {
                        machine
                            .structural_place(source.place)?
                            .kind
                            .produced_structural_type()
                    })
                    .ok_or(LoweringError::UnsupportedReferenceCustody(operation.id))?;
                let owner = terminal_semantics::runtime_structural_path_tip(
                    structural_types.iter(),
                    root,
                    &source.path,
                )
                .ok_or(LoweringError::UnsupportedReferenceCustody(operation.id))?;
                abstract_operations::PlacedReferenceBacking::Resident(
                    abstract_operations::ResidentReferenceBacking {
                        owner: owner.id,
                        schema: plan.schema,
                        carrier: plan.carrier,
                        roles: *roles,
                        placement_commitment: plan.placement.content_interpretation().commitment(),
                    },
                )
            } else {
                abstract_operations::PlacedReferenceBacking::OrdinaryRam(
                    terminal_semantics::placed_ram_plan_correspondence(
                        structural_types,
                        placed_view_plans,
                        plan,
                        plan.schema,
                    )
                    .map_err(|_| LoweringError::UnsupportedReferenceCustody(operation.id))?,
                )
            };
            Ok(AbstractOperation::EstablishPlacedReference {
                psi_operation: operation.id,
                result,
                source: source.clone(),
                correspondence,
            })
        }
        OperationKind::SelectImplementation { .. } | OperationKind::CallDynamic { .. } => {
            calls::lower(operation, machine)
        }
        OperationKind::StorageAddress { source } => {
            let result = operation.result.scalar()
                .filter(|result| matches!(result.scalar_type, ScalarType::Integer(integer) if integer.is_address()))
                .ok_or(LoweringError::UnsupportedReferenceCustody(operation.id))?;
            if source.access != terminal_psi::StructuralAccess::SharedBorrow {
                return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
            }
            let root = super::primitive_storage::parameter(machine, source.place)
                .map(|parameter| parameter.structural_type)
                .or_else(|| {
                    machine
                        .structural_place(source.place)?
                        .kind
                        .produced_structural_type()
                })
                .ok_or(LoweringError::UnsupportedReferenceCustody(operation.id))?;
            // Address exposure observes the exact referent, not its contents.
            // Inline scalar fields and structural referents have distinct leaf
            // declarations, but both retain the same checked source path.
            if terminal_semantics::primitive_read_projection_type(
                structural_types.iter(),
                root,
                &source.path,
            )
            .is_none()
                && terminal_semantics::runtime_structural_path_tip(
                    structural_types.iter(),
                    root,
                    &source.path,
                )
                .is_none()
            {
                return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
            }
            Ok(AbstractOperation::StorageAddress {
                psi_operation: operation.id,
                result: result.into(),
                source: source.place,
                path: source.path.clone(),
            })
        }
        OperationKind::EstablishReference { source, storage } => {
            let Some(result) = operation.result.structural().cloned() else {
                return Err(LoweringError::UnsupportedReferenceCustody(operation.id));
            };
            Ok(AbstractOperation::EstablishReference {
                psi_operation: operation.id,
                result,
                source: source.clone(),
                storage: *storage,
            })
        }
        OperationKind::ReleaseReference { source, path } => {
            Ok(AbstractOperation::ReleaseReference {
                psi_operation: operation.id,
                source: *source,
                path: path.clone(),
            })
        }
        OperationKind::StructuralCaseMembership { source, path, case } => {
            let result = operation
                .result
                .scalar()
                .filter(|result| result.scalar_type == ScalarType::Boolean)
                .ok_or(LoweringError::InvalidStructuralCaseMembership(operation.id))?;
            Ok(AbstractOperation::StructuralCaseMembership {
                psi_operation: operation.id,
                result: result.into(),
                source: *source,
                path: path.clone(),
                case: *case,
            })
        }
        OperationKind::EstablishPrimitiveLocal { .. }
        | OperationKind::PrimitiveScalarRead { .. } => {
            super::primitive_storage::lower(operation, machine, structural_types, value_types)
        }
        OperationKind::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
        } => {
            let result = operation
                .result
                .scalar()
                .ok_or(LoweringError::InvalidByteSequenceLength(operation.id))?;
            if !matches!(result.scalar_type, ScalarType::Integer(integer) if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64))
            {
                return Err(LoweringError::InvalidByteSequenceLength(operation.id));
            }
            Ok(AbstractOperation::StructuralByteSequenceFieldLength {
                psi_operation: operation.id,
                result: result.into(),
                source: *source,
                path: path.clone(),
                field: *field,
            })
        }
        OperationKind::StructuralByteSequenceFieldRead {
            source,
            path,
            field,
            index,
            length,
            obligation,
        } => {
            let result = operation.result.scalar().ok_or(
                LoweringError::UnsupportedStructuralByteSequenceFieldRead(operation.id),
            )?;
            if !matches!(result.scalar_type, ScalarType::Integer(integer)
                if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8))
            {
                return Err(LoweringError::UnsupportedStructuralByteSequenceFieldRead(
                    operation.id,
                ));
            }
            Ok(AbstractOperation::StructuralByteSequenceFieldRead {
                psi_operation: operation.id,
                result: result.into(),
                source: *source,
                path: path.clone(),
                field: *field,
                index: *index,
                length: *length,
                obligation: *obligation,
            })
        }
        OperationKind::ByteSequencePredicate { operand, predicate } => {
            let result = operation
                .result
                .scalar()
                .filter(|result| result.scalar_type == ScalarType::Boolean)
                .ok_or(LoweringError::InvalidByteSequencePredicate(operation.id))?;
            Ok(AbstractOperation::ByteSequencePredicate {
                psi_operation: operation.id,
                result: result.into(),
                operand: operand.clone(),
                predicate: *predicate,
            })
        }
        OperationKind::ByteSequenceEqual { left, right } => {
            let result = operation
                .result
                .scalar()
                .filter(|result| result.scalar_type == ScalarType::Boolean)
                .ok_or(LoweringError::InvalidByteSequenceEqual(operation.id))?;
            Ok(AbstractOperation::ByteSequenceEqual {
                psi_operation: operation.id,
                result: result.into(),
                left: left.clone(),
                right: right.clone(),
            })
        }
        OperationKind::StructuralByteSequenceFieldByteStore {
            destination,
            path,
            field,
            index,
            value,
            length,
            obligation,
        } => {
            if operation.result != terminal_psi::OperationResult::Unit {
                return Err(LoweringError::InvalidStructuralByteSequenceFieldStore(
                    operation.id,
                ));
            }
            Ok(AbstractOperation::StructuralByteSequenceFieldByteStore {
                psi_operation: operation.id,
                destination: *destination,
                path: path.clone(),
                field: *field,
                index: *index,
                value: *value,
                length: *length,
                obligation: *obligation,
            })
        }
        OperationKind::ByteSequenceWrite {
            destination,
            index,
            value,
            length,
            obligation,
        } => {
            if operation.result != terminal_psi::OperationResult::Unit {
                return Err(LoweringError::InvalidByteSequenceWrite(operation.id));
            }
            Ok(AbstractOperation::ByteSequenceWrite {
                psi_operation: operation.id,
                destination: *destination,
                index: *index,
                value: *value,
                length: *length,
                obligation: *obligation,
            })
        }
        OperationKind::ByteSequenceStore {
            destination,
            source,
            length,
            obligation,
        } => {
            if operation.result != terminal_psi::OperationResult::Unit {
                return Err(LoweringError::InvalidStructuralByteSequenceFieldStore(
                    operation.id,
                ));
            }
            Ok(AbstractOperation::ByteSequenceStore {
                psi_operation: operation.id,
                destination: *destination,
                source: *source,
                length: *length,
                obligation: *obligation,
            })
        }
        OperationKind::StructuralByteViewFieldStore {
            destination,
            path,
            field,
            source,
            predicate_obligation,
        } => {
            if operation.result != terminal_psi::OperationResult::Unit {
                return Err(LoweringError::InvalidStructuralByteViewFieldStore(
                    operation.id,
                ));
            }
            Ok(AbstractOperation::StructuralByteViewFieldStore {
                psi_operation: operation.id,
                destination: *destination,
                path: path.clone(),
                field: *field,
                source: *source,
                predicate_obligation: *predicate_obligation,
            })
        }
        OperationKind::StructuralByteSequenceFieldStore {
            destination,
            path,
            field,
            source,
            length,
            obligation,
        } => {
            if operation.result != terminal_psi::OperationResult::Unit {
                return Err(LoweringError::InvalidStructuralByteSequenceFieldStore(
                    operation.id,
                ));
            }
            Ok(AbstractOperation::StructuralByteSequenceFieldStore {
                psi_operation: operation.id,
                destination: *destination,
                path: path.clone(),
                field: *field,
                source: *source,
                length: *length,
                obligation: *obligation,
            })
        }
        OperationKind::ByteSequenceRead {
            source,
            index,
            length,
            obligation,
        } => {
            let result = operation
                .result
                .scalar()
                .ok_or(LoweringError::InvalidByteSequenceRead(operation.id))?;
            if !matches!(result.scalar_type, ScalarType::Integer(integer)
                if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8))
            {
                return Err(LoweringError::InvalidByteSequenceRead(operation.id));
            }
            Ok(AbstractOperation::ByteSequenceRead {
                psi_operation: operation.id,
                result: result.into(),
                source: *source,
                index: *index,
                length: *length,
                obligation: *obligation,
            })
        }
        OperationKind::ByteSequenceSubslice {
            source,
            start,
            end,
            length,
            obligation,
        } => {
            let result = operation
                .result
                .structural()
                .ok_or(LoweringError::InvalidByteSequenceSubslice(operation.id))?;
            Ok(AbstractOperation::ByteSequenceSubslice {
                psi_operation: operation.id,
                result: result.clone(),
                source: source.clone(),
                start: *start,
                end: *end,
                length: *length,
                obligation: *obligation,
            })
        }
        OperationKind::ByteSequenceLength { source } => {
            let result = operation
                .result
                .scalar()
                .ok_or(LoweringError::InvalidByteSequenceLength(operation.id))?;
            if !matches!(result.scalar_type, ScalarType::Integer(integer) if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64))
            {
                return Err(LoweringError::InvalidByteSequenceLength(operation.id));
            }
            Ok(AbstractOperation::ByteSequenceLength {
                psi_operation: operation.id,
                result: result.into(),
                source: source.clone(),
            })
        }
        terminal_psi::element_view_establish_pair_pattern!(
            destination,
            source,
            element,
            region,
        ) => {
            // One descriptor derivation for both spellings; the argument's
            // `MutableBorrow` access keeps the exclusive-loan distinction
            // consumers must see.
            let result = operation
                .result
                .structural()
                .cloned()
                .ok_or(LoweringError::InvalidElementViewEstablishment(operation.id))?;
            if result.place != *destination {
                return Err(LoweringError::InvalidElementViewEstablishment(operation.id));
            }
            Ok(AbstractOperation::EstablishElementView {
                psi_operation: operation.id,
                result,
                destination: *destination,
                source: source.clone(),
                element: *element,
                region: *region,
            })
        }
        OperationKind::ElementViewLength { source } => {
            let result = operation
                .result
                .scalar()
                .ok_or(LoweringError::InvalidElementViewLength(operation.id))?;
            if !matches!(result.scalar_type, ScalarType::Integer(integer) if Ok(integer) == semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64))
            {
                return Err(LoweringError::InvalidElementViewLength(operation.id));
            }
            Ok(AbstractOperation::ElementViewLength {
                psi_operation: operation.id,
                result: result.into(),
                source: source.clone(),
            })
        }
        OperationKind::ElementViewRead {
            source,
            index,
            length,
            obligation,
            path,
        } => {
            let result = operation
                .result
                .scalar()
                .ok_or(LoweringError::InvalidElementViewRead(operation.id))?;
            Ok(AbstractOperation::ElementViewRead {
                psi_operation: operation.id,
                result: result.into(),
                source: *source,
                index: *index,
                length: *length,
                obligation: *obligation,
                path: path.clone(),
            })
        }
        OperationKind::ElementViewWrite {
            path,
            destination,
            index,
            value,
            length,
            obligation,
        } => Ok(AbstractOperation::ElementViewWrite {
            path: path.clone(),
            psi_operation: operation.id,
            destination: destination.clone(),
            index: *index,
            value: *value,
            length: *length,
            obligation: *obligation,
        }),
        OperationKind::ElementViewSubslice {
            source,
            start,
            end,
            length,
            obligation,
        } => {
            let result = operation
                .result
                .structural()
                .ok_or(LoweringError::InvalidElementViewSubslice(operation.id))?;
            Ok(AbstractOperation::ElementViewSubslice {
                psi_operation: operation.id,
                result: result.clone(),
                source: source.clone(),
                start: *start,
                end: *end,
                length: *length,
                obligation: *obligation,
            })
        }
        OperationKind::EstablishScalarArray { .. }
        | OperationKind::EstablishStructuralArray { .. }
        | OperationKind::EstablishScalarCase { .. }
        | OperationKind::EstablishStructuralCase { .. }
        | OperationKind::EstablishNativeCodeLayout { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::EstablishByteSequenceConcatenation { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. }
        | OperationKind::EstablishZeroedStructural { .. }
        | OperationKind::EstablishStructuralLocal { .. }
        | OperationKind::EstablishRecord { .. } => structural_establishment::lower(
            operation,
            block,
            machine,
            structural_types,
            byte_sequence_literals,
            unit_affine_locals,
            lowered_unit_affine_locals,
        ),
        OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | terminal_psi::call_structural_pair_pattern!(..)
        | OperationKind::BoundaryCall { .. }
        | OperationKind::Call { .. } => calls::lower(operation, machine),
        OperationKind::PortWrite { .. }
        | OperationKind::RegisterAccess { .. }
        | OperationKind::WriteOnlyPrimitiveStore { .. } => {
            effects::lower(operation, machine, structural_types, value_types)
        }
        OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::ScalarStructuralField { .. } => structural_scalar_fields::lower(
            operation,
            machine,
            structural_types,
            module,
            value_types,
        ),
        terminal_psi::move_or_store_structural_field_pattern!() => {
            borrowed_windows::lower(operation, machine, structural_types)
        }
        OperationKind::TransformContent {
            source,
            parents,
            derivations,
        } => {
            let result =
                operation
                    .result
                    .structural()
                    .ok_or(LoweringError::InvalidOwnedValueTransfer {
                        machine: machine.id,
                        operation: operation.id,
                        reason: "content transformation has no structural result",
                    })?;
            Ok(AbstractOperation::TransformContent {
                psi_operation: operation.id,
                result: result.clone(),
                source: source.clone(),
                parents: parents.clone(),
                derivations: derivations.clone(),
            })
        }
        OperationKind::MoveStructuralValue { source } => {
            let result =
                operation
                    .result
                    .structural()
                    .ok_or(LoweringError::InvalidOwnedValueTransfer {
                        machine: machine.id,
                        operation: operation.id,
                        reason: "owned transfer has no structural result",
                    })?;
            // Psi has checked the occurrence's qualification. Keep the transfer
            // explicit: native ownership consumes the source, and realization
            // transports its bytes without constructing a second owner.
            Ok(AbstractOperation::MoveStructuralValue {
                psi_operation: operation.id,
                result: result.clone(),
                source: source.clone(),
            })
        }
        OperationKind::StructuralLeafCopy { source, path } => structural_leaf_copy::lower(
            operation,
            machine,
            module,
            structural_types,
            *source,
            abstract_operations::StructuralCopyPath::Ordinary(path.clone()),
        ),
        OperationKind::StructuralCaseLeafCopy { source, path } => structural_leaf_copy::lower(
            operation,
            machine,
            module,
            structural_types,
            *source,
            abstract_operations::StructuralCopyPath::Case(path.clone()),
        ),
        OperationKind::IntegerConstant { .. } => integer_constants_and_relations::lower(operation),
        OperationKind::IeeeFloatCompare { .. }
        | OperationKind::FloatResultPolicy { .. }
        | OperationKind::IeeeFloatConstant { .. }
        | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. } => {
            ieee_float::lower(operation, value_types)
        }
        OperationKind::BooleanConstant { .. }
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::BooleanNot { .. }
        | OperationKind::BooleanEqual { .. } => boolean::lower(operation),
        OperationKind::IntegerEqual { .. }
        | terminal_psi::integer_less_or_equal_pair_pattern!(..) => {
            integer_constants_and_relations::lower(operation)
        }
        OperationKind::IntegerBitwiseNot { .. } => integer_bitwise::lower(operation),
        OperationKind::IntegerWiden { .. }
        | OperationKind::IntegerExactCast { .. }
        | OperationKind::ScalarRepresentationView { .. } => {
            integer_conversion::lower(operation, value_types)
        }
        OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. } => integer_bitwise::lower(operation),
        OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. }
        | OperationKind::ExactIntegerShiftLeft { .. }
        | OperationKind::ExactIntegerShiftRight { .. } => shifts::lower(operation, value_types),
        terminal_psi::integer_arithmetic_operations!() => arithmetic::lower(operation),
        OperationKind::TrappingInteger { .. } => trapping_integer::lower(operation, value_types),
        OperationKind::AtomicFence { ordering } => {
            Ok(abstract_operations::AbstractOperation::AtomicEvent {
                psi_operation: operation.id,
                event: abstract_operations::AbstractAtomicEvent::Fence {
                    ordering: *ordering,
                },
                resident_obligation: None,
                reads_from: None,
                modification_after: None,
            })
        }
        OperationKind::AtomicAccess { .. } => {
            atomic_events::lower(operation, machine, structural_types)
        }
    }
}
