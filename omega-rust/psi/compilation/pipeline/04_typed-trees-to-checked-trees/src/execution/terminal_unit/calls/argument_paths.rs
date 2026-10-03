//! Checked scalar arguments and projected argument paths: field and index
//! paths, projected call support, contract mentions and crash expressions.

use crate::execution::terminal_unit::types::byte_sequence_carrier;
use crate::execution::terminal_unit::types::{
    base_type_identity, byte_sequence_type_identity, parameter_root_symbol,
    structural_access_for_type_reference, terminal_field_identity,
    type_graph_requires_nominal_drop,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarParameterPlan, CheckedUnitCallCoordinate,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, ExpressionNode,
    MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, ProofFact, SignatureContractKind, StateParameter,
    StatementNode, SymbolHandle, TypeReferenceHandle, TypeReferenceNode, TypedTrees, is_reference,
};

pub(crate) fn checked_call_scalar_arguments(
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    parameters: &[CheckedStructuralScalarParameterPlan],
    boundary: bool,
) -> Option<Vec<checked_trees::CheckedCallScalarArgument>> {
    parameters
        .iter()
        .enumerate()
        .map(|(argument_ordinal, parameter)| {
            let role = if boundary {
                CheckedScalarExpressionRole::BoundaryCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                }
            } else {
                CheckedScalarExpressionRole::UnitCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                }
            };
            let computations = &facts.values.scalar_computations;
            let roots = computations
                .roots
                .iter()
                .filter(|(_, root)| {
                    root.state == caller_state
                        && root.statement_ordinal == coordinate.statement_index
                        && root.role == role
                })
                .map(|(_, root)| root)
                .collect::<Vec<_>>();
            if let [root] = roots.as_slice() {
                if !computations.nodes.is_valid(root.root)
                    || computations.nodes.get(root.root).primitive_type != parameter.primitive_type
                    || facts
                        .values
                        .scalar_expressions
                        .expressions
                        .iter()
                        .any(|expression| {
                            expression.state == caller_state
                                && expression.statement_ordinal == coordinate.statement_index
                                && expression.role == role
                        })
                {
                    return None;
                }
                return Some(checked_trees::CheckedCallScalarArgument::Computation(
                    root.root,
                ));
            }
            if !roots.is_empty() {
                return None;
            }
            let (_, expression) = facts.values.scalar_expressions.bound_expression_at(
                caller_state,
                coordinate.statement_index,
                role,
            )?;
            (crate::values::scalar_expression_type(expression)? == parameter.primitive_type)
                .then(|| checked_trees::CheckedCallScalarArgument::Pure(expression.clone()))
        })
        .collect()
}

/// Proof-only erased actuals of an in-module Unit call: the retained proof
/// terms recorded under `ErasedUnitCallArgument`, in the callee's dense
/// erased proof-formal order. Each term's constructed (or forwarded) data
/// identity must match the formal's canonical `Nat`-style identity.
pub(crate) fn checked_call_erased_proof_arguments(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    parameters: &[checked_trees::CheckedErasedProofParameterPlan],
) -> Option<Vec<checked_trees::CheckedProofTerm>> {
    parameters
        .iter()
        .enumerate()
        .map(|(erased_ordinal, parameter)| {
            let role = checked_trees::CheckedProofTermRole::ErasedUnitCallArgument {
                call_ordinal: coordinate.call_ordinal,
                erased_ordinal: u32::try_from(erased_ordinal).ok()?,
            };
            let term =
                facts
                    .values
                    .proof_terms
                    .term_at(caller_state, coordinate.statement_index, role)?;
            proof_term_type_matches(program, caller_state, term, &parameter.type_identity)
                .then(|| term.clone())
        })
        .collect()
}

/// The construction or forwarded formal carries exactly the formal's declared
/// proof-only data identity.
fn proof_term_type_matches(
    program: &TypedTrees,
    caller_state: SymbolHandle,
    term: &checked_trees::CheckedProofTerm,
    type_identity: &str,
) -> bool {
    match term {
        checked_trees::CheckedProofTerm::Construction {
            type_identity: actual,
            ..
        } => actual == type_identity,
        checked_trees::CheckedProofTerm::Formal { parameter_symbol } => {
            crate::semantic::calls::find_state(program, caller_state).is_some_and(|state| {
                program.state_parameters(state).iter().any(|parameter| {
                    parameter.symbol == *parameter_symbol
                        && base_type_identity(program, parameter.type_reference, &[]).as_deref()
                            == Some(type_identity)
                })
            })
        }
        // A bare scalar leaf never satisfies a proof-formal position: scalar
        // leaves only occur nested inside a construction.
        checked_trees::CheckedProofTerm::Scalar(_) => false,
    }
}

/// Proof-only erased actuals of an in-module Unit call: the retained pure
/// expressions recorded under `ErasedUnitCallArgument`, in the callee's dense
/// erased-formal order.
pub(crate) fn checked_call_erased_scalar_arguments(
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<Vec<checked_trees::CheckedCallScalarArgument>> {
    parameters
        .iter()
        .enumerate()
        .map(|(erased_ordinal, parameter)| {
            let role = CheckedScalarExpressionRole::ErasedUnitCallArgument {
                call_ordinal: coordinate.call_ordinal,
                erased_ordinal: u32::try_from(erased_ordinal).ok()?,
            };
            let (_, expression) = facts.values.scalar_expressions.bound_expression_at(
                caller_state,
                coordinate.statement_index,
                role,
            )?;
            (crate::values::scalar_expression_type(expression)? == parameter.primitive_type)
                .then(|| checked_trees::CheckedCallScalarArgument::Pure(expression.clone()))
        })
        .collect()
}

fn checked_nonempty_field_path(path: &[CheckedUnitStructuralPathSegment]) -> bool {
    !path.is_empty()
        && path
            .iter()
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
}

fn checked_literal_index_path(
    path: &[CheckedUnitStructuralPathSegment],
) -> Option<&[CheckedUnitStructuralPathSegment]> {
    let field_count = path
        .iter()
        .position(|segment| matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(_)))?;
    let (fields, indexes) = path.split_at(field_count);
    (!indexes.is_empty()
        && fields
            .iter()
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
        && indexes
            .iter()
            .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(_))))
    .then_some(fields)
}

