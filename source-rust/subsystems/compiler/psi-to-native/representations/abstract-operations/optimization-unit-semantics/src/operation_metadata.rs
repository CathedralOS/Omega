//! Reconstructs operation bookkeeping independently of the optimization-unit constructor.
//! The producer cannot depend on this receiving crate. Explicit no-result,
//! no-input and no-ownership cases make new instruction kinds a compile error
//! until both producer and receiver classify them. These checks establish
//! metadata consistency, not an independent proof of the instruction semantics.
use abstract_operations::{AbstractOperation, AbstractStructuralCallResult};
use optimization_unit::{ValueDefinition, ValueDefinitionSite};
use semantic_vocabulary::{BlockId, ScalarType, ValueId};

fn scalar_definition(operation: &AbstractOperation) -> Option<(ValueId, ScalarType)> {
    use AbstractOperation as O;
    match operation {
        O::RegisterAccess { result, .. } => result.map(|result| (result.value, result.scalar_type)),
        O::ActivateTaskRegister { result, .. }
        | O::ReadTaskRegister { result, .. }
        | O::ReloadSegmentDescriptors { result, .. }
        | O::ReadSegmentSelector { result, .. } => Some((result.value, result.scalar_type)),
        O::Call {
            result,
            scalar_type,
            ..
        }
        | O::IntegerConstant {
            result,
            scalar_type,
            ..
        } => Some((*result, *scalar_type)),
        O::IeeeFloatConstant { result, value, .. } => {
            Some((*result, ScalarType::IeeeFloat(value.format())))
        }
        O::NearestIeeeFloatFusedMultiplyAdd { result, format, .. }
        | O::FloatResultPolicy { result, format, .. } => {
            Some((*result, ScalarType::IeeeFloat(*format)))
        }
        O::CallStructural {
            result: AbstractStructuralCallResult::Scalar(result),
            ..
        }
        | O::SelectImplementation { result, .. }
        | O::SelectMachine { result, .. }
        | O::ObserveStorageLocation { result, .. }
        | O::PrimitiveScalarRead { result, .. }
        | O::StructuralCaseMembership { result, .. }
        | O::ByteSequenceRead { result, .. }
        | O::ByteSequencePredicate { result, .. }
        | O::ByteSequenceEqual { result, .. }
        | O::ByteSequenceLength { result, .. }
        | O::ElementViewLength { result, .. }
        | O::ElementViewRead { result, .. }
        | O::StructuralByteSequenceFieldLength { result, .. }
        | O::StructuralByteSequenceFieldRead { result, .. }
        | O::StructuralScalarFieldRead { result, .. } => Some((result.value, result.scalar_type)),
        O::BoundaryCall {
            result: abstract_operations::AbstractBoundaryResult::Scalar(result),
            ..
        } => Some((result.value, result.scalar_type)),
        O::BooleanConstant { result, .. }
        | O::BooleanNot { result, .. }
        | O::BooleanEqual { result, .. }
        | O::IeeeFloatCompare { result, .. }
        | O::IntegerEqual { result, .. }
        | O::IntegerLessThan { result, .. }
        | O::IntegerLessOrEqual { result, .. } => Some((*result, ScalarType::Boolean)),
        O::IntegerBitwiseNot {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseAnd {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseOr {
            result,
            scalar_type,
            ..
        }
        | O::IntegerBitwiseXor {
            result,
            scalar_type,
            ..
        }
        | O::IntegerArithmetic {
            result,
            scalar_type,
            ..
        }
        | O::TrappingInteger {
            result,
            scalar_type,
            ..
        } => Some((*result, ScalarType::Integer(*scalar_type))),
        O::IntegerWiden {
            result,
            target_type,
            ..
        }
        | O::IntegerExactCast {
            result,
            target_type,
            ..
        } => Some((*result, ScalarType::Integer(*target_type))),
        O::IntegerShift {
            result, value_type, ..
        } => Some((*result, ScalarType::Integer(*value_type))),
        O::ScalarRepresentationView {
            result,
            target_type,
            ..
        } => Some((*result, *target_type)),
        O::AtomicEvent { event, .. } => {
            use abstract_operations::AbstractAtomicEvent as E;
            match event {
                E::Load { result, .. } => Some((result.value, result.scalar_type)),
                E::ReadModifyWrite { prior, .. }
                | E::Swap { prior, .. }
                | E::CompareExchange {
                    observed: prior, ..
                } => Some((prior.value, prior.scalar_type)),
                E::Store { .. } | E::CompareExchangeOnce { .. } | E::Fence { .. } => None,
            }
        }
        O::EstablishActivationClaim { .. }
        | O::ReleaseActivationClaim { .. }
        | O::SplitActivationClaim { .. }
        | O::MergeActivationClaim { .. }
        | O::EstablishNativeCodeLayout { .. }
        | O::GlobalDescriptorTable { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishStructuralArray { .. }
        | O::EstablishPrimitiveLocal { .. }
        | O::PrimitiveLocalStore { .. }
        | O::ByteSequenceSubslice { .. }
        | O::ByteSequenceWrite { .. }
        | O::StructuralByteSequenceFieldByteStore { .. }
        | O::ByteSequenceStore { .. }
        | O::StructuralByteSequenceFieldStore { .. }
        | O::StructuralByteViewFieldStore { .. }
        | O::EstablishElementView { .. }
        | O::ElementViewWrite { .. }
        | O::ElementViewSubslice { .. }
        | O::ElementViewFixedArray { .. }
        | O::WriteOnlyPrimitiveStore { .. }
        | O::StructuralScalarFieldStore { .. }
        | O::MoveStructuralField { .. }
        | O::MoveStructuralValue { .. }
        | O::DecomposeStructuralValue { .. }
        | O::TransformContent { .. }
        | O::PackedIntegerUnary { .. }
        | O::PackedIntegerBinary { .. }
        | O::StructuralLeafCopy { .. }
        | O::StoreStructuralField { .. }
        | O::EstablishCase { .. }
        | O::EstablishByteSequenceConcatenation { .. }
        | O::EstablishByteSequenceLiteral { .. }
        | O::EstablishTrivialAffineLocal { .. }
        | O::EstablishZeroedStructural { .. }
        | O::EstablishStructuralLocal { .. }
        | O::EstablishRecord { .. }
        | O::PortWrite { .. }
        | O::Jump { .. }
        | O::Conditional { .. }
        | O::StructuralCase { .. }
        | O::Return { .. }
        | O::ReturnUnit { .. }
        | O::ReturnStructural { .. }
        | O::EstablishAccessReference { .. }
        | O::EstablishReference { .. }
        | O::ReleaseReference { .. }
        | O::InvokeNoNormal { .. }
        | O::ExternalCompletion { .. }
        | O::EliminateEmpty { .. }
        | O::Crash { .. }
        | O::CallStructural {
            result:
                AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Structural { .. },
            ..
        }
        | O::BoundaryCall {
            result:
                abstract_operations::AbstractBoundaryResult::Unit
                | abstract_operations::AbstractBoundaryResult::Structural(_),
            ..
        } => None,
    }
}

pub(crate) fn expected_definitions(
    operation: &AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueDefinition> {
    scalar_definition(operation)
        .into_iter()
        .map(|(value, scalar_type)| ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node { block, node },
        })
        .collect()
}
