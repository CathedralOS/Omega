//! Resolve range storage, establish a descriptor when needed, and narrow it.
//!
//! Arrays have backing but no descriptor; views already have one. That type
//! distinction, not the source's local/parameter origin or projection length,
//! decides whether EstablishElementView precedes narrowing. Reservation never
//! evaluates an operand: each consumer emits the preparation at the range's
//! authored position. Source replay and Terminal loan/bounds verification remain
//! independent of this choice.
use super::{CheckedTrees, LoweringError, unsupported};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::emission::operation_emission::view_subslice::{self, ViewFamily};
use crate::expression_preparation::bindings::ScalarBindings;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedSubsliceSite,
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
    pub(crate) start: &'a Option<CheckedScalarExpression>,
    pub(crate) end: &'a Option<CheckedScalarExpression>,
}

/// The range `argument` narrows, of either view family.
pub(crate) fn range(argument: &CheckedUnitStructuralArgumentPlan) -> Option<Range<'_>> {
    let (root, family, expression, start, end) = match &argument.source {
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root,
            expression,
            start,
            end,
        } => (*root, ViewFamily::Bytes, *expression, start, end),
        CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root,
            expression,
            start,
            end,
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
    array_element: Option<StructuralTypeId>,
}

/// One ordinary collection-view operation, shared by scalar and Unit schedules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CollectionView {
    pub(crate) source: terminal_psi::StructuralArgument,
    pub(crate) element: StructuralTypeId,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
}

impl RangeStorage {
    pub(crate) fn requires_establishment(&self) -> bool {
        self.array_element.is_some()
    }