/// Translate an authored place into the checked structural path a call
/// argument retains, and report the exact type that path projects to. Every
/// admitted fixed index is checked against its literal array length, so the
/// returned path names storage the place actually contains. Root and field
/// resolution belong to `canonical_place_type_reference`, which already owns
/// the two `self` spellings.
///
/// Both call lanes share this owner. What does not belong here is the admission
/// decision: which segment shapes a lane permits, and which rule the projected
/// type must satisfy against the target parameter. The ordinary transitive lane
/// admits exact mutable byte-view presentations as well as equal normalized
/// identity; each call owner retains its own access and path restrictions.
pub(crate) fn projected_argument_path(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    projected_argument_path_impl(program, state_symbol, statement_index, place, None, None)
}

/// `projected_argument_path` with runtime-index admission for the borrowed
/// lanes — a `&`-rooted call receiver or argument, or a `self.nodes[i]`-shape
/// copied leaf. A runtime `Index` segment retains `RuntimeIndex` naming the
/// checked selector only while the selector resolves to an immutable integer
/// operand of the calling state: its dense scalar parameter position, or an
/// integer field read below a structural parameter. Terminal re-proves
/// `0 <= index < extent` for the segment's obligation; the segment itself
/// carries no interval. Every other caller keeps rejecting runtime indexes
/// through `projected_argument_path`.
pub(crate) fn projected_borrowed_path(
    program: &TypedTrees,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    projected_argument_path_impl(
        program,
        state_symbol,
        statement_index,
        place,
        Some(machine),
        None,
    )
}

/// `projected_borrowed_path` for a case observation of a runtime-selected
/// element, whose selector may also be an immutable scalar local:
/// `local_selector` resolves one selector expression to its `Local` position
/// in the observing computation's own scalar namespace. Only the observation
/// owner holds that namespace, so no other lane admits local selectors.
pub(crate) fn projected_observed_path(
    program: &TypedTrees,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
    local_selector: &dyn Fn(
        typed_trees::expression::ExpressionHandle,
    ) -> Option<checked_trees::CheckedRuntimeIndex>,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    projected_argument_path_impl(
        program,
        state_symbol,
        statement_index,
        place,
        Some(machine),
        Some(local_selector),
    )
}

fn projected_argument_path_impl(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
    runtime_index_machine: Option<SymbolHandle>,
    local_selector: Option<
        &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<checked_trees::CheckedRuntimeIndex>,
    >,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    let mut path = Vec::with_capacity(place.segments.len());
    let mut payload_field = None;
    for (position, segment) in place.segments.iter().enumerate() {
        match segment {
            facts::PlaceSegment::Field { symbol } => {
                let identity = match payload_field.take() {
                    Some((expected, identity)) if expected == *symbol => identity,
                    Some(_) => return None,
                    None => terminal_field_identity(program, *symbol)?,
                };
                path.push(CheckedUnitStructuralPathSegment::Field(identity));
            }
            facts::PlaceSegment::FixedIndex { index } => {
                let container = crate::flow::CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..position].to_vec(),
                };
                let container = crate::flow::canonical_place_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    &container,
                )?;
                fixed_index_within_extent(program, container, *index)?;
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(*index).ok()?,
                ));
            }
            facts::PlaceSegment::Index { expression } => {
                // An immutable local scalar initialized to a literal is a
                // fixed index in every way that matters to this walk: the
                // binding can never change, so the selected element is as
                // pinned as an authored constant. Fold it through the same
                // literal-bound check `FixedIndex` segments take rather than
                // reaching for runtime-selector evidence.
                if let Some(index) = constant_local_index(program, state_symbol, *expression) {
                    let container = crate::flow::CanonicalPlace {
                        root: place.root,
                        segments: place.segments[..position].to_vec(),
                    };
                    let container = crate::flow::canonical_place_type_reference(
                        program,
                        state_symbol,
                        statement_index,
                        &container,
                    )?;
                    fixed_index_within_extent(program, container, index)?;
                    path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                        u64::try_from(index).ok()?,
                    ));
                    continue;
                }
                let machine = runtime_index_machine?;
                let runtime_index =
                    runtime_index_parameter_position(program, state_symbol, *expression)
                        .map(|position| checked_trees::CheckedRuntimeIndex::Parameter { position })
                        .or_else(|| {
                            runtime_index_field_selector(
                                program,
                                machine,
                                state_symbol,
                                statement_index,
                                *expression,
                            )
                        })
                        .or_else(|| local_selector.and_then(|resolve| resolve(*expression)))?;
                path.push(CheckedUnitStructuralPathSegment::RuntimeIndex(
                    runtime_index,
                ));
            }
            facts::PlaceSegment::FixedRange { start, end } => {
                // A borrowed byte view's window is itself part of the place:
                // `self.source[0..2]` lends the fixed byte range the view
                // names. The container must be fixed byte-array backing whose
                // literal extent bounds the window — the same shape the
                // `FixedIndex` walk proves — since a stored view carries no
                // runtime bound evidence of its own.
                let container = crate::flow::CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..position].to_vec(),
                };
                let container = crate::flow::canonical_place_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    &container,
                )?;
                fixed_byte_range_within_extent(program, container, *start, *end)?;
                path.push(CheckedUnitStructuralPathSegment::FixedByteRange {
                    start: u64::try_from(*start).ok()?,
                    end: u64::try_from(*end).ok()?,
                });
            }
            facts::PlaceSegment::Case { variant } => {
                // Resolve the exact case and its following field together.
                // The canonical type walker alone skips Case markers; it is
                // not evidence that a payload belongs to the selected case.
                let container = crate::flow::CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..position].to_vec(),
                };
                let reference = crate::flow::canonical_place_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    &container,
                )?;
                let reference = validation::unwrapped_type_reference(program, reference)?;
                let owner = program.closed_data_definition(reference)?;
                let mut cases =
                    program
                        .data_members(owner)
                        .iter()
                        .filter_map(|member| match member {
                            typed_trees::data::DataMember::Variant(case)
                                if variant.is_valid() && case.symbol == *variant =>
                            {
                                Some(case)
                            }
                            _ => None,
                        });
                let case = cases.next()?;
                let Some(facts::PlaceSegment::Field { symbol }) = place.segments.get(position + 1)
                else {
                    return None;
                };
                let mut fields = program.data_payload_fields(case).iter().filter(|field| {
                    symbol.is_valid() && field.symbol == *symbol && !field.relevance.is_erased()
                });
                let field = fields.next()?;
                if cases.next().is_some() || fields.next().is_some() {
                    return None;
                }
                path.push(CheckedUnitStructuralPathSegment::Case(case.path_identity()));
                payload_field = Some((*symbol, field.path_identity()));
            }
        }
    }
    let projected =
        crate::flow::canonical_place_type_reference(program, state_symbol, statement_index, place)?;
    Some((projected, path))
}

