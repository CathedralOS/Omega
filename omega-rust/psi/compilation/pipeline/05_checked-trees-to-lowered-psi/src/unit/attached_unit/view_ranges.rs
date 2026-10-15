//! Resolve range storage, establish a descriptor when needed, and narrow it.
//!
//! Arrays have backing but no descriptor; views already have one. That type
//! distinction, not the source's local/parameter origin or projection length,
//! decides whether EstablishElementView precedes narrowing. Reservation never
//! evaluates an operand: each consumer emits the preparation at the range's
//! authored position. Terminal loan/bounds verification remains independent of
//! this choice.
//!
//! Inclusive and exclusive ranges retain the same authored endpoint identities.
//! Normalize an inclusive end only after capturing its value: a rewritten source
//! expression would lose that identity and could repeat a call's effects. Both
//! schedules then narrow the same half-open descriptor with exact arithmetic.
use super::{CheckedTrees, LoweringError, unsupported};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::{
    LoweredDirectExpression, LoweredElementPathSegment, TERMINAL_NESTED_SELECTOR_LIMIT,
    emit_prepared_path, path_with_current_bounds,
};
use crate::emission::operation_emission::view_subslice::{self, ViewFamily};
use crate::expression_preparation::bindings::ScalarBindings;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{
    CheckedCallScalarArgument, CheckedScalarExpressionRole, CheckedSubsliceSite,
    CheckedUnitStructuralPathSegment,
};
use checked_trees::{
    CheckedStorageRoot, CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use semantic_vocabulary::{PlaceId, StructuralTypeId};
use terminal_psi::{StructuralPlaceDeclaration, ValueDeclaration};

/// One range argument as its checked plan retains it.
#[derive(Clone, Copy)]
pub(crate) struct Range<'a> {
    pub(crate) root: CheckedStorageRoot,
    pub(crate) family: ViewFamily,
    pub(crate) expression: ExpressionHandle,
    pub(crate) start: &'a Option<CheckedCallScalarArgument>,
    pub(crate) end: &'a Option<CheckedCallScalarArgument>,
    pub(crate) end_inclusive: bool,
    /// The occurrence whose endpoint facts `start` and `end` came from.
    pub(crate) site: CheckedSubsliceSite,
}

/// The range `argument` narrows, of either view family.
pub(crate) fn range(argument: &CheckedUnitStructuralArgumentPlan) -> Option<Range<'_>> {
    let (root, family, expression, start, end, end_inclusive, site) = match &argument.source {
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root,
            expression,
            start,
            end,
            end_inclusive,
            site,
            storage_type_identity: _,
        } => (
            *root,
            ViewFamily::Bytes,
            *expression,
            start,
            end,
            *end_inclusive,
            *site,
        ),
        CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root,
            expression,
            start,
            end,
            end_inclusive,
            site,
            storage_type_identity: _,
        } => (
            *root,
            ViewFamily::Elements,
            *expression,
            start,
            end,
            *end_inclusive,
            *site,
        ),
        _ => return None,
    };
    Some(Range {
        root,
        family,
        expression,
        start,
        end,
        end_inclusive,
        site,
    })
}

#[derive(Clone)]
pub(crate) struct RangeStorage {
    symbol: symbols::SymbolHandle,
    collection: Vec<CheckedUnitStructuralPathSegment>,
    structural_type: StructuralTypeId,
    family: ViewFamily,
    collection_element: Option<StructuralTypeId>,
    access: checked_trees::CheckedStructuralAccess,
}

/// One ordinary collection-view operation, shared by scalar and Unit schedules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionView {
    pub(crate) source: PlaceId,
    pub(crate) access: terminal_psi::StructuralAccess,
    pub(crate) path: Vec<LoweredElementPathSegment>,
    pub(crate) element: StructuralTypeId,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) byte_region: Option<(LoweredDirectExpression, LoweredDirectExpression)>,
}