    /// Bind the replayed source to the current activation's storage. Array
    /// borrowing reuses the ordinary place join, including its access checks.
    pub(crate) fn bind(
        &self,
        checked: &CheckedTrees,
        state: symbols::SymbolHandle,
        bindings: &ScalarBindings,
        destination: Option<PlaceId>,
    ) -> Result<(ViewRangeSource, Option<CollectionView>), LoweringError> {
        let (machine, authored) =
            crate::expression_preparation::source_custody::authored_state(checked, state)?;
        let (place, establishment) = if let Some(element) = self.array_element {
            let destination = destination.ok_or(LoweringError::Unsupported(
                "array range has no reserved whole-view descriptor",
            ))?;
            let argument = checked_trees::CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: self.symbol,
                },
                path: self.collection.clone(),
                access: checked_trees::CheckedStructuralAccess::SharedBorrow,
                type_identity: String::new(),
            };
            let source = bindings.shared_structural_argument(
                &argument,
                machine,
                checked.state_parameters(authored),
            )?;
            (
                destination,
                Some(CollectionView {
                    source,
                    element,
                    place: destination,
                    structural_type: self.structural_type,
                }),
            )
        } else {
            let (symbol, place) = bindings.shared_view_source(
                self.root,
                checked.state_parameters(authored),
                checked,
                self.structural_type,
                self.family == ViewFamily::Bytes,
            )?;
            if symbol != self.symbol || !self.collection.is_empty() {
                return unsupported("range descriptor changed its storage identity");
            }
            (place, None)
        };
        Ok((
            ViewRangeSource {
                symbol: self.symbol,
                place,
                structural_type: self.structural_type,
                family: self.family,
            },
            establishment,
        ))
    }
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
    if argument.access != checked_trees::CheckedStructuralAccess::SharedBorrow {
        return unsupported("range storage requires a shared loan");
    }
    let (machine, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let operation = validation::shared_subslice_expression(&checked.typed, range.expression)
        .ok_or(LoweringError::Unsupported(
            "range storage lost its shared expression",
        ))?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(operation) else {
        return unsupported("range storage lost its collection");
    };
    let source = crate::emission::call_source_custody::projected_receivers::source(
        checked,
        machine.symbol,
        state,
        statement as usize,
        indexed.collection,
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
    if source.root != symbol || source.path != argument.path {
        return unsupported("range storage changed its root or projection");
    }
    let mut reference = validation::declared_place_type_raw(
        &checked.typed,
        machine,
        Some(authored),
        indexed.collection,
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
        if shell.access != language_core::ReferenceAccess::Shared
            && shell.access != language_core::ReferenceAccess::Mutable
        {
            return unsupported("range collection is not readable");
        }
        reference = shell.referee;
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
    let array_element = match collection.shape {
        terminal_psi::StructuralTypeShape::FixedArray { element, .. } => {
            validate_element(element, view, range.family, declarations)?;
            Some(element)
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
    Ok(RangeStorage {
        symbol,
        root: range.root,
        collection: source.path,
        structural_type: view.id,
        family: range.family,
        array_element,
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
    operations: &mut OperationBuffer,
) -> StructuralPlaceDeclaration {
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
                multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
        ),
        kind: terminal_psi::OperationKind::EstablishElementView {
            destination: view.place,
            source: view.source.clone(),
            element: view.element,
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
    /// The authored name the range's collection must spell.
    pub(crate) symbol: symbols::SymbolHandle,
    pub(crate) place: PlaceId,
    pub(crate) structural_type: StructuralTypeId,
    pub(crate) family: ViewFamily,
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
        &'a Option<CheckedScalarExpression>,
        &'a Option<CheckedScalarExpression>,
    )>,
}

/// Replay the authored collection and narrow its established descriptor.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit(
    checked: &CheckedTrees,
    site: ViewRangeSite<'_>,
    source: ViewRangeSource,
    collection: &[CheckedUnitStructuralPathSegment],
    result_type: StructuralTypeId,
    destination: PlaceId,
    bindings: &ScalarBindings,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let (start, end) = endpoints(checked, &site, source.symbol, collection, bindings)?;
    if result_type != source.structural_type {
        return unsupported("view subslice changed its view type");
    }
    view_subslice::emit(
        source.family,
        source.place,
        start.as_ref(),
        end.as_ref(),
        destination,
        result_type,
        values,
        next_value,
        operations,
    )
}

/// Bind replayed endpoints into the current scalar namespace. Replay itself
/// performs no evaluation and also serves computed-call source custody.
pub(crate) fn endpoints(
    checked: &CheckedTrees,
    site: &ViewRangeSite<'_>,
    source_symbol: symbols::SymbolHandle,
    collection: &[CheckedUnitStructuralPathSegment],
    bindings: &ScalarBindings,
) -> Result<
    (
        Option<LoweredDirectExpression>,
        Option<LoweredDirectExpression>,
    ),
    LoweringError,
> {
    let (start, end) = replay(checked, site, source_symbol, collection)?;
    let lower = |present: bool, role| {
        present
            .then(|| bindings.expression_at(checked, site.state, site.statement, role))
            .transpose()
    };
    Ok((
        lower(
            start.is_some(),
            CheckedScalarExpressionRole::SubsliceStart { site: site.site },
        )?,
        lower(
            end.is_some(),
            CheckedScalarExpressionRole::SubsliceEnd { site: site.site },
        )?,
    ))
}

/// The site's lowered endpoints after replaying its authored range.
pub(crate) fn replay(
    checked: &CheckedTrees,
    site: &ViewRangeSite<'_>,
    source_symbol: symbols::SymbolHandle,
    collection: &[CheckedUnitStructuralPathSegment],
) -> Result<
    (
        Option<CheckedScalarExpression>,
        Option<CheckedScalarExpression>,
    ),
    LoweringError,
> {
    let (machine, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, site.state)?;
    let operation = validation::shared_subslice_expression(&checked.typed, site.expression).ok_or(
        LoweringError::Unsupported("view subslice has no shared range operation"),
    )?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(operation) else {
        return unsupported("view subslice has no authored indexed expression");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("view subslice has no authored range");
    };
    // Reconstruct the same storage relationship ordinary operands use. Empty
    // projections name whole storage, not necessarily an existing descriptor;
    // establishing a view is the emitter's separate responsibility.
    let source = crate::emission::call_source_custody::projected_receivers::source(
        checked,
        machine.symbol,
        site.state,
        site.statement as usize,
        indexed.collection,
    )?;
    if range.end_inclusive
        || !source_symbol.is_valid()
        || source.root != source_symbol
        || source.path != collection
    {
        return unsupported("view subslice changed its exact collection root or projection");
    }
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
                    role: CheckedScalarExpressionRole,
                    retained: Option<&Option<CheckedScalarExpression>>|
     -> Result<Option<CheckedScalarExpression>, LoweringError> {
        if !authored_endpoint.is_valid() {
            return Ok(None);
        }
        let (binding, selected) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(site.state, site.statement, role)
            .ok_or(LoweringError::Unsupported(
                "view subslice endpoint has no source-bound scalar plan",
            ))?;
        if binding.expression != authored_endpoint
            || binding.destination.is_valid()
            || selected.primitive_type() != Some(checked_trees::types::PrimitiveType::U64)
            || retained.is_some_and(|retained| retained.as_ref() != Some(selected))
        {
            return unsupported("view subslice endpoint differs from its source-bound plan");
        }
        Ok(Some(selected.clone()))
    };
    Ok((
        endpoint(
            range.start,
            CheckedScalarExpressionRole::SubsliceStart { site: site.site },
            site.retained.map(|(start, _)| start),
        )?,
        endpoint(
            range.end,
            CheckedScalarExpressionRole::SubsliceEnd { site: site.site },
            site.retained.map(|(_, end)| end),
        )?,
    ))
}

/// The immutable view local an `EstablishViewSubslice` binds: its result
/// statement must be that `let`, whose initializer is exactly the retained
/// range and whose declared type is the result's view type.
pub(crate) fn binding_local<'a>(
    checked: &'a CheckedTrees,
    state: symbols::SymbolHandle,
    operation: &checked_trees::CheckedUnitEffectOperationPlan,
) -> Result<&'a checked_trees::statement::TableLocalData, LoweringError> {
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
    let (_, authored) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(authored.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return unsupported("view subslice binding lost its authored local");
    };
    if local.is_mutable
        || !local.symbol.is_valid()
        || local.initial_value != *expression
        || source.access != checked_trees::CheckedStructuralAccess::SharedBorrow
        || source.type_identity != result.type_identity
        || result.multiplicity != language_semantics::Multiplicity::Unrestricted
    {
        return unsupported("view subslice binding changed its local, range or custody");
    }
    Ok(local)
}