/// The checked segment a borrow-carried `Index` selector resolves to. An
/// erased loan alias injects its captured `Index` segment into a consumer's
/// walk, but that selector was never an authored target coordinate — no
/// `AssignmentIndex` exists for it — so it resolves here exactly the way
/// the same selector resolves in an argument path: a constant local folds
/// to `FixedIndex`, a bare parameter or field selector mints the runtime
/// coordinate Terminal re-proves against the container's extent.
pub(crate) fn borrowed_index_segment(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitStructuralPathSegment> {
    if let Some(index) = constant_local_index(program, state, expression) {
        return Some(CheckedUnitStructuralPathSegment::FixedIndex(
            u64::try_from(index).ok()?,
        ));
    }
    let runtime_index = runtime_index_parameter_position(program, state, expression)
        .map(|position| checked_trees::CheckedRuntimeIndex::Parameter { position })
        .or_else(|| {
            runtime_index_field_selector(program, machine, state, statement_index, expression)
        })?;
    Some(CheckedUnitStructuralPathSegment::RuntimeIndex(
        runtime_index,
    ))
}

/// The literal index an immutable local scalar names: `let i: u64 = 2`
/// selects exactly element 2 for the whole state because nothing can write
/// the binding. Only a bare `Name` selector spelled against a state-local
/// immutable binding with a constant initializer folds here; every other
/// shape stays on the runtime-selector routes.
fn constant_local_index(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<usize> {
    let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
        return None;
    };
    let source_state = crate::semantic::calls::find_state(program, state_symbol)?;
    let local = program
        .statement_table
        .statements(source_state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == name.symbol && !local.is_mutable => {
                Some(local)
            }
            _ => None,
        })?;
    usize::try_from(
        program
            .expression_table
            .constant_integer_value(local.initial_value)?,
    )
    .ok()
}

/// The dense scalar position a runtime-index selector names. The selector
/// expression must be one bare direct scalar parameter of the calling state
/// -- the coordinate `CheckedScalarExpression::Parameter` and the Terminal
/// caller's `parameters` vector share. The segment carries no bound: the
/// terminal verifier re-proves `0 <= index < extent` from the facts that
/// hold before the operation (the state's published requires, a dominating
/// guard), so admission here only resolves the selector.
fn runtime_index_parameter_position(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<u32> {
    let state = crate::semantic::calls::find_state(program, state_symbol)?;
    let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
        return None;
    };
    let parameters = program.state_parameters(state);
    let authored_position = crate::values::parameter_position(program, name, parameters)?;
    // A mutable selector's live value is storage, not the immutable entry
    // operand the dense position names; no storage read is minted for path
    // segments, so only an immutable parameter may be a selector. Terminal
    // replays the bound against an integer operand, so a non-integer
    // primitive (bool, float, address) is refused here.
    let parameter = &parameters[authored_position];
    if parameter.is_mutable
        || !matches!(
            program.primitive_type_reference(parameter.type_reference),
            Some(
                typed_trees::types::PrimitiveType::U8
                    | typed_trees::types::PrimitiveType::U16
                    | typed_trees::types::PrimitiveType::U32
                    | typed_trees::types::PrimitiveType::U64
                    | typed_trees::types::PrimitiveType::I8
                    | typed_trees::types::PrimitiveType::I16
                    | typed_trees::types::PrimitiveType::I32
                    | typed_trees::types::PrimitiveType::I64
            )
        )
    {
        return None;
    }
    let selector = parameters[..authored_position]
        .iter()
        .filter(|parameter| crate::values::occupies_scalar_position(program, parameter))
        .count();
    u32::try_from(selector).ok()
}

