use super::{ElementViewRegion, OperationKind};
use crate::{ClaimTransfer, StructuralArgument};
use semantic_vocabulary::{MachineId, ValueId};

impl OperationKind {
    pub fn direct_callee(&self) -> Option<MachineId> {
        match self {
            Self::Call { callee, .. }
            | Self::CallUnit { callee, .. }
            | Self::CallStructuralScalar { callee, .. }
            | Self::CallStructural { callee, .. }
            | Self::CallStructuralWithScalarArguments { callee, .. } => Some(*callee),
            _ => None,
        }
    }

    pub fn constructor_arguments(&self) -> impl Iterator<Item = &StructuralArgument> {
        let (elements, fields): (&[StructuralArgument], &[crate::RecordFieldInitializer]) =
            match self {
                Self::EstablishStructuralArray { elements } => (elements, &[]),
                Self::EstablishRecord { fields } | Self::EstablishStructuralCase { fields, .. } => {
                    (&[], fields)
                }
                _ => (&[], &[]),
            };
        elements
            .iter()
            .chain(fields.iter().filter_map(|field| match &field.value {
                crate::RecordFieldValue::Structural(argument) => Some(argument),
                crate::RecordFieldValue::Scalar { .. } => None,
            }))
    }

    pub fn map_scalar_uses(&mut self, map: &mut impl FnMut(ValueId) -> ValueId) {
        for path in self.structural_projection_paths_mut() {
            for segment in path {
                if let crate::StructuralPathSegment::RuntimeIndex { index, .. } = segment {
                    *index = map(*index);
                }
            }
        }
        match self {
            Self::EstablishNativeCodeLayout { .. }
            | Self::ReleaseActivationClaim { .. }
            | Self::MergeActivationClaim { .. }
            | Self::PackedIntegerUnary { .. } | Self::PackedIntegerBinary { .. } => {}
            Self::EstablishActivationClaim { committed, .. }
            | Self::SplitActivationClaim {
                committed_at: committed,
                ..
            } => *committed = map(*committed),
            Self::EstablishReference { storage, .. } => {
                if let Some(storage) = storage {
                    storage.byte_offset = map(storage.byte_offset);
                }
            }
            Self::EstablishAccessReference { .. }
            | Self::StorageAddress { .. }
            | Self::StorageAlignment { .. }
            | Self::GlobalDescriptorTable { .. } => {}
            Self::EstablishByteSequenceConcatenation { left, right, .. } => {
                left.remap_length(map);
                right.remap_length(map);
            }
            Self::EstablishScalarArray { elements } => {
                for element in elements {
                    *element = map(*element);
                }
            }
            Self::EstablishScalarCase { fields, .. } => {
                for field in fields {
                    field.value = map(field.value);
                }
            }
            Self::EstablishStructuralCase { fields, .. } => {
                for field in fields {
                    if let crate::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                        *value = map(*value);
                    }
                }
            }
            Self::EstablishRecord { fields } => {
                for field in fields {
                    if let crate::RecordFieldValue::Scalar { value, .. } = &mut field.value {
                        *value = map(*value);
                    }
                }
            }
            Self::EstablishPrimitiveLocal { value, .. }
            | Self::WriteOnlyPrimitiveStore { value, .. }
            | Self::StructuralScalarFieldStore { value, .. }
            | Self::BooleanNot { operand: value }
            | Self::IntegerBitwiseNot { operand: value }
            | Self::IntegerWiden { operand: value }
            | Self::IntegerExactCast { operand: value, .. }
            | Self::ScalarRepresentationView { operand: value, .. } => *value = map(*value),
            Self::StructuralByteSequenceFieldStore { length, .. }
            | Self::ByteSequenceStore { length, .. } => {
                *length = map(*length);
            }
            Self::StructuralByteSequenceFieldByteStore {
                index,
                value,
                length,
                ..
            }
            | Self::ByteSequenceWrite {
                index,
                value,
                length,
                ..
            } => {
                *index = map(*index);
                *value = map(*value);
                *length = map(*length);
            }
            Self::ByteSequenceRead { index, length, .. } => {
                *index = map(*index);
                *length = map(*length);
            }
            Self::StructuralByteSequenceFieldRead { index, length, .. } => {
                *index = map(*index);
                *length = map(*length);
            }
            Self::ByteSequencePredicate { operand, .. } => operand.remap_length(map),
            Self::ByteSequenceEqual { left, right } => {
                left.remap_length(map);
                right.remap_length(map);
            }
            Self::EstablishElementView { region, .. }
            | Self::EstablishMutableElementView { region, .. } => {
                if let ElementViewRegion::RepresentationBytes { start, end, .. } = region {
                    *start = map(*start);
                    *end = map(*end);
                } else if let ElementViewRegion::WholeRepresentation { source_length, .. } = region
                {
                    *source_length = map(*source_length);
                }
            }
            Self::ByteSequenceSubslice {
                start, end, length, ..
            }
            | Self::ElementViewSubslice {
                start, end, length, ..
            } => {
                *start = map(*start);
                *end = map(*end);
                *length = map(*length);
            }
            Self::ElementViewRead { index, length, .. } => {
                *index = map(*index);
                *length = map(*length);
            }
            Self::ElementViewFixedArray { length, .. } => *length = map(*length),
            Self::ElementViewWrite {
                index,
                value,
                length,
                ..
            } => {
                *index = map(*index);
                *value = map(*value);
                *length = map(*length);
            }
            Self::BooleanEqual { left, right }
            | Self::IeeeFloatCompare { left, right, .. }
            | Self::IntegerEqual { left, right }
            | Self::IntegerLessThan { left, right }
            | Self::IntegerLessOrEqual { left, right }
            | Self::IntegerBitwiseAnd { left, right }
            | Self::IntegerBitwiseOr { left, right }
            | Self::IntegerBitwiseXor { left, right }
            | Self::WrappingIntegerAdd { left, right }
            | Self::SaturatingIntegerAdd { left, right }
            | Self::WrappingIntegerSubtract { left, right }
            | Self::SaturatingIntegerSubtract { left, right }
            | Self::WrappingIntegerMultiply { left, right }
            | Self::SaturatingIntegerMultiply { left, right }
            | Self::ExactIntegerAdd { left, right, .. }
            | Self::ExactIntegerSubtract { left, right, .. }
            | Self::ExactIntegerMultiply { left, right, .. }
            | Self::ExactIntegerDivide { left, right, .. }
            | Self::ExactIntegerRemainder { left, right, .. }
            | Self::WrappingIntegerDivide { left, right, .. }
            | Self::WrappingIntegerRemainder { left, right, .. }
            | Self::SaturatingIntegerDivide { left, right, .. }
            | Self::SaturatingIntegerRemainder { left, right, .. } => {
                *left = map(*left);
                *right = map(*right);
            }
            Self::WrappingIntegerShiftLeft { value, count }
            | Self::WrappingIntegerShiftRight { value, count }
            | Self::ExactIntegerShiftLeft { value, count, .. }
            | Self::ExactIntegerShiftRight { value, count, .. } => {
                *value = map(*value);
                *count = map(*count);
            }
            Self::TrappingInteger { operation } => operation.map_operands(map),
            Self::FloatResultPolicy { policy } => policy.map_operands(map),
            Self::AtomicAccess { event, .. } => event.map_operands(map),
            Self::ActivateTaskRegister { selector, .. }
            | Self::ReloadSegmentDescriptors { selector, .. } => *selector = map(*selector),
            Self::ReadTaskRegister { .. } | Self::ReadSegmentSelector { .. } => {}
            Self::RegisterAccess { event, .. } => event.map_operands(map),
            Self::NearestIeeeFloatFusedMultiplyAdd {
                left,
                right,
                addend,
            } => {
                *left = map(*left);
                *right = map(*right);
                *addend = map(*addend);
            }
            Self::Call { arguments, .. }
            | Self::CallUnit { arguments, .. }
            | Self::CallStructuralScalar { arguments, .. }
            | Self::CallStructuralWithScalarArguments { arguments, .. }
            | Self::BoundaryCall { arguments, .. } => {
                for argument in arguments {
                    *argument = map(*argument);
                }
            }
            Self::CallMachine {
                callee, arguments, ..
            } => {
                *callee = map(*callee);
                for argument in arguments {
                    *argument = map(*argument);
                }
            }
            Self::CallDynamic {
                selector,
                arguments,
                family_actuals,
                ..
            } => {
                *selector = map(*selector);
                for argument in arguments {
                    *argument = map(*argument);
                }
                for actual in family_actuals {
                    *actual = map(*actual);
                }
            }
            Self::SelectImplementation { .. }
            | Self::SelectMachine { .. }
            | Self::ReleaseReference { .. }
            | Self::PrimitiveScalarRead { .. }
            | Self::MoveStructuralField { .. }
            | Self::MoveStructuralValue { .. }
            | Self::DecomposeStructuralValue { .. }
            | Self::TransformContent { .. }
            | Self::StoreStructuralField { .. }
            | Self::StructuralCaseMembership { .. }
            | Self::StructuralLeafCopy { .. }
            | Self::StructuralCaseLeafCopy { .. }
            | Self::EstablishByteSequenceLiteral { .. }
            | Self::ByteSequenceLength { .. }
            | Self::ElementViewLength { .. }
            | Self::StructuralByteSequenceFieldLength { .. }
            | Self::StructuralByteViewFieldStore { .. }
            | Self::EstablishTrivialAffineLocal { .. }
            | Self::CallStructural { .. }
            | Self::PortWrite { .. }
            | Self::AtomicFence { .. }
            | Self::IntegerConstant { .. }
            | Self::ReadScalarApplication { .. }
            | Self::BooleanConstant { .. }
            | Self::IeeeFloatConstant { .. }
            | Self::EstablishStructuralArray { .. }
            | Self::EstablishZeroedStructural { .. }
            | Self::EstablishStructuralLocal { .. }
            | Self::BooleanStructuralField { .. }
            | Self::ScalarStructuralField { .. } => {}
        }
    }

    pub fn structural_arguments(&self) -> &[StructuralArgument] {
        match self {
            crate::call_structural_argument_variants!(structural_arguments) => structural_arguments,
            _ => &[],
        }
    }

    pub fn claim_transfers(&self) -> &[ClaimTransfer] {
        match self {
            Self::CallUnit {
                claim_transfers, ..
            }
            | Self::CallStructuralScalar {
                claim_transfers, ..
            }
            | Self::CallStructural {
                claim_transfers, ..
            }
            | Self::CallStructuralWithScalarArguments {
                claim_transfers, ..
            }
            | Self::CallDynamic {
                claim_transfers, ..
            }
            | Self::CallMachine {
                claim_transfers, ..
            } => claim_transfers,
            _ => &[],
        }
    }

    pub fn structural_arguments_mut(&mut self) -> Option<&mut Vec<StructuralArgument>> {
        match self {
            crate::call_structural_argument_variants!(structural_arguments) => {
                Some(structural_arguments)
            }
            _ => None,
        }
    }
}
