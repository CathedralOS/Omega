//! Resolve range storage, establish a descriptor when needed, and narrow it.
//!
//! Arrays have backing but no descriptor; views already have one. That type
//! distinction, not the source's local/parameter origin or projection length,
//! decides whether EstablishElementView precedes narrowing. Reservation never
//! evaluates an operand: each consumer emits the preparation at the range's
//! authored position. Source replay and Terminal loan/bounds verification remain
//! independent of this choice.
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
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
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
pub(crate) struct Range<'a> {
    pub(crate) root: CheckedStorageRoot,
    pub(crate) family: ViewFamily,
    pub(crate) expression: ExpressionHandle,
    pub(crate) start: &'a Option<CheckedCallScalarArgument>,
    pub(crate) end: &'a Option<CheckedCallScalarArgument>,
}

/// The range `argument` narrows, of either view family.
pub(crate) fn range(argument: &CheckedUnitStructuralArgumentPlan) -> Option<Range<'_>> {
    let (root, family, expression, start, end) = match &argument.source {
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root,
            expression,
            start,
            end,
            storage_type_identity: _,
        } => (*root, ViewFamily::Bytes, *expression, start, end),
        CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root,
            expression,
            start,
            end,
            storage_type_identity: _,
        } => (*root, ViewFamily::Elements, *expression, start, end),
        _ => return None,
    };
    Some(Range {
        root,
        family,
        expression,
        start,
        end,
    })
}

#[derive(Clone)]
pub(crate) struct RangeStorage {
    symbol: symbols::SymbolHandle,
    root: checked_trees::CheckedStorageRoot,
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
}

impl RangeStorage {
    pub(crate) fn requires_establishment(&self) -> bool {
        self.collection_element.is_some()
    }

