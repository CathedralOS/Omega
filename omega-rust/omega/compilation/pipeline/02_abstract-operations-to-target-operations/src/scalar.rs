/*
  Scalar expression, integer arithmetic/bitwise/shift, byte-view, element-view,
  and descriptor-operand lowering (`try_lower_integer_operation`, `byte_views`,
  `element_views`, `descriptor_operands`).

  Resolves scalar value homes, integer bounds obligations, and read-only or
  mutable byte/element view descriptors for the common control-flow graph.
*/
pub(crate) mod byte_views {
    use super::descriptor_operands::observes_measured_length;
    use super::descriptor_operands::refusal;
    use super::{KnownInteger, KnownScalar};
    use crate::LoweringError;
    use crate::scalar::expressions::{insert_value, unsigned_u64_endpoint};
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::{AbstractFunction, AbstractOperation, AbstractStructuralCallResult};
    use calling_conventions::ValueShape;
    use semantic_vocabulary::IntegerValue;
    use semantic_vocabulary::{
        IntegerSign, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
    };
    use std::collections::BTreeMap;
    use target_operations::TargetByteView;
    use target_operations::{
        TargetBooleanExpression, TargetIntegerExpression, TargetStructuralParameter,
    };
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, StructuralTypeShape};
    use terminal_semantics::descriptor_results::{is_descriptor, result_access};
    use terminal_semantics::{descriptor_parameter_access, fixed_byte_extent};

    pub(crate) fn block_source(
        function: &AbstractFunction,
        place: PlaceId,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> Option<(
        target_operations::TargetStructuralArgumentSource,
        StructuralTypeId,
    )> {
        function.block_entries.iter().find_map(|entry| {
            let parameter = entry.structural_parameter(place)?;
            (entry.block != function.entry
                && is_immutable_byte_parameter(parameter, structural_types))
            .then_some((
                target_operations::TargetStructuralArgumentSource::BlockParameter {
                    block: entry.block,
                    place,
                },
                parameter.structural_type,
            ))
        })
    }

    pub(crate) fn is_immutable_byte_parameter(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> bool {
        parameter.access == StructuralAccess::SharedBorrow
            && is_byte_parameter(parameter, structural_types)
    }

    pub(crate) fn is_immutable_byte_carrier_parameter(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> bool {
        parameter.access == StructuralAccess::SharedBorrow
            && parameter.multiplicity == StructuralMultiplicity::Unrestricted
            && structural_types
                .get(parameter.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        )
                    )
                })
    }

    pub(crate) fn is_byte_parameter(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> bool {
        terminal_semantics::view_descriptor_parameter(parameter, structural_types.values().copied())
            && structural_types
                .get(parameter.structural_type)
                .is_some_and(|declaration| declaration.shape.is_borrowed_byte_carrier())
    }

    pub(crate) fn byte_view_referent_size(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> Option<u32> {
        match structural_types.get(parameter.structural_type)?.shape {
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
            ) => Some(16),
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedView {
                ..
            }) => Some(8),
            StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
            ) => u32::try_from(capacity.checked_add(8)?).ok(),
            _ => None,
        }
    }

    pub(crate) fn is_owner_loan(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> bool {
        abstract_operations::control_flow::owner_loans::is_bounded_owner_loan(
            parameter,
            |identity| structural_types.get(identity).copied(),
        )
    }

    pub(crate) fn is_owned_byte_parameter(
        parameter: &terminal_psi::StructuralParameterDeclaration,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> bool {
        abstract_operations::control_flow::owner_loans::is_owned_bounded_byte_parameter(
            parameter,
            |identity| structural_types.get(identity).copied(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_byte_observation_with_lengths(
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
            let (selected, indices) = super::descriptor_operands::descriptor_metadata_projection(
                function,
                structural_types,
                backings,
                source,
            )?;
            if structural_types
                .get(selected)
                .is_some_and(|row| matches!(row.shape, StructuralTypeShape::FixedArray { .. }))
            {
                let length = terminal_semantics::fixed_byte_extent(
                    structural_types.values().copied(),
                    selected,
                )
                .ok_or_else(invalid)?;
                insert_value(
                    values,
                    result.value,
                    KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Immediate(IntegerValue::Unsigned(u128::from(length))),
                    },
                )?;
                provenance.push(*psi_operation);
                return Ok(true);
            }
            let Some(terminal_psi::StructuralTypeDeclaration {
                shape: StructuralTypeShape::ByteSequence(carrier),
                ..
            }) = structural_types.get(selected).copied()
            else {
                return Err(invalid());
            };
            let value = match carrier {
                terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. } => {
                    KnownInteger::Immediate(IntegerValue::Unsigned(u128::from(*capacity)))
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
                    || !observes_measured_length(values, lengths, *length, source, true)
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
    pub(crate) fn resolve_byte_view(
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        parameters: &[TargetStructuralParameter],
        backings: super::element_views::ElementViewBackings<'_>,
        values: &BTreeMap<ValueId, KnownScalar>,
        lengths: &BTreeMap<ValueId, PlaceId>,
        place: PlaceId,
        active: &mut [PlaceId],
    ) -> Result<(TargetByteView, StructuralAccess), LoweringError> {
        let invalid = || {
            refusal(
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
                .get(result.structural_type)
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
                result_access(structural_types.values().copied(), result).ok_or_else(invalid)?,
            ));
        }
        if let Some((psi_operation, result)) = owned_byte_result(function, place, structural_types)
        {
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
            || structural_types.get(structural_type.id).copied() != Some(structural_type)
        { return Err(invalid()); }
        return Ok((TargetByteView::Literal {
            psi_operation: *psi_operation, place, structural_type: structural_type.id,
        }, StructuralAccess::SharedBorrow));
    }
        if let Some(AbstractOperation::EstablishElementView {
            psi_operation,
            result,
            ..
        }) = function.operations.iter().find(|operation| {
            matches!(operation,
            AbstractOperation::EstablishElementView { destination, .. } if *destination == place)
        }) {
            if !structural_types
                .get(result.structural_type)
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
                .get(semantic.structural_type)
                .ok_or_else(invalid)?;
            let StructuralTypeShape::ByteSequence(carrier) = declaration.shape else {
                return Err(invalid());
            };
            let backing_access = match carrier {
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    if semantic.multiplicity == StructuralMultiplicity::Unrestricted =>
                {
                    semantic.access
                }
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                | terminal_psi::ByteSequenceCarrier::BoundedView { .. } => {
                    descriptor_parameter_access(semantic, structural_types.values().copied())
                        .ok_or_else(invalid)?
                }
                _ => return Err(invalid()),
            };
            let owned = semantic.access == StructuralAccess::Owned;
            if !semantic.projected_qualifications.is_empty() {
                return Err(invalid());
            }
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
                    backing_access,
                ));
            }
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.place == place)
                .ok_or_else(invalid)?;
            let expected_shape = if owned {
                crate::structural_layout::structural_shape(
                    semantic.structural_type,
                    structural_types,
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
                backing_access,
            ));
        }
        let Some(AbstractOperation::ByteSequenceSubslice { psi_operation, result, source, start, end, length, obligation }) =
        function.operations.iter().find(|operation| matches!(operation, AbstractOperation::ByteSequenceSubslice { result, .. } if result.place == place))
        else { return Err(super::descriptor_operands::refusal(function, &("missing subslice producer", place, function.operations.len()))); };
        if !matches!(
            result.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        ) || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !result.claims.is_empty()
            || !structural_types
                .get(result.structural_type)
                .is_some_and(|declaration| {
                    matches!(
                        declaration.shape,
                        StructuralTypeShape::ByteSequence(
                            terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                        )
                    )
                })
        {
            return Err(super::descriptor_operands::refusal(
                function,
                &(
                    "subslice result carrier",
                    result,
                    structural_types.get(result.structural_type),
                ),
            ));
        }
        let invalid_endpoint = || {
            super::descriptor_operands::refusal(
                function,
                &("subslice endpoint", start, end, values),
            )
        };
        let endpoint = unsigned_u64_endpoint(values, &invalid_endpoint);
        let start = endpoint(*start)?;
        let end = endpoint(*end)?;
        if !super::descriptor_operands::observes_count(function, values, *length, source, true) {
            return Err(refusal(
                function,
                &("subslice count observation", length, source),
            ));
        }
        let (selected, indices) = super::descriptor_operands::descriptor_projection(
            function,
            structural_types,
            backings,
            source,
        )?;
        if !structural_types.get(selected).is_some_and(|declaration| {
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
                    .get(result.structural_type)
                    .is_some_and(|declaration| declaration.shape.is_bounded_owned_byte_sequence()))
            .then_some((psi_operation, result))
        });
        let result = matching.next()?;
        matching.next().is_none().then_some(result)
    }

    pub(crate) fn completed_descriptor<'function>(
        function: &'function AbstractFunction,
        place: PlaceId,
        types: &StructuralTypeLookup<'_>,
        homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
    ) -> Option<(
        OperationId,
        &'function terminal_psi::StructuralOperationResult,
    )> {
        let (producer, stored) = homes.get(&place)?.operation_result()?;
        let mut producers = function.operations.iter().filter_map(|operation| {
            let result = operation.structural_result();
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
            && terminal_semantics::view_descriptor_projected_qualifications(
                types.values().copied(),
                result.structural_type,
                &result.projected_qualifications,
            )
            && result.qualification_establishments.is_empty()
            && result.claims.is_empty()
            && is_descriptor(types.values().copied(), result.structural_type))
        .then_some((operation, result))
    }

    pub(crate) fn descriptor_result<'function>(
        function: &'function AbstractFunction,
        place: PlaceId,
        types: &StructuralTypeLookup<'_>,
        homes: &BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
    ) -> Option<(
        OperationId,
        &'function terminal_psi::StructuralOperationResult,
    )> {
        let (producer, result) = completed_descriptor(function, place, types, homes)?;
        function
            .operations
            .iter()
            .any(|operation| operation.materialized_structural_result() == Some((producer, result)))
            .then_some((producer, result))
    }

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
                || !structural_types.get(selected).is_some_and(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::ByteSequence(_))
                        || matches!(declaration.shape, StructuralTypeShape::FixedArray { .. })
                            && fixed_byte_extent(structural_types.values().copied(), selected)
                                .is_some()
                })
                || !super::descriptor_operands::observes_count(
                    function, values, *length, source, true,
                )
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
pub(crate) mod descriptor_operands {
    use super::element_views::ElementViewBackings;
    use crate::LoweringError;
    use crate::control_flow::scalar_sources::runtime_indices;
    use crate::structural_layout::reference_projection_regions;
    use crate::structural_layout::runtime_descriptor_projection;
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::AbstractFunction;
    use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};
    use std::collections::BTreeMap;
    use target_operations::TargetIntegerExpression;
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralParameterDeclaration,
        StructuralPathSegment, StructuralTypeShape,
    };
    use terminal_semantics::collection_backing;

    pub(crate) fn descriptor_projection(
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        backings: ElementViewBackings<'_>,
        source: &StructuralArgument,
    ) -> Result<
        (
            semantic_vocabulary::StructuralTypeId,
            Vec<target_operations::TargetStructuralRuntimeIndex>,
        ),
        LoweringError,
    > {
        descriptor_projection_access(function, structural_types, backings, source, false)
    }

    pub(crate) fn descriptor_metadata_projection(
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        backings: ElementViewBackings<'_>,
        source: &StructuralArgument,
    ) -> Result<
        (
            semantic_vocabulary::StructuralTypeId,
            Vec<target_operations::TargetStructuralRuntimeIndex>,
        ),
        LoweringError,
    > {
        descriptor_projection_access(function, structural_types, backings, source, true)
    }

    fn descriptor_projection_access(
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        backings: ElementViewBackings<'_>,
        source: &StructuralArgument,
        metadata: bool,
    ) -> Result<
        (
            semantic_vocabulary::StructuralTypeId,
            Vec<target_operations::TargetStructuralRuntimeIndex>,
        ),
        LoweringError,
    > {
        let invalid = || refusal(function, &(source, backings.homes.get(&source.place)));
        if !matches!(
            source.access,
            StructuralAccess::Owned
                | StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
        ) && !(metadata && source.access == StructuralAccess::WriteOnlyBorrow)
        {
            return Err(invalid());
        }
        if source.access == StructuralAccess::WriteOnlyBorrow
            && source
                .path
                .iter()
                .any(|segment| matches!(segment, StructuralPathSegment::Referent))
        {
            return Err(invalid());
        }
        let (root, access, multiplicity) = function
            .arrival_parameter(source.place)
            .map(|parameter| {
                (
                    parameter.structural_type,
                    parameter.access,
                    parameter.multiplicity,
                )
            })
            .or_else(|| {
                backings
                    .homes
                    .get(&source.place)
                    .map(|home| (home.structural_type(), home.access(), home.multiplicity()))
            })
            .or_else(|| {
                backings.view_result(function, source.place).map(|result| {
                    (
                        result.structural_type,
                        StructuralAccess::Owned,
                        result.multiplicity,
                    )
                })
            })
            .or_else(|| {
                function.operations.iter().find_map(|operation| {
                    let abstract_operations::AbstractOperation::EstablishByteSequenceLiteral {
                        place,
                        structural_type,
                        ..
                    } = operation
                    else {
                        return None;
                    };
                    (place.id == source.place
                        && structural_type.shape.is_borrowed_byte_view()
                        && structural_types.get(structural_type.id).copied()
                            == Some(structural_type)
                        && matches!(
                            place.kind,
                            semantic_vocabulary::StructuralPlaceKind::ByteSequenceLiteral {
                                structural_type: identity,
                                ..
                            } if identity == structural_type.id
                        ))
                    .then_some((
                        structural_type.id,
                        StructuralAccess::SharedBorrow,
                        terminal_psi::StructuralMultiplicity::Unrestricted,
                    ))
                })
            })
            .ok_or_else(invalid)?;
        if !(access == source.access
            || access == StructuralAccess::Owned
            || access == StructuralAccess::MutableBorrow
                && source.access == StructuralAccess::SharedBorrow)
        {
            return Err(invalid());
        }
        let requested = if source.access == StructuralAccess::Owned {
            StructuralAccess::SharedBorrow
        } else {
            source.access
        };
        let accessible = if source.access == StructuralAccess::WriteOnlyBorrow {
            terminal_semantics::reference_source_projection_accessible(
                structural_types.values().copied(),
                root,
                multiplicity,
                access,
                &source.path,
                requested,
            )
        } else {
            terminal_semantics::descriptor_source_projection_accessible(
                structural_types.values().copied(),
                root,
                multiplicity,
                access,
                &source.path,
                requested,
            )
        };
        if !accessible {
            return Err(invalid());
        }
        let regions = reference_projection_regions(root, &source.path, structural_types)
            .ok_or_else(invalid)?;
        let (selected, _, _, leaf_indices) =
            runtime_descriptor_projection(regions.root, regions.path, structural_types)?;
        let indices = regions.selector_runs(leaf_indices);
        let indices = runtime_indices(indices, function, &backings.selectors)?;
        Ok((selected, indices))
    }

    pub(crate) fn observes_count(
        function: &AbstractFunction,
        values: &BTreeMap<semantic_vocabulary::ValueId, super::KnownScalar>,
        length: semantic_vocabulary::ValueId,
        source: &StructuralArgument,
        bytes: bool,
    ) -> bool {
        let Some(super::KnownScalar::Integer { scalar_type, .. }) = values.get(&length) else {
            return false;
        };
        if scalar_type.sign() != semantic_vocabulary::IntegerSign::Unsigned
            || scalar_type.bits() != 64
        {
            return false;
        }
        function.operations.iter().any(|operation| match operation {
            abstract_operations::AbstractOperation::ByteSequenceLength {
                result,
                source: observed,
                ..
            } if bytes => {
                result.value == length
                    && terminal_semantics::same_descriptor_coordinate(observed, source)
            }
            abstract_operations::AbstractOperation::ElementViewLength {
                result,
                source: observed,
                ..
            } if !bytes => {
                result.value == length
                    && terminal_semantics::same_descriptor_coordinate(observed, source)
            }
            _ => false,
        })
    }

    pub(crate) fn observes_measured_length(
        values: &BTreeMap<semantic_vocabulary::ValueId, super::KnownScalar>,
        lengths: &BTreeMap<semantic_vocabulary::ValueId, semantic_vocabulary::PlaceId>,
        length: semantic_vocabulary::ValueId,
        source: semantic_vocabulary::PlaceId,
        bytes: bool,
    ) -> bool {
        let Some(super::KnownScalar::Integer { scalar_type, value }) = values.get(&length) else {
            return false;
        };
        if scalar_type.sign() != semantic_vocabulary::IntegerSign::Unsigned
            || scalar_type.bits() != 64
        {
            return false;
        }
        match value {
            super::KnownInteger::Runtime(TargetIntegerExpression::ByteSequenceLength {
                source: observed,
                source_value,
                ..
            }) if bytes => measured(observed, *source_value, length, source),
            super::KnownInteger::Runtime(TargetIntegerExpression::ElementViewLength {
                source: observed,
                source_value,
                ..
            }) if !bytes => measured(observed, *source_value, length, source),
            super::KnownInteger::Runtime(TargetIntegerExpression::ScalarHome(home)) => {
                home.source_value == length
                    && home.scalar_type == ScalarType::Integer(*scalar_type)
                    && lengths.get(&length) == Some(&source)
            }
            _ => false,
        }
    }

    fn measured(
        observed: &StructuralArgument,
        source_value: semantic_vocabulary::ValueId,
        length: semantic_vocabulary::ValueId,
        source: semantic_vocabulary::PlaceId,
    ) -> bool {
        observed.place == source && observed.path.is_empty() && source_value == length
    }

    pub(crate) fn storage_backs_view_parameter(
        collection: StructuralTypeId,
        path: &[StructuralPathSegment],
        parameter: &StructuralParameterDeclaration,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> bool {
        if !terminal_semantics::view_descriptor_parameter(
            parameter,
            structural_types.values().copied(),
        ) {
            return false;
        }
        let Some(declaration) = structural_types.get(parameter.structural_type).copied() else {
            return false;
        };
        let view_element = match &declaration.shape {
            StructuralTypeShape::ElementView { element, .. } => Some(*element),
            shape if shape.is_borrowed_byte_carrier() => None,
            _ => return false,
        };
        let element = view_element.or_else(|| {
            structural_types.values().find_map(|declaration| {
                (declaration.scalar_restriction.is_none()
                    && matches!(declaration.shape,
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
                    if Ok(integer) == IntegerType::new(IntegerSign::Unsigned, 8)))
                .then_some(declaration.id)
                .filter(|element| {
                    collection_backing(structural_types.values().copied(), collection, *element)
                        .is_some()
                })
            })
        });
        let Some(element) = element else {
            return false;
        };
        let Some(backing) = terminal_semantics::collection_backing(
            structural_types.values().copied(),
            collection,
            element,
        ) else {
            return false;
        };
        terminal_semantics::collection_view_descriptor(
            &declaration.shape,
            Some(backing.extent),
            terminal_psi::ElementViewRegion::WholeCollection,
            path,
            structural_types.get(element).copied(),
        )
        .is_some()
    }

    #[track_caller]
    pub(crate) fn refusal(
        function: &AbstractFunction,
        context: &impl std::fmt::Debug,
    ) -> LoweringError {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            let site = std::panic::Location::caller();
            eprintln!(
                "descriptor lowering refusal {:?} at {}:{}: {context:?}",
                function.machine,
                site.file(),
                site.line()
            );
        }
        LoweringError::UnsupportedOperationInScalarFunction(function.machine)
    }
}
pub(crate) mod element_views {
    use super::descriptor_operands::observes_measured_length;
    use super::descriptor_operands::refusal;
    use super::{KnownInteger, KnownScalar};
    use crate::LoweringError;
    use crate::scalar::expressions::{insert_value, unsigned_u64_endpoint};
    use crate::structural_layout::{
        direct_scalar_field_geometry, runtime_projection, runtime_storage_projection,
    };
    use crate::structural_type_lookup::StructuralTypeLookup;
    use abstract_operations::control_flow::address_joins::is_address_join;
    use abstract_operations::{AbstractFunction, AbstractOperation};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use target_operations::{
        TargetElementView, TargetIntegerExpression, TargetStructuralArgumentSource,
        TargetStructuralHomeOrigin, TargetStructuralHomeRequirement, TargetStructuralParameter,
    };
    use terminal_psi::{
        StructuralAccess, StructuralArgument, StructuralMultiplicity,
        StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
        StructuralTypeShape,
    };
    use terminal_semantics::descriptor_results::result_access;
    use terminal_semantics::{borrowed_storage_parameter, descriptor_parameter_access};