/// The checked coordinate for a runtime-index selector spelled as a field
/// read below one structural entry parameter of the calling state —
/// `self.count` in `self.scan_results[self.count]`. A field selector's value
/// is storage, not an operand: the place must be a pure record-field chain
/// rooted at an entry parameter (a `self` root counts by the machine
/// identity), and its leaf must be an integer primitive — the same integer
/// rule the parameter selector enforces. Terminal emits the read and
/// re-proves `0 <= value < extent` from the facts holding before the call;
/// admission here only resolves the selector's subject.
fn runtime_index_field_selector(
    program: &TypedTrees,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<checked_trees::CheckedRuntimeIndex> {
    let place = crate::flow::canonical_place_from_expression(program, expression)?;
    let facts::PlaceRoot::Symbol(root) =
        crate::flow::normalized_event_place_root(program, place.root)
    else {
        return None;
    };
    let parameters =
        program.state_parameters(crate::semantic::calls::find_state(program, state_symbol)?);
    let parameter_position = parameters.iter().position(|parameter| {
        crate::flow::normalized_event_place_root(
            program,
            facts::PlaceRoot::Symbol(parameter.symbol),
        ) == facts::PlaceRoot::Symbol(root)
            || (parameter.is_self && root == machine)
    })?;
    if place.segments.is_empty() {
        // A bare parameter selector is the `Parameter` coordinate, never a
        // field snapshot.
        return None;
    }
    let mut path = Vec::with_capacity(place.segments.len());
    for segment in &place.segments {
        let facts::PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        path.push(checked_trees::CheckedStructuralPredicatePathSegment::Field(
            terminal_field_identity(program, *symbol)?,
        ));
    }
    let leaf = crate::flow::canonical_place_type_reference(
        program,
        state_symbol,
        statement_index,
        &place,
    )?;
    if !matches!(
        program.primitive_type_reference(leaf),
        Some(
            typed_trees::types::PrimitiveType::U8
                | typed_trees::types::PrimitiveType::U16
                | typed_trees::types::PrimitiveType::U32
                | typed_trees::types::PrimitiveType::U64
                | typed_trees::types::PrimitiveType::I8
                | typed_trees::types::PrimitiveType::I16
                | typed_trees::types::PrimitiveType::I32
                | typed_trees::types::PrimitiveType::I64
        )
    ) {
        return None;
    }
    Some(checked_trees::CheckedRuntimeIndex::StructuralField(
        checked_trees::CheckedStructuralParameterField {
            parameter_position: u32::try_from(parameter_position).ok()?,
            path,
        },
    ))
}

/// The shared owner under the ordinary rule: the projected type must carry the
/// target parameter's exact normalized identity.
pub(crate) fn projected_argument_path_with_identity(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
    target_identity: &str,
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let (projected, path) = projected_argument_path(program, state_symbol, statement_index, place)?;
    (base_type_identity(program, projected, &[])? == target_identity).then_some(path)
}

/// The literal element count of a fixed array reached through any number of
/// domain constraints and borrows. A length that is not an authored literal
/// bounds no fixed index here.
/// `index` selects inside the container: a literal fixed-array length
/// bounds it directly here. A slice carries no declared extent — its
/// length is a live observation — so the hop stays unproven exactly the
/// way an emitted segment's Terminal obligation re-proves it against the
/// view's own witness; every other container declines.
fn fixed_index_within_extent(
    program: &TypedTrees,
    container: typed_trees::types::TypeReferenceHandle,
    index: usize,
) -> Option<()> {
    match fixed_array_literal_length(program, container) {
        Some(length) => (index < length).then_some(()),
        None => slice_type_reference(program, container).then_some(()),
    }
}

/// The window's bounds against a fixed byte-array container's literal
/// extent. The `FixedByteRange` segment names a byte view's own window, so
/// unlike the index arm a runtime-length slice cannot carry it — a stored
/// `&[u8]` has no literal bound to re-prove against.
pub(crate) fn fixed_byte_range_within_extent(
    program: &TypedTrees,
    container: typed_trees::types::TypeReferenceHandle,
    start: usize,
    end: usize,
) -> Option<()> {
    let mut container = container;
    while let TypeReferenceNode::Constrained { base_type, .. }
    | TypeReferenceNode::Reference {
        referee: base_type, ..
    } = program.type_reference_table.type_reference(container)
    {
        container = *base_type;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(container)
    else {
        return None;
    };
    if program.primitive_type_reference(*element_type)
        != Some(typed_trees::types::PrimitiveType::U8)
    {
        return None;
    }
    (start <= end && end <= usize::try_from(*length).ok()?).then_some(())
}

/// The container is a slice through any number of domain constraints and
/// borrows.
fn slice_type_reference(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    while let TypeReferenceNode::Constrained { base_type, .. }
    | TypeReferenceNode::Reference {
        referee: base_type, ..
    } = program.type_reference_table.type_reference(type_reference)
    {
        type_reference = *base_type;
    }
    matches!(
        program.type_reference_table.type_reference(type_reference),
        TypeReferenceNode::Slice { .. }
    )
}

fn fixed_array_literal_length(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<usize> {
    while let TypeReferenceNode::Constrained { base_type, .. }
    | TypeReferenceNode::Reference {
        referee: base_type, ..
    } = program.type_reference_table.type_reference(type_reference)
    {
        type_reference = *base_type;
    }
    let TypeReferenceNode::FixedArray {
        length: typed_trees::types::FixedArrayLength::Literal(length),
        ..
    } = program.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    Some(*length)
}

/// A range loan stays a projection of its original fixed-array backing. The
/// call owns its lifetime, so no independently live mutable descriptor is
/// manufactured. Runtime endpoints need separate extent evidence; this route
/// retains only canonical fixed bounds and rechecks the actual array extent.
pub(crate) fn fixed_byte_array_range_path(
    program: &TypedTrees,
    state: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
    target: TypeReferenceHandle,
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let (facts::PlaceSegment::FixedRange { start, end }, fields) = place.segments.split_last()?
    else {
        return None;
    };
    byte_array_range_path(
        program,
        state,
        statement_index,
        place,
        fields,
        *start,
        Some(*end),
        target,
    )
}

/// The open-ended spelling `x[..]`/`x[a..]` reaches the argument walk as a
/// runtime `Index` segment: its range end is not an authored constant, so
/// place canonicalization cannot pin it. Against a fixed byte array the
/// omitted end is the collection's literal extent, which this route resolves
/// from the backing type; a start that is present must still be an authored
/// constant. An end that survived canonicalization names a dynamic bound and
/// is not a fixed range.
pub(crate) fn open_byte_range_path(
    program: &TypedTrees,
    state: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
    target: TypeReferenceHandle,
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let (facts::PlaceSegment::Index { expression }, fields) = place.segments.split_last()? else {
        return None;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(*expression) else {
        return None;
    };
    let start = if range.start.is_valid() {
        usize::try_from(
            program
                .expression_table
                .constant_integer_value(range.start)?,
        )
        .ok()?
    } else {
        0
    };
    if range.end.is_valid() {
        return None;
    }
    byte_array_range_path(
        program,
        state,
        statement_index,
        place,
        fields,
        start,
        None,
        target,
    )
}

fn byte_array_range_path(
    program: &TypedTrees,
    state: SymbolHandle,
    statement_index: usize,
    place: &crate::flow::CanonicalPlace,
    fields: &[facts::PlaceSegment],
    start: usize,
    end: Option<usize>,
    target: TypeReferenceHandle,
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    if !fields
        .iter()
        .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. }))
    {
        return None;
    }
    let backing = crate::flow::CanonicalPlace {
        root: place.root,
        segments: fields.to_vec(),
    };
    let (backing_type, mut path) =
        projected_argument_path(program, state, statement_index, &backing)?;
    let extent = fixed_array_literal_length(program, backing_type)?;
    // An omitted end names the backing's literal extent.
    let end = end.unwrap_or(extent);
    if extent == 0
        || start > end
        || end > extent
        || !super::boundary_admission::fixed_byte_array_view_is_admitted(
            program,
            backing_type,
            target,
        )
    {
        return None;
    }
    path.push(CheckedUnitStructuralPathSegment::FixedByteRange {
        start: u64::try_from(start).ok()?,
        end: u64::try_from(end).ok()?,
    });
    Some(path)
}

pub(crate) fn ordinary_projected_call_is_supported(
    program: &TypedTrees,
    facts: &CheckFacts,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    call_site: &crate::semantic::calls::CallSite<'_>,
    arguments: &[CheckedUnitStructuralArgumentPlan],
    allow_field_path_projection: bool,
) -> bool {
    // A range descriptor's path selects its backing collection, already
    // checked by range admission. The callee receives the descriptor itself,
    // not a projected subloan, regardless of how the collection was reached.
    let projected = |argument: &CheckedUnitStructuralArgumentPlan| {
        !argument.path.is_empty() && !super::view_subslice::is_view_subslice(argument)
    };
    if !arguments.iter().any(projected) {
        return true;
    }
    // A retained result carrier presents its borrowed referent, not a moved
    // aggregate field. Exact producing local/loan custody was rejoined by the
    // shared result argument builder and is independently replayed in lowering.
    //
    // This is a rule about ONE argument, so it is written as one and applied
    // per argument below. Asking it of every projected argument at once meant
    // a call mixing the two projection sources supported neither: in
    // `RoomEvents::roll_event(&mut builder.events, depth, random, &mut
    // room.event)` the first is a parameter projection and the last a result
    // binding's, each disqualifying the other's rule.
    let retained_result_projection = |argument: &CheckedUnitStructuralArgumentPlan| {
        argument
            .source_structural_result_binding_ordinal()
            .is_some()
            && (argument.path.split_last().is_some_and(|(last, prefix)| {
                *last == CheckedUnitStructuralPathSegment::Referent
                    && prefix.iter().all(|segment| {
                        matches!(segment, CheckedUnitStructuralPathSegment::Field(_))
                    })
            }) || argument.path.split_first().is_some_and(|(first, suffix)| {
                // A returned plain-record permission lends a static child
                // without moving the carrier or creating an escaping loan.
                *first == CheckedUnitStructuralPathSegment::Referent
                    && !suffix.is_empty()
                    && suffix.iter().all(|segment| {
                        matches!(
                            segment,
                            CheckedUnitStructuralPathSegment::Field(_)
                                | CheckedUnitStructuralPathSegment::FixedIndex(_)
                        )
                    })
            }))
            && argument.access == CheckedStructuralAccess::MutableBorrow
    };
    if arguments
        .iter()
        .all(|argument| !projected(argument) || retained_result_projection(argument))
    {
        return true;
    }

    let caller_source_parameters = program.state_parameters(caller_state);
    let target_source_parameters = program.state_parameters(target_state);
    // Before completed-signature reconciliation, explicit operands retain
    // every authored formal. Only an implicit borrowed subject is absent.
    let explicit_subject = crate::semantic::calls::call_site_supplies_explicit_subject(
        program,
        call_site,
        target_source_parameters,
    );
    let target_parameters = target_source_parameters
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && !(!explicit_subject
                    && parameter.is_self
                    && is_reference(program, parameter.type_reference))
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .collect::<Vec<_>>();
    if target_parameters.len() != arguments.len() {
        return false;
    }
    // A live result binding lends a record subtree through a borrow: the
    // binding itself is not moved, the checked loan evidence was already
    // rejoined by the result-argument builder, and lowering replays the same
    // projected place against its own result roster. A shared loan
    // (`&dash.engine`) is admitted alongside the exclusive ones; the equality
    // with the callee parameter's own access below keeps a shared loan from
    // reaching a mutable parameter or the reverse.
    if target_machine.supply_mode == MachineSupplyMode::CheckedBody
        && arguments
            .iter()
            .zip(&target_parameters)
            .all(|(argument, target)| {
                if !projected(argument) {
                    return true;
                }
                argument
                    .source_structural_result_binding_ordinal()
                    .is_some()
                    && argument.path.iter().all(|segment| {
                        matches!(
                            segment,
                            CheckedUnitStructuralPathSegment::Field(_)
                                | CheckedUnitStructuralPathSegment::FixedIndex(_)
                                | CheckedUnitStructuralPathSegment::FixedByteRange { .. }
                                | CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                        )
                    })
                    && matches!(
                        argument.access,
                        CheckedStructuralAccess::SharedBorrow
                            | CheckedStructuralAccess::MutableBorrow
                            | CheckedStructuralAccess::WriteOnlyBorrow
                    )
                    && structural_access_for_type_reference(program, target.type_reference)
                        == Some(argument.access)
            })
    {
        return true;
    }
    let has_content_evidence = |machine, state| {
        facts
            .qualifications
            .content
            .identity_reshuffles
            .iter()
            .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
            || facts
                .qualifications
                .content
                .partition_compositions
                .iter()
                .any(|fact| fact.machine_symbol == machine && fact.state_symbol == state)
    };
    if has_content_evidence(caller_machine.symbol, caller_state.symbol)
        || has_content_evidence(target_machine.symbol, target_state.symbol)
        || arguments
            .iter()
            .zip(&target_parameters)
            .any(|(argument, parameter)| {
                projected(argument)
                    && target_contract_mentions_projected_parameter(
                        program,
                        facts,
                        target_machine,
                        target_state,
                        parameter,
                    )
            })
    {
        return false;
    }

    // Argument construction has already rejoined each canonical place, exact
    // referent type, and authored loan. Borrowed projections compose per
    // argument; neither the number of sibling loans nor the caller's state
    // count grants or removes access to those places.
    if target_machine.supply_mode == MachineSupplyMode::CheckedBody
        && arguments
            .iter()
            .zip(&target_parameters)
            .all(|(argument, target)| {
                if !projected(argument) {
                    return true;
                }
                // An argument the result-binding rule already admits composes
                // beside a parameter projection. Requiring every projected
                // argument to reach THIS rule is what made a mixed call
                // unsupported even though each of its arguments is supported.
                if retained_result_projection(argument) {
                    return true;
                }
                let Some(source) = argument
                    .source_parameter_index()
                    .and_then(|index| caller_parameters.get(index as usize))
                else {
                    return false;
                };
                if !source.qualifications.is_empty()
                    || caller_source_parameters
                        .get(source.position as usize)
                        .is_none()
                    || structural_access_for_type_reference(program, target.type_reference)
                        != Some(argument.access)
                {
                    return false;
                }
                let field_path = checked_nonempty_field_path(&argument.path);
                let indexed_fields = checked_literal_index_path(&argument.path);
                let static_path = argument.path.iter().all(|segment| {
                    matches!(
                        segment,
                        CheckedUnitStructuralPathSegment::Field(_)
                            | CheckedUnitStructuralPathSegment::Case(_)
                            | CheckedUnitStructuralPathSegment::FixedIndex(_)
                            | CheckedUnitStructuralPathSegment::FixedByteRange { .. }
                    )
                });
                // Both readable loan kinds share geometry. Runtime selectors
                // retain their own bounds contract; static case selection keeps
                // the exact case and field for membership verification.
                let borrow_path = static_path
                    || (argument.path.iter().all(|segment| {
                        matches!(
                            segment,
                            CheckedUnitStructuralPathSegment::Field(_)
                                | CheckedUnitStructuralPathSegment::FixedIndex(_)
                                | CheckedUnitStructuralPathSegment::RuntimeIndex(_)
                        )
                    }) && argument.path.iter().any(|segment| {
                        matches!(segment, CheckedUnitStructuralPathSegment::RuntimeIndex(_))
                    }));
                match argument.access {
                    CheckedStructuralAccess::SharedBorrow => {
                        // A shared subloan replays statically or through a
                        // runtime index: the verifier re-proves each
                        // selector's bound against the array it selects in.
                        if source.multiplicity == Multiplicity::Unrestricted {
                            crate::checks::type_multiplicity(program, target.type_reference)
                                == Multiplicity::Unrestricted
                                && borrow_path
                        } else {
                            // An affine source lends a shared view of one
                            // subtree: custody of the root stays with the
                            // caller's place, so the loan presents
                            // unrestricted. Terminal verification admits the
                            // subloan only when the source is a borrow or an
                            // owned affine root.
                            source.multiplicity == Multiplicity::Affine
                                && matches!(
                                    source.access,
                                    CheckedStructuralAccess::SharedBorrow
                                        | CheckedStructuralAccess::MutableBorrow
                                        | CheckedStructuralAccess::Owned
                                )
                                && crate::checks::type_multiplicity(program, target.type_reference)
                                    == Multiplicity::Unrestricted
                                && borrow_path
                        }
                    }
                    CheckedStructuralAccess::MutableBorrow => {
                        // An exclusive subloan replays statically or through
                        // a runtime index over the same element the
                        // verification resolves; the selector's bound is
                        // re-proven against its owning array.
                        source.access == CheckedStructuralAccess::MutableBorrow
                            && ((source.multiplicity == Multiplicity::Unrestricted && borrow_path)
                                || ((field_path
                                    || indexed_fields.is_some_and(|fields| !fields.is_empty()))
                                    && byte_sequence_carrier(program, target.type_reference, &[])
                                        .is_some_and(|carrier| {
                                            carrier.is_borrowed()
                                                // A bounded owner field can be lent out of an
                                                // affine record without moving its siblings.
                                                // Argument construction retains the exact loan;
                                                // receiving replay checks capacity and restores
                                                // both directions of its predicate contract.
                                                || (source.multiplicity == Multiplicity::Affine
                                                    && matches!(
                                                        carrier,
                                                        checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                                                    ))
                                        })))
                    }
                    CheckedStructuralAccess::WriteOnlyBorrow => {
                        matches!(
                            source.access,
                            CheckedStructuralAccess::MutableBorrow
                                | CheckedStructuralAccess::WriteOnlyBorrow
                        ) && source.multiplicity == Multiplicity::Unrestricted
                            && static_path
                    }
                    CheckedStructuralAccess::Owned => false,
                }
            })
    {
        return true;
    }
    // A dying-continuation consumer may mix several projected operands with
    // ordinary whole operands. Each projected anonymous temporary rejoins its
    // own result through `binding_ordinal`, and the appended call
    // continuation keeps each owner's residual rows separately. A projected
    // owned parameter joins the same operand shape; its untouched complement
    // stays with the caller and publishes on the caller's return edge
    // instead of the call continuation.
    if allow_field_path_projection
        && target_machine.supply_mode == MachineSupplyMode::CheckedBody
        && arguments.iter().any(|argument| {
            (argument
                .source_structural_result_binding_ordinal()
                .is_some()
                || argument.source_parameter_index().is_some())
                && argument.access == CheckedStructuralAccess::Owned
                && !argument.path.is_empty()
        })
        && arguments
            .iter()
            .zip(&target_parameters)
            .all(|(argument, target)| {
                let result_projection = argument
                    .source_structural_result_binding_ordinal()
                    .is_some();
                // The projected parameter root must itself be claim-free
                // owned affine: its complement publishes residual rows on
                // the caller's return edge, which has no vocabulary for
                // borrowed or qualified custody.
                let parameter_projection = !result_projection
                    && !argument.path.is_empty()
                    && argument.source_parameter_index().is_some_and(|index| {
                        caller_parameters.get(index as usize).is_some_and(|source| {
                            source.access == CheckedStructuralAccess::Owned
                                && source.multiplicity == Multiplicity::Affine
                                && source.qualifications.is_empty()
                        })
                    });
                if result_projection || parameter_projection {
                    return argument.access == CheckedStructuralAccess::Owned
                        && !argument.path.is_empty()
                        && argument.path.iter().all(|segment| {
                            matches!(
                                segment,
                                CheckedUnitStructuralPathSegment::Field(_)
                                    | CheckedUnitStructuralPathSegment::FixedIndex(_)
                            )
                        })
                        && !target.is_self
                        && crate::checks::type_multiplicity(program, target.type_reference)
                            == Multiplicity::Affine
                        && !type_graph_requires_nominal_drop(program, target.type_reference);
                }
                !projected(argument)
            })
        && program.machine_states(caller_machine).len() == 1
        && program.machine_states(target_machine).len() == 1
        && facts
            .contract_plans
            .for_machine(caller_machine.symbol)
            .is_some()
        && facts
            .contract_plans
            .for_machine(target_machine.symbol)
            .is_some()
    {
        return true;
    }
    if arguments.len() != 1 {
        return false;
    }
    // Only the partial-cleanup owner enables projections from its exact
    // earlier result roster. Ordinary whole-value sequencing leaves it off.
    let result_projection = allow_field_path_projection
        && arguments[0]
            .source_structural_result_binding_ordinal()
            .is_some()
        && arguments[0].access == CheckedStructuralAccess::Owned;
    let caller_parameter = if result_projection {
        None
    } else {
        // A projected owned parameter names its root by index; the caller
        // may carry any number of structural siblings because the residual
        // complement publishes on the caller's return edge, not the call.
        let Some(parameter_index) = arguments[0].source_parameter_index() else {
            return false;
        };
        caller_parameters.get(parameter_index as usize)
    };
    if !result_projection
        && caller_parameter
            .and_then(|parameter| {
                usize::try_from(parameter.position)
                    .ok()
                    .and_then(|position| caller_source_parameters.get(position))
            })
            .is_none()
    {
        return false;
    };

    let owned_affine_projection = allow_field_path_projection
        && (result_projection
            || caller_parameter.is_some_and(|parameter| {
                parameter.access == CheckedStructuralAccess::Owned
                    && parameter.multiplicity == Multiplicity::Affine
            }))
        && arguments[0].access == CheckedStructuralAccess::Owned
        && !arguments[0].path.is_empty()
        && arguments[0].path.iter().all(|segment| {
            matches!(
                segment,
                CheckedUnitStructuralPathSegment::Field(_)
                    | CheckedUnitStructuralPathSegment::FixedIndex(_)
            )
        });
    // Naming the projected root by index among siblings is the affine
    // carrier's privilege: only it can publish the untouched complement on
    // the caller's return edge. The legacy indexed cohort — a linear or
    // otherwise non-affine root — has no residual lane, so its caller must
    // still carry exactly one structural parameter and project that one.
    if !result_projection
        && !owned_affine_projection
        && (caller_parameters.len() != 1
            || caller_source_parameters.len() != 1
            || arguments[0].source_parameter_index() != Some(0))
    {
        return false;
    }
    let field_path = checked_nonempty_field_path(&arguments[0].path);
    let literal_index_fields = checked_literal_index_path(&arguments[0].path);
    let literal_index_path = literal_index_fields.is_some();
    let literal_indexed_field_path = literal_index_fields.is_some_and(|fields| !fields.is_empty());
    if field_path && !allow_field_path_projection {
        return false;
    }
    if literal_indexed_field_path && !owned_affine_projection {
        return false;
    }
    if !field_path && !literal_index_path && !owned_affine_projection {
        return false;
    }

    if field_path || owned_affine_projection {
        let [target_parameter] = target_parameters.as_slice() else {
            return false;
        };
        return program.machine_states(caller_machine).len() == 1
            && program.machine_states(target_machine).len() == 1
            && (result_projection
                || caller_parameter.is_some_and(|parameter| {
                    parameter.multiplicity == Multiplicity::Affine
                        && parameter.qualifications.is_empty()
                }))
            && !target_parameter.is_self
            && crate::checks::type_multiplicity(program, target_parameter.type_reference)
                == Multiplicity::Affine
            && !type_graph_requires_nominal_drop(program, target_parameter.type_reference)
            && facts
                .contract_plans
                .for_machine(caller_machine.symbol)
                .is_some()
            && facts
                .contract_plans
                .for_machine(target_machine.symbol)
                .is_some();
    }

    arguments
        .iter()
        .zip(target_parameters)
        .filter(|(argument, _)| !argument.path.is_empty())
        .all(|(_, parameter)| {
            let mut type_reference = parameter.type_reference;
            loop {
                match program.type_reference_table.type_reference(type_reference) {
                    TypeReferenceNode::Constrained {
                        base_type,
                        constraints,
                    } => {
                        if !program
                            .type_reference_table
                            .constraints(*constraints)
                            .is_empty()
                        {
                            return false;
                        }
                        type_reference = *base_type;
                    }
                    TypeReferenceNode::Reference { referee, .. } => type_reference = *referee,
                    _ => break,
                }
            }

            let expected_root =
                facts::PlaceRoot::Symbol(parameter_root_symbol(target_machine.symbol, parameter));
            let matching = facts
                .flow
                .ownership
                .permissions
                .iter()
                .filter(|(_, event)| {
                    event.machine_symbol == target_machine.symbol
                        && event.state_symbol == target_state.symbol
                        && event.source == PermissionEventSource::StateEntry
                        && event.kind == PermissionEventKind::Establish
                        && event.access == PermissionAccess::Owned
                        && event.multiplicity == Multiplicity::Linear
                        && event.obligation_live
                        && event.root == expected_root
                })
                .map(|(_, event)| event)
                .collect::<Vec<_>>();
            let [claim] = matching.as_slice() else {
                return false;
            };
            claim.claim_identity != PermissionClaimIdentity::Unknown
                && facts
                    .flow
                    .ownership
                    .segments
                    .span_or_empty(claim.segments)
                    .is_empty()
        })
}

pub(crate) fn target_contract_mentions_projected_parameter(
    program: &TypedTrees,
    facts: &CheckFacts,
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    parameter: &StateParameter,
) -> bool {
    let expected_root = parameter_root_symbol(target_machine.symbol, parameter);
    let runtime_arithmetic_requires_are_terminal = facts
        .contract_plans
        .for_machine(target_machine.symbol)
        .is_some_and(|contract| {
            contract.crash.uses_structural_proof_gated_arithmetic()
                && contract.crash.structural_runtime_requirements().is_some()
        });
    let authored_contract_mentions_parameter = program
        .state_contracts(target_state)
        .iter()
        .filter(|contract| match contract.kind {
            SignatureContractKind::Crashes { .. } => false,
            SignatureContractKind::Requires if runtime_arithmetic_requires_are_terminal => false,
            SignatureContractKind::Requires
            | SignatureContractKind::Ensures
            | SignatureContractKind::EnsuresForResultCase { .. } => true,
        })
        .flat_map(|contract| {
            program
                .proof_facts
                .span_or_empty(contract.facts)
                .iter()
                .map(move |fact| {
                    (
                        matches!(contract.kind, SignatureContractKind::Requires),
                        fact,
                    )
                })
        })
        .any(|(requires, fact)| {
            let ProofFact::Membership(membership) = fact else {
                return false;
            };
            crate::flow::canonical_place_from_expression_in_state(
                program,
                target_state.symbol,
                0,
                membership.value,
            )
            .is_some_and(|place| {
                (place.root == facts::PlaceRoot::Symbol(expected_root)
                    || place.root == facts::PlaceRoot::Symbol(parameter.symbol))
                    && !(requires
                        && place.segments.is_empty()
                        && byte_parameter_qualification_carries_membership(
                            program, parameter, membership,
                        ))
            })
        });
    if authored_contract_mentions_parameter {
        return true;
    }

    facts
        .contract_plans
        .for_machine(target_machine.symbol)
        .is_some_and(|contract| {
            contract.crash.published().iter().any(|bucket| {
                bucket.alternative_guards().iter().any(|guard| match guard {
                    checked_trees::CrashRouteGuard::Truth => false,
                    checked_trees::CrashRouteGuard::Predicate(predicate) => {
                        if matches!(
                            predicate.scalar_expression(),
                            Some(
                                checked_trees::CheckedBooleanExpression::StructuralParameterField {
                                    parameter_position: 0,
                                    path,
                                }
                            ) if !path.is_empty()
                        ) {
                            return false;
                        }
                        if predicate.expression().is_some_and(|expression| {
                            crash_expression_is_nonempty_member_path_from_parameter(expression, 0)
                        }) {
                            return false;
                        }
                        predicate.expression().is_none_or(|expression| {
                            crash_expression_mentions_parameter_outside_member_path(expression, 0)
                        })
                    }
                })
            })
        })
}

/// A declared byte qualification also appears as a source requires fact. Its
/// exact domain is already carried by the formal and independently checked by
/// bounded-owner loan admission, or for a shared view formal by the stored
/// view field's declared predicates; it does not need a second
/// projected-contract representation. Other memberships and all exit
/// contracts still need theirs.
fn byte_parameter_qualification_carries_membership(
    program: &TypedTrees,
    parameter: &StateParameter,
    membership: &typed_trees::domain::ProofMembershipFact,
) -> bool {
    let carrier_carries_domain = match byte_sequence_carrier(program, parameter.type_reference, &[])
    {
        Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }) => true,
        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }) => {
            structural_access_for_type_reference(program, parameter.type_reference)
                == Some(CheckedStructuralAccess::SharedBorrow)
        }
        _ => false,
    };
    if !carrier_carries_domain
        || !membership.semantic_domain.is_valid()
        || !membership.domain_arguments.is_empty()
        || !program.domain_definitions().iter().any(|definition| {
            definition.symbol == membership.domain_symbol
                && definition.semantic_id == membership.semantic_domain
        })
        || typed_trees::byte_predicates::domain_byte_predicate(program, membership.domain_symbol)
            .is_none()
    {
        return false;
    }
    let mut reference = parameter.type_reference;
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained {
                base_type,
                constraints,
            } => {
                if program
                    .type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .any(|constraint| {
                        matches!(constraint, typed_trees::types::TypeConstraintNode::Domain(domain)
                        if domain.symbol == membership.domain_symbol
                            && domain.semantic_id == membership.semantic_domain
                            && domain.arguments.is_empty())
                    })
                {
                    return true;
                }
                reference = *base_type;
            }
            _ => return false,
        }
    }
}

