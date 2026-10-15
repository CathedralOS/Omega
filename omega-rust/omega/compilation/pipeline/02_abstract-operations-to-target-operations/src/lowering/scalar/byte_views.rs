//! Byte observation and view operations for common graph lowering.
//! Arrival transport resolves storage once, independently of whether an entry
//! or state edge supplied it. Operation validation owns access permissions;
//! native geometry must not reinterpret qualification or frozen-content rules.
use super::{KnownInteger, KnownScalar};
use crate::LoweringError;
use crate::lowering::scalar::expressions::{insert_value, unsigned_u64_endpoint};
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation, AbstractStructuralCallResult};
use calling_conventions::ValueShape;
use semantic_vocabulary::{
    IntegerSign, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::TargetByteView;
use target_operations::{
    TargetBooleanExpression, TargetIntegerExpression, TargetStructuralParameter,
};
use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};

pub(in crate::lowering) fn block_source(
    function: &AbstractFunction,
    place: PlaceId,
    structural_types: &StructuralTypeLookup<'_>,
) -> Option<(
    target_operations::TargetStructuralArgumentSource,
    StructuralTypeId,
)> {
    function.block_entries.iter().find_map(|entry| {
        let parameter = entry.structural_parameter(place)?;
        (entry.block != function.entry && is_immutable_byte_parameter(parameter, structural_types))
            .then_some((
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block: entry.block,
                    place,
                },
                parameter.structural_type,
            ))
    })
}

pub(in crate::lowering) fn is_immutable_byte_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> bool {
    parameter.access == StructuralAccess::SharedBorrow
        && is_byte_parameter(parameter, structural_types)
}

/// An immutable dynamic descriptor has the same physical representation
/// whether its contents carry primitive qualifications or none. Semantic
/// admission independently checks those qualifications against the live source.
pub(in crate::lowering) fn is_immutable_byte_carrier_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> bool {
    parameter.access == StructuralAccess::SharedBorrow
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && structural_types
            .get(&parameter.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    )
                )
            })
}

pub(in crate::lowering) fn is_byte_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> bool {
    terminal_semantics::view_descriptor_parameter(parameter, structural_types.values().copied())
        && structural_types
            .get(&parameter.structural_type)
            .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
}

/// The borrowed-referent byte size a byte-view parameter's placement must
/// carry: the fat descriptor for a slice view, the single pointer cell for a
/// bounded view, and the whole `{length, bytes}` storage a shared bounded
/// owner loan addresses.
pub(in crate::lowering) fn byte_view_referent_size(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> Option<u32> {
    match structural_types.get(&parameter.structural_type)?.shape {
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            ..
        }) => Some(16),
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedView {
            ..
        }) => Some(8),
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
            capacity,
        }) => u32::try_from(capacity.checked_add(8)?).ok(),
        _ => None,
    }
}