    mod establishment {
        use super::*;

        pub(crate) fn source(
            function: &AbstractFunction,
            structural_types: &StructuralTypeLookup<'_>,
            parameters: &[TargetStructuralParameter],
            backings: ElementViewBackings<'_>,
            source: &StructuralArgument,
            element: StructuralTypeId,
            region: terminal_psi::ElementViewRegion,
        ) -> Result<
            (
                StructuralTypeId,
                u32,
                Vec<target_operations::TargetStructuralRuntimeIndex>,
                target_operations::TargetViewExtent,
                u32,
                TargetStructuralArgumentSource,
            ),
            LoweringError,
        > {
            let invalid = || {
                crate::scalar::descriptor_operands::refusal(
                    function,
                    &(
                        source,
                        element,
                        region,
                        function.structural_parameter(source.place),
                        backings.homes.get(&source.place),
                        backings.address_joins.contains(&source.place),
                    ),
                )
            };
            let (field_path, byte_window) = match source.path.last() {
                Some(StructuralPathSegment::FixedRange { start, end }) => {
                    (&source.path[..source.path.len() - 1], Some((*start, *end)))
                }
                _ => (&source.path[..], None),
            };
            if !matches!(source.access, terminal_psi::reading_borrow_pair_pattern!()) {
                return Err(invalid());
            }
            let (root_structural_type, backing) =
                if let Some(semantic) = function.structural_parameter(source.place) {
                    let parameter = parameters
                        .iter()
                        .find(|parameter| parameter.place == source.place)
                        .ok_or_else(invalid)?;
                    if parameter.structural_type != semantic.structural_type {
                        return Err(invalid());
                    }
                    (
                        semantic.structural_type,
                        TargetStructuralArgumentSource::from(parameter.placement.clone()),
                    )
                } else if let Some(backing) = live_aggregate_backing(backings.homes, source.place) {
                    backing
                } else {
                    backings
                        .descriptor(function, structural_types, source)
                        .or_else(|| backings.address(function, structural_types, source))
                        .ok_or_else(invalid)?
                };
            let (declared_root, multiplicity, access) = backings
                .homes
                .get(&source.place)
                .map(|home| (home.structural_type(), home.multiplicity(), home.access()))
                .or_else(|| {
                    function.arrival_parameter(source.place).map(|parameter| {
                        (
                            parameter.structural_type,
                            parameter.multiplicity,
                            parameter.access,
                        )
                    })
                })
                .or_else(|| {
                    backings.view_result(function, source.place).map(|result| {
                        (
                            result.structural_type,
                            result.multiplicity,
                            StructuralAccess::Owned,
                        )
                    })
                })
                .ok_or_else(invalid)?;
            if declared_root != root_structural_type
                || !terminal_semantics::descriptor_source_projection_accessible(
                    structural_types.values().copied(),
                    root_structural_type,
                    multiplicity,
                    access,
                    field_path,
                    source.access,
                )
            {
                return Err(invalid());
            }
            let regions = crate::structural_layout::reference_projection_regions(
                root_structural_type,
                field_path,
                structural_types,
            )
            .filter(|regions| regions.readable)
            .ok_or_else(invalid)?;
            let (projected_storage, projected_shape, source_byte_offset, runtime_elements) =
                runtime_storage_projection(regions.root, regions.path, structural_types)?;
            let indices = crate::control_flow::scalar_sources::runtime_indices(
                regions.selector_runs(runtime_elements),
                function,
                &backings.selectors,
            )?;
            let collection_shape = projected_storage
                .shape(structural_types)
                .ok_or_else(invalid)?;
            let geometry = terminal_semantics::element_view_shape_region_geometry(
                structural_types.values().copied(),
                &collection_shape,
                element,
                region,
                source.access,
            )
            .ok_or_else(invalid)?;
            if region == terminal_psi::ElementViewRegion::WholeCollection
                && let StructuralTypeShape::FixedArray {
                    element: original, ..
                } = &*collection_shape
                && *original != element
            {
                let relation = terminal_semantics::element_representation_relation(
                    structural_types.values().copied(),
                    *original,
                    element,
                    source.access,
                )
                .ok_or_else(invalid)?;
                for (identity, layout) in [
                    (*original, &relation.source_layout),
                    (element, &relation.target_layout),
                ] {
                    let shape =
                        crate::structural_layout::structural_shape(identity, structural_types)?;
                    if u64::from(shape.byte_size) != layout.size
                        || u64::from(shape.alignment) != layout.alignment
                        || layout.leaves.iter().any(|leaf| {
                            scalar_leaf_offset(
                                structural_types,
                                identity,
                                &leaf.path,
                                leaf.scalar_type,
                            )
                            .map(u64::from)
                                != Some(leaf.offset)
                        })
                    {
                        return Err(invalid());
                    }
                }
            }
            let mut representation_source_stride = None;
            let (extent_kind, storage, representation) = match geometry {
                terminal_semantics::ElementViewRegionGeometry::WholeCollection(collection) => {
                    (collection.extent, collection.storage, None)
                }
                terminal_semantics::ElementViewRegionGeometry::WholeRepresentation {
                    backing,
                    source: source_layout,
                    target: target_layout,
                } => {
                    let original = match &*collection_shape {
                        StructuralTypeShape::FixedArray { element, .. }
                        | StructuralTypeShape::ElementView { element, .. } => *element,
                        _ => return Err(invalid()),
                    };
                    if byte_window.is_some()
                        || source.access != StructuralAccess::SharedBorrow
                        || target_layout.size == 0
                        || source_layout.size == 0
                        || source_layout.size % target_layout.size != 0
                        || source_layout.alignment < target_layout.alignment
                        || matches!(
                            backing.extent,
                            terminal_semantics::CollectionExtent::LiveLength { .. }
                        )
                    {
                        return Err(invalid());
                    }
                    let source_shape =
                        crate::structural_layout::structural_shape(original, structural_types)?;
                    if u64::from(source_shape.byte_size) != source_layout.size
                        || u64::from(source_shape.alignment) != source_layout.alignment
                        || source_layout.leaves.iter().any(|leaf| {
                            scalar_leaf_offset(
                                structural_types,
                                original,
                                &leaf.path,
                                leaf.scalar_type,
                            )
                            .map(u64::from)
                                != Some(leaf.offset)
                        })
                        || matches!(backing.extent,
                    terminal_semantics::CollectionExtent::Fixed(count)
                        if count.checked_mul(source_layout.size / target_layout.size).is_none())
                    {
                        return Err(invalid());
                    }
                    representation_source_stride = Some(source_layout.size);
                    (backing.extent, backing.storage, Some(target_layout))
                }
                terminal_semantics::ElementViewRegionGeometry::RepresentationBytes {
                    byte_extent,
                    element: layout,
                } => {
                    if byte_window.is_some() {
                        return Err(invalid());
                    }
                    let storage =
                        if matches!(&*collection_shape, StructuralTypeShape::ByteSequence(_)) {
                            terminal_semantics::CollectionStorage::Borrowed
                        } else {
                            terminal_semantics::CollectionStorage::Inline
                        };
                    (
                        terminal_semantics::CollectionExtent::Fixed(byte_extent),
                        storage,
                        Some(layout),
                    )
                }
            };
            let element_shape =
                crate::structural_layout::structural_shape(element, structural_types)?;
            let element_stride = crate::structural_layout::checked_align_up_u32(
                element_shape.byte_size,
                u32::from(element_shape.alignment),
            )
            .ok_or_else(invalid)?;
            if element_stride == 0
                || match storage {
                    terminal_semantics::CollectionStorage::Inline => {
                        (if let Some(source_stride) = representation_source_stride {
                            match extent_kind {
                                terminal_semantics::CollectionExtent::Fixed(length) => {
                                    source_stride.checked_mul(length)
                                }
                                _ => None,
                            }
                        } else if representation.is_some() {
                            match extent_kind {
                                terminal_semantics::CollectionExtent::Fixed(length) => Some(length),
                                _ => None,
                            }
                        } else {
                            match extent_kind {
                                terminal_semantics::CollectionExtent::Fixed(length) => {
                                    u64::from(element_stride).checked_mul(length)
                                }
                                terminal_semantics::CollectionExtent::LiveLength { capacity } => {
                                    u64::from(element_stride)
                                        .checked_mul(capacity)
                                        .and_then(|span| span.checked_add(8))
                                }
                                terminal_semantics::CollectionExtent::StoredLength => None,
                            }
                        }) != Some(u64::from(projected_shape.byte_size))
                    }
                    terminal_semantics::CollectionStorage::Borrowed => {
                        let carrier_size = match extent_kind {
                            terminal_semantics::CollectionExtent::StoredLength => 16,
                            terminal_semantics::CollectionExtent::Fixed(_) => 8,
                            terminal_semantics::CollectionExtent::LiveLength { .. } => {
                                return Err(invalid());
                            }
                        };
                        projected_shape.byte_size != carrier_size || byte_window.is_some()
                    }
                }
            {
                return Err(invalid());
            }
            if let Some(layout) = representation
                && (layout.size != u64::from(element_stride)
                    || layout.alignment != u64::from(element_shape.alignment)
                    || layout.leaves.iter().any(|leaf| {
                        scalar_leaf_offset(structural_types, element, &leaf.path, leaf.scalar_type)
                            .map(u64::from)
                            != Some(leaf.offset)
                    }))
            {
                return Err(invalid());
            }
            let (window_offset, extent) = match (byte_window, extent_kind) {
                (Some((start, end)), terminal_semantics::CollectionExtent::Fixed(length))
                    if start <= end && end <= length =>
                {
                    (
                        u32::try_from(
                            start
                                .checked_mul(u64::from(element_stride))
                                .ok_or_else(invalid)?,
                        )
                        .map_err(|_| invalid())?,
                        target_operations::TargetViewExtent::Fixed(end - start),
                    )
                }
                (Some(_), _) => return Err(invalid()),
                (None, terminal_semantics::CollectionExtent::Fixed(length)) => {
                    (0, target_operations::TargetViewExtent::Fixed(length))
                }
                (None, terminal_semantics::CollectionExtent::LiveLength { .. }) => {
                    (8, target_operations::TargetViewExtent::LiveLength)
                }
                (None, terminal_semantics::CollectionExtent::StoredLength) => {
                    (0, target_operations::TargetViewExtent::StoredLength)
                }
            };
            let source_byte_offset = source_byte_offset
                .checked_add(window_offset)
                .ok_or_else(invalid)?;
            Ok((
                root_structural_type,
                source_byte_offset,
                indices,
                extent,
                element_stride,
                backing,
            ))
        }
    }