    /// Bind the replayed source to the current activation's storage. Array
    /// borrowing reuses the ordinary place join, including its access checks.
    pub(crate) fn bind(
        &self,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        bindings: &ScalarBindings,
        destination: Option<PlaceId>,
        declarations: &[terminal_psi::StructuralTypeDeclaration],
    ) -> Result<(ViewRangeSource, Option<CollectionView>), LoweringError> {
        let (machine, authored) = crate::declarations::state_declaration(checked, state)?;
        let (place, establishment) =
            if let Some(element) = self.collection_element {
                let destination = destination.ok_or(LoweringError::Unsupported(
                    "collection range has no reserved whole-view descriptor",
                ))?;
                let argument = checked_trees::CheckedUnitStructuralArgumentPlan {
                    source:
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                            symbol: self.symbol,
                        },
                    path: self.collection.clone(),
                    access: self.access,
                    type_identity: String::new(),
                };
                let source = bindings.borrowed_structural_home(
                    &argument,
                    machine,
                    checked.state_parameters(authored),
                )?;
                let root = bindings.structural_argument_type(&source).ok_or(
                    LoweringError::Unsupported("collection view lost its storage type"),
                )?;
                (
                destination,
                Some(CollectionView {
                    source: source.place,
                    access:
                        crate::expression_preparation::bindings::structural_paths::terminal_access(
                            self.access,
                        ),
                    path: source.path.into_iter().map(LoweredElementPathSegment::Static)
                        .chain(collection_path(&self.collection, bindings, root, declarations)?)
                        .collect(),
                    element,
                    place: destination,
                    structural_type: self.structural_type,
                }),
            )
            } else {
                let (_, place) = bindings.borrowed_view_source(
                    self.root,
                    checked.state_parameters(authored),
                    checked,
                    self.structural_type,
                    self.family == ViewFamily::Bytes,
                    crate::expression_preparation::bindings::structural_paths::terminal_access(
                        self.access,
                    ),
                )?;
                (place, None)
            };
        Ok((
            ViewRangeSource {
                place,
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
    path: &[CheckedUnitStructuralPathSegment],
    bindings: &ScalarBindings,
    root: semantic_vocabulary::StructuralTypeId,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<LoweredElementPathSegment>, LoweringError> {
    use crate::expression_preparation::bindings::structural_paths::{
        PreparedStructuralPathSegment, prepare_argument_path,
    };
    let mut runtime_indices = 0;
    prepare_argument_path(path, root, declarations)?
        .into_iter()
        .map(|segment| match segment {
            PreparedStructuralPathSegment::Static(segment) => {
                Ok(LoweredElementPathSegment::Static(segment))
            }
            PreparedStructuralPathSegment::ComputedIndex(_)
            | PreparedStructuralPathSegment::ViewExtent { .. } => {
                unsupported("computed view selector requires its owning operand evaluation")
            }
            PreparedStructuralPathSegment::RuntimeIndex(index) => {
                runtime_indices += 1;
                if runtime_indices > TERMINAL_NESTED_SELECTOR_LIMIT {
                    return unsupported("collection view exceeds the runtime selector namespace");
                }
                let selector = match index {
                    checked_trees::CheckedRuntimeIndex::Parameter { position } => {
                        bindings.runtime_index_parameter(position)?
                    }
                    checked_trees::CheckedRuntimeIndex::Local { position } => {
                        bindings.runtime_index_local(position)?
                    }
                    checked_trees::CheckedRuntimeIndex::StructuralField(field) => {
                        bindings.runtime_index_field(&field)?
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
                })
            }
        })
        .collect()
}

/// Resolve the collection's declared type before reserving descriptor storage.
/// This does not confer a loan or discharge range bounds.
pub(crate) fn storage(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    statement: u32,
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
    let (machine, authored) = crate::declarations::state_declaration(checked, state)?;
    // The `coll[a..b]` spelling carries its collection through the indexed
    // operation; the whole-extent spelling (`let v: &T = coll`) spells no
    // `..` at all — its expression is the collection itself, behind the
    // same readable-borrow unwrap the subslice recognizer applies.
    let collection_expression = if let Some(operation) =
        validation::subslice_expression(&checked.typed, range.expression)
    {
        let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(operation)
        else {
            return unsupported("range storage lost its collection");
        };
        indexed.collection
    } else {
        match checked.expression_table.expression(range.expression) {
            ExpressionNode::Borrow(borrow)
                if matches!(
                    borrow.access,
                    language_core::ReferenceAccess::Shared
                        | language_core::ReferenceAccess::Mutable
                ) =>
            {
                borrow.target
            }
            ExpressionNode::Borrow(_) => {
                return unsupported("range storage lost its readable expression");
            }
            _ => range.expression,
        }
    };
    let source = crate::emission::call_source_custody::projected_receivers::source(
        checked,
        machine.symbol,
        state,
        statement as usize,
        collection_expression,
    )?;
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
    let mut reference = validation::declared_place_type_raw(
        &checked.typed,
        machine,
        Some(authored),
        collection_expression,
    )
    .ok_or(LoweringError::Unsupported(
        "range collection has no declared type",
    ))?;
    // A root reference is represented by its parameter/view-local custody.
    // A stored reference needs an explicit Referent projection, not erasure.
    if source.path.is_empty()
        && !matches!(
            range.root,
            checked_trees::CheckedStorageRoot::OwnedLocal { .. }
        )
        && let Some(shell) = validation::reference_shell(&checked.typed, reference)
    {
        if argument.access == checked_trees::CheckedStructuralAccess::MutableBorrow
            && shell.access != language_core::ReferenceAccess::Mutable
        {
            return unsupported("range storage escalated shared backing authority");
        }
        if shell.access != language_core::ReferenceAccess::Shared
            && shell.access != language_core::ReferenceAccess::Mutable
        {
            return unsupported("range collection is not readable");
        }
        // Bounded byte references are pointer carriers with their own exact
        // identity. Other root references use their referent representation.
        if !matches!(
            validation::byte_sequence_carrier(&checked.typed, reference, &[]),
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. })
        ) {
            reference = shell.referee;
        }
    }
    let reference = super::parameters::structural_carrier_type(checked, reference)?;
    let identity = checked.normalized_type_identity(reference);
    let collection = declarations
        .iter()
        .find(|declaration| declaration.identity == identity.as_str())
        .ok_or(LoweringError::Unsupported(
            "range collection has no exact Terminal type",
        ))?;
    let view = declarations
        .iter()
        .find(|declaration| declaration.identity == argument.type_identity)
        .ok_or(LoweringError::Unsupported(
            "range result has no exact Terminal type",
        ))?;
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
        _ if collection.id == view.id
            && source.path.is_empty()
            && !matches!(
                range.root,
                checked_trees::CheckedStorageRoot::OwnedLocal { .. }
            ) =>
        {
            None
        }
        _ => return unsupported("range collection is neither backing nor its exact descriptor"),
    };
    if let Some(element) = collection_element {
        terminal_semantics::collection_backing(declarations.iter(), collection.id, element).ok_or(
            LoweringError::Unsupported("range backing changed its exact collection element"),
        )?;
        validate_element(element, view, range.family, declarations)?;
    }
    Ok(RangeStorage {
        symbol,
        root: range.root,
        collection: source.path,
        structural_type: view.id,
        family: range.family,
        collection_element,
        access: argument.access,
    })
}

fn validate_element(
    element: StructuralTypeId,
    view: &terminal_psi::StructuralTypeDeclaration,
    family: ViewFamily,
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LoweringError> {
    let matches = match (family, &view.shape) {
        (
            ViewFamily::Elements,
            terminal_psi::StructuralTypeShape::ElementView { element: expected },
        ) => *expected == element,
        (ViewFamily::Bytes, shape) if shape.is_borrowed_byte_view() => {
            let byte = crate::emission::scalar_types::terminal_scalar_type(
                checked_trees::types::PrimitiveType::U8,
            )?;
            declarations.iter().any(|declaration| {
                declaration.id == element
                    && declaration.shape == terminal_psi::StructuralTypeShape::PrimitiveScalar(byte)
            })
        }
        _ => false,
    };
    if !matches {
        return unsupported("range backing element disagrees with its view type");
    }
    Ok(())
}

pub(crate) fn establish(
    view: &CollectionView,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> StructuralPlaceDeclaration {
    let path = emit_prepared_path(&view.path, 0, values, next_value, operations);
    let source = terminal_psi::StructuralArgument {
        place: view.source,
        access: view.access,
        path: path_with_current_bounds(&path, operations.next_identity),
    };
    let producer = operations.allocate();
    operations.push(terminal_psi::Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: producer,
        result: terminal_psi::OperationResult::Structural(
            terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place: view.place,
                structural_type: view.structural_type,
                multiplicity: if view.access == terminal_psi::StructuralAccess::MutableBorrow {
                    terminal_psi::StructuralMultiplicity::Affine
                } else {
                    terminal_psi::StructuralMultiplicity::Unrestricted
                },
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
        ),
        kind: if view.access == terminal_psi::StructuralAccess::MutableBorrow {
            terminal_psi::OperationKind::EstablishMutableElementView {
                destination: view.place,
                source,
                element: view.element,
                region: terminal_psi::ElementViewRegion::WholeCollection,
            }
        } else {
            terminal_psi::OperationKind::EstablishElementView {
                destination: view.place,
                source,
                element: view.element,
                region: terminal_psi::ElementViewRegion::WholeCollection,
            }
        },
    });
    StructuralPlaceDeclaration {
        id: view.place,
        kind: semantic_vocabulary::StructuralPlaceKind::OperationResult {
            producer,
            structural_type: view.structural_type,
        },
    }
}

/// The established view a range narrows, as the emitting route found it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ViewRangeSource {
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) family: ViewFamily,
    pub(crate) access: terminal_psi::StructuralAccess,
}

