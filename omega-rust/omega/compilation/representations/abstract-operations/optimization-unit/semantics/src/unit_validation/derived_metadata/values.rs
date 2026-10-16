//! Independently reconstructed scalar definitions and uses.
use abstract_operations::binding_arguments;
use optimization_unit::{ValueDefinition, ValueDefinitionSite, ValueUse};
use semantic_vocabulary::BlockId;

pub(crate) fn expected_definitions(
    operation: &abstract_operations::AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueDefinition> {
    let definition = optimization_unit::operation_definition(operation);
    definition
        .into_iter()
        .map(|(value, scalar_type)| ValueDefinition {
            value,
            scalar_type,
            site: ValueDefinitionSite::Node { block, node },
        })
        .collect()
}

pub(crate) fn expected_uses(
    operation: &abstract_operations::AbstractOperation,
    block: BlockId,
    node: u32,
) -> Vec<ValueUse> {
    use abstract_operations::AbstractOperation as O;
    let values = match operation {
        O::RegisterAccess { event, .. } => event.inputs().copied().collect(),
        O::EstablishStructuralArray { .. } => Vec::new(),
        O::EstablishScalarArray { elements, .. } => elements.clone(),
        O::EstablishReference { storage, .. } => {
            storage.iter().map(|view| view.byte_offset).collect()
        }
        abstract_operations::establish_case_or_record_fields_pattern!(fields) => fields
            .iter()
            .filter_map(|initializer| match &initializer.value {
                terminal_psi::RecordFieldValue::Scalar { value, .. } => Some(*value),
                terminal_psi::RecordFieldValue::Structural(_) => None,
            })
            .collect(),
        abstract_operations::byte_read_or_field_read_pattern!(index, length, ..) => {
            vec![*index, *length]
        }
        O::ElementViewRead { index, length, .. } => vec![*index, *length],
        O::EstablishByteSequenceConcatenation { left, right, .. } => {
            vec![left.length(), right.length()]
        }
        O::ByteSequencePredicate { operand, .. } => vec![operand.length()],
        O::ByteSequenceEqual { left, right, .. } => {
            vec![left.length(), right.length()]
        }
        abstract_operations::byte_sequence_store_pair_pattern!(length, ..) => vec![*length],
        abstract_operations::byte_sequence_write_pair_pattern!(index, value, length, ..) => {
            vec![*index, *value, *length]
        }
        O::ByteSequenceSubslice {
            start, end, length, ..
        } => vec![*start, *end, *length],
        O::ElementViewSubslice {
            start, end, length, ..
        } => vec![*start, *end, *length],
        O::EstablishElementView {
            region: terminal_psi::ElementViewRegion::RepresentationBytes { start, end, .. },
            ..
        } => vec![*start, *end],
        abstract_operations::call_or_call_structural_pattern!(arguments, ..)
        | O::BoundaryCall { arguments, .. }
        | O::ExternalCompletion { arguments, .. } => arguments.clone(),
        abstract_operations::primitive_local_triple_pattern!(value, ..)
        | O::StructuralScalarFieldStore { value, .. } => {
            vec![value.value]
        }
        O::BooleanNot { operand, .. }
        | O::IntegerBitwiseNot { operand, .. }
        | abstract_operations::integer_widen_or_exact_cast_pattern!(operand, ..)
        | O::ScalarRepresentationView { operand, .. } => vec![*operand],
        O::BooleanEqual { left, right, .. }
        | abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
        | O::IntegerLessOrEqual { left, right, .. }
        | O::IntegerBitwiseAnd { left, right, .. }
        | abstract_operations::bitwise_or_xor_pair_pattern!(left, right, ..)
        | O::IntegerArithmetic { left, right, .. } => vec![*left, *right],
        O::TrappingInteger { operation, .. } => operation.operands(),
        O::FloatResultPolicy { policy, .. } => policy.operands(),
        O::IeeeFloatCompare { left, right, .. } => vec![*left, *right],
        O::NearestIeeeFloatFusedMultiplyAdd {
            left,
            right,
            addend,
            ..
        } => vec![*left, *right, *addend],
        O::IntegerShift { value, count, .. } => vec![*value, *count],
        O::Jump { bindings, .. } => binding_arguments(bindings).collect(),
        O::Conditional {
            condition,
            when_true,
            when_false,
        } => std::iter::once(*condition)
            .chain(binding_arguments(&when_true.bindings))
            .chain(binding_arguments(&when_false.bindings))
            .collect(),
        O::Return { value, .. } => vec![*value],
        O::AtomicEvent { event, .. } => {
            use abstract_operations::AbstractAtomicEvent as E;
            // Operands are real scalar uses: an atomic event reads its
            // stored value, read-modify-write operand, or
            // comparison/replacement pair. The observed prior and results
            // are definitions, not uses.
            match event {
                E::Store { value, .. } | E::Swap { value, .. } => vec![*value],
                E::ReadModifyWrite { operand, .. } => vec![*operand],
                E::CompareExchange {
                    expected,
                    replacement,
                    ..
                }
                | E::CompareExchangeOnce {
                    expected,
                    replacement,
                    ..
                } => vec![*expected, *replacement],
                E::Load { .. } | E::Fence { .. } => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    // Runtime-selected path elements read their selectors after the direct
    // operands, as the unit's own use index records them.
    values
        .into_iter()
        .chain(
            operation
                .runtime_indices()
                .into_iter()
                .map(|(index, _)| index),
        )
        .map(|value| ValueUse { value, block, node })
        .collect()
}