    #[derive(Clone, Copy)]
    pub(crate) struct ElementViewBackings<'a> {
        pub(crate) homes: &'a BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
        views: &'a BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
        address_joins: &'a BTreeSet<PlaceId>,
        pub(crate) selectors: crate::control_flow::scalar_sources::ScalarSources<'a>,
    }

    impl<'a> ElementViewBackings<'a> {
        pub(crate) fn new(
            homes: &'a BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
            views: &'a BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
            address_joins: &'a BTreeSet<PlaceId>,
            selectors: crate::control_flow::scalar_sources::ScalarSources<'a>,
        ) -> Self {
            Self {
                homes,
                views,
                address_joins,
                selectors,
            }
        }

        pub(crate) fn view_result(
            self,
            function: &AbstractFunction,
            place: PlaceId,
        ) -> Option<&terminal_psi::StructuralOperationResult> {
            let (producer, structural_type) = self.views.get(&place)?;
            let mut results = function.operations.iter().filter_map(|operation| {
                operation
                    .structural_result()
                    .filter(|(_, result)| result.place == place)
            });
            let (operation, result) = results.next()?;
            (results.next().is_none()
                && operation == *producer
                && result.structural_type == *structural_type)
                .then_some(result)
        }

        fn address(
            self,
            function: &AbstractFunction,
            structural_types: &StructuralTypeLookup<'_>,
            source: &StructuralArgument,
        ) -> Option<(StructuralTypeId, TargetStructuralArgumentSource)> {
            if !self.address_joins.contains(&source.place) {
                return None;
            }
            let (entry, parameter) = function.block_entries.iter().find_map(|entry| {
                entry
                    .structural_parameter(source.place)
                    .map(|parameter| (entry, parameter))
            })?;
            if entry.block == function.entry
                || !is_address_join(parameter, |identity| {
                    structural_types.get(identity).copied()
                })
                || (source.access == StructuralAccess::MutableBorrow
                    && parameter.access != StructuralAccess::MutableBorrow)
            {
                return None;
            }
            Some((
                parameter.structural_type,
                TargetStructuralArgumentSource::BlockParameter {
                    block: entry.block,
                    place: parameter.place,
                },
            ))
        }

        fn descriptor(
            self,
            function: &AbstractFunction,
            structural_types: &StructuralTypeLookup<'_>,
            source: &StructuralArgument,
        ) -> Option<(StructuralTypeId, TargetStructuralArgumentSource)> {
            let (entry, parameter) = function.block_entries.iter().find_map(|entry| {
                entry
                    .structural_parameter(source.place)
                    .map(|parameter| (entry, parameter))
            })?;
            let descriptor_access = terminal_semantics::descriptor_parameter_access(
                parameter,
                structural_types.values().copied(),
            )?;
            if entry.block == function.entry
                || borrowed_storage_parameter(parameter, structural_types.values().copied())
                || !source.access.admits_referent(descriptor_access)
                || !parameter.projected_qualifications.is_empty()
            {
                return None;
            }
            Some((
                parameter.structural_type,
                TargetStructuralArgumentSource::BlockParameter {
                    block: entry.block,
                    place: parameter.place,
                },
            ))
        }
    }

    pub(crate) fn is_element_view_parameter(
        parameter: &StructuralParameterDeclaration,
        structural_types: &StructuralTypeLookup<'_>,
    ) -> bool {
        terminal_semantics::view_descriptor_parameter(parameter, structural_types.values().copied())
            && parameter.qualifications.is_empty()
            && structural_types
                .get(parameter.structural_type)
                .is_some_and(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn lower_element_observation_with_lengths(
        operation: &AbstractOperation,
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        parameters: &[TargetStructuralParameter],
        backings: ElementViewBackings<'_>,
        values: &mut BTreeMap<ValueId, KnownScalar>,
        lengths: &BTreeMap<ValueId, PlaceId>,
        provenance: &mut Vec<OperationId>,
    ) -> Result<bool, LoweringError> {
        if let AbstractOperation::ElementViewSubslice {
            psi_operation,
            result,
            ..
        } = operation
        {
            resolve_element_view(
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
        if let AbstractOperation::ElementViewLength {
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
            let (selected, indices) = super::descriptor_operands::descriptor_metadata_projection(
                function,
                structural_types,
                backings,
                source,
            )?;
            if !structural_types.get(selected).is_some_and(|declaration| {
                matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
            }) {
                return Err(invalid());
            }
            insert_value(
                values,
                result.value,
                KnownScalar::Integer {
                    scalar_type,
                    value: KnownInteger::Runtime(TargetIntegerExpression::ElementViewLength {
                        psi_operation: *psi_operation,
                        source_value: result.value,
                        source: source.clone(),
                        indices,
                        length_byte_offset: 8,
                    }),
                },
            )?;
            provenance.push(*psi_operation);
            return Ok(true);
        }
        let (psi_operation, result, source) = match operation {
            AbstractOperation::ElementViewRead {
                psi_operation,
                result,
                source,
                ..
            } => (*psi_operation, *result, *source),
            _ => return Ok(false),
        };
        let invalid = || super::descriptor_operands::refusal(function, &operation);
        let (view, view_structural_type, _) = resolve_element_view(
            function,
            structural_types,
            parameters,
            backings,
            values,
            lengths,
            source,
            &mut Vec::new(),
        )?;
        let known = match operation {
            AbstractOperation::ElementViewRead {
                index,
                length,
                obligation,
                path,
                ..
            } => {
                if !observes_measured_length(values, lengths, *length, source, false) {
                    return Err(invalid());
                }
                let element_byte_offset = element_leaf_offset(
                    structural_types,
                    view_structural_type,
                    path,
                    result.scalar_type,
                )
                .ok_or_else(invalid)?;
                let Some(KnownScalar::Integer {
                    scalar_type: index_type,
                    value: index_value,
                }) = values.get(index)
                else {
                    return Err(invalid());
                };
                if index_type.sign() != IntegerSign::Unsigned || index_type.bits() != 64 {
                    return Err(invalid());
                }
                match result.scalar_type {
                    ScalarType::Integer(scalar_type) => KnownScalar::Integer {
                        scalar_type,
                        value: KnownInteger::Runtime(TargetIntegerExpression::ElementViewRead {
                            psi_operation,
                            source_value: result.value,
                            source,
                            view: Box::new(view),
                            index: Box::new(index_value.clone().into_expression(*index)),
                            length: *length,
                            obligation: *obligation,
                            path: path.clone(),
                            element_byte_offset,
                        }),
                    },
                    ScalarType::Boolean => KnownScalar::BooleanRuntime(
                        target_operations::TargetBooleanExpression::ElementViewRead {
                            psi_operation,
                            source_value: result.value,
                            source,
                            view: Box::new(view),
                            index: Box::new(index_value.clone().into_expression(*index)),
                            length: *length,
                            obligation: *obligation,
                            path: path.clone(),
                            element_byte_offset,
                        },
                    ),
                    _ => return Err(invalid()),
                }
            }
            _ => return Ok(false),
        };
        insert_value(values, result.value, known)?;
        provenance.push(psi_operation);
        Ok(true)
    }

    pub(crate) fn float_element_read(
        operation: &AbstractOperation,
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        parameters: &[TargetStructuralParameter],
        backings: ElementViewBackings<'_>,
        values: &BTreeMap<ValueId, KnownScalar>,
        lengths: &BTreeMap<ValueId, PlaceId>,
    ) -> Result<target_operations::TargetScalarExpression, LoweringError> {
        let invalid = || super::descriptor_operands::refusal(function, operation);
        let AbstractOperation::ElementViewRead {
            psi_operation,
            result,
            source,
            index,
            length,
            obligation,
            path,
        } = operation
        else {
            return Err(invalid());
        };
        let ScalarType::IeeeFloat(format) = result.scalar_type else {
            return Err(invalid());
        };
        let (view, view_type, _) = resolve_element_view(
            function,
            structural_types,
            parameters,
            backings,
            values,
            lengths,
            *source,
            &mut Vec::new(),
        )?;
        if !observes_measured_length(values, lengths, *length, *source, false) {
            return Err(invalid());
        }
        let element_byte_offset =
            element_leaf_offset(structural_types, view_type, path, result.scalar_type)
                .ok_or_else(invalid)?;
        let Some(KnownScalar::Integer { scalar_type, value }) = values.get(index) else {
            return Err(invalid());
        };
        if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
            return Err(invalid());
        }
        Ok(
            target_operations::TargetScalarExpression::IeeeFloatElementViewRead {
                format,
                psi_operation: *psi_operation,
                source_value: result.value,
                source: *source,
                view: Box::new(view),
                index: Box::new(value.clone().into_expression(*index)),
                length: *length,
                obligation: *obligation,
                path: path.clone(),
                element_byte_offset,
            },
        )
    }

    fn element_leaf_offset(
        structural_types: &StructuralTypeLookup<'_>,
        view_structural_type: StructuralTypeId,
        path: &[StructuralPathSegment],
        scalar: ScalarType,
    ) -> Option<u32> {
        let StructuralTypeDeclaration {
            shape: StructuralTypeShape::ElementView { element, .. },
            ..
        } = structural_types.get(view_structural_type)?
        else {
            return None;
        };
        scalar_leaf_offset(structural_types, *element, path, scalar)
    }

    fn scalar_leaf_offset(
        structural_types: &StructuralTypeLookup<'_>,
        element: StructuralTypeId,
        path: &[StructuralPathSegment],
        scalar: ScalarType,
    ) -> Option<u32> {
        if terminal_semantics::element_view_leaf_scalar(
            structural_types.catalog().as_slice(),
            element,
            path,
        ) != Some(scalar)
        {
            return None;
        }
        if path.is_empty() {
            return Some(0);
        }
        if let Some((StructuralPathSegment::Field(identity), prefix)) = path.split_last() {
            let (parent, parent_offset) = if prefix.is_empty() {
                (element, 0)
            } else {
                let (parent, _, offset, _) =
                    runtime_projection(element, prefix, structural_types).ok()?;
                (parent, offset)
            };
            let StructuralTypeShape::Record { fields, .. } = &structural_types.get(parent)?.shape
            else {
                return None;
            };
            let field = terminal_psi::field_by_identity(fields, *identity)?;
            if field.field_type.scalar_type() == Some(scalar) {
                return parent_offset.checked_add(
                    direct_scalar_field_geometry(parent, field.id, scalar, structural_types)
                        .ok()?
                        .0,
                );
            }
        }
        runtime_projection(element, path, structural_types)
            .ok()
            .map(|(_, _, offset, _)| offset)
    }

    pub(crate) fn resolve_element_view(
        function: &AbstractFunction,
        structural_types: &StructuralTypeLookup<'_>,
        parameters: &[TargetStructuralParameter],
        backings: ElementViewBackings<'_>,
        values: &BTreeMap<ValueId, KnownScalar>,
        lengths: &BTreeMap<ValueId, PlaceId>,
        place: PlaceId,
        active: &mut [PlaceId],
    ) -> Result<(TargetElementView, StructuralTypeId, StructuralAccess), LoweringError> {
        let invalid = || {
            refusal(
                function,
                &(place, backings.homes.get(&place), values, lengths),
            )
        };
        if active.contains(&place) {
            return Err(invalid());
        }
        if let Some((psi_operation, result)) =
            super::byte_views::descriptor_result(function, place, structural_types, backings.homes)
            && matches!(
                structural_types
                    .get(result.structural_type)
                    .map(|row| &row.shape),
                Some(StructuralTypeShape::ElementView { .. })
            )
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
                TargetElementView::Result {
                    psi_operation,
                    place,
                    structural_type: result.structural_type,
                },
                result.structural_type,
                result_access(structural_types.values().copied(), result).ok_or_else(invalid)?,
            ));
        }
        if let Some(AbstractOperation::EstablishElementView {
            psi_operation,
            result,
            source,
            element,
            region,
            ..
        }) = function.operations.iter().find(|operation| {
            matches!(operation,
            AbstractOperation::EstablishElementView { destination, .. } if *destination == place)
        }) {
            let expected_multiplicity = match source.access {
                StructuralAccess::MutableBorrow => StructuralMultiplicity::Affine,
                _ => StructuralMultiplicity::Unrestricted,
            };
            if result.place != place
                || result.multiplicity != expected_multiplicity
                || !result.qualifications.is_empty()
                || result
                    .projected_qualifications
                    .iter()
                    .any(|row| !row.is_element_membership())
                || !result.claims.is_empty()
                || !structural_types
                    .get(result.structural_type)
                    .is_some_and(|declaration| match &declaration.shape {
                        StructuralTypeShape::ElementView {
                            element: declared, ..
                        } => declared == element,
                        StructuralTypeShape::ByteSequence(_) => declaration
                            .shape
                            .is_borrowed_byte_carrier()
                            && structural_types
                                .get(*element)
                                .is_some_and(|element_declaration| {
                                    matches!(
                                        element_declaration.shape,
                                        StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                                            ref integer
                                        )) if *integer
                                            == IntegerType::new(IntegerSign::Unsigned, 8)
                                                .expect("u8 is a valid integer type")
                                    )
                                }),
                        _ => false,
                    })
            {
                return Err(invalid());
            }
            let (
                root_structural_type,
                source_byte_offset,
                indices,
                extent,
                element_stride,
                source_home,
            ) = establishment::source(
                function,
                structural_types,
                parameters,
                backings,
                source,
                *element,
                *region,
            )?;
            let region = match region {
                terminal_psi::ElementViewRegion::WholeCollection => {
                    target_operations::TargetElementViewRegion::WholeCollection
                }
                terminal_psi::ElementViewRegion::WholeRepresentation {
                    source_length,
                    obligation,
                } => {
                    unsigned_u64_endpoint(values, &invalid)(*source_length)?;
                    target_operations::TargetElementViewRegion::WholeRepresentation {
                        source_length: *source_length,
                        obligation: *obligation,
                    }
                }
                terminal_psi::ElementViewRegion::RepresentationBytes {
                    start,
                    end,
                    obligation,
                } => {
                    let endpoint = unsigned_u64_endpoint(values, &invalid);
                    target_operations::TargetElementViewRegion::RepresentationBytes {
                        start: endpoint(*start)?,
                        end: endpoint(*end)?,
                        obligation: *obligation,
                    }
                }
            };
            return Ok((
                TargetElementView::Established {
                    psi_operation: *psi_operation,
                    place,
                    structural_type: result.structural_type,
                    element: *element,
                    root_structural_type,
                    source_byte_offset,
                    indices,
                    extent,
                    region,
                    element_stride,
                    source: source_home,
                },
                result.structural_type,
                source.access,
            ));
        }
        if let Some((entry, parameter)) = function.block_entries.iter().find_map(|entry| {
            entry
                .structural_parameter(place)
                .map(|parameter| (entry, parameter))
        }) {
            let access = descriptor_parameter_access(parameter, structural_types.values().copied())
                .ok_or_else(invalid)?;
            if entry.block == function.entry
                || !structural_types
                    .get(parameter.structural_type)
                    .is_some_and(|row| matches!(row.shape, StructuralTypeShape::ElementView { .. }))
            {
                return Err(invalid());
            }
            return Ok((
                TargetElementView::BlockParameter {
                    block: entry.block,
                    place,
                    structural_type: parameter.structural_type,
                },
                parameter.structural_type,
                access,
            ));
        }
        if let Some(semantic) = function.structural_parameter(place) {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.place == place)
                .ok_or_else(invalid)?;
            let access = descriptor_parameter_access(semantic, structural_types.values().copied())
                .ok_or_else(invalid)?;
            let referent = crate::structural_layout::structural_shape(
                semantic.structural_type,
                structural_types,
            )?;
            let expected_shape =
                crate::structural_signature::StructuralCallSignature::parameter_shape(
                    crate::structural_signature::ParameterLane::Internal,
                    semantic.structural_type,
                    referent,
                    semantic.access,
                    structural_types,
                );
            if !structural_types
                .get(semantic.structural_type)
                .is_some_and(|row| matches!(row.shape, StructuralTypeShape::ElementView { .. }))
                || parameter.structural_type != semantic.structural_type
                || parameter.access != semantic.access
                || parameter.multiplicity != semantic.multiplicity
                || parameter.projected_qualifications != semantic.projected_qualifications
                || parameter.shape != expected_shape
            {
                return Err(invalid());
            }
            return Ok((
                TargetElementView::Parameter {
                    place,
                    placement: parameter.placement.clone(),
                },
                semantic.structural_type,
                access,
            ));
        }
        let Some(AbstractOperation::ElementViewSubslice {
            psi_operation,
            result,
            source,
            start,
            end,
            length,
            obligation,
        }) = function.operations.iter().find(|operation| {
            matches!(operation, AbstractOperation::ElementViewSubslice { result, .. }
            if result.place == place)
        })
        else {
            return Err(invalid());
        };
        if !matches!(
            result.multiplicity,
            terminal_psi::nonlinear_multiplicity_pair_pattern!()
        ) || !result.qualifications.is_empty()
            || result
                .projected_qualifications
                .iter()
                .any(|row| !row.is_element_membership())
            || !result.claims.is_empty()
            || !structural_types
                .get(result.structural_type)
                .is_some_and(|declaration| {
                    matches!(declaration.shape, StructuralTypeShape::ElementView { .. })
                })
        {
            return Err(invalid());
        }
        let endpoint = unsigned_u64_endpoint(values, &invalid);
        let start = endpoint(*start)?;
        let end = endpoint(*end)?;
        if !super::descriptor_operands::observes_count(function, values, *length, source, false) {
            return Err(invalid());
        }
        let (source_type, indices) = super::descriptor_operands::descriptor_projection(
            function,
            structural_types,
            backings,
            source,
        )?;
        let source_access = source.access;
        let access = match result.multiplicity {
            StructuralMultiplicity::Unrestricted => StructuralAccess::SharedBorrow,
            StructuralMultiplicity::Affine if source_access == StructuralAccess::MutableBorrow => {
                StructuralAccess::MutableBorrow
            }
            _ => return Err(invalid()),
        };
        if !structural_types
            .get(source_type)
            .zip(structural_types.get(result.structural_type))
            .is_some_and(|(source, result)| {
                matches!((&source.shape, &result.shape),
            (StructuralTypeShape::ElementView { element: source, .. },
             StructuralTypeShape::ElementView { element: result, access: ceiling })
                if source == result && ceiling.is_none_or(|ceiling| ceiling == access))
            })
        {
            return Err(invalid());
        }
        Ok((
            TargetElementView::Subslice {
                psi_operation: *psi_operation,
                place,
                source: source.clone(),
                indices,
                start,
                end,
                length: *length,
                obligation: *obligation,
            },
            result.structural_type,
            access,
        ))
    }

    fn live_aggregate_backing(
        homes: &BTreeMap<PlaceId, TargetStructuralHomeRequirement>,
        place: PlaceId,
    ) -> Option<(StructuralTypeId, TargetStructuralArgumentSource)> {
        let home = homes.get(&place)?;
        if home.place() != place
            || home.access() != StructuralAccess::Owned
            || !matches!(
                home.multiplicity(),
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            )
            || home.has_claims()
        {
            return None;
        }
        let source = match &home.origin {
            TargetStructuralHomeOrigin::OperationResult { operation, .. } => {
                TargetStructuralArgumentSource::StructuralHome {
                    psi_operation: *operation,
                }
            }
            TargetStructuralHomeOrigin::BlockParameter { block, declaration } => {
                TargetStructuralArgumentSource::BlockParameter {
                    block: *block,
                    place: declaration.place,
                }
            }
        };
        Some((home.structural_type(), source))
    }
}
mod expressions {
    use crate::LoweringError;
    use abstract_operations::AbstractParameter;
    use calling_conventions::{ValueLocation, ValuePlacement, ValueShape};
    use semantic_vocabulary::{
        IntegerSign, IntegerType, IntegerValue, OperationId, ScalarType, ValueId,
    };
    use std::collections::BTreeMap;
    use target_operations::{
        ScalarParameterLocation, TargetBooleanExpression, TargetIntegerExpression,
        TargetScalarExpression,
    };