/// A readable loan of a bounded byte owner: carried as the owner's address,
/// read through the owner layout (length word at 0, bytes from 8).
pub(in crate::lowering) fn is_owner_loan(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> bool {
    abstract_operations::control_flow::owner_loans::is_bounded_owner_loan(parameter, |identity| {
        structural_types.get(&identity).copied()
    })
}

pub(in crate::lowering) fn is_owned_byte_parameter(
    parameter: &terminal_psi::StructuralParameterDeclaration,
    structural_types: &StructuralTypeLookup<'_>,
) -> bool {
    abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter(
        parameter,
        |identity| structural_types.get(&identity).copied(),
    )
}

/// Unit scalar homes retain residence, while this separate live roster retains
/// which exact descriptor each already executed length observation measured.
#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn lower_byte_observation_with_lengths(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    backings: super::element_views::ElementViewBackings<'_>,
    values: &mut BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    provenance: &mut Vec<OperationId>,
) -> Result<bool, LoweringError> {
    if let AbstractOperation::ByteSequenceSubslice {
        psi_operation,
        result,
        ..
    } = operation
    {
        resolve_byte_view(
            function,
            structural_types,
            parameters,
            backings,
            values,
            lengths,
            result.place,
            &mut Vec::new(),
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let AbstractOperation::ByteSequencePredicate {
        psi_operation,
        result,
        operand,
        predicate,
    } = operation
    {
        let invalid = || super::descriptor_operands::refusal(function, &operation);
        if result.scalar_type != ScalarType::Boolean {
            return Err(invalid());
        }
        validate_whole_operand(
            function,
            structural_types,
            backings,
            values,
            operand,
            &invalid,
        )?;
        insert_value(
            values,
            result.value,
            KnownScalar::BooleanRuntime(TargetBooleanExpression::ByteSequencePredicate {
                psi_operation: *psi_operation,
                source_value: result.value,
                operand: operand.clone(),
                predicate: *predicate,
            }),
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let AbstractOperation::ByteSequenceEqual {
        psi_operation,
        result,
        left,
        right,
    } = operation
    {
        let invalid = || super::descriptor_operands::refusal(function, &operation);
        if result.scalar_type != ScalarType::Boolean {
            return Err(invalid());
        }
        for operand in [left, right] {
            validate_whole_operand(
                function,
                structural_types,
                backings,
                values,
                operand,
                &invalid,
            )?;
        }
        insert_value(
            values,
            result.value,
            KnownScalar::BooleanRuntime(TargetBooleanExpression::ByteSequenceEqual {
                psi_operation: *psi_operation,
                source_value: result.value,
                left: left.clone(),
                right: right.clone(),
            }),
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    if let AbstractOperation::ByteSequenceLength {
        psi_operation,
        result,
        source,
    } = operation
    {
        let invalid = || super::descriptor_operands::refusal(function, &operation);
        let Some(scalar_type) = result.scalar_type.as_integer() else {
            return Err(invalid());
        };
        if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
            return Err(invalid());
        }
        let (selected, indices) = super::descriptor_operands::descriptor_projection(
            function,
            structural_types,
            backings,
            source,
        )?;
        let Some(terminal_psi::StructuralTypeDeclaration {
            shape: StructuralTypeShape::ByteSequence(carrier),
            ..
        }) = structural_types.get(&selected).copied()
        else {
            return Err(invalid());
        };
        let value = match carrier {
            terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } => {
                KnownInteger::Immediate(semantic_vocabulary::IntegerValue::Unsigned(u128::from(
                    *capacity,
                )))
            }
            _ => KnownInteger::Runtime(TargetIntegerExpression::ByteSequenceLength {
                psi_operation: *psi_operation,
                source_value: result.value,
                source: source.clone(),
                indices,
                length_byte_offset: if matches!(
                    carrier,
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                ) {
                    0
                } else {
                    8
                },
            }),
        };
        insert_value(
            values,
            result.value,
            KnownScalar::Integer { scalar_type, value },
        )?;
        provenance.push(*psi_operation);
        return Ok(true);
    }
    let (psi_operation, result, source) = match operation {
        AbstractOperation::ByteSequenceRead {
            psi_operation,
            result,
            source,
            ..
        } => (*psi_operation, *result, *source),
        _ => return Ok(false),
    };
    let invalid = || super::descriptor_operands::refusal(function, &operation);
    let (view, _) = resolve_byte_view(
        function,
        structural_types,
        parameters,
        backings,
        values,
        lengths,
        source,
        &mut Vec::new(),
    )?;
    let Some(scalar_type) = result.scalar_type.as_integer() else {
        return Err(invalid());
    };
    let expression = match operation {
        AbstractOperation::ByteSequenceRead {
            index,
            length,
            obligation,
            ..
        } => {
            if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 8 {
                return Err(invalid());
            }
            let Some(KnownScalar::Integer {
                scalar_type: index_type,
                value: index_value,
            }) = values.get(index)
            else {
                return Err(invalid());
            };
            if index_type.sign() != IntegerSign::Unsigned
                || index_type.bits() != 64
                || !observes_length(values, lengths, *length, source)
            {
                return Err(invalid());
            }
            TargetIntegerExpression::ByteSequenceRead {
                psi_operation,
                source_value: result.value,
                source,
                view: Box::new(view),
                index: Box::new(index_value.clone().into_expression(*index)),
                length: *length,
                obligation: *obligation,
            }
        }
        _ => return Ok(false),
    };
    insert_value(
        values,
        result.value,
        KnownScalar::Integer {
            scalar_type,
            value: KnownInteger::Runtime(expression),
        },
    )?;
    provenance.push(psi_operation);
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::lowering) fn resolve_byte_view(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    parameters: &[TargetStructuralParameter],
    backings: super::element_views::ElementViewBackings<'_>,
    values: &BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    place: PlaceId,
    active: &mut Vec<PlaceId>,
) -> Result<(TargetByteView, StructuralAccess), LoweringError> {
    let invalid = || {
        super::descriptor_operands::refusal(
            function,
            &(place, backings.homes.get(&place), values, lengths),
        )
    };
    if active.contains(&place) {
        return Err(invalid());
    }
    if let Some((psi_operation, result)) =
        descriptor_result(function, place, structural_types, backings.homes)
        && structural_types
            .get(&result.structural_type)
            .is_some_and(|row| row.shape.is_borrowed_byte_carrier())
    {
        if backings
            .homes
            .get(&place)
            .and_then(|home| home.operation_result())
            != Some((psi_operation, result))
        {
            return Err(invalid());
        }
        return Ok((
            TargetByteView::Result {
                psi_operation,
                place,
                structural_type: result.structural_type,
            },
            terminal_semantics::descriptor_results::result_access(
                structural_types.values().copied(),
                result,
            )
            .ok_or_else(invalid)?,
        ));
    }
    if let Some((psi_operation, result)) = owned_byte_result(function, place, structural_types) {
        return Ok((
            TargetByteView::Owned {
                psi_operation,
                place,
                structural_type: result.structural_type,
            },
            StructuralAccess::Owned,
        ));
    }
    if let Some(AbstractOperation::EstablishByteSequenceLiteral {
        psi_operation, place: declaration, structural_type, ..
    }) = function.operations.iter().find(|operation|
        matches!(operation, AbstractOperation::EstablishByteSequenceLiteral { place: declaration, .. }
            if declaration.id == place))
    {
        if !matches!(declaration.kind,
            semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral { structural_type: identity, .. }
                if identity == structural_type.id)
            || !structural_type.shape.is_borrowed_byte_view()
            || structural_types.get(&structural_type.id).copied() != Some(structural_type)
        { return Err(invalid()); }
        return Ok((TargetByteView::Literal {
            psi_operation: *psi_operation, place, structural_type: structural_type.id,
        }, StructuralAccess::SharedBorrow));
    }
    // Collection establishments already own the exact backing/type/access
    // join. Byte observations reuse that descriptor geometry and permission.
    if let Some(AbstractOperation::EstablishElementView {
        psi_operation,
        result,
        ..
    }) = function.operations.iter().find(|operation| {
        matches!(operation,
            AbstractOperation::EstablishElementView { destination, .. } if *destination == place)
    }) {
        if !structural_types
            .get(&result.structural_type)
            .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
        {
            return Err(invalid());
        }
        let (_, structural_type, access) = super::element_views::resolve_element_view(
            function,
            structural_types,
            parameters,
            backings,
            values,
            lengths,
            place,
            active,
        )?;
        return Ok((
            TargetByteView::Literal {
                psi_operation: *psi_operation,
                place,
                structural_type,
            },
            access,
        ));
    }
    if let Some(semantic) = function.arrival_parameter(place) {
        let declaration = structural_types
            .get(&semantic.structural_type)
            .ok_or_else(invalid)?;
        let StructuralTypeShape::ByteSequence(carrier) = declaration.shape else {
            return Err(invalid());
        };
        let owned = semantic.access == StructuralAccess::Owned;
        if semantic.multiplicity != StructuralMultiplicity::Unrestricted
            || !semantic.projected_qualifications.is_empty()
            || owned
                && !matches!(
                    carrier,
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                )
        {
            return Err(invalid());
        }
        // The semantic declaration is authenticated above. Only physical
        // residence differs: state arrivals name an edge carrier, while an
        // entry arrival must rejoin its exact ABI placement.
        if let Some(entry) = function
            .block_entries
            .iter()
            .find(|entry| entry.structural_parameter(place).is_some())
        {
            return Ok((
                TargetByteView::BlockParameter {
                    block: entry.block,
                    place,
                    structural_type: semantic.structural_type,
                },
                semantic.access,
            ));
        }
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.place == place)
            .ok_or_else(invalid)?;
        // An owned byte formal uses the ordinary aggregate ABI. Its incoming
        // value-copy address supports observations; it is not a borrowed
        // descriptor or a loan of the caller's original payload.
        let expected_shape = if owned {
            crate::lowering::structural_layout::structural_shape(
                semantic.structural_type,
                structural_types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?
        } else {
            ValueShape::borrowed_reference(
                byte_view_referent_size(semantic, structural_types).ok_or_else(invalid)?,
                8,
            )
        };
        if parameter.structural_type != semantic.structural_type
            || parameter.access != semantic.access
            || parameter.multiplicity != semantic.multiplicity
            || parameter.projected_qualifications != semantic.projected_qualifications
            || parameter.shape != expected_shape
        {
            return Err(invalid());
        }
        return Ok((
            TargetByteView::Parameter {
                place,
                placement: parameter.placement.clone(),
            },
            semantic.access,
        ));
    }
    let Some(AbstractOperation::ByteSequenceSubslice { psi_operation, result, source, start, end, length, obligation }) =
        function.operations.iter().find(|operation| matches!(operation, AbstractOperation::ByteSequenceSubslice { result, .. } if result.place == place))
        else { return Err(invalid()); };
    if !matches!(
        result.multiplicity,
        terminal_psi::nonlinear_multiplicity_pair_pattern!()
    ) || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.claims.is_empty()
        || !structural_types
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    )
                )
            })
    {
        return Err(invalid());
    }
    let endpoint = unsigned_u64_endpoint(values, &invalid);
    let start = endpoint(*start)?;
    let end = endpoint(*end)?;
    if !super::descriptor_operands::observes_count(function, values, *length, source, true) {
        return Err(invalid());
    }
    let (selected, indices) = super::descriptor_operands::descriptor_projection(
        function,
        structural_types,
        backings,
        source,
    )?;
    if !structural_types.get(&selected).is_some_and(|declaration| {
        matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
    }) {
        return Err(invalid());
    }
    let source_access = source.access;
    let access = match result.multiplicity {
        StructuralMultiplicity::Unrestricted
            if matches!(
                source_access,
                StructuralAccess::SharedBorrow | terminal_psi::mutating_access_pair_pattern!()
            ) =>
        {
            StructuralAccess::SharedBorrow
        }
        StructuralMultiplicity::Affine if source_access == StructuralAccess::MutableBorrow => {
            StructuralAccess::MutableBorrow
        }
        _ => return Err(invalid()),
    };
    Ok((
        TargetByteView::Subslice {
            psi_operation: *psi_operation,
            place,
            source: source.clone(),
            indices,
            start,
            end,
            length: *length,
            obligation: *obligation,
        },
        access,
    ))
}