pub(crate) fn crash_expression_is_nonempty_member_path_from_parameter(
    expression: &checked_trees::CrashPredicateExpression,
    parameter: u32,
) -> bool {
    use checked_trees::CrashPredicateExpression;

    let mut expression = expression;
    let mut nonempty = false;
    while let CrashPredicateExpression::Member { receiver, .. } = expression {
        nonempty = true;
        expression = receiver;
    }
    nonempty
        && matches!(expression, CrashPredicateExpression::Parameter(index) if *index == parameter)
}

pub(crate) fn crash_expression_mentions_parameter_outside_member_path(
    expression: &checked_trees::CrashPredicateExpression,
    parameter: u32,
) -> bool {
    use checked_trees::CrashPredicateExpression;

    match expression {
        CrashPredicateExpression::Parameter(index) => *index == parameter,
        CrashPredicateExpression::Binary { left, right, .. } => {
            crash_expression_mentions_parameter_outside_member_path(left, parameter)
                || crash_expression_mentions_parameter_outside_member_path(right, parameter)
        }
        CrashPredicateExpression::Unary { operand, .. }
        | CrashPredicateExpression::IntegerWiden { operand, .. } => {
            crash_expression_mentions_parameter_outside_member_path(operand, parameter)
        }
        CrashPredicateExpression::Member { receiver, .. } => {
            if crash_expression_is_nonempty_member_path_from_parameter(expression, parameter) {
                false
            } else {
                crash_expression_mentions_parameter_outside_member_path(receiver, parameter)
            }
        }
        CrashPredicateExpression::Indexed { collection, index } => {
            crash_expression_mentions_parameter_outside_member_path(collection, parameter)
                || crash_expression_mentions_parameter_outside_member_path(index, parameter)
        }
        CrashPredicateExpression::Range { start, end, .. } => {
            crash_expression_mentions_parameter_outside_member_path(start, parameter)
                || crash_expression_mentions_parameter_outside_member_path(end, parameter)
        }
        CrashPredicateExpression::Call {
            receiver,
            arguments,
            ..
        } => {
            crash_expression_mentions_parameter_outside_member_path(receiver, parameter)
                || arguments.iter().any(|argument| {
                    crash_expression_mentions_parameter_outside_member_path(argument, parameter)
                })
        }
        CrashPredicateExpression::Invalid
        | CrashPredicateExpression::Opaque(_)
        | CrashPredicateExpression::ContentConservation(_) => true,
        CrashPredicateExpression::Integer(_)
        | CrashPredicateExpression::Float(_)
        | CrashPredicateExpression::Boolean(_)
        | CrashPredicateExpression::Name(_) => false,
    }
}