    pub(crate) fn scalar_shape(
        value: ValueId,
        scalar_type: ScalarType,
        require_native_parameter: bool,
    ) -> Result<ValueShape, LoweringError> {
        let bytes = match scalar_type {
            ScalarType::Callable(_) => 8,
            ScalarType::Boolean => 1,
            ScalarType::Integer(integer_type) => {
                let bits = integer_type.bits();
                if require_native_parameter && !matches!(bits, 8 | 16 | 32 | 64) {
                    return Err(LoweringError::ParameterWidthNotNativelySupported { value, bits });
                }
                bits.div_ceil(8)
            }
            ScalarType::IeeeFloat(format) => {
                return Ok(ValueShape::float(format.byte_size()));
            }
        };
        Ok(ValueShape::integer(
            u32::from(bytes),
            bytes.next_power_of_two().min(8),
        ))
    }

    pub(crate) fn scalar_parameter_location(
        parameter: &AbstractParameter,
        placement: &ValuePlacement,
    ) -> Result<ScalarParameterLocation, LoweringError> {
        let expected_bytes = scalar_shape(parameter.value, parameter.scalar_type, true)?.byte_size;
        match placement.locations.as_slice() {
            [
                ValueLocation::Register {
                    register,
                    value_byte_offset: 0,
                    byte_size,
                },
            ] if *byte_size == expected_bytes => Ok(ScalarParameterLocation::Register(*register)),
            [
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset: 0,
                    byte_size,
                    ..
                },
            ] if *byte_size == expected_bytes => Ok(ScalarParameterLocation::IncomingStack {
                byte_offset: *stack_byte_offset,
            }),
            _ => Err(LoweringError::UnsupportedScalarParameterPlacement(
                parameter.value,
            )),
        }
    }

    pub(crate) fn insert_value(
        values: &mut BTreeMap<ValueId, KnownScalar>,
        id: ValueId,
        value: KnownScalar,
    ) -> Result<(), LoweringError> {
        if values.insert(id, value).is_some() {
            return Err(LoweringError::DuplicateValue(id));
        }
        Ok(())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum KnownScalar {
        Boolean(bool),
        BooleanRuntime(TargetBooleanExpression),
        Integer {
            scalar_type: IntegerType,
            value: KnownInteger,
        },
    }

    impl KnownScalar {
        pub(crate) fn into_expression(
            self,
            source_value: ValueId,
        ) -> Result<TargetScalarExpression, LoweringError> {
            Ok(match self {
                Self::Boolean(value) => {
                    TargetScalarExpression::Boolean(TargetBooleanExpression::Immediate {
                        source_value,
                        value,
                    })
                }
                Self::BooleanRuntime(expression) => TargetScalarExpression::Boolean(expression),
                Self::Integer { scalar_type, value } => TargetScalarExpression::Integer {
                    scalar_type,
                    expression: value.into_expression(source_value),
                },
            })
        }
    }

    pub(crate) fn negate_boolean(
        value: KnownScalar,
        psi_operation: OperationId,
        result: ValueId,
    ) -> Result<KnownScalar, LoweringError> {
        match value {
            KnownScalar::Boolean(value) => Ok(KnownScalar::Boolean(!value)),
            KnownScalar::BooleanRuntime(TargetBooleanExpression::Not { operand, .. }) => {
                Ok(KnownScalar::BooleanRuntime(*operand))
            }
            KnownScalar::BooleanRuntime(expression) => {
                Ok(KnownScalar::BooleanRuntime(TargetBooleanExpression::Not {
                    psi_operation,
                    operand: Box::new(expression),
                }))
            }
            KnownScalar::Integer { .. } => Err(LoweringError::ValueTypeMismatch(result)),
        }
    }

    pub(crate) fn equal_boolean(
        left: KnownScalar,
        right: KnownScalar,
        psi_operation: OperationId,
        result: ValueId,
    ) -> Result<KnownScalar, LoweringError> {
        match (left, right) {
            (KnownScalar::Boolean(left), KnownScalar::Boolean(right)) => {
                Ok(KnownScalar::Boolean(left == right))
            }
            (value, KnownScalar::Boolean(true)) | (KnownScalar::Boolean(true), value) => Ok(value),
            (value, KnownScalar::Boolean(false)) | (KnownScalar::Boolean(false), value) => {
                negate_boolean(value, psi_operation, result)
            }
            (KnownScalar::BooleanRuntime(left), KnownScalar::BooleanRuntime(right)) => Ok(
                KnownScalar::BooleanRuntime(TargetBooleanExpression::Equal {
                    psi_operation,
                    left: Box::new(left),
                    right: Box::new(right),
                }),
            ),
            _ => Err(LoweringError::ValueTypeMismatch(result)),
        }
    }

    fn integer_pair(
        left: KnownScalar,
        right: KnownScalar,
        result: ValueId,
    ) -> Result<(IntegerType, KnownInteger, IntegerType, KnownInteger), LoweringError> {
        let (
            KnownScalar::Integer {
                scalar_type: left_type,
                value: left,
            },
            KnownScalar::Integer {
                scalar_type: right_type,
                value: right,
            },
        ) = (left, right)
        else {
            return Err(LoweringError::ValueTypeMismatch(result));
        };
        if left_type != right_type && (left_type.is_address() || right_type.is_address()) {
            return Err(LoweringError::ValueTypeMismatch(result));
        }
        Ok((left_type, left, right_type, right))
    }

    pub(crate) fn equal_integer(
        left_id: ValueId,
        left: KnownScalar,
        right_id: ValueId,
        right: KnownScalar,
        psi_operation: OperationId,
        result: ValueId,
    ) -> Result<KnownScalar, LoweringError> {
        let (left_type, left, right_type, right) = integer_pair(left, right, result)?;
        match (left, right) {
            (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                Ok(KnownScalar::Boolean(
                    left_type
                        .compare_with(left, right_type, right)
                        .ok_or(LoweringError::ValueTypeMismatch(result))?
                        .is_eq(),
                ))
            }
            (left, right) => Ok(KnownScalar::BooleanRuntime(
                TargetBooleanExpression::IntegerEqual {
                    psi_operation,
                    scalar_type: left_type,
                    right_scalar_type: right_type,
                    left: Box::new(left.into_expression(left_id)),
                    right: Box::new(right.into_expression(right_id)),
                },
            )),
        }
    }

    pub(crate) fn order_integer(
        left_id: ValueId,
        left: KnownScalar,
        right_id: ValueId,
        right: KnownScalar,
        psi_operation: OperationId,
        result: ValueId,
        inclusive: bool,
    ) -> Result<KnownScalar, LoweringError> {
        let (left_type, left, right_type, right) = integer_pair(left, right, result)?;
        match (left, right) {
            (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                let ordering = left_type
                    .compare_with(left, right_type, right)
                    .ok_or(LoweringError::ValueTypeMismatch(result))?;
                Ok(KnownScalar::Boolean(if inclusive {
                    !ordering.is_gt()
                } else {
                    ordering.is_lt()
                }))
            }
            (left, right) => {
                let left = Box::new(left.into_expression(left_id));
                let right = Box::new(right.into_expression(right_id));
                Ok(KnownScalar::BooleanRuntime(if inclusive {
                    TargetBooleanExpression::IntegerLessOrEqual {
                        psi_operation,
                        scalar_type: left_type,
                        right_scalar_type: right_type,
                        left,
                        right,
                    }
                } else {
                    TargetBooleanExpression::IntegerLessThan {
                        psi_operation,
                        scalar_type: left_type,
                        right_scalar_type: right_type,
                        left,
                        right,
                    }
                }))
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum KnownInteger {
        Immediate(IntegerValue),
        Runtime(TargetIntegerExpression),
    }

    impl KnownInteger {
        pub(crate) fn into_expression(self, source_value: ValueId) -> TargetIntegerExpression {
            match self {
                Self::Immediate(value) => TargetIntegerExpression::Immediate {
                    source_value,
                    value,
                },
                Self::Runtime(expression) => expression,
            }
        }
    }

    pub(crate) fn unsigned_u64_endpoint<'a, E>(
        values: &'a BTreeMap<ValueId, KnownScalar>,
        invalid: impl Fn() -> E + 'a,
    ) -> impl Fn(ValueId) -> Result<Box<TargetIntegerExpression>, E> + 'a {
        move |value| {
            let Some(KnownScalar::Integer {
                scalar_type,
                value: known,
            }) = values.get(&value)
            else {
                return Err(invalid());
            };
            if scalar_type.sign() != IntegerSign::Unsigned || scalar_type.bits() != 64 {
                return Err(invalid());
            }
            Ok(Box::new(known.clone().into_expression(value)))
        }
    }
}
mod integer_binary {
    use super::{KnownInteger, KnownScalar};
    use crate::LoweringError;
    use abstract_operations::{
        IntegerArithmetic, IntegerArithmeticOperator, IntegerOverflowPolicy,
    };
    use semantic_vocabulary::{IntegerType, IntegerValue, ValueId};
    use std::collections::BTreeMap;
    use target_operations::TargetIntegerExpression;
    #[derive(Clone, Copy)]
    pub(crate) enum IntegerBinaryKind {
        BitwiseAnd,
        BitwiseOr,
        BitwiseXor,
        Arithmetic(IntegerArithmetic),
    }
    pub(crate) fn lower_integer_binary(
        values: &BTreeMap<ValueId, KnownScalar>,
        result: ValueId,
        scalar_type: IntegerType,
        left_id: ValueId,
        right_id: ValueId,
        kind: IntegerBinaryKind,
        psi_operation: semantic_vocabulary::OperationId,
    ) -> Result<KnownInteger, LoweringError> {
        let operand = |id| match values.get(&id).cloned() {
            Some(KnownScalar::Integer {
                scalar_type: operand_type,
                value,
            }) => Ok((operand_type, value)),
            Some(_) => Err(kind.mismatch(result)),
            None => Err(LoweringError::UnknownValue(id)),
        };
        let (left_type, left) = operand(left_id)?;
        let (right_type, right) = operand(right_id)?;
        let operands_match = if matches!(kind, IntegerBinaryKind::Arithmetic(arithmetic)
        if arithmetic.is(IntegerOverflowPolicy::Exact, IntegerArithmeticOperator::Add))
        {
            scalar_type.admits_exact_add_operands(left_type, right_type)
        } else if matches!(kind, IntegerBinaryKind::Arithmetic(arithmetic)
        if arithmetic.is(IntegerOverflowPolicy::Exact, IntegerArithmeticOperator::Subtract))
        {
            scalar_type.admits_exact_subtract_operands(left_type, right_type)
        } else {
            left_type == scalar_type && right_type == scalar_type
        };
        if !operands_match {
            return Err(kind.mismatch(result));
        }
        if kind.is_proof_bearing() {
            return Ok(KnownInteger::Runtime(kind.expression(
                psi_operation,
                left.into_expression(left_id),
                right.into_expression(right_id),
            )));
        }
        Ok(match (left, right) {
            (KnownInteger::Immediate(left), KnownInteger::Immediate(right)) => {
                KnownInteger::Immediate(
                    kind.fold(scalar_type, left, right)
                        .ok_or(kind.mismatch(result))?,
                )
            }
            (left, right) => KnownInteger::Runtime(kind.expression(
                psi_operation,
                left.into_expression(left_id),
                right.into_expression(right_id),
            )),
        })
    }

    impl IntegerBinaryKind {
        fn mismatch(self, result: ValueId) -> LoweringError {
            use IntegerArithmeticOperator as Operator;
            use IntegerOverflowPolicy as Policy;
            let arithmetic = match self {
                Self::BitwiseAnd | Self::BitwiseOr | Self::BitwiseXor => {
                    return LoweringError::IntegerBitwiseOperandTypeMismatch(result);
                }
                Self::Arithmetic(arithmetic) => arithmetic,
            };
            match (arithmetic.policy(), arithmetic.operator()) {
                (Policy::Wrapping | Policy::Exact, Operator::Add) => {
                    LoweringError::WrappingAddOperandTypeMismatch(result)
                }
                (Policy::Saturating, Operator::Add) => {
                    LoweringError::SaturatingAddOperandTypeMismatch(result)
                }
                (Policy::Wrapping | Policy::Exact, Operator::Subtract) => {
                    LoweringError::WrappingSubtractOperandTypeMismatch(result)
                }
                (Policy::Saturating, Operator::Subtract) => {
                    LoweringError::SaturatingSubtractOperandTypeMismatch(result)
                }
                (Policy::Wrapping | Policy::Exact, Operator::Multiply) => {
                    LoweringError::WrappingMultiplyOperandTypeMismatch(result)
                }
                (Policy::Saturating, Operator::Multiply) => {
                    LoweringError::SaturatingMultiplyOperandTypeMismatch(result)
                }
                (Policy::Exact, Operator::Divide) => {
                    LoweringError::ExactDivideOperandTypeMismatch(result)
                }
                (Policy::Exact, Operator::Remainder) => {
                    LoweringError::ExactRemainderOperandTypeMismatch(result)
                }
                (Policy::Wrapping, Operator::Divide) => {
                    LoweringError::WrappingDivideOperandTypeMismatch(result)
                }
                (Policy::Wrapping, Operator::Remainder) => {
                    LoweringError::WrappingRemainderOperandTypeMismatch(result)
                }
                (Policy::Saturating, Operator::Divide) => {
                    LoweringError::SaturatingDivideOperandTypeMismatch(result)
                }
                (Policy::Saturating, Operator::Remainder) => {
                    LoweringError::SaturatingRemainderOperandTypeMismatch(result)
                }
            }
        }

        fn fold(
            self,
            scalar_type: IntegerType,
            left: IntegerValue,
            right: IntegerValue,
        ) -> Option<IntegerValue> {
            match self {
                Self::BitwiseAnd => scalar_type.bitwise_and(left, right),
                Self::BitwiseOr => scalar_type.bitwise_or(left, right),
                Self::BitwiseXor => scalar_type.bitwise_xor(left, right),
                Self::Arithmetic(arithmetic) => arithmetic.evaluate(scalar_type, left, right),
            }
        }

        fn expression(
            self,
            psi_operation: semantic_vocabulary::OperationId,
            left: TargetIntegerExpression,
            right: TargetIntegerExpression,
        ) -> TargetIntegerExpression {
            let left = Box::new(left);
            let right = Box::new(right);
            match self {
                Self::BitwiseAnd => TargetIntegerExpression::BitwiseAnd {
                    psi_operation,
                    left,
                    right,
                },
                Self::BitwiseOr => TargetIntegerExpression::BitwiseOr {
                    psi_operation,
                    left,
                    right,
                },
                Self::BitwiseXor => TargetIntegerExpression::BitwiseXor {
                    psi_operation,
                    left,
                    right,
                },
                Self::Arithmetic(arithmetic) => TargetIntegerExpression::Arithmetic {
                    psi_operation,
                    arithmetic,
                    left,
                    right,
                },
            }
        }

        fn is_proof_bearing(self) -> bool {
            match self {
                Self::BitwiseAnd | Self::BitwiseOr | Self::BitwiseXor => false,
                Self::Arithmetic(arithmetic) => arithmetic.obligation().is_some(),
            }
        }
    }
}
mod integer_operation {
    use super::{IntegerBinaryKind, KnownInteger, KnownScalar, lower_integer_binary, lower_shift};
    use crate::LoweringError;
    use crate::scalar::expressions::insert_value;
    use abstract_operations::AbstractOperation;
    use semantic_vocabulary::ValueId;
    use std::collections::BTreeMap;
    use target_operations::TargetIntegerExpression;
    pub(crate) fn try_lower_integer_operation(
        operation: &AbstractOperation,
        values: &mut BTreeMap<ValueId, KnownScalar>,
        provenance: &mut Vec<semantic_vocabulary::OperationId>,
    ) -> Result<bool, LoweringError> {
        let (psi_operation, result, scalar_type, value) = match operation {
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } => {
                let Some(integer_type) = scalar_type.as_integer() else {
                    return Err(LoweringError::IntegerConstantHasNonIntegerType(*result));
                };
                if !integer_type.admits(*value) {
                    return Err(LoweringError::IntegerConstantOutsideType(*result));
                }
                (
                    *psi_operation,
                    *result,
                    integer_type,
                    KnownInteger::Immediate(*value),
                )
            }
            AbstractOperation::IntegerArithmetic {
                psi_operation,
                result,
                scalar_type,
                arithmetic,
                left,
                right,
            } => (
                *psi_operation,
                *result,
                *scalar_type,
                lower_integer_binary(
                    values,
                    *result,
                    *scalar_type,
                    *left,
                    *right,
                    IntegerBinaryKind::Arithmetic(*arithmetic),
                    *psi_operation,
                )?,
            ),
            AbstractOperation::IntegerBitwiseNot {
                psi_operation,
                result,
                scalar_type,
                operand,
            } => {
                let operand_value = match values.get(operand).cloned() {
                    Some(KnownScalar::Integer {
                        scalar_type: operand_type,
                        value,
                    }) if operand_type == *scalar_type => value,
                    Some(_) => {
                        return Err(LoweringError::IntegerBitwiseOperandTypeMismatch(*result));
                    }
                    None => return Err(LoweringError::UnknownValue(*operand)),
                };
                let value = match operand_value {
                    KnownInteger::Immediate(value) => KnownInteger::Immediate(
                        scalar_type
                            .bitwise_not(value)
                            .ok_or(LoweringError::IntegerBitwiseOperandTypeMismatch(*result))?,
                    ),
                    KnownInteger::Runtime(expression) => {
                        KnownInteger::Runtime(TargetIntegerExpression::BitwiseNot {
                            psi_operation: *psi_operation,
                            operand: Box::new(expression),
                        })
                    }
                };
                (*psi_operation, *result, *scalar_type, value)
            }
            AbstractOperation::IntegerWiden {
                psi_operation,
                result,
                source_type,
                target_type,
                operand,
            } => {
                let operand_value = match values.get(operand).cloned() {
                    Some(KnownScalar::Integer {
                        scalar_type: operand_type,
                        value,
                    }) if operand_type == *source_type
                        && source_type.can_widen_to(*target_type) =>
                    {
                        value
                    }
                    Some(_) => return Err(LoweringError::IntegerWidenTypeMismatch(*result)),
                    None => return Err(LoweringError::UnknownValue(*operand)),
                };
                let value = match operand_value {
                    KnownInteger::Immediate(value) => KnownInteger::Immediate(
                        source_type
                            .widen_value_to(*target_type, value)
                            .ok_or(LoweringError::IntegerWidenTypeMismatch(*result))?,
                    ),
                    KnownInteger::Runtime(expression) => {
                        KnownInteger::Runtime(TargetIntegerExpression::IntegerWiden {
                            psi_operation: *psi_operation,
                            source_type: *source_type,
                            operand: Box::new(expression),
                        })
                    }
                };
                (*psi_operation, *result, *target_type, value)
            }
            AbstractOperation::IntegerExactCast {
                psi_operation,
                obligation,
                result,
                source_type,
                target_type,
                operand,
            } => {
                let operand_value = match values.get(operand).cloned() {
                    Some(KnownScalar::Integer {
                        scalar_type: operand_type,
                        value,
                    }) if operand_type == *source_type
                        && source_type.can_exact_cast_to(*target_type) =>
                    {
                        value
                    }
                    Some(_) => return Err(LoweringError::IntegerExactCastTypeMismatch(*result)),
                    None => return Err(LoweringError::UnknownValue(*operand)),
                };
                let value = KnownInteger::Runtime(TargetIntegerExpression::IntegerExactCast {
                    psi_operation: *psi_operation,
                    obligation: *obligation,
                    source_type: *source_type,
                    operand: Box::new(operand_value.into_expression(*operand)),
                });
                (*psi_operation, *result, *target_type, value)
            }
            AbstractOperation::IntegerBitwiseAnd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            }
            | abstract_operations::bitwise_or_xor_pair_pattern!(
                psi_operation,
                result,
                scalar_type,
                left,
                right,
                ..
            ) => {
                let kind = match operation {
                    AbstractOperation::IntegerBitwiseAnd { .. } => IntegerBinaryKind::BitwiseAnd,
                    AbstractOperation::IntegerBitwiseOr { .. } => IntegerBinaryKind::BitwiseOr,
                    AbstractOperation::IntegerBitwiseXor { .. } => IntegerBinaryKind::BitwiseXor,
                    _ => unreachable!(),
                };
                (
                    *psi_operation,
                    *result,
                    *scalar_type,
                    lower_integer_binary(
                        values,
                        *result,
                        *scalar_type,
                        *left,
                        *right,
                        kind,
                        *psi_operation,
                    )?,
                )
            }
            AbstractOperation::IntegerShift {
                psi_operation,
                result,
                value_type,
                count_type,
                policy,
                direction,
                value,
                count,
            } => (
                *psi_operation,
                *result,
                *value_type,
                lower_shift(
                    values,
                    *result,
                    *value_type,
                    *count_type,
                    *value,
                    *count,
                    *policy,
                    *direction,
                    *psi_operation,
                )?,
            ),
            _ => return Ok(false),
        };
        insert_value(values, result, KnownScalar::Integer { scalar_type, value })?;
        provenance.push(psi_operation);
        Ok(true)
    }
}
mod shift {
    use super::{KnownInteger, KnownScalar};
    use crate::LoweringError;
    use abstract_operations::{IntegerShiftDirection, IntegerShiftPolicy};
    use semantic_vocabulary::{IntegerType, ValueId};
    use std::collections::BTreeMap;
    use target_operations::TargetIntegerExpression;

