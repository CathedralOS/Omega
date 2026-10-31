//! Checked scalar arguments and projected argument paths: field and index
//! paths, projected call support, contract mentions and crash expressions.

use super::super::types::parameter_is_root;
use super::boundary_admission::fixed_byte_array_view_is_admitted;
use super::view_subslice::is_view_subslice;
use crate::execution::terminal_unit::cleanup::machine_has_content_evidence;
use crate::execution::terminal_unit::types::byte_sequence_carrier;
use crate::execution::terminal_unit::types::{
    base_type_identity, byte_sequence_type_identity, dense_scalar_parameter_index,
    structural_access_for_type_reference, terminal_field_identity,
};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedScalarExpressionRole as ExpressionRole,
    CheckedStructuralAccess as StructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedUnitCallCoordinate as CallCoordinate, CheckedUnitStructuralArgumentPlan as ArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment as PathSegment,
    Multiplicity, SignatureContractKind, StateParameter, StatementNode, SymbolHandle,
    TypeReferenceHandle, TypeReferenceNode,
};
use crate::flow::{expressions, state_statements};
use crate::places::place_at;
use crate::values::lower_unit_scalar_argument;
use crate::values::scalar_expression_type;
use checked_trees::CheckedByteSequenceCarrier;
use checked_trees::CheckedCallScalarArgument;
use checked_trees::CheckedErasedProofParameterPlan;
use checked_trees::CheckedMemberIdentity;
use checked_trees::CheckedOperatorFacts;
use checked_trees::CheckedProofTerm;
use checked_trees::CheckedProofTermRole;
use checked_trees::CheckedRuntimeIndex;
use checked_trees::CheckedScalarExpression;
use checked_trees::CheckedStructuralParameterField;
use checked_trees::CheckedStructuralPredicatePathSegment;
use checked_trees::CrashRouteGuard;
use facts::PlaceRoot;
use facts::PlaceSegment;
use language_semantics::ReferenceAccess;
use typed_trees::data::DataMember;
use typed_trees::domain::ProofMembershipFact;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::state_parameter_position;
use typed_trees::state::State;
use typed_trees::type_identity::TypeIdentityId;
use typed_trees::types::FixedArrayLength;
use typed_trees::types::PrimitiveType;
use typed_trees::types::TypeConstraintNode;
use validation::CanonicalPlace;
use validation::canonical_place_from_expression;
use validation::canonical_place_type_reference;
use validation::expression_type_reference_in_state;
use validation::field_domains::domain_constraint_symbols;
use validation::field_domains::string_literal_expression_grants_domain;
use validation::reference_shell;
use validation::stored_structural_type_identity;
use validation::stores_reference_shape;
use validation::structural_storage_type;
use validation::unwrapped_type_reference;