fn owned_byte_result<'a>(
    function: &'a AbstractFunction,
    place: PlaceId,
    structural_types: &StructuralTypeLookup<'_>,
) -> Option<(OperationId, &'a terminal_psi::StructuralOperationResult)> {
    let mut matching = function.operations.iter().filter_map(|operation| {
        let (psi_operation, result) = match operation {
            AbstractOperation::EstablishByteSequenceConcatenation {
                psi_operation,
                result,
                ..
            }
            | AbstractOperation::CallStructural {
                psi_operation,
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            } if result.place == place => (*psi_operation, result),
            _ => return None,
        };
        (result.multiplicity == StructuralMultiplicity::Unrestricted
            && result.projected_qualifications.is_empty()
            && result.qualification_establishments.is_empty()
            && result.claims.is_empty()
            && structural_types
                .get(&result.structural_type)
                .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence()))
        .then_some((psi_operation, result))
    });
    let result = matching.next()?;
    matching.next().is_none().then_some(result)
}

fn observes_length(
    values: &BTreeMap<ValueId, KnownScalar>,
    lengths: &BTreeMap<ValueId, PlaceId>,
    length: ValueId,
    source: PlaceId,
) -> bool {
    let Some(KnownScalar::Integer { scalar_type, value }) = values.get(&length) else {
        return false;
    };
    if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
        return false;
    }
    match value {
        KnownInteger::Runtime(TargetIntegerExpression::ByteSequenceLength {
            source: observed,
            source_value,
            ..
        }) => observed.place == source && observed.path.is_empty() && *source_value == length,
        KnownInteger::Runtime(TargetIntegerExpression::ScalarHome(home)) => {
            home.source_value == length
                && home.scalar_type == ScalarType::Integer(*scalar_type)
                && lengths.get(&length) == Some(&source)
        }
        _ => false,
    }
}