    #[allow(clippy::too_many_arguments)]
    pub(in crate::scalar) fn lower_shift(
        values: &BTreeMap<ValueId, KnownScalar>,
        result: ValueId,
        value_type: IntegerType,
        count_type: IntegerType,
        value_id: ValueId,
        count_id: ValueId,
        policy: IntegerShiftPolicy,
        direction: IntegerShiftDirection,
        psi_operation: semantic_vocabulary::OperationId,
    ) -> Result<KnownInteger, LoweringError> {
        let mismatch = match policy {
            IntegerShiftPolicy::Wrapping => LoweringError::WrappingShiftOperandTypeMismatch(result),
            IntegerShiftPolicy::Exact { .. } => {
                LoweringError::ExactShiftOperandTypeMismatch(result)
            }
        };
        let operand = |id, expected_type| match values.get(&id).cloned() {
            Some(KnownScalar::Integer { scalar_type, value }) if scalar_type == expected_type => {
                Ok(value)
            }
            Some(_) => Err(mismatch.clone()),
            None => Err(LoweringError::UnknownValue(id)),
        };
        let value = operand(value_id, value_type)?;
        let count = operand(count_id, count_type)?;
        if policy == IntegerShiftPolicy::Wrapping
            && let (
                KnownInteger::Immediate(value_constant),
                KnownInteger::Immediate(count_constant),
            ) = (&value, &count)
        {
            let shifted = policy
                .evaluate(
                    direction,
                    value_type,
                    *value_constant,
                    count_type,
                    *count_constant,
                )
                .ok_or(mismatch)?;
            return Ok(KnownInteger::Immediate(shifted));
        }
        Ok(KnownInteger::Runtime(TargetIntegerExpression::Shift {
            psi_operation,
            policy,
            direction,
            count_type,
            value: Box::new(value.into_expression(value_id)),
            count: Box::new(count.into_expression(count_id)),
        }))
    }
}

pub(crate) use expressions::{
    KnownInteger, KnownScalar, equal_boolean, equal_integer, order_integer,
    scalar_parameter_location, scalar_shape,
};
pub(crate) use integer_binary::{IntegerBinaryKind, lower_integer_binary};
pub(crate) use integer_operation::try_lower_integer_operation;
use shift::lower_shift;