pub(crate) fn checked_call_scalar_arguments(
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    coordinate: CallCoordinate,
    parameters: &[CheckedStructuralScalarParameterPlan],
    boundary: bool,
) -> Option<Vec<CheckedCallScalarArgument>> {
    parameters
        .iter()
        .enumerate()
        .map(|(argument_ordinal, parameter)| {
            let role = if boundary {
                ExpressionRole::BoundaryCallArgument {
                    call_ordinal: coordinate.call_ordinal,
                    argument_ordinal: u32::try_from(argument_ordinal).ok()?,
                }
            } else {
                ExpressionRole::UnitCallArgument {
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
                return Some(CheckedCallScalarArgument::Computation(root.root));
            }
            if !roots.is_empty() {
                return None;
            }
            let (_, expression) = facts.values.scalar_expressions.bound_expression_at(
                caller_state,
                coordinate.statement_index,
                role,
            )?;
            (scalar_expression_type(expression)? == parameter.primitive_type)
                .then(|| CheckedCallScalarArgument::Pure(expression.clone()))
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
    coordinate: CallCoordinate,
    parameters: &[CheckedErasedProofParameterPlan],
) -> Option<Vec<CheckedProofTerm>> {
    parameters
        .iter()
        .enumerate()
        .map(|(erased_ordinal, parameter)| {
            let role = CheckedProofTermRole::ErasedUnitCallArgument {
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
    term: &CheckedProofTerm,
    type_identity: TypeIdentityId,
) -> bool {
    match term {
        CheckedProofTerm::Mathematical {
            type_identity: actual,
            ..
        }
        | CheckedProofTerm::MathematicalApplication {
            type_identity: actual,
            ..
        }
        | CheckedProofTerm::Construction {
            type_identity: actual,
            ..
        }
        | CheckedProofTerm::Array {
            type_identity: actual,
            ..
        } => *actual == type_identity,
        CheckedProofTerm::Formal { parameter_symbol } => {
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
        CheckedProofTerm::Scalar(_) => false,
    }
}

/// Proof-only erased actuals of an in-module Unit call: the retained pure
/// expressions recorded under `ErasedUnitCallArgument`, in the callee's dense
/// erased-formal order.
pub(crate) fn checked_call_erased_scalar_arguments(
    facts: &CheckFacts,
    caller_state: SymbolHandle,
    coordinate: CallCoordinate,
    parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<Vec<CheckedCallScalarArgument>> {
    parameters
        .iter()
        .enumerate()
        .map(|(erased_ordinal, parameter)| {
            let role = ExpressionRole::ErasedUnitCallArgument {
                call_ordinal: coordinate.call_ordinal,
                erased_ordinal: u32::try_from(erased_ordinal).ok()?,
            };
            let (_, expression) = facts.values.scalar_expressions.bound_expression_at(
                caller_state,
                coordinate.statement_index,
                role,
            )?;
            (scalar_expression_type(expression)? == parameter.primitive_type)
                .then(|| CheckedCallScalarArgument::Pure(expression.clone()))
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
    place: &CanonicalPlace,
) -> Option<(typed_trees::types::TypeReferenceHandle, Vec<PathSegment>)> {
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
    mut path: Vec<PathSegment>,
    formal: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Vec<PathSegment>)> {
    let Some(requested) = reference_shell(program, formal) else {
        return Some((selected, path));
    };
    let requested_storage = stored_structural_type_identity(program, requested.referee, &[], &[]);
    if !requested_storage.is_valid() {
        return None;
    }
    for _ in 0..=program.type_reference_table.type_reference_count() {
        // The requested referent may itself be a descriptor. Stop before
        // loading it: lending a pointer slot and lending its backing are
        // different coordinates and different permission obligations.
        let selected_storage = stored_structural_type_identity(program, selected, &[], &[]);
        if !selected_storage.is_valid() {
            return None;
        }
        if selected_storage == requested_storage {
            return Some((selected, path));
        }
        let Some(shell) = reference_shell(program, selected)
            .filter(|_| stores_reference_shape(program, selected, &[]))
        else {
            return Some((selected, path));
        };
        if !shell.access.is_readable()
            || (shell.access != requested.access && shell.access != ReferenceAccess::Mutable)
        {
            return None;
        }
        path.push(PathSegment::Referent);
        selected = shell.referee;
    }
    None
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
    place: &CanonicalPlace,
    local_selector: &dyn Fn(
        typed_trees::expression::ExpressionHandle,
    ) -> Option<CheckedRuntimeIndex>,
) -> Option<(typed_trees::types::TypeReferenceHandle, Vec<PathSegment>)> {
    projected_argument_path(
        program,
        state_symbol,
        statement_index,
        place,
        Some(machine),
        Some(&|expression| local_selector(expression).map(PathSegment::RuntimeIndex)),
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
    place: &CanonicalPlace,
    local_selector: &dyn Fn(
        typed_trees::expression::ExpressionHandle,
    ) -> Option<CheckedRuntimeIndex>,
) -> Option<(typed_trees::types::TypeReferenceHandle, Vec<PathSegment>)> {
    let (mut projected, mut path) = projected_selected_path(
        program,
        machine,
        state_symbol,
        statement_index,
        place,
        local_selector,
    )?;
    if let PlaceRoot::Symbol(root) = place.root
        && let Some(state) = program.state_by_symbol(state_symbol)
        && let Some((ordinal, local)) = program.state_local_declaration(state, root)
        && ordinal < statement_index
    {
        let mut reference = local.type_reference;
        let mut crossings = Vec::new();
        while let Some(shell) = reference_shell(program, reference) {
            if shell.access == ReferenceAccess::WriteOnly {
                return None;
            }
            if !stores_reference_shape(program, reference, &[]) {
                break;
            }
            crossings.push(PathSegment::Referent);
            reference = shell.referee;
        }
        crossings.append(&mut path);
        path = crossings;
    }
    while let Some(shell) = reference_shell(program, projected) {
        if shell.access == ReferenceAccess::WriteOnly {
            return None;
        }
        // The local root crossing above already reaches its direct referee.
        // A projected stored reference is a distinct descriptor at the leaf.
        if place
            .segments
            .iter()
            .any(|segment| !matches!(segment, PlaceSegment::Case { .. }))
        {
            path.push(PathSegment::Referent);
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
    place: &CanonicalPlace,
    selector: Option<&dyn Fn(typed_trees::expression::ExpressionHandle) -> Option<PathSegment>>,
) -> Option<(TypeReferenceHandle, Vec<PathSegment>)> {
    projected_argument_path(program, state, statement, place, Some(machine), selector)
}

/// The segment prefix ending before `position` re-rooted as its own place and
/// resolved to its container type — the descend every member, index, window or
/// case segment takes to learn the extent it must stay inside.
fn prefix_container_type_reference(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
    position: usize,
) -> Option<TypeReferenceHandle> {
    canonical_place_type_reference(
        program,
        state_symbol,
        statement_index,
        &CanonicalPlace {
            root: place.root,
            segments: place.segments[..position].to_vec(),
        },
    )
}

pub(crate) fn projected_argument_path(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
    runtime_index_machine: Option<SymbolHandle>,
    local_selector: Option<
        &dyn Fn(typed_trees::expression::ExpressionHandle) -> Option<PathSegment>,
    >,
) -> Option<(typed_trees::types::TypeReferenceHandle, Vec<PathSegment>)> {
    let mut path = Vec::with_capacity(place.segments.len());
    // A borrowed root's own declared reference shells denote storage the
    // rooted subject already names — a `&`-parameter's member projection
    // does not re-cross them. Case segments leave the root's type intact,
    // so while the prefix has only selected cases, every shell the
    // container reports is the root's own. Once a member, element, or
    // window descend has projected a new leaf type, any shell it carries
    // belongs to a stored reference the path actually crossed.
    let inherited_crossings = {
        let mut reference = canonical_place_type_reference(
            program,
            state_symbol,
            statement_index,
            &CanonicalPlace {
                root: place.root,
                segments: Vec::new(),
            },
        )?;
        let mut count = 0usize;
        while let Some(shell) = reference_shell(program, reference)
            && stores_reference_shape(program, reference, &[])
        {
            count += 1;
            reference = shell.referee;
        }
        count
    };
    let prefix_container = |position| {
        prefix_container_type_reference(program, state_symbol, statement_index, place, position)
    };
    // Case selection keeps the same carrier. Remember crossings already
    // consumed there; a real field/element projection starts a new carrier.
    let mut consumed_carrier_crossings = inherited_crossings;
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
                PlaceSegment::Field { .. }
                    | PlaceSegment::Case { .. }
                    | PlaceSegment::FixedIndex { .. }
                    | PlaceSegment::Index { .. }
            )
        {
            let mut reference = prefix_container(position)?;
            let mut crossings = 0usize;
            while let Some(shell) = reference_shell(program, reference)
                && stores_reference_shape(program, reference, &[])
            {
                crossings += 1;
                reference = shell.referee;
            }
            for _ in 0..crossings.saturating_sub(consumed_carrier_crossings) {
                path.push(PathSegment::Referent);
            }
            consumed_carrier_crossings = crossings;
        }
        if !matches!(segment, PlaceSegment::Case { .. }) {
            consumed_carrier_crossings = 0;
        }
        match segment {
            PlaceSegment::Field { symbol } => {
                let identity = match payload_field.take() {
                    Some((expected, identity)) if expected == *symbol => identity,
                    Some(_) => return None,
                    None => terminal_field_identity(program, *symbol)?,
                };
                path.push(PathSegment::Field(identity));
            }
            PlaceSegment::FixedIndex { index } => {
                let container = prefix_container(position)?;
                fixed_index_within_extent(program, container, *index)?;
                path.push(PathSegment::FixedIndex(u64::try_from(*index).ok()?));
            }
            PlaceSegment::Index { expression } => {
                // An immutable local scalar initialized to a literal is a
                // fixed index in every way that matters to this walk: the
                // binding can never change, so the selected element is as
                // pinned as an authored constant. Fold it through the same
                // literal-bound check `FixedIndex` segments take rather than
                // reaching for runtime-selector evidence.
                if let Some(index) = constant_local_index(program, state_symbol, *expression) {
                    let container = prefix_container(position)?;
                    fixed_index_within_extent(program, container, index)?;
                    path.push(PathSegment::FixedIndex(u64::try_from(index).ok()?));
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
                            .map(|position| CheckedRuntimeIndex::Parameter { position })
                            .or_else(|| {
                                runtime_index_field_selector(
                                    program,
                                    state_symbol,
                                    statement_index,
                                    *expression,
                                )
                            })
                            .map(PathSegment::RuntimeIndex)
                    })?;
                path.push(runtime_index);
            }
            PlaceSegment::OpaqueRange => return None,
            PlaceSegment::FixedRange { start, end } => {
                // A borrowed byte view's window is itself part of the place:
                // `self.source[0..2]` lends the fixed byte range the view
                // names. The container must be fixed byte-array backing whose
                // literal extent bounds the window — the same shape the
                // `FixedIndex` walk proves — since a stored view carries no
                // runtime bound evidence of its own.
                let container = prefix_container(position)?;
                fixed_byte_range_within_extent(program, container, *start, *end)?;
                path.push(PathSegment::FixedByteRange {
                    start: u64::try_from(*start).ok()?,
                    end: u64::try_from(*end).ok()?,
                });
            }
            PlaceSegment::Case { variant } => {
                // Resolve the exact case and its following field together.
                // The canonical type walker alone skips Case markers; it is
                // not evidence that a payload belongs to the selected case.
                let reference = prefix_container(position)?;
                let reference = unwrapped_type_reference(program, reference)?;
                let owner = program.closed_data_definition(reference)?;
                let mut cases =
                    program
                        .data_members(owner)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Variant(case)
                                if variant.is_valid() && case.symbol == *variant =>
                            {
                                Some(case)
                            }
                            _ => None,
                        });
                let case = cases.next()?;
                let Some(PlaceSegment::Field { symbol }) = place.segments.get(position + 1) else {
                    return None;
                };
                let mut fields = program.data_payload_fields(case).iter().filter(|field| {
                    symbol.is_valid() && field.symbol == *symbol && !field.relevance.is_erased()
                });
                let field = fields.next()?;
                if cases.next().is_some() || fields.next().is_some() {
                    return None;
                }
                path.push(PathSegment::Case(CheckedMemberIdentity::of_member(
                    case.identity,
                    case.name.clone(),
                )));
                payload_field = Some((
                    *symbol,
                    CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
                ));
            }
        }
    }
    let mut projected =
        canonical_place_type_reference(program, state_symbol, statement_index, place)?;
    // A whole parameter already denotes its ingress storage, just as the
    // root of a member/index path above does. Its declared reference is not
    // an extra stored descriptor to cross. Locals and completed values keep
    // their stored reference shells; their producer owns those descriptors.
    // View carriers already contain their bounds and backing: only the common
    // stored-reference classification denotes a shell to peel here.
    if place.segments.is_empty()
        && stores_reference_shape(program, projected, &[])
        && let Some(symbol) = place.root.as_symbol()
        && let Some(state) = program.state_by_symbol(state_symbol)
        && program
            .state_parameters(state)
            .iter()
            .any(|parameter| parameter_is_root(parameter, symbol))
    {
        projected = structural_storage_type(program, projected, &[]);
        if !projected.is_valid() {
            return None;
        }
    }
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
) -> Option<PathSegment> {
    if let Some(index) = constant_local_index(program, state, expression) {
        return Some(PathSegment::FixedIndex(u64::try_from(index).ok()?));
    }
    let runtime_index = runtime_index_parameter_position(program, state, expression)
        .map(|position| CheckedRuntimeIndex::Parameter { position })
        .or_else(|| runtime_index_field_selector(program, state, statement_index, expression))?;
    Some(PathSegment::RuntimeIndex(runtime_index))
}

/// A value projection consumes the same checked scalar operands as other
/// values, not an assignment destination's pre-RHS capture slot. Dense
/// parameter/local positions and declared field observations retain their
/// scalar namespace. A storage source names the established home to read at
/// this program point; it never substitutes the declaration's initializer.
pub(crate) fn value_index_segment(
    program: &dyn ::validation::CheckingInput,
    operators: &CheckedOperatorFacts,
    state: &State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<PathSegment> {
    let reference =
        expression_type_reference_in_state(program, state.symbol, statement_index, expression)?;
    let primitive = program.primitive_type_reference(reference)?;
    if !primitive.is_fixed_integer() {
        return None;
    }
    let operand = lower_unit_scalar_argument(
        program,
        operators,
        state,
        statement_index,
        expression,
        primitive,
    )?;
    let runtime = match operand {
        CheckedScalarExpression::StorageRead { symbol, .. } => {
            CheckedRuntimeIndex::StorageRead { symbol }
        }
        CheckedScalarExpression::Parameter { position, .. } => CheckedRuntimeIndex::Parameter {
            position: u32::try_from(position).ok()?,
        },
        CheckedScalarExpression::Local { position, .. } => CheckedRuntimeIndex::Local {
            position: u32::try_from(position).ok()?,
        },
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            ..
        } => CheckedRuntimeIndex::StructuralField(CheckedStructuralParameterField {
            parameter_position,
            path,
        }),
        _ => return None,
    };
    Some(PathSegment::RuntimeIndex(runtime))
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
    let local = state_statements(program, source_state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == name.symbol && !local.is_mutable => {
                Some(local)
            }
            _ => None,
        })?;
    usize::try_from(
        expressions(program).constant_integer_value(local.ordinary_value_initializer()?)?,
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
                PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
            )
        )
    {
        return None;
    }
    dense_scalar_parameter_index(program, parameters, authored_position)
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
) -> Option<CheckedRuntimeIndex> {
    let place = canonical_place_from_expression(program, expression)?;
    let root = place.root.as_symbol()?;
    let parameters = program.state_parameters(program.state_by_symbol(state_symbol)?);
    let parameter_position = state_parameter_position(parameters, root)?;
    if place.segments.is_empty() {
        // A bare parameter selector is the `Parameter` coordinate, never a
        // field snapshot.
        return None;
    }
    let mut path = Vec::with_capacity(place.segments.len());
    for segment in &place.segments {
        let PlaceSegment::Field { symbol } = segment else {
            return None;
        };
        path.push(CheckedStructuralPredicatePathSegment::Field(
            terminal_field_identity(program, *symbol)?,
        ));
    }
    let leaf = canonical_place_type_reference(program, state_symbol, statement_index, &place)?;
    if !matches!(
        program.primitive_type_reference(leaf),
        Some(
            PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
                | PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
        )
    ) {
        return None;
    }
    Some(CheckedRuntimeIndex::StructuralField(
        CheckedStructuralParameterField {
            parameter_position: u32::try_from(parameter_position).ok()?,
            path,
        },
    ))
}

/// The shared owner under the ordinary rule: the projected type must carry the
/// target parameter's exact normalized identity.
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
    let container = unwrapped_type_reference(program, container)?;
    let TypeReferenceNode::FixedArray {
        element_type,
        length: FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(container)
    else {
        return None;
    };
    if program.primitive_type_reference(*element_type) != Some(PrimitiveType::U8) {
        return None;
    }
    (start <= end && end <= usize::try_from(*length).ok()?).then_some(())
}

/// The container is a slice through any number of domain constraints and
/// borrows.
fn slice_type_reference(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    unwrapped_type_reference(program, type_reference).is_some_and(|type_reference| {
        matches!(
            program.type_reference_table.type_reference(type_reference),
            TypeReferenceNode::Slice { .. }
        )
    })
}

fn fixed_array_literal_length(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Option<usize> {
    let type_reference = unwrapped_type_reference(program, type_reference)?;
    let TypeReferenceNode::FixedArray {
        length: FixedArrayLength::Literal(length),
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
    place: &CanonicalPlace,
    target: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Vec<PathSegment>)> {
    let (PlaceSegment::FixedRange { start, end }, fields) = place.segments.split_last()? else {
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
    place: &CanonicalPlace,
    target: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Vec<PathSegment>)> {
    let (PlaceSegment::Index { expression }, fields) = place.segments.split_last()? else {
        return None;
    };
    let range = program.expression_table.range_node(*expression)?;
    let start = if range.start.is_valid() {
        usize::try_from(expressions(program).constant_integer_value(range.start)?).ok()?
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
    place: &CanonicalPlace,
    fields: &[PlaceSegment],
    start: usize,
    end: Option<usize>,
    target: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, Vec<PathSegment>)> {
    if !fields
        .iter()
        .all(|segment| matches!(segment, PlaceSegment::Field { .. }))
    {
        return None;
    }
    let backing = CanonicalPlace {
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
        || !fixed_byte_array_view_is_admitted(program, backing_type, target)
    {
        return None;
    }
    path.push(PathSegment::FixedByteRange {
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
    caller_machine: &Machine,
    caller_state: &State,
    target_machine: &Machine,
    target_state: &State,
    arguments: &[ArgumentPlan],
) -> bool {
    let projected =
        |argument: &ArgumentPlan| !argument.path.is_empty() && !is_view_subslice(argument);
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
    let has_content_evidence = |machine, state| machine_has_content_evidence(facts, machine, state);
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
                && argument.access != StructuralAccess::Owned;
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
    target_machine: &Machine,
    target_state: &State,
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
            place_at(program, target_state.symbol, 0, membership.value).is_some_and(|place| {
                (place.root == PlaceRoot::Symbol(expected_root)
                    || place.root == PlaceRoot::Symbol(parameter.symbol))
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
                    CrashRouteGuard::Truth => false,
                    CrashRouteGuard::Predicate(predicate) => {
                        // Checked predicates retain every formal and typed
                        // projection. Terminal substitution rebases those
                        // roots onto the actuals; source member spelling and
                        // a hard-coded first parameter supply no extra proof.
                        predicate.scalar_expression().is_none()
                    }
                })
            })
        })
}

/// A declared root qualification also appears as a source requires fact.
/// Argument formation already retains and checks that exact domain; projecting
/// the actual does not create a second contract obligation or membership grant.
/// Indexed families use their normalized instance identity, not the family's
/// declaration identity or an empty-argument restriction.
/// Extra authored memberships and all exit contracts still need their own
/// represented correspondence. This is independent of the carrier's storage.
fn parameter_qualification_carries_membership(
    program: &dyn ::validation::CheckingInput,
    parameter: &StateParameter,
    membership: &ProofMembershipFact,
) -> bool {
    if !membership.semantic_domain.is_valid()
        || program
            .domain_definition(membership.domain_symbol)
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
                        matches!(constraint, TypeConstraintNode::Domain(domain)
                        if domain.symbol == membership.domain_symbol
                            && domain.semantic_id == membership.semantic_domain)
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

pub(crate) fn byte_sequence_literal_argument(
    program: &dyn ::validation::CheckingInput,
    parameter_type: TypeReferenceHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<ArgumentPlan> {
    let bytes = program.expression_table.string_node(expression)?;
    let access = structural_access_for_type_reference(program, parameter_type)?;
    match (byte_sequence_carrier(program, parameter_type, &[])?, access) {
        (CheckedByteSequenceCarrier::BorrowedView { .. }, StructuralAccess::SharedBorrow) => {}
        (CheckedByteSequenceCarrier::BoundedOwned { capacity }, StructuralAccess::Owned)
            if program.type_multiplicity(parameter_type) == Multiplicity::Unrestricted
                && u64::try_from(bytes.len()).ok()? <= capacity =>
        {
            // This is construction at the formal, not ownership of immutable
            // image storage. Lowering must establish fresh bounded backing.
            for symbol in domain_constraint_symbols(program, parameter_type) {
                let domain = program.domain_definition(symbol)?;
                if !domain.establishment_routes.is_empty()
                    || domain.semantic_roles != Default::default()
                    || !string_literal_expression_grants_domain(program, expression, symbol)
                {
                    return None;
                }
            }
        }
        _ => return None,
    }
    Some(ArgumentPlan {
        projection_evaluations: Default::default(),
        source: CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral {
            bytes: bytes.to_vec(),
        },
        path: Vec::new(),
        type_identity: byte_sequence_type_identity(program, parameter_type, &[], &[])?,
        access,
    })
}