pub(crate) fn element_type(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    structural_type: StructuralTypeId,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<StructuralTypeId, LoweringError> {
    let declaration = declarations
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .ok_or(LoweringError::Unsupported(
            "element view lost its descriptor carrier",
        ))?;
    match declaration.shape {
        terminal_psi::StructuralTypeShape::ElementView { element, .. } => Ok(element),
        terminal_psi::StructuralTypeShape::ByteSequence(_) => {
            let identity = crate::terminal_identities::byte_view_element_identity(
                checked
                    .facts
                    .flow
                    .terminal_unit_effects
                    .structural_types
                    .iter(),
            )
            .ok_or(LoweringError::Unsupported(
                "byte view lost its element identity",
            ))?;
            crate::terminal_identities::structural_declaration_by_identity(
                declarations,
                identities,
                checked,
                identity,
                "byte view lost its element carrier",
            )
            .map(|declaration| declaration.id)
        }
        _ => unsupported("element view has no descriptor carrier"),
    }
}

impl RangeStorage {
    pub(crate) fn requires_establishment(&self) -> bool {
        self.collection_element.is_some()
    }

    /// Bind the range source to the current activation's storage. Array
    /// borrowing reuses the ordinary place join, including its access checks.
    pub(crate) fn bind(
        &self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        bindings: &ScalarBindings,
        destination: Option<PlaceId>,
        declarations: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<(ViewRangeSource, Option<CollectionView>), LoweringError> {
        let (_, authored) = crate::declarations::state_declaration(checked, state)?;
        let argument = checked_trees::CheckedUnitStructuralArgumentPlan {
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                symbol: self.symbol,
            },
            path: self.collection.clone(),
            access: self.access,
            type_identity: Default::default(),
        };
        let source =
            bindings.borrowed_structural_home(&argument, checked.state_parameters(authored))?;
        let root = bindings
            .structural_argument_type(&source)
            .ok_or(LoweringError::Unsupported(
                "range source lost its published storage type",
            ))?;
        let path = source
            .path
            .into_iter()
            .map(LoweredElementPathSegment::Static)
            .chain(collection_path(
                identities,
                &self.collection,
                bindings,
                source.place,
                root,
                declarations,
            )?)
            .collect::<Vec<_>>();
        let (place, path, establishment) = if let Some(element) = self.collection_element {
            let destination = destination.ok_or(LoweringError::Unsupported(
                "collection range has no reserved whole-view descriptor",
            ))?;
            (
                destination,
                Vec::new(),
                Some(CollectionView {
                    source: source.place,
                    access:
                        crate::expression_preparation::bindings::structural_paths::terminal_access(
                            self.access,
                        ),
                    path,
                    element,
                    place: destination,
                    structural_type: self.structural_type,
                    byte_region: None,
                }),
            )
        } else {
            (source.place, path, None)
        };
        Ok((
            ViewRangeSource {
                place,
                path,
                structural_type: self.structural_type,
                family: self.family,
                access: crate::expression_preparation::bindings::structural_paths::terminal_access(
                    self.access,
                ),
            },
            establishment,
        ))
    }
}

/// Bind selectors without observing them. Establishment evaluates the complete
/// projection before range endpoints, under the occurrence's current frame. The
/// same typed argument-path walk other operand lanes take resolves the
/// projection against the root's declared storage, so record members and
/// selected case payloads lower alike.
fn collection_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    path: &[CheckedUnitStructuralPathSegment],
    bindings: &ScalarBindings,
    source: PlaceId,
    root: semantic_vocabulary::StructuralTypeId,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<LoweredElementPathSegment>, LoweringError> {
    use crate::expression_preparation::bindings::structural_paths::{
        PreparedStructuralPathSegment, prepare_argument_path,
    };
    let mut runtime_indices = 0;
    let mut dynamic_extent = false;
    prepare_argument_path(identities, path, root, declarations)?
        .into_iter()
        .map(|segment| match segment {
            PreparedStructuralPathSegment::Static(segment) => {
                if dynamic_extent
                    && let terminal_psi::StructuralPathSegment::FixedIndex(index) = segment
                {
                    dynamic_extent = false;
                    runtime_indices += 1;
                    if runtime_indices > TERMINAL_NESTED_SELECTOR_LIMIT {
                        return unsupported(
                            "collection view exceeds the runtime selector namespace",
                        );
                    }
                    return Ok(LoweredElementPathSegment::RuntimeIndex {
                        selector: LoweredDirectExpression::IntegerLiteral {
                            value: semantic_vocabulary::IntegerValue::Unsigned(u128::from(index)),
                            scalar_type: crate::emission::scalar_types::terminal_scalar_type(
                                checked_trees::types::PrimitiveType::U64,
                            )?,
                        },
                        extent: None,
                        bound_decided: false,
                    });
                }
                Ok(LoweredElementPathSegment::Static(segment))
            }
            PreparedStructuralPathSegment::ViewExtent { bytes } => {
                dynamic_extent = true;
                Ok(LoweredElementPathSegment::ViewExtent { source, bytes })
            }
            PreparedStructuralPathSegment::ComputedIndex(_)
            | PreparedStructuralPathSegment::CapturedIndex(_) => {
                unsupported("computed view selector requires its owning operand evaluation")
            }
            PreparedStructuralPathSegment::RuntimeIndex(index) => {
                dynamic_extent = false;
                runtime_indices += 1;
                if runtime_indices > TERMINAL_NESTED_SELECTOR_LIMIT {
                    return unsupported("collection view exceeds the runtime selector namespace");
                }
                let selector = match index {
                    checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
                        bindings.runtime_index_storage(symbol)?
                    }
                    checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                        bindings.runtime_index_parameter(position)?
                    }
                    checked_trees::CheckedRuntimeIndex::Local { position } => {
                        bindings.runtime_index_local(position)?
                    }
                    checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                        bindings.runtime_index_field(identities, &field)?
                    }
                    _ => {
                        return unsupported("collection view cannot borrow an assignment selector");
                    }
                };
                Ok(LoweredElementPathSegment::RuntimeIndex {
                    selector: crate::expression_preparation::prepare_expression::exact_u64_index(
                        selector,
                    )?,
                    extent: None,
                    bound_decided: false,
                })
            }
        })
        .collect()
}

