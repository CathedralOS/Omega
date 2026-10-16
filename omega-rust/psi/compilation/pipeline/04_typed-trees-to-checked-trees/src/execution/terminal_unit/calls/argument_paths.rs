//! Checked scalar arguments and projected argument paths: field and index
//! paths, projected call support, contract mentions and crash expressions.

use crate::execution::terminal_unit::types::byte_sequence_carrier;
use crate::execution::terminal_unit::types::{
    base_type_identity, byte_sequence_type_identity, structural_access_for_type_reference,
    terminal_field_identity,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarParameterPlan, CheckedUnitCallCoordinate,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, Multiplicity, SignatureContractKind, StateParameter,
    StatementNode, SymbolHandle, TypeReferenceHandle, TypeReferenceNode,
};
use typed_trees::expression::ExpressionHandle;

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
    program: &dyn ::validation::CheckingInput,
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
            proof_term_type_matches(program, caller_state, term, parameter.type_identity)
                .then(|| term.clone())
        })
        .collect()
}

/// The construction or forwarded formal carries exactly the formal's declared
/// proof-only data identity.
fn proof_term_type_matches(
    program: &dyn ::validation::CheckingInput,
    caller_state: SymbolHandle,
    term: &checked_trees::CheckedProofTerm,
    type_identity: typed_trees::type_identity::TypeIdentityId,
) -> bool {
    match term {
        checked_trees::CheckedProofTerm::Construction {
            type_identity: actual,
            ..
        }
        | checked_trees::CheckedProofTerm::Array {
            type_identity: actual,
            ..
        } => *actual == type_identity,
        checked_trees::CheckedProofTerm::Formal { parameter_symbol } => {
            program.state_by_symbol(caller_state).is_some_and(|state| {
                program.state_parameters(state).iter().any(|parameter| {
                    parameter.symbol == *parameter_symbol
                        && base_type_identity(program, parameter.type_reference, &[], &[])
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
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    projected_argument_path(
        program,
        state_symbol,
        statement_index,
        place,
        Some(machine),
        None,
    )
}

/// Select the lent endpoint, retaining each stored-pointer crossing beside its
/// type. Call operands use this after ordinary geometry resolution regardless
/// of whether their home came from a parameter, local or completed producer.
/// View descriptors already name their carriers and never gain a pointer hop.
pub(crate) fn borrowed_argument_endpoint(
    program: &dyn ::validation::CheckingInput,
    mut selected: TypeReferenceHandle,
    mut path: Vec<CheckedUnitStructuralPathSegment>,
    formal: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Vec<CheckedUnitStructuralPathSegment>)> {
    let Some(requested) = validation::reference_shell(program, formal) else {
        return Some((selected, path));
    };
    while let Some(shell) = validation::reference_shell(program, selected)
        && validation::stores_reference_shape(program, selected, &[])
    {
        if shell.access != requested.access
            && shell.access != language_semantics::ReferenceAccess::Mutable
        {
            return None;
        }
        path.push(CheckedUnitStructuralPathSegment::Referent);
        selected = shell.referee;
    }
    Some((selected, path))
}

/// `projected_borrowed_path` for an observation of a runtime-selected
/// element, whose selector may also be an immutable scalar local:
/// `local_selector` resolves one selector expression to its `Local` position
/// in the observing computation's own scalar namespace. Only the observation
/// owner holds that namespace. The path ends at the place itself, so a
/// reference-typed place names its descriptor: a byte operand observes the
/// view (base and extent), not the bytes behind it.
pub(crate) fn projected_selected_path(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    local_selector: &dyn Fn(
        typed_trees::expression::ExpressionHandle,
    ) -> Option<checked_trees::CheckedRuntimeIndex>,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    projected_argument_path(
        program,
        state_symbol,
        statement_index,
        place,
        Some(machine),
        Some(&|expression| {
            local_selector(expression).map(CheckedUnitStructuralPathSegment::RuntimeIndex)
        }),
    )
}

/// `projected_selected_path` for a case observation, which reads the case
/// value's storage rather than a descriptor. A retained local descriptor is
/// crossed here; a borrowed parameter already denotes its referee's storage.
/// A view is itself the indexed carrier, not a Reference to that carrier.
pub(crate) fn projected_observed_path(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    local_selector: &dyn Fn(
        typed_trees::expression::ExpressionHandle,
    ) -> Option<checked_trees::CheckedRuntimeIndex>,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    let (mut projected, mut path) = projected_selected_path(
        program,
        machine,
        state_symbol,
        statement_index,
        place,
        local_selector,
    )?;
    if let facts::PlaceRoot::Symbol(root) = place.root
        && let Some(state) = program.state_by_symbol(state_symbol)
        && let Some((ordinal, local)) = program.state_local_declaration(state, root)
        && ordinal < statement_index
    {
        let mut reference = local.type_reference;
        let mut crossings = Vec::new();
        while let Some(shell) = validation::reference_shell(program, reference) {
            if shell.access == language_semantics::ReferenceAccess::WriteOnly {
                return None;
            }
            if !validation::stores_reference_shape(program, reference, &[]) {
                break;
            }
            crossings.push(CheckedUnitStructuralPathSegment::Referent);
            reference = shell.referee;
        }
        crossings.append(&mut path);
        path = crossings;
    }
    while let Some(shell) = validation::reference_shell(program, projected) {
        if shell.access == language_semantics::ReferenceAccess::WriteOnly {
            return None;
        }
        // The local root crossing above already reaches its direct referee.
        // A projected stored reference is a distinct descriptor at the leaf.
        if place
            .segments
            .iter()
            .any(|segment| !matches!(segment, facts::PlaceSegment::Case { .. }))
        {
            path.push(CheckedUnitStructuralPathSegment::Referent);
        }
        projected = shell.referee;
    }
    Some((projected, path))
}

/// A computation owner supplies executable selectors at the same projected
/// storage join. Other callers pass no evaluator and cannot invent a value.
pub(crate) fn projected_evaluated_path(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    place: &validation::CanonicalPlace,
    selector: Option<
        &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<CheckedUnitStructuralPathSegment>,
    >,
) -> Option<(TypeReferenceHandle, Vec<CheckedUnitStructuralPathSegment>)> {
    projected_argument_path(program, state, statement, place, Some(machine), selector)
}

/// The segment prefix ending before `position` re-rooted as its own place and
/// resolved to its container type — the descend every member, index, window or
/// case segment takes to learn the extent it must stay inside.
fn prefix_container_type_reference(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    position: usize,
) -> Option<TypeReferenceHandle> {
    validation::canonical_place_type_reference(
        program,
        state_symbol,
        statement_index,
        &validation::CanonicalPlace {
            root: place.root,
            segments: place.segments[..position].to_vec(),
        },
    )
}

pub(crate) fn projected_argument_path(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    runtime_index_machine: Option<SymbolHandle>,
    local_selector: Option<
        &dyn Fn(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<CheckedUnitStructuralPathSegment>,
    >,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    Vec<CheckedUnitStructuralPathSegment>,
)> {
    let mut path = Vec::with_capacity(place.segments.len());
    // A borrowed root's own declared reference shells denote storage the
    // rooted subject already names — a `&`-parameter's member projection
    // does not re-cross them. Case segments leave the root's type intact,
    // so while the prefix has only selected cases, every shell the
    // container reports is the root's own. Once a member, element, or
    // window descend has projected a new leaf type, any shell it carries
    // belongs to a stored reference the path actually crossed.
    let inherited_crossings = {
        let mut reference = validation::canonical_place_type_reference(
            program,
            state_symbol,
            statement_index,
            &validation::CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )?;
        let mut count = 0usize;
        while let Some(shell) = validation::reference_shell(program, reference)
            && validation::stores_reference_shape(program, reference, &[])
        {
            count += 1;
            reference = shell.referee;
        }
        count
    };
    let mut payload_field = None;
    for (position, segment) in place.segments.iter().enumerate() {
        // Parameters already denote their borrowed storage. Below that root,
        // a stored Reference shape retains a pointer crossing. Slice and
        // byte-view shells already store descriptors, so indexing them must
        // not invent another Referent hop. Use the shape collector's same
        // classification before selecting the next member. The canonical source
        // place keeps the authored field path for borrow-fact correspondence.
        if position > 0
            && matches!(
                segment,
                facts::PlaceSegment::Field { .. }
                    | facts::PlaceSegment::FixedIndex { .. }
                    | facts::PlaceSegment::Index { .. }
            )
        {
            let mut reference = prefix_container_type_reference(
                program,
                state_symbol,
                statement_index,
                place,
                position,
            )?;
            let mut crossings = 0usize;
            while let Some(shell) = validation::reference_shell(program, reference)
                && validation::stores_reference_shape(program, reference, &[])
            {
                crossings += 1;
                reference = shell.referee;
            }
            let inherited = place.segments[..position]
                .iter()
                .all(|prefix| matches!(prefix, facts::PlaceSegment::Case { .. }))
                .then_some(inherited_crossings)
                .unwrap_or(0);
            for _ in 0..crossings.saturating_sub(inherited) {
                path.push(CheckedUnitStructuralPathSegment::Referent);
            }
        }
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
                let container = prefix_container_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    place,
                    position,
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
                    let container = prefix_container_type_reference(
                        program,
                        state_symbol,
                        statement_index,
                        place,
                        position,
                    )?;
                    fixed_index_within_extent(program, container, index)?;
                    path.push(CheckedUnitStructuralPathSegment::FixedIndex(
                        u64::try_from(index).ok()?,
                    ));
                    continue;
                }
                // The runtime-selector route is admitted only in a machine
                // context: `projected_argument_path` passes `None` and refuses a
                // runtime index outright, while the borrowed and observed routes
                // admit one. The context's symbol itself is no longer read — a
                // parameter- or field-rooted selector roots at the parameter it
                // names, since static machines replaced the receiver — so this
                // tests the argument's presence and nothing else. Removing the
                // test would admit runtime selectors on the argument route.
                runtime_index_machine?;
                // An executable operand owner supplies the occurrence's
                // evaluation. Only a path without that owner may fall back
                // to a parameter or field snapshot of the current frame.
                let runtime_index = local_selector
                    .and_then(|resolve| resolve(*expression))
                    .or_else(|| {
                        runtime_index_parameter_position(program, state_symbol, *expression)
                            .map(|position| checked_trees::CheckedRuntimeIndex::Parameter {
                                position,
                            })
                            .or_else(|| {
                                runtime_index_field_selector(
                                    program,
                                    state_symbol,
                                    statement_index,
                                    *expression,
                                )
                            })
                            .map(CheckedUnitStructuralPathSegment::RuntimeIndex)
                    })?;
                path.push(runtime_index);
            }
            facts::PlaceSegment::OpaqueRange => return None,
            facts::PlaceSegment::FixedRange { start, end } => {
                // A borrowed byte view's window is itself part of the place:
                // `self.source[0..2]` lends the fixed byte range the view
                // names. The container must be fixed byte-array backing whose
                // literal extent bounds the window — the same shape the
                // `FixedIndex` walk proves — since a stored view carries no
                // runtime bound evidence of its own.
                let container = prefix_container_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    place,
                    position,
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
                let reference = prefix_container_type_reference(
                    program,
                    state_symbol,
                    statement_index,
                    place,
                    position,
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
                path.push(CheckedUnitStructuralPathSegment::Case(
                    checked_trees::CheckedMemberIdentity::of_member(
                        case.identity,
                        case.name.clone(),
                    ),
                ));
                payload_field = Some((
                    *symbol,
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
                ));
            }
        }
    }
    let projected =
        validation::canonical_place_type_reference(program, state_symbol, statement_index, place)?;
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
    program: &dyn ::validation::CheckingInput,
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
        .or_else(|| runtime_index_field_selector(program, state, statement_index, expression))?;
    Some(CheckedUnitStructuralPathSegment::RuntimeIndex(
        runtime_index,
    ))
}

/// A value projection consumes the same checked scalar operands as other
/// values, not an assignment destination's pre-RHS capture slot. Dense
/// parameter/local positions and declared field observations retain their
/// scalar namespace. A storage source names the established home to read at
/// this program point; it never substitutes the declaration's initializer.
pub(crate) fn value_index_segment(
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<CheckedUnitStructuralPathSegment> {
    let reference = validation::expression_type_reference_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    )?;
    let primitive = program.primitive_type_reference(reference)?;
    if !primitive.is_fixed_integer() {
        return None;
    }
    let operand = crate::values::lower_unit_scalar_argument(
        program,
        operators,
        state,
        statement_index,
        expression,
        primitive,
    )?;
    let runtime = match operand {
        checked_trees::CheckedScalarExpression::StorageRead { symbol, .. } => {
            checked_trees::CheckedRuntimeIndex::StorageRead { symbol }
        }
        checked_trees::CheckedScalarExpression::Parameter { position, .. } => {
            checked_trees::CheckedRuntimeIndex::Parameter {
                position: u32::try_from(position).ok()?,
            }
        }
        checked_trees::CheckedScalarExpression::Local { position, .. } => {
            checked_trees::CheckedRuntimeIndex::Local {
                position: u32::try_from(position).ok()?,
            }
        }
        checked_trees::CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            ..
        } => checked_trees::CheckedRuntimeIndex::StructuralField(
            checked_trees::CheckedStructuralParameterField {
                parameter_position,
                path,
            },
        ),
        _ => return None,
    };
    Some(CheckedUnitStructuralPathSegment::RuntimeIndex(runtime))
}

/// The literal index an immutable local scalar names: `let i: u64 = 2`
/// selects exactly element 2 for the whole state because nothing can write
/// the binding. Only a bare `Name` selector spelled against a state-local
/// immutable binding with a constant initializer folds here; every other
/// shape stays on the runtime-selector routes.
fn constant_local_index(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<usize> {
    let name = program.expression_table.name_path_node(expression)?;
    let source_state = program.state_by_symbol(state_symbol)?;
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
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<u32> {
    let state = program.state_by_symbol(state_symbol)?;
    let name = program.expression_table.name_path_node(expression)?;
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
/// `scan.count` in `scan.results[scan.count]`. A field selector's value is
/// storage, not an operand: the place must be a pure record-field chain
/// rooted at an entry parameter, and its leaf must be an integer primitive —
/// the same integer rule the parameter selector enforces. The owning machine
/// is no longer consulted: with static machines a selector roots at the
/// parameter it names, so the parameter roster answers the question alone.
/// Terminal emits the read and re-proves `0 <= value < extent` from the facts
/// holding before the call; admission here only resolves the selector's
/// subject.
fn runtime_index_field_selector(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<checked_trees::CheckedRuntimeIndex> {
    let place = validation::canonical_place_from_expression(program, expression)?;
    let root = place.root.as_symbol()?;
    let parameters = program.state_parameters(program.state_by_symbol(state_symbol)?);
    let parameter_position = typed_trees::signature::state_parameter_position(parameters, root)?;
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
    let leaf =
        validation::canonical_place_type_reference(program, state_symbol, statement_index, &place)?;
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
pub(crate) fn projected_argument_path_for_identity(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    target_identity: typed_trees::type_identity::TypeIdentityId,
) -> Option<Vec<CheckedUnitStructuralPathSegment>> {
    let (projected, path) =
        projected_argument_path(program, state_symbol, statement_index, place, None, None)?;
    (base_type_identity(program, projected, &[], &[])? == target_identity).then_some(path)
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    target: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Vec<CheckedUnitStructuralPathSegment>)> {
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
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    target: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Vec<CheckedUnitStructuralPathSegment>)> {
    let (facts::PlaceSegment::Index { expression }, fields) = place.segments.split_last()? else {
        return None;
    };
    let range = program.expression_table.range_node(*expression)?;
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
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    place: &validation::CanonicalPlace,
    fields: &[facts::PlaceSegment],
    start: usize,
    end: Option<usize>,
    target: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Vec<CheckedUnitStructuralPathSegment>)> {
    if !fields
        .iter()
        .all(|segment| matches!(segment, facts::PlaceSegment::Field { .. }))
    {
        return None;
    }
    let backing = validation::CanonicalPlace {
        root: place.root,
        segments: fields.to_vec(),
    };
    let (backing_type, mut path) =
        projected_argument_path(program, state, statement_index, &backing, None, None)?;
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
    Some((backing_type, path))
}

/// Argument formation owns storage identity, path geometry, exact access and
/// transfer/loan custody. Do not classify them again by the caller's state
/// count, sibling arguments, or the callee's result/body shape here. This join
/// retains the remaining content-contract limitation: executing a projection
/// cannot invent the missing correspondence for a content-qualified contract.
pub(crate) fn projected_call_contracts_are_represented(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller_machine: &typed_trees::machine::Machine,
    caller_state: &typed_trees::state::State,
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    arguments: &[CheckedUnitStructuralArgumentPlan],
) -> bool {
    let projected = |argument: &CheckedUnitStructuralArgumentPlan| {
        !argument.path.is_empty() && !super::view_subslice::is_view_subslice(argument)
    };
    if !arguments.iter().any(projected) {
        return true;
    }
    let parameters = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| {
            !parameter.relevance.is_erased()
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() || !target_machine.supplies_checked_body() {
        return false;
    }
    let has_content_evidence = |machine, state| {
        crate::execution::terminal_unit::cleanup::machine_has_content_evidence(
            facts, machine, state,
        )
    };
    let content_evidence = has_content_evidence(caller_machine.symbol, caller_state.symbol)
        || has_content_evidence(target_machine.symbol, target_state.symbol);
    arguments
        .iter()
        .zip(parameters)
        .all(|(argument, parameter)| {
            if !projected(argument) {
                return true;
            }
            // A retained result loan keeps its represented referent predicates;
            // it does not transfer the content-qualified owner's value.
            let retained_loan = argument
                .source_structural_result_binding_ordinal()
                .is_some()
                && argument.access != CheckedStructuralAccess::Owned;
            structural_access_for_type_reference(program, parameter.type_reference)
                == Some(argument.access)
                && (retained_loan
                    || (!content_evidence
                        && !target_contract_mentions_projected_parameter(
                            program,
                            facts,
                            target_machine,
                            target_state,
                            parameter,
                        )))
        })
}

pub(crate) fn target_contract_mentions_projected_parameter(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    target_machine: &typed_trees::machine::Machine,
    target_state: &typed_trees::state::State,
    parameter: &StateParameter,
) -> bool {
    let expected_root = parameter.symbol;
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
            let Some(membership) = fact.as_membership() else {
                return false;
            };
            validation::canonical_place_from_expression_in_state(
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
                        && parameter_qualification_carries_membership(
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

/// A declared root qualification also appears as a source requires fact.
/// Argument formation already retains and checks that exact domain; projecting
/// the actual does not create a second contract obligation or membership grant.
/// Extra authored memberships and all exit contracts still need their own
/// represented correspondence. This is independent of the carrier's storage.
fn parameter_qualification_carries_membership(
    program: &dyn ::validation::CheckingInput,
    parameter: &StateParameter,
    membership: &typed_trees::domain::ProofMembershipFact,
) -> bool {
    if !membership.semantic_domain.is_valid()
        || !membership.domain_arguments.is_empty()
        || !program.domain_definitions().iter().any(|definition| {
            definition.symbol == membership.domain_symbol
                && definition.semantic_id == membership.semantic_domain
        })
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
    program: &dyn ::validation::CheckingInput,
    parameter_type: TypeReferenceHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let bytes = program.expression_table.string_node(expression)?;
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
                validation::field_domains::domain_constraint_symbols(program, parameter_type)
            {
                let domain = program.domain_definition(symbol)?;
                if !domain.establishment_routes.is_empty()
                    || domain.semantic_roles != Default::default()
                    || !validation::field_domains::string_literal_expression_grants_domain(
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