/// Where the range sits and what the checked plan retained for it.
pub(crate) struct ViewRangeSite<'a> {
    /// The checked plan's state, whose scalar facts key the endpoints.
    pub(crate) state: symbols::SymbolHandle,
    pub(crate) statement: u32,
    pub(crate) site: CheckedSubsliceSite,
    pub(crate) expression: ExpressionHandle,
    /// Endpoints a call-argument or binding plan retains; a transfer retains
    /// none and replays its site's facts alone.
    pub(crate) retained: Option<(
        &'a Option<CheckedCallScalarArgument>,
        &'a Option<CheckedCallScalarArgument>,
    )>,
}

/// Evaluate the authored endpoints in order, then narrow the established descriptor.
/// Completed endpoint slots survive control splits and retire after formation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
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
    let (start, end) = replay(checked, &site)?;
    let (machine, _) = crate::declarations::state_declaration(checked, site.state)?;
    let retained_count = values.len();
    let mut positions = [None, None];
    for (ordinal, (value, role)) in [
        (
            start.as_ref(),
            CheckedScalarExpressionRole::SubsliceStart { site: site.site },
        ),
        (
            end.as_ref(),
            CheckedScalarExpressionRole::SubsliceEnd { site: site.site },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if let Some(value) = value {
            let completed = evaluation.source_value(
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
    let count_type = crate::emission::scalar_types::terminal_scalar_type(
        checked_trees::types::PrimitiveType::U64,
    )?;
    let [start, end] = positions.map(|position| {
        position.map(|position| LoweredDirectExpression::Parameter {
            position,
            scalar_type: count_type,
        })
    });
    let end = exclusive_end(checked, site.expression, end)?;
    let declaration = view_subslice::emit(
        source.family,
        evaluation.current_structural_place(source.place),
        source.access,
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

/// The site's lowered endpoints after replaying its authored range.
pub(crate) fn replay(
    checked: &CheckedTrees,
    site: &ViewRangeSite<'_>,
) -> Result<
    (
        Option<CheckedCallScalarArgument>,
        Option<CheckedCallScalarArgument>,
    ),
    LoweringError,
> {
    let (machine, authored) = crate::declarations::state_declaration(checked, site.state)?;
    let Some(operation) = validation::subslice_expression(&checked.typed, site.expression) else {
        // The whole-extent spelling `let v: &T = coll` authors no `..`
        // operation — its endpoints stay omitted and its expression names
        // the collection itself, behind the same readable-borrow unwrap
        // `storage` applies.
        let collection_expression = match checked.expression_table.expression(site.expression) {
            ExpressionNode::Borrow(borrow)
                if matches!(
                    borrow.access,
                    language_core::ReferenceAccess::Shared
                        | language_core::ReferenceAccess::Mutable
                ) =>
            {
                borrow.target
            }
            _ => site.expression,
        };
        crate::emission::call_source_custody::projected_receivers::source(
            checked,
            machine.symbol,
            site.state,
            site.statement as usize,
            collection_expression,
        )?;
        return Ok((None, None));
    };
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(operation) else {
        return unsupported("view subslice has no authored indexed expression");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("view subslice has no authored range");
    };
    // Reconstruct the same storage relationship ordinary operands use. Empty
    // projections name whole storage, not necessarily an existing descriptor;
    // establishing a view is the emitter's separate responsibility.
    crate::emission::call_source_custody::projected_receivers::source(
        checked,
        machine.symbol,
        site.state,
        site.statement as usize,
        indexed.collection,
    )?;
    if let Some((start, end)) = site.retained
        && (range.start.is_valid() != start.is_some() || range.end.is_valid() != end.is_some())
    {
        return unsupported("view subslice endpoint presence changed");
    }
    if checked.facts.operators.uses.iter().any(|(_, selected)| {
        selected.expression == operation
            && (selected.spelling != language_core::OperatorSpelling::Range
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
    }) || !validation::has_builtin_subslice_meaning(
        &checked.typed,
        machine,
        Some(authored),
        operation,
    ) {
        return unsupported("view subslice no longer selects the builtin range");
    }
    let endpoint = |authored_endpoint: ExpressionHandle,
                    role: CheckedScalarExpressionRole|
     -> Result<Option<CheckedCallScalarArgument>, LoweringError> {
        if !authored_endpoint.is_valid() {
            return Ok(None);
        }
        let selected = if let Some(root) = checked
            .facts
            .values
            .scalar_computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .find(|root| {
                root.state == site.state
                    && root.statement_ordinal == site.statement
                    && root.role == role
            }) {
            CheckedCallScalarArgument::Computation(root.root)
        } else {
            let (_, selected) = checked
                .facts
                .values
                .scalar_expressions
                .bound_expression_at(site.state, site.statement, role)
                .ok_or(LoweringError::Unsupported(
                    "view subslice endpoint has no source-bound scalar plan",
                ))?;
            CheckedCallScalarArgument::Pure(selected.clone())
        };
        Ok(Some(selected))
    };
    Ok((
        endpoint(
            range.start,
            CheckedScalarExpressionRole::SubsliceStart { site: site.site },
        )?,
        endpoint(
            range.end,
            CheckedScalarExpressionRole::SubsliceEnd { site: site.site },
        )?,
    ))
}

/// Normalize only the completed endpoint, never its authored computation.
/// Inclusive bounds are checked as `end < length` in Psi; the exact successor
/// here preserves that boundary without wrapping or evaluating an effect twice.
/// Both scalar and Unit schedules use this same half-open descriptor operation.
pub(crate) fn exclusive_end(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
    end: Option<LoweredDirectExpression>,
) -> Result<Option<LoweredDirectExpression>, LoweringError> {
    let Some(operation) = validation::subslice_expression(&checked.typed, expression) else {
        return Ok(end);
    };
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(operation) else {
        return unsupported("view range lost its indexed expression");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("view range lost its endpoints");
    };
    if !range.end_inclusive {
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
        formation_decided: false,
    }))
}

/// Rejoin a descriptor producer with the occurrence that evaluates its range.
/// A local initializer and a completion use identical range operations; only
/// the source-bound endpoint site differs. Returning never borrows a temporary
/// descriptor carrier: the resulting value retains the original backing.
pub(crate) fn binding_site(
    checked: &CheckedTrees,
    state: symbols::SymbolHandle,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
) -> Result<checked_trees::CheckedSubsliceSite, LoweringError> {
    let checked_trees::CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source } =
        operation
    else {
        return unsupported("view subslice binding has no producer");
    };
    let (checked_trees::CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
        expression,
        ..
    }
    | checked_trees::CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
        expression,
        ..
    }) = &source.source
    else {
        return unsupported("view subslice binding lost its range source");
    };
    let (machine, authored) = crate::declarations::state_declaration(checked, state)?;
    let statements = checked.statement_table.statements(authored.statement_nodes);
    let (declared_type, site) = match statements.get(result.statement_index as usize) {
        Some(checked_trees::statement::StatementNode::LocalData(local))
            if !local.is_mutable
                && local.symbol.is_valid()
                && local.initial_value == *expression =>
        {
            (
                crate::expression_preparation::source_custody::structural::declared_view_binding_type(
                    checked,
                    machine,
                    authored,
                    local.type_reference,
                    local.initial_value,
                ),
                checked_trees::CheckedSubsliceSite::LocalBinding,
            )
        }
        Some(
            checked_trees::statement::StatementNode::Expression(_)
            | checked_trees::statement::StatementNode::Transition(_),
        ) if result.statement_index as usize + 1 == statements.len()
            && crate::reference_custody::tail_return_expression(checked, authored)
                == Some(*expression) =>
        {
            (
                authored.return_type,
                checked_trees::CheckedSubsliceSite::Expression {
                    expression: *expression,
                },
            )
        }
        _ => return unsupported("view subslice binding lost its authored occurrence"),
    };
    let declared = validation::reference_shell(&checked.typed, declared_type).ok_or(
        LoweringError::Unsupported("range result lost its declared reference"),
    )?;
    if !matches!(
        declared.access,
        language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
    ) {
        return unsupported("range result has unreadable access");
    }
    Ok(site)
}