/// Resolve the collection's declared type before reserving descriptor storage.
/// This does not confer a loan or discharge range bounds.
pub(crate) fn storage(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    _statement: u32,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<RangeStorage, LoweringError> {
    let range = range(argument).ok_or(LoweringError::Unsupported(
        "range storage has no range operation",
    ))?;
    if !matches!(
        argument.access,
        checked_trees::CheckedStructuralAccess::SharedBorrow
            | checked_trees::CheckedStructuralAccess::MutableBorrow
    ) {
        return unsupported("range storage requires a readable loan");
    }
    let (_machine, authored) = crate::declarations::state_declaration(checked, state)?;
    let (checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        storage_type_identity,
        ..
    }
    | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
        storage_type_identity,
        ..
    }) = &argument.source
    else {
        return unsupported("range storage has no range operation");
    };
    let identity = checked
        .type_identity_text(*storage_type_identity)
        .unwrap_or_default();
    let symbol = match range.root {
        checked_trees::CheckedStorageRoot::Parameter { index } => checked
            .state_parameters(authored)
            .iter()
            .filter(|parameter| {
                !parameter.relevance.is_erased()
                    && checked
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .nth(index as usize)
            .map(|parameter| parameter.symbol)
            .ok_or(LoweringError::Unsupported(
                "range storage lost its parameter",
            ))?,
        checked_trees::CheckedStorageRoot::OwnedLocal { symbol }
        | checked_trees::CheckedStorageRoot::ViewLocal { symbol } => symbol,
    };
    let collection =
        terminal_psi::structural_type_by_identity(declarations, identities.intern(&identity))
            .ok_or(LoweringError::Unsupported(
                "range collection has no exact Terminal type",
            ))?;
    let view = crate::terminal_identities::structural_declaration_by_identity(
        declarations,
        identities,
        checked,
        argument.type_identity,
        "range result has no exact Terminal type",
    )?;
    let collection_element = match collection.shape {
        terminal_psi::StructuralTypeShape::FixedArray { element, .. } => Some(element),
        terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            | terminal_psi::ByteSequenceCarrier::BoundedOwned { .. },
        ) if range.family == ViewFamily::Bytes && view.shape.is_borrowed_byte_view() => {
            let byte = crate::emission::scalar_types::terminal_scalar_type(
                checked_trees::types::PrimitiveType::U8,
            )?;
            Some(
                declarations
                    .iter()
                    .find(|declaration| {
                        declaration.shape
                            == terminal_psi::StructuralTypeShape::PrimitiveScalar(byte)
                    })
                    .map(|declaration| declaration.id)
                    .ok_or(LoweringError::Unsupported(
                        "bounded byte backing has no exact primitive element declaration",
                    ))?,
            )
        }
        terminal_psi::StructuralTypeShape::ElementView { element, .. }
            if range.family == ViewFamily::Elements
                && matches!(view.shape, terminal_psi::StructuralTypeShape::ElementView { element: actual, .. } if actual == element) =>
        {
            None
        }
        terminal_psi::StructuralTypeShape::ByteSequence(
            terminal_psi::byte_sequence_view_carrier_pattern!(),
        ) if range.family == ViewFamily::Bytes && view.shape.is_borrowed_byte_view() => None,
        _ => return unsupported("range collection is neither backing nor its exact descriptor"),
    };
    Ok(RangeStorage {
        symbol,
        collection: argument.path.clone(),
        structural_type: view.id,
        family: range.family,
        collection_element,
        access: argument.access,
    })
}