pub(crate) fn byte_sequence_literal_argument(
    program: &TypedTrees,
    parameter_type: TypeReferenceHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let ExpressionNode::String(bytes) = program.expression_table.expression(expression) else {
        return None;
    };
    let access = structural_access_for_type_reference(program, parameter_type)?;
    match (byte_sequence_carrier(program, parameter_type, &[])?, access) {
        (
            checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. },
            CheckedStructuralAccess::SharedBorrow,
        ) => {}
        (
            checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity },
            CheckedStructuralAccess::Owned,
        ) if program.type_multiplicity(parameter_type) == Multiplicity::Unrestricted
            && u64::try_from(bytes.len()).ok()? <= capacity =>
        {
            // This is construction at the formal, not ownership of immutable
            // image storage. Lowering must establish fresh bounded backing.
            for symbol in
                crate::facts::field_domain::domain_constraint_symbols(program, parameter_type)
            {
                let domain = program
                    .domain_definitions()
                    .iter()
                    .find(|domain| domain.symbol == symbol)?;
                if !domain.establishment_routes.is_empty()
                    || domain.semantic_roles != Default::default()
                    || !crate::facts::field_domain::string_literal_expression_grants_domain(
                        program, expression, symbol,
                    )
                {
                    return None;
                }
            }
        }
        _ => return None,
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
            bytes: bytes.to_vec(),
        },
        path: Vec::new(),
        type_identity: byte_sequence_type_identity(program, parameter_type, &[], &[])?,
        access,
    })
}
