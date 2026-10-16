//! Input row and control contracts. Whole-unit validation owns SSA and effect-chain validity.
use super::{
    AbstractOperation, IntegerSign, OptimizationNode, PsiOptimizationFunction, PsiProvenance,
    ScalarType, ValueDefinitionSite, ValueId, scalar_shape,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::exact_cast_has_native_carriers;
use crate::legalization::scalar_graph_input::integer_type;
use crate::legalization::scalar_graph_input::u8_type;
use crate::legalization::scalar_graph_input::u64_type;
use crate::legalization::scalar_graph_input::value_type;
use crate::legalization::scalar_graph_input::{
    saturating_carrier, supports_wrapping_division, trapping_form, trapping_operands_rejoin,
};
use abstract_operations::{
    AbstractStructuralCallResult, IntegerArithmetic, IntegerArithmeticOperator,
    IntegerOverflowPolicy,
};
use optimization_unit::OptimizationBlock;
use semantic_vocabulary::{IntegerType, OperationId};
/// Why a node has no legal instruction row. Only the first kind is a custody
/// defect; the second is a limit of this stage that the diagnostic must name.
pub(in crate::legalization) enum NodeRejection {
    /// A listed family whose own payload violates the representation: an
    /// out-of-range literal, a byte read that does not produce u8, a length
    /// that is not u64.
    Malformed,
    /// A well-formed operation with no legalized kind at its scalar type, or
    /// with a payload (crash continuations, claim transfers) this stage does
    /// not realize.
    UnsupportedFamily,
}

pub(in crate::legalization) fn instruction(
    node: &OptimizationNode,
) -> Option<(OperationId, Option<ValueId>)> {
    admit(node).ok()
}

/// Admit one node to the ordinary scalar graph, distinguishing a malformed
/// node from an operation family this stage cannot yet legalize.
pub(in crate::legalization) fn admit(
    node: &OptimizationNode,
) -> Result<(OperationId, Option<ValueId>), NodeRejection> {
    if let AbstractOperation::RegisterAccess {
        psi_operation,
        event,
        result,
        ..
    } = &node.operation
    {
        if !event.operation().is_admitted()
            || event.operation().produces_value() != result.is_some()
        {
            return Err(NodeRejection::Malformed);
        }
        return Ok((*psi_operation, result.map(|result| result.value)));
    }
    if let AbstractOperation::AtomicEvent {
        psi_operation,
        event,
        ..
    } = &node.operation
    {
        if matches!(
            event,
            abstract_operations::AbstractAtomicEvent::Fence { .. }
        ) {
            return Ok((*psi_operation, None));
        }
        let (event, result) =
            super::atomic_access::event(event).ok_or(NodeRejection::UnsupportedFamily)?;
        if !event.ordering_is_legal() {
            return Err(NodeRejection::Malformed);
        }
        return Ok((*psi_operation, result.map(|result| result.value)));
    }
    if let abstract_operations::establish_or_subslice_set_pattern!(psi_operation, ..)
    | AbstractOperation::PortWrite { psi_operation, .. }
    | AbstractOperation::EstablishRecord { psi_operation, .. }
    | AbstractOperation::EstablishReference { psi_operation, .. }
    | AbstractOperation::EstablishPlacedReference { psi_operation, .. }
    | AbstractOperation::ReleaseReference { psi_operation, .. }
    | AbstractOperation::EstablishCase { psi_operation, .. }
    | AbstractOperation::CallStructural {
        psi_operation,
        result: AbstractStructuralCallResult::Unit | AbstractStructuralCallResult::Structural { .. },
        ..
    }
    | AbstractOperation::PrimitiveLocalStore { psi_operation, .. }
    | AbstractOperation::EstablishByteSequenceLiteral { psi_operation, .. }
    | AbstractOperation::ByteSequenceWrite { psi_operation, .. }
    | AbstractOperation::ElementViewWrite { psi_operation, .. }
    | AbstractOperation::StructuralByteSequenceFieldByteStore { psi_operation, .. }
    | AbstractOperation::ByteSequenceStore { psi_operation, .. }
    | AbstractOperation::StructuralByteSequenceFieldStore { psi_operation, .. }
    | AbstractOperation::StructuralByteViewFieldStore { psi_operation, .. }
    | AbstractOperation::WriteOnlyPrimitiveStore { psi_operation, .. }
    | AbstractOperation::StructuralLeafCopy { psi_operation, .. }
    | AbstractOperation::MoveStructuralField { psi_operation, .. }
    | AbstractOperation::StoreStructuralField { psi_operation, .. }
    | AbstractOperation::EstablishZeroedStructural { psi_operation, .. }
    | AbstractOperation::EstablishStructuralLocal { psi_operation, .. }
    | AbstractOperation::EstablishTrivialAffineLocal { psi_operation, .. }
    | AbstractOperation::StructuralScalarFieldStore { psi_operation, .. } = &node.operation
    {
        Ok((*psi_operation, None))
    } else if let AbstractOperation::BoundaryCall {
        psi_operation,
        result,
        ..
    } = &node.operation
    {
        // Exact native boundary realization is checked by the source reader;
        // operand origin and result kind do not change operation identity.
        Ok((*psi_operation, result.scalar().map(|result| result.value)))
    } else {
        scalar_instruction(node).map(|(operation, result)| (operation, Some(result)))
    }
}
fn scalar_instruction(node: &OptimizationNode) -> Result<(OperationId, ValueId), NodeRejection> {
    match &node.operation {
        AbstractOperation::SelectImplementation {
            psi_operation,
            result,
            ..
        } if result.scalar_type == target_operations::selector_carrier_type() => {
            Ok((*psi_operation, result.value))
        }
        AbstractOperation::IeeeFloatCompare {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::FloatResultPolicy {
            psi_operation,
            result,
            ..
        } => Ok((*psi_operation, *result)),
        abstract_operations::scalar_read_membership_triple_pattern!(psi_operation, result, ..) => {
            Ok((*psi_operation, result.value))
        }
        AbstractOperation::IeeeFloatConstant {
            psi_operation,
            result,
            ..
        } => Ok((*psi_operation, *result)),
        AbstractOperation::BooleanConstant {
            psi_operation,
            result,
            ..
        } => Ok((*psi_operation, *result)),
        AbstractOperation::CallStructural {
            psi_operation,
            result: AbstractStructuralCallResult::Scalar(result),
            ..
        } if scalar_shape(result.scalar_type).is_some() => {
            // The result defines one scalar regardless of owned arguments.
            // Projection retains claim transfers, requirements and crash
            // continuations; independent replay rejoins their exact source
            // rosters and the ordinary argument storage contract.
            Ok((*psi_operation, result.value))
        }
        abstract_operations::byte_read_or_field_read_pattern!(psi_operation, result, ..)
            if result.scalar_type == ScalarType::Integer(u8_type()) =>
        {
            Ok((*psi_operation, result.value))
        }
        AbstractOperation::ByteSequenceLength {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ElementViewLength {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::StructuralByteSequenceFieldLength {
            psi_operation,
            result,
            ..
        } if result.scalar_type == ScalarType::Integer(u64_type()) => {
            Ok((*psi_operation, result.value))
        }
        abstract_operations::byte_read_predicate_equal_pattern!(psi_operation, result, ..)
            if result.scalar_type == ScalarType::Boolean =>
        {
            Ok((*psi_operation, result.value))
        }
        AbstractOperation::ElementViewRead {
            psi_operation,
            result,
            ..
        } if scalar_shape(result.scalar_type).is_some() => Ok((*psi_operation, result.value)),
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } if valid_literal(*scalar_type, *value) => Ok((*psi_operation, *result)),
        AbstractOperation::Call {
            psi_operation,
            result,
            scalar_type,
            ..
        } if scalar_shape(*scalar_type).is_some() => {
            // A crash ceiling does not execute a crash or require unwinding.
            // Ordinary call projection carries the exact continuation roster,
            // and independent replay compares it with this source operation.
            // The callee's no-successor leaves own actual crash emission.
            Ok((*psi_operation, *result))
        }
        AbstractOperation::IntegerBitwiseAnd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | abstract_operations::bitwise_or_xor_pair_pattern!(
            psi_operation,
            result,
            scalar_type,
            ..
        ) if scalar_shape(ScalarType::Integer(*scalar_type)).is_some() => {
            Ok((*psi_operation, *result))
        }
        AbstractOperation::IntegerBitwiseNot {
            psi_operation,
            result,
            scalar_type,
            ..
        } if scalar_shape(ScalarType::Integer(*scalar_type)).is_some() => {
            Ok((*psi_operation, *result))
        }
        AbstractOperation::IntegerArithmetic {
            psi_operation,
            result,
            scalar_type,
            arithmetic,
            ..
        } if integer_arithmetic_admitted(*arithmetic, *scalar_type) => {
            Ok((*psi_operation, *result))
        }
        AbstractOperation::IntegerShift {
            psi_operation,
            result,
            value_type,
            count_type,
            ..
        } if scalar_shape(ScalarType::Integer(*value_type)).is_some()
            && scalar_shape(ScalarType::Integer(*count_type)).is_some() =>
        {
            // Both carriers must be fixed native widths: the value is the
            // shifted carrier and the count rides in its own register for
            // the hardware modulo reduction.
            Ok((*psi_operation, *result))
        }
        AbstractOperation::TrappingInteger {
            psi_operation,
            result,
            scalar_type,
            operand_type,
            operation,
        } if trapping_form(*scalar_type, *operand_type, operation.primitive()).is_some() => {
            Ok((*psi_operation, *result))
        }
        abstract_operations::integer_equal_less_pair_pattern!(psi_operation, result, ..)
        | AbstractOperation::IntegerLessOrEqual {
            psi_operation,
            result,
            ..
        } => Ok((*psi_operation, *result)),
        abstract_operations::boolean_not_or_equal_pair_pattern!(psi_operation, result, ..)
        | AbstractOperation::IntegerWiden {
            psi_operation,
            result,
            ..
        }
        | AbstractOperation::ScalarRepresentationView {
            psi_operation,
            result,
            ..
        } => Ok((*psi_operation, *result)),
        AbstractOperation::IntegerExactCast {
            psi_operation,
            result,
            ..
        } => Ok((*psi_operation, *result)),
        // A guard above failed for a listed family: an invalid literal or a
        // byte read/length whose result type the representation fixes is a
        // malformed plan, not a missing lowering.
        AbstractOperation::IntegerConstant { .. }
        | abstract_operations::byte_read_or_field_read_pattern!(..)
        | AbstractOperation::ByteSequenceLength { .. }
        | abstract_operations::byte_read_predicate_equal_pattern!(..)
        | AbstractOperation::StructuralByteSequenceFieldLength { .. } => {
            Err(NodeRejection::Malformed)
        }
        _ => Err(NodeRejection::UnsupportedFamily),
    }
}
/// The carriers each integer arithmetic pair is realized on. Saturating pairs
/// need a saturating carrier and wrapping divide/remainder a divisible one.
/// Every other pair admits each fixed native carrier: exact divide/remainder
/// normalize non-u64 carriers into signed i64, where the proven NonZeroDivisor
/// and ResultRepresentable obligations exclude every input the signed divide
/// would fault on (u64 keeps its unsigned entry because its dividend range
/// escapes i64), and exact add/sub/mul use the full-register operation over
/// sign/zero normalized inputs, where the retained representability proof
/// makes the result canonical without the truncation wrapping arithmetic
/// needs. Signedness is not an admission fence.
fn integer_arithmetic_admitted(arithmetic: IntegerArithmetic, scalar_type: IntegerType) -> bool {
    match (arithmetic.policy(), arithmetic.operator()) {
        (IntegerOverflowPolicy::Saturating, _) => saturating_carrier(scalar_type).is_some(),
        (
            IntegerOverflowPolicy::Wrapping,
            IntegerArithmeticOperator::Divide | IntegerArithmeticOperator::Remainder,
        ) => supports_wrapping_division(scalar_type),
        (IntegerOverflowPolicy::Wrapping | IntegerOverflowPolicy::Exact, _) => {
            scalar_shape(ScalarType::Integer(scalar_type)).is_some()
        }
    }
}

fn valid_literal(scalar: ScalarType, value: semantic_vocabulary::IntegerValue) -> bool {
    if let Some(integer) = scalar.as_integer()
        && matches!(integer.bits(), 8 | 16 | 32 | 64)
    {
        return match value {
            semantic_vocabulary::IntegerValue::Unsigned(value) => {
                integer.sign() == IntegerSign::Unsigned && value < (1_u128 << integer.bits())
            }
            semantic_vocabulary::IntegerValue::Signed(value) => {
                integer.sign() == IntegerSign::Signed
                    && value >= -(1_i128 << (integer.bits() - 1))
                    && value < (1_i128 << (integer.bits() - 1))
            }
        };
    }
    false
}
pub(super) fn validate(
    block: &OptimizationBlock,
    optimized: &PsiOptimizationFunction,
    types: &[terminal_psi::StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    identities: &terminal_psi::TerminalIdentityTable,
) -> Result<(), LegalizationError> {
    let (terminator, body) = block
        .nodes
        .split_last()
        .ok_or(LegalizationError::custody())?;
    for (position, parameter) in block.parameters.iter().enumerate() {
        if (integer_type(parameter.scalar_type).is_none()
            && !(matches!(
                parameter.scalar_type,
                ScalarType::Boolean | ScalarType::IeeeFloat(_)
            ))
            && !(matches!(parameter.scalar_type, ScalarType::Integer(_))
                && scalar_shape(parameter.scalar_type).is_some()))
            || parameter.site
                != (ValueDefinitionSite::BlockParameter {
                    block: block.id,
                    position: position as u32,
                })
        {
            return Err(LegalizationError::custody());
        }
    }
    for (position, node) in body.iter().enumerate() {
        // Every runtime-selected path element names an integer selector the
        // 64-bit address model can extend; its bound is the operation's own
        // verified obligation.
        if !node
            .operation
            .runtime_indices()
            .into_iter()
            .all(|(index, _)| {
                matches!(value_type(optimized, index), Some(ScalarType::Integer(integer))
                if integer.bits() <= 64)
            })
        {
            return Err(LegalizationError::custody());
        }
        let (operation, result) = admit(node).map_err(|rejection| match rejection {
            NodeRejection::Malformed => LegalizationError::custody(),
            NodeRejection::UnsupportedFamily => LegalizationError::UnsupportedScalarOperation {
                machine: optimized.machine,
                operation: node.operation.clone(),
            },
        })?;
        if !node.successors.is_empty()
            || node.provenance != [PsiProvenance::Operation(operation)]
            || node.fuel.is_empty()
            || node
                .fuel
                .iter()
                .any(|fuel| fuel.site != PsiProvenance::Operation(operation))
        {
            return Err(LegalizationError::custody());
        }
        if matches!(
            node.operation,
            AbstractOperation::EstablishPrimitiveLocal { .. }
                | AbstractOperation::PrimitiveLocalStore { .. }
        ) {
            if result.is_some() || !node.definitions.is_empty() {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::EstablishByteSequenceConcatenation {
            psi_operation,
            result: output,
            left,
            right,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || output.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                || !output.claims.is_empty()
                || !output.projected_qualifications.is_empty()
                || !output.qualification_establishments.is_empty()
                || [left, right].iter().any(|operand| {
                    value_type(optimized, operand.length()) != Some(ScalarType::Integer(u64_type()))
                })
                || !optimized.structural_places.iter().any(|place| {
                    place.id == output.place
                        && place.kind
                            == (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                                producer: *psi_operation,
                                structural_type: output.structural_type,
                            })
                })
            {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::ByteSequenceSubslice {
            source,
            start,
            end,
            length,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || !super::element_views::descriptor_geometry(optimized, source, types).is_some_and(
                    |geometry| {
                        geometry.readable
                            && types.iter().any(|declaration| {
                                declaration.id == geometry.selected
                                    && matches!(
                                        declaration.shape,
                                        terminal_psi::StructuralTypeShape::ByteSequence(_)
                                    )
                            })
                    },
                )
                || [start, end, length].iter().any(|value| {
                    value_type(optimized, **value) != Some(ScalarType::Integer(u64_type()))
                })
            {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::ElementViewSubslice {
            source,
            start,
            end,
            length,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || !super::element_views::descriptor_geometry(optimized, source, types).is_some_and(
                    |geometry| {
                        geometry.readable
                            && types.iter().any(|declaration| {
                                declaration.id == geometry.selected
                                    && matches!(
                                        declaration.shape,
                                        terminal_psi::StructuralTypeShape::ElementView { .. }
                                    )
                            })
                    },
                )
                || [start, end, length].iter().any(|value| {
                    value_type(optimized, **value) != Some(ScalarType::Integer(u64_type()))
                })
            {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::EstablishElementView {
            psi_operation,
            result: op_result,
            destination,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || !optimized.structural_places.iter().any(|place| {
                    place.id == *destination
                        && place.kind
                            == (semantic_vocabulary::StructuralPlaceKind::OperationResult {
                                producer: *psi_operation,
                                structural_type: op_result.structural_type,
                            })
                })
            {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::ByteSequenceStore { length, .. }
        | AbstractOperation::StructuralByteSequenceFieldStore { length, .. } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
            {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        // A descriptor replacement has no scalar operand or result.
        if let AbstractOperation::StructuralByteViewFieldStore { .. } = &node.operation {
            if result.is_some() || !node.definitions.is_empty() {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::StructuralByteSequenceFieldByteStore {
            index,
            value,
            length,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || value_type(optimized, *index) != Some(ScalarType::Integer(u64_type()))
                || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
                || value_type(optimized, *value) != Some(ScalarType::Integer(u8_type()))
            {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::ByteSequenceWrite {
            destination,
            index,
            value,
            length,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || !super::byte_views::writable_bytes(optimized, *destination, types)
                || value_type(optimized, *index) != Some(ScalarType::Integer(u64_type()))
                || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
                || value_type(optimized, *value) != Some(ScalarType::Integer(u8_type()))
            {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::ElementViewWrite {
            path,
            destination,
            index,
            value,
            length,
            ..
        } = &node.operation
        {
            // Source authority follows the shared descriptor lineage query,
            // not the producer family. Narrowing preserves its parent's loan;
            // affine descriptor custody alone cannot create mutable access.
            let geometry = super::element_views::descriptor_geometry(optimized, destination, types);
            let mutable_view = destination.access == terminal_psi::StructuralAccess::MutableBorrow
                && geometry.as_ref().is_some_and(|geometry| geometry.readable);
            let element = geometry.and_then(|geometry| {
                types
                    .iter()
                    .find_map(|declaration| match declaration.shape {
                        terminal_psi::StructuralTypeShape::ElementView { element, .. }
                            if declaration.id == geometry.selected =>
                        {
                            Some(element)
                        }
                        _ => None,
                    })
            });
            if result.is_some()
                || !node.definitions.is_empty()
                || !mutable_view
                || value_type(optimized, *index) != Some(ScalarType::Integer(u64_type()))
                || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
                || element.and_then(
                    |element| terminal_semantics::element_view_leaf_scalar(types, element, path),
                ) != value_type(optimized, *value)
                // A scalar store has the same native storage carriers as a
                // scalar read. The exact declared leaf above, not an integer
                // family recognizer, owns its width and representation.
                || value_type(optimized, *value).and_then(scalar_shape).is_none()
            {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::StructuralScalarFieldStore {
            destination, value, ..
        }
        | AbstractOperation::WriteOnlyPrimitiveStore {
            destination, value, ..
        } = &node.operation
        {
            let declared = match destination {
                abstract_operations::StructuralScalarStoreDestination::Parameter(parameter) => {
                    (optimized.structural_parameters.contains(parameter)
                        || matches!(
                            parameter.access,
                            terminal_psi::StructuralAccess::Owned
                                | terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        ) && optimized
                            .blocks
                            .iter()
                            .any(|block| block.structural_parameters.contains(parameter)))
                        && matches!(
                            parameter.access,
                            terminal_psi::StructuralAccess::Owned
                                | terminal_psi::StructuralAccess::MutableBorrow
                                | terminal_psi::StructuralAccess::WriteOnlyBorrow
                        )
                }
                abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                    producer,
                    result,
                } => super::structural_case::source_result(optimized, result.place).is_ok_and(
                    |(operation, declared)| operation == *producer && declared == result,
                ),
                // Field and primitive leaf stores resolve the same exact
                // owned arrival as reads of this place.
                abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                    block,
                    parameter,
                } => {
                    matches!(
                        super::structural_case::source_owner(optimized, parameter.place),
                        Ok(legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                            block: owner,
                            declaration,
                        }) if owner == *block && declaration == *parameter
                    )
                }
            };
            let preserved = match &node.operation {
                AbstractOperation::StructuralScalarFieldStore { path, field, .. } => {
                    terminal_semantics::projected_field_declaration(
                        types.iter(),
                        destination.structural_type(),
                        path,
                        *field,
                    )
                    .is_some_and(|field| {
                        let mut written = path.clone();
                        written.push(terminal_psi::StructuralPathSegment::Field(field.identity));
                        optimization_unit_semantics::structural_write_preserves_memberships(
                            optimized,
                            destination.place(),
                            &written,
                            types,
                            domains,
                            identities,
                        )
                    })
                }
                AbstractOperation::WriteOnlyPrimitiveStore {
                    path,
                    range_obligation,
                    ..
                } => {
                    // A primitive replacement can preserve an overlapping
                    // membership through its checked receiving restriction.
                    // Reuse that contract instead of requiring disjointness;
                    // the unit catalog owns the exact scalar-range authority.
                    terminal_semantics::primitive_store_restriction(
                        types.iter(),
                        domains.iter(),
                        destination.structural_type(),
                        destination.qualifications(),
                        destination.projected_qualifications(),
                        path,
                    )
                    .is_some_and(|restriction| restriction.is_some() == range_obligation.is_some())
                        && terminal_semantics::store_preserves_content(
                            types.iter(),
                            domains.iter(),
                            identities,
                            destination.structural_type(),
                            destination.place(),
                            optimized.content_entry_claims.iter(),
                            path,
                        )
                }
                _ => false,
            };
            let refusal = if result.is_some() || !node.definitions.is_empty() {
                Some("scalar store unexpectedly defines a result")
            } else if !declared {
                Some("scalar store destination does not rejoin its declaration")
            } else if !preserved {
                Some("scalar store does not preserve destination memberships or content")
            } else if value_type(optimized, value.value) != Some(value.scalar_type) {
                Some("scalar store value disagrees with its declared type")
            } else if scalar_shape(value.scalar_type).is_none() {
                Some("scalar store value has no native carrier")
            } else {
                None
            };
            if let Some(reason) = refusal {
                return Err(LegalizationError::InvalidScalarStore {
                    machine: optimized.machine,
                    operation,
                    reason,
                });
            }
            continue;
        }
        if let AbstractOperation::EstablishByteSequenceLiteral {
            psi_operation,
            place,
            structural_type,
            ..
        } = &node.operation
        {
            if result.is_some()
                || !node.definitions.is_empty()
                || super::literals::declaration_producer(optimized, place.id)
                    != Some((*psi_operation, structural_type.id))
            {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if matches!(
            node.operation,
            AbstractOperation::EstablishRecord { .. }
                | AbstractOperation::EstablishReference { .. }
                | AbstractOperation::EstablishPlacedReference { .. }
                | AbstractOperation::ReleaseReference { .. }
                | AbstractOperation::EstablishStructuralArray { .. }
                | AbstractOperation::EstablishScalarArray { .. }
                | AbstractOperation::EstablishCase { .. }
                | AbstractOperation::StructuralLeafCopy { .. }
                | abstract_operations::move_or_store_structural_field_pattern!()
                | AbstractOperation::EstablishTrivialAffineLocal { .. }
                | AbstractOperation::EstablishZeroedStructural { .. }
                | AbstractOperation::EstablishStructuralLocal { .. }
                | AbstractOperation::CallStructural {
                    result: AbstractStructuralCallResult::Structural { .. },
                    ..
                }
        ) {
            if result.is_some() || !node.definitions.is_empty() {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        // A Unit call defines no scalar. Its ownership and continuation
        // rosters remain attached to the call for exact source replay.
        if let AbstractOperation::CallStructural {
            result: AbstractStructuralCallResult::Unit,
            ..
        } = &node.operation
        {
            if result.is_some() || !node.definitions.is_empty() {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::BoundaryCall {
            arguments,
            result: boundary_result,
            ..
        } = &node.operation
        {
            if arguments
                .iter()
                .any(|value| value_type(optimized, *value).is_none())
            {
                return Err(LegalizationError::custody());
            }
            if boundary_result.scalar().is_none() {
                if result.is_some() || !node.definitions.is_empty() {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
        }
        if let AbstractOperation::RegisterAccess { result: None, .. } = &node.operation {
            if result.is_some() || !node.definitions.is_empty() {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if matches!(node.operation, AbstractOperation::PortWrite { .. }) {
            if result.is_some() || !node.definitions.is_empty() {
                return Err(LegalizationError::custody());
            }
            continue;
        }
        if let AbstractOperation::AtomicEvent { event, .. } = &node.operation {
            if matches!(
                event,
                abstract_operations::AbstractAtomicEvent::Fence { .. }
            ) {
                if result.is_some() || !node.definitions.is_empty() {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
            let (_, observed) =
                super::atomic_access::event(event).ok_or(LegalizationError::custody())?;
            if observed.is_none() {
                if result.is_some() || !node.definitions.is_empty() {
                    return Err(LegalizationError::custody());
                }
                continue;
            }
        }
        let [definition] = node.definitions.as_slice() else {
            return Err(LegalizationError::custody());
        };
        if Some(definition.value) != result
            || definition.site
                != (ValueDefinitionSite::Node {
                    block: block.id,
                    node: position as u32,
                })
        {
            return Err(LegalizationError::custody());
        }
        let expected_type = match &node.operation {
            AbstractOperation::RegisterAccess {
                result: Some(result),
                ..
            } => result.scalar_type,
            AbstractOperation::SelectImplementation { result, .. } => result.scalar_type,
            AbstractOperation::AtomicEvent { event, .. } => {
                super::atomic_access::event(event)
                    .and_then(|(_, result)| result)
                    .ok_or(LegalizationError::custody())?
                    .scalar_type
            }
            AbstractOperation::FloatResultPolicy { format, policy, .. } => {
                let scalar_type = ScalarType::IeeeFloat(*format);
                if !policy.accepts_operand_types(*format, |value| value_type(optimized, value)) {
                    return Err(LegalizationError::custody());
                }
                scalar_type
            }
            AbstractOperation::IeeeFloatCompare {
                format,
                left,
                right,
                ..
            } => {
                if value_type(optimized, *left) != Some(ScalarType::IeeeFloat(*format))
                    || value_type(optimized, *right) != Some(ScalarType::IeeeFloat(*format))
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Boolean
            }
            AbstractOperation::IeeeFloatConstant { value, .. } => {
                ScalarType::IeeeFloat(value.format())
            }
            AbstractOperation::BooleanConstant { .. } => ScalarType::Boolean,
            AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Scalar(result),
                ..
            }
            | AbstractOperation::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Scalar(result),
                ..
            }
            | AbstractOperation::PrimitiveScalarRead { result, .. }
            | AbstractOperation::StructuralScalarFieldRead { result, .. }
            | AbstractOperation::StructuralByteSequenceFieldLength { result, .. }
            | AbstractOperation::StructuralCaseMembership { result, .. } => result.scalar_type,
            AbstractOperation::ByteSequenceRead {
                source,
                index,
                length,
                ..
            } => {
                if !super::byte_views::contains_readable_bytes(optimized, *source, types)
                    || value_type(optimized, *index) != Some(ScalarType::Integer(u64_type()))
                    || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(u8_type())
            }
            AbstractOperation::ByteSequenceLength { source, .. } => {
                if super::element_views::byte_length_geometry(optimized, source, types).is_none()
                    && super::element_views::fixed_byte_length(optimized, source, types).is_none()
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(u64_type())
            }
            // A bounded field is record storage, not a view: its readable
            // parameter and field are admitted by `structural_fields::byte_read`.
            AbstractOperation::StructuralByteSequenceFieldRead { index, length, .. } => {
                if value_type(optimized, *index) != Some(ScalarType::Integer(u64_type()))
                    || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(u8_type())
            }
            AbstractOperation::ElementViewRead {
                result,
                source,
                index,
                length,
                ..
            } => {
                if !super::byte_views::contains_view(optimized, *source, types)
                    || value_type(optimized, *index) != Some(ScalarType::Integer(u64_type()))
                    || value_type(optimized, *length) != Some(ScalarType::Integer(u64_type()))
                {
                    return Err(LegalizationError::custody());
                }
                result.scalar_type
            }
            AbstractOperation::ElementViewLength { source, .. } => {
                if super::element_views::length_geometry(optimized, source, types).is_none() {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(u64_type())
            }
            AbstractOperation::ByteSequencePredicate { operand, .. } => {
                if value_type(optimized, operand.length()) != Some(ScalarType::Integer(u64_type()))
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Boolean
            }
            AbstractOperation::ByteSequenceEqual { left, right, .. } => {
                for operand in [left, right] {
                    if value_type(optimized, operand.length())
                        != Some(ScalarType::Integer(u64_type()))
                    {
                        return Err(LegalizationError::custody());
                    }
                }
                ScalarType::Boolean
            }
            AbstractOperation::IntegerConstant { scalar_type, .. }
            | AbstractOperation::Call { scalar_type, .. } => *scalar_type,
            AbstractOperation::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
                ..
            }
            | abstract_operations::bitwise_or_xor_pair_pattern!(scalar_type, left, right, ..) => {
                if value_type(optimized, *left) != Some(ScalarType::Integer(*scalar_type))
                    || value_type(optimized, *right) != Some(ScalarType::Integer(*scalar_type))
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(*scalar_type)
            }
            AbstractOperation::IntegerBitwiseNot {
                scalar_type,
                operand,
                ..
            } => {
                if value_type(optimized, *operand) != Some(ScalarType::Integer(*scalar_type)) {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(*scalar_type)
            }
            // Exact pairs keep their proven operand custody; every other
            // pair rejoins both operands to the carrier it is admitted on.
            AbstractOperation::IntegerArithmetic {
                scalar_type,
                arithmetic,
                left,
                right,
                ..
            } => {
                if arithmetic.policy() != IntegerOverflowPolicy::Exact
                    && (!integer_arithmetic_admitted(*arithmetic, *scalar_type)
                        || value_type(optimized, *left) != Some(ScalarType::Integer(*scalar_type))
                        || value_type(optimized, *right) != Some(ScalarType::Integer(*scalar_type)))
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(*scalar_type)
            }
            AbstractOperation::TrappingInteger {
                scalar_type,
                operand_type,
                operation,
                ..
            } => {
                if trapping_form(*scalar_type, *operand_type, operation.primitive()).is_none()
                    || !trapping_operands_rejoin(optimized, *scalar_type, *operand_type, *operation)
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(*scalar_type)
            }
            AbstractOperation::IntegerShift {
                value_type: shift_value_type,
                count_type,
                value,
                count,
                ..
            } => {
                if scalar_shape(ScalarType::Integer(*shift_value_type)).is_none()
                    || scalar_shape(ScalarType::Integer(*count_type)).is_none()
                    || value_type(optimized, *value) != Some(ScalarType::Integer(*shift_value_type))
                    || value_type(optimized, *count) != Some(ScalarType::Integer(*count_type))
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(*shift_value_type)
            }
            abstract_operations::integer_equal_less_pair_pattern!(left, right, ..)
            | AbstractOperation::IntegerLessOrEqual { left, right, .. } => {
                for operand in [*left, *right] {
                    let Some(ScalarType::Integer(carrier)) = value_type(optimized, operand) else {
                        return Err(LegalizationError::custody());
                    };
                    if scalar_shape(ScalarType::Integer(carrier)).is_none() {
                        return Err(LegalizationError::custody());
                    }
                }
                ScalarType::Boolean
            }
            AbstractOperation::BooleanEqual { left, right, .. } => {
                if value_type(optimized, *left) != Some(ScalarType::Boolean)
                    || value_type(optimized, *right) != Some(ScalarType::Boolean)
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Boolean
            }
            AbstractOperation::BooleanNot { operand, .. } => {
                if value_type(optimized, *operand) != Some(ScalarType::Boolean) {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Boolean
            }
            AbstractOperation::IntegerWiden {
                operand,
                source_type,
                target_type,
                ..
            } => {
                if !matches!(source_type.bits(), 8 | 16 | 32 | 64)
                    || target_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                    || !matches!(target_type.bits(), 16 | 32 | 64)
                    || !source_type.can_widen_to(*target_type)
                    || value_type(optimized, *operand) != Some(ScalarType::Integer(*source_type))
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(*target_type)
            }
            // The verified pair is replayed from the two carriers; a pair
            // outside the shared table, or an operand of another carrier, is a
            // malformed plan.
            AbstractOperation::ScalarRepresentationView {
                operand,
                source_type,
                target_type,
                ..
            } => {
                if terminal_semantics::ScalarRepresentationViewPair::classify(
                    *source_type,
                    *target_type,
                )
                .is_none()
                    || value_type(optimized, *operand) != Some(*source_type)
                {
                    return Err(LegalizationError::custody());
                }
                *target_type
            }
            AbstractOperation::IntegerExactCast {
                operand,
                source_type,
                target_type,
                ..
            } => {
                if !exact_cast_has_native_carriers(*source_type, *target_type)
                    || value_type(optimized, *operand) != Some(ScalarType::Integer(*source_type))
                {
                    return Err(LegalizationError::custody());
                }
                ScalarType::Integer(*target_type)
            }
            _ => return Err(LegalizationError::custody()),
        };
        if definition.scalar_type != expected_type {
            return Err(LegalizationError::custody());
        }
    }
    super::control::validate(terminator, body, optimized, types)
}