pub(crate) fn establish(
    view: &CollectionView,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let path = emit_prepared_path(&view.path, 0, values, next_value, operations);
    let endpoints = view.byte_region.as_ref().map(|(start, end)| {
        (
            crate::emission::operation_emission::expressions::emit_direct_expression(
                start, values, next_value, operations,
            ),
            crate::emission::operation_emission::expressions::emit_direct_expression(
                end, values, next_value, operations,
            ),
        )
    });
    let producer = operations.allocate();
    // Endpoint evaluation can append operations. Bind path obligations to
    // this descriptor's producer, not to the earlier endpoint operation.
    let source = terminal_psi::StructuralArgument {
        place: view.source,
        access: view.access,
        path: path_with_current_bounds(&path, producer.get()),
    };
    let region = match endpoints {
        None => terminal_psi::ElementViewRegion::WholeCollection,
        Some((start, end)) => terminal_psi::ElementViewRegion::RepresentationBytes {
            start,
            end,
            obligation: crate::terminal_identities::obligation_id(
                producer
                    .get()
                    .checked_add(1)
                    .ok_or(LoweringError::Unsupported(
                        "representation view obligation identity overflows",
                    ))?,
            ),
        },
    };
    Ok(emit_view(
        view.place,
        view.structural_type,
        source,
        view.element,
        region,
        if view.access == terminal_psi::StructuralAccess::MutableBorrow {
            terminal_psi::StructuralMultiplicity::Affine
        } else {
            terminal_psi::StructuralMultiplicity::Unrestricted
        },
        producer,
        operations,
    ))
}

/// Scalar and Unit schedules establish the same descriptor operation. The
/// schedule captures endpoints; this emitter retains backing, region and
/// result custody without reconstructing the source expression.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_view(
    place: PlaceId,
    structural_type: StructuralTypeId,
    source: terminal_psi::StructuralArgument,
    element: StructuralTypeId,
    region: terminal_psi::ElementViewRegion,
    multiplicity: terminal_psi::StructuralMultiplicity,
    producer: semantic_vocabulary::OperationId,
    operations: &mut OperationBuffer,
) -> StructuralPlaceDeclaration {
    let access = source.access;
    operations.push(terminal_psi::Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: producer,
        result: terminal_psi::OperationResult::Structural(
            terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type,
                multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
        ),
        kind: if access == terminal_psi::StructuralAccess::MutableBorrow {
            terminal_psi::OperationKind::EstablishMutableElementView {
                destination: place,
                source,
                element,
                region,
            }
        } else {
            terminal_psi::OperationKind::EstablishElementView {
                destination: place,
                source,
                element,
                region,
            }
        },
    });
    StructuralPlaceDeclaration {
        id: place,
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        },
    }
}

/// The established view a range narrows, as the emitting route found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewRangeSource {
    pub(crate) place: PlaceId,
    pub(crate) path: Vec<LoweredElementPathSegment>,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) family: ViewFamily,
    pub(crate) access: terminal_psi::StructuralAccess,
}