/// Authenticate a completed descriptor through its allocated result home and
/// exact producer declaration, independent of the producer's call origin.
/// Calls, copies and declarations materialize the same descriptor carrier;
/// establishment recipes keep their separate SSA geometry route. Retained
/// byte qualifications describe the produced value without changing its
/// descriptor layout. Backing provenance is
/// retained separately by call and descriptor custody; a result home never
/// becomes a new independent allocation merely because it has a descriptor
/// layout.
pub(in crate::lowering) fn descriptor_result<'a>(
    function: &'a AbstractFunction,
    place: PlaceId,
    types: &StructuralTypeLookup<'_>,
    homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
) -> Option<(OperationId, &'a terminal_psi::StructuralOperationResult)> {
    let (producer, stored) = homes.get(&place)?.operation_result()?;
    let mut producers = function.operations.iter().filter_map(|operation| {
        let result = operation.materialized_structural_result();
        result.filter(|(_, result)| result.place == place)
    });
    let (operation, result) = producers.next()?;
    (producers.next().is_none()
        && operation == producer
        && result == stored
        && matches!(
            result.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        )
        && result.projected_qualifications.is_empty()
        && result.qualification_establishments.is_empty()
        && result.claims.is_empty()
        && terminal_semantics::descriptor_results::is_descriptor(
            types.values().copied(),
            result.structural_type,
        ))
    .then_some((operation, result))
}

/// A whole operand's own checks: the source must resolve to a live view and
/// the spelled length must be the length observation that measured it.
#[allow(clippy::too_many_arguments)]
fn validate_whole_operand(
    function: &AbstractFunction,
    structural_types: &StructuralTypeLookup<'_>,
    backings: super::element_views::ElementViewBackings<'_>,
    values: &BTreeMap<ValueId, KnownScalar>,
    operand: &terminal_psi::control_flow::ByteSequenceOperand,
    invalid: &dyn Fn() -> LoweringError,
) -> Result<(), LoweringError> {
    if let terminal_psi::control_flow::ByteSequenceOperand::Whole { source, length } = operand {
        let (selected, indices) = super::descriptor_operands::descriptor_projection(
            function,
            structural_types,
            backings,
            source,
        )?;
        if source.access != terminal_psi::StructuralAccess::SharedBorrow
            || !indices.is_empty()
            || !structural_types.get(&selected).is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
            })
            || !super::descriptor_operands::observes_count(function, values, *length, source, true)
        {
            return Err(invalid());
        }
    }
    Ok(())
}