/// Where the range sits and the range its checked plan retained there.
pub(crate) struct ViewRangeSite<'a> {
    /// The checked plan's state, whose scalar facts key the endpoints.
    pub(crate) state: symbols::SymbolHandle,
    pub(crate) statement: u32,
    pub(crate) range: Range<'a>,
}

/// Complete the selected collection path before evaluating authored endpoints,
/// then narrow that exact descriptor. Endpoint slots survive control splits and
/// retire after formation; the source loan prevents replacement meanwhile.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    site: ViewRangeSite<'_>,
    source: ViewRangeSource,
    result_type: StructuralTypeId,
    destination: PlaceId,
    evaluation: &mut super::argument_evaluation::Evaluation,
    source_value_count: usize,
    values: &mut Vec<ValueDeclaration>,
    next_value: &mut u64,
    next_block: &mut u64,
    next_edge: &mut u64,
    operations: &mut OperationBuffer,
    calls: &mut crate::emission::operation_emission::calls::CallEmissionContext<'_>,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let (start, end) = (site.range.start.clone(), site.range.end.clone());
    let (machine, _) = crate::declarations::state_declaration(checked, site.state)?;
    // Complete the collection selectors before any endpoint can call or write.
    // The checked loan retains the selected descriptor through those effects.
    let path = emit_prepared_path(&source.path, 0, values, next_value, operations);
    let retained_count = values.len();
    let mut positions = [None, None];
    for (ordinal, (value, role)) in [
        (
            start.as_ref(),
            CheckedScalarExpressionRole::SubsliceStart {
                site: site.range.site,
            },
        ),
        (
            end.as_ref(),
            CheckedScalarExpressionRole::SubsliceEnd {
                site: site.range.site,
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(value) = value {
            let completed = evaluation.source_value(
                identities,
                checked,
                machine.symbol,
                site.state,
                site.statement,
                role,
                value,
                source_value_count,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
                calls,
            )?;
            positions[ordinal] = Some(values.len());
            values.push(completed);
        }
    }
    let [start, end] = positions.map(|position| {
        position
            .map(|position| {
                view_subslice::coordinate(LoweredDirectExpression::Parameter {
                    position,
                    scalar_type: values[position].scalar_type,
                })
            })
            .transpose()
    });
    let (start, end) = (start?, end?);
    let end = exclusive_end(site.range.end_inclusive, end)?;
    let declaration = view_subslice::emit(
        source.family,
        terminal_psi::StructuralArgument {
            place: evaluation.current_structural_place(source.place),
            access: source.access,
            path,
        },
        start.as_ref(),
        end.as_ref(),
        destination,
        result_type,
        values,
        next_value,
        operations,
    )?;
    values.truncate(retained_count);
    Ok(declaration)
}

/// Normalize only the completed endpoint, never its authored computation.
/// Inclusive bounds are checked as `end < length` in Psi; the exact successor
/// here preserves that boundary without wrapping or evaluating an effect twice.
/// Both scalar and Unit schedules use this same half-open descriptor operation.
pub(crate) fn exclusive_end(
    end_inclusive: bool,
    end: Option<LoweredDirectExpression>,
) -> Result<Option<LoweredDirectExpression>, LoweringError> {
    let end = end.map(view_subslice::coordinate).transpose()?;
    if !end_inclusive {
        return Ok(end);
    }
    let end = end.ok_or(LoweringError::Unsupported(
        "inclusive range has no final endpoint",
    ))?;
    let scalar_type = crate::emission::scalar_types::terminal_scalar_type(
        checked_trees::types::PrimitiveType::U64,
    )?;
    Ok(Some(LoweredDirectExpression::IntegerBinary {
        kind: crate::emission::operation_emission::integer::LoweredIntegerBinaryKind::ExactAdd,
        scalar_type,
        left: Box::new(end),
        right: Box::new(LoweredDirectExpression::IntegerLiteral {
            value: semantic_vocabulary::IntegerValue::Unsigned(1),
            scalar_type,
        }),
        // `end < length` bounds `end + 1 <= length <= u64::MAX`, and the
        // checker rejects an inclusive end whose successor overflows — the
        // source decided this exact sum's representability at admission.
        formation_decided: true,
    }))
}
