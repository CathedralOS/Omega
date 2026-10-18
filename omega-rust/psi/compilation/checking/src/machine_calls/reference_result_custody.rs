//! Reference values join ordinary result bindings to exact source loan events.
//! Call results borrow the source checker's complete local-loan span. Selecting
//! one possible referent at runtime does not discard the other sources' loan
//! obligations, and retiring the descriptor is one operation for that whole
//! group. Reconstructing argument initializers here duplicated source admission
//! and made its single-source recognizers a second language restriction.
//! Construction and projected transfer retain their own source/custody joins;
//! none of these queries creates authority or replaces borrow checking.
//! Formation identifies the captured occurrence; current use reads its exact
//! checked entry constraint. Neither judgment requires a future last-use
//! release: a carrier may instead move or be replaced after evaluating a RHS.
//! Retirement alone consumes weakening boundaries and must preserve their phase.
//!
//! Runtime element origins are symbolic relations, not an enumeration of every
//! slot or a recipe for forming descriptors. Expression and named-transition
//! returns both consume captured normal-return custody. Static argument paths
//! remain separate because selecting an existing descriptor requires its exact
//! owner identity, not a possible backing region.

use crate::type_applications::DeclarationScopes;
use checked_trees::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralResultBindingPlan,
    ReferenceCustodyFacts, ReferenceCustodyRow,
};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

mod primitive_locals;
pub use primitive_locals::primitive_local_source;
mod stored_carrier_sources;
pub use stored_carrier_sources::{StoredCarrierSource, stored_carrier_sources};
mod call_reception;
pub use call_reception::{RecordCallReception, record_call_reception};
mod captured_returns;
mod declared_returns;
pub use captured_returns::call_sources as call_return_sources;
pub use captured_returns::returned_sources as captured_return_sources;
pub use declared_returns::returned_sources as declared_return_sources;

// An owned formal has a compact type DAG but its returned-origin roster is
// materialized by path. Bound that expansion before semantic classification or
// allocation. These are private production limits, not reference semantics;
// the leaf limit matches the executable ingress roster's current capacity.
const MAX_REFERENCE_ORIGIN_LEAVES: usize = 4096;
const MAX_REFERENCE_ORIGIN_VISITS: usize = MAX_REFERENCE_ORIGIN_LEAVES * 2;
const MAX_REFERENCE_ORIGIN_DEPTH: usize = 128;

/// The mutable structural state-parameter test the custody walks share: not
/// const, and not a primitive type (primitives carry scalar, not structural,
/// state).
pub(crate) fn is_mutable_structural_parameter(
    program: &dyn crate::CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    !parameter.is_const
        && program
            .primitive_type_reference(parameter.type_reference)
            .is_none()
}

fn reference_origin_expansion_is_bounded(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    fn visit(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        depth: usize,
        visits: &mut usize,
        leaves: &mut usize,
        scopes: &mut DeclarationScopes,
        scope: usize,
    ) -> Option<()> {
        *visits = visits.checked_add(1)?;
        if *visits > MAX_REFERENCE_ORIGIN_VISITS || depth > MAX_REFERENCE_ORIGIN_DEPTH {
            return None;
        }
        let (reference, scope) = scopes.head(program, reference, scope);
        if program.primitive_type_reference(reference).is_some() {
            return Some(());
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } => {
                *leaves = leaves.checked_add(1)?;
                return (*leaves <= MAX_REFERENCE_ORIGIN_LEAVES).then_some(());
            }
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {}
            TypeReferenceNode::Constrained { base_type, .. } => {
                return visit(
                    program,
                    *base_type,
                    depth + 1,
                    visits,
                    leaves,
                    scopes,
                    scope,
                );
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let before = *leaves;
                visit(
                    program,
                    *element_type,
                    depth + 1,
                    visits,
                    leaves,
                    scopes,
                    scope,
                )?;
                let element_leaves = leaves.checked_sub(before)?;
                if element_leaves == 0 {
                    return Some(());
                }
                let typed_trees::types::FixedArrayLength::Literal(length) = length else {
                    return None;
                };
                // Bound the expanded permission roster without traversing
                // every element of a compact nested array type.
                *leaves = before.checked_add(element_leaves.checked_mul(*length)?)?;
                return (*leaves <= MAX_REFERENCE_ORIGIN_LEAVES).then_some(());
            }
            TypeReferenceNode::Slice { element_type } => {
                return visit(
                    program,
                    *element_type,
                    depth + 1,
                    visits,
                    leaves,
                    scopes,
                    scope,
                );
            }
            _ => return Some(()),
        }
        // Count the same applied fields the custody roster will read. A finite
        // unprepared application is not evidence of an empty reference shape.
        let (data, scope) = scopes.definition(program, reference, scope)?;
        for field in program.data_member_fields(program.data_members(data)) {
            if !field.relevance.is_erased() {
                visit(
                    program,
                    field.type_reference,
                    depth + 1,
                    visits,
                    leaves,
                    scopes,
                    scope,
                )?;
            }
        }
        Some(())
    }
    visit(
        program,
        reference,
        0,
        &mut 0,
        &mut 0,
        &mut DeclarationScopes::default(),
        0,
    )
    .is_some()
}

/// Closed aggregate construction can contain real reference carriers.
/// Arrays and guarded case fields recurse through the same stored-value tree; the
/// owned aggregate holds each loan descriptor, never the referent's bytes.
/// This classifier grants neither an initializer origin nor a loan lifetime.
/// Reference leaves include slice descriptors as well as stored pointers;
/// their different geometry belongs to shape collection and loan transport,
/// not to whether the enclosing owned value carries borrowed storage.
/// Linear settlement and nominal cleanup belong to the carrier's owned custody,
/// not its stored descriptors. Excluding those carriers here loses their input
/// referent identities and makes descriptor copies appear to borrow the home.
pub fn is_reference_record(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    if !reference_origin_expansion_is_bounded(program, reference) {
        return false;
    }
    fn visit(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        seen: &mut Vec<TypeReferenceHandle>,
        scopes: &mut DeclarationScopes,
        scope: usize,
    ) -> Option<bool> {
        let (reference, scope) = scopes.head(program, reference, scope);
        if crate::reference_shell(program, reference).is_some() {
            return Some(true);
        }
        if crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::NumericOwned,
        ) {
            return Some(false);
        }
        if let TypeReferenceNode::FixedArray { element_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            return visit(program, *element_type, seen, scopes, scope);
        }
        if let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            return visit(program, *base_type, seen, scopes, scope);
        }
        let (data, scope) = scopes.definition(program, reference, scope)?;
        if seen.contains(&reference) {
            return None;
        }
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
            return None;
        }
        seen.push(reference);
        let mut contains_reference = false;
        for field in program.data_member_fields(program.data_members(data)) {
            if field.relevance.is_erased() {
                continue;
            }
            contains_reference |= visit(program, field.type_reference, seen, scopes, scope)?;
        }
        seen.pop();
        Some(contains_reference)
    }
    // A standalone view descriptor or exclusive pointer has its own custody
    // route. Finding those leaves inside an owned aggregate does not turn the
    // standalone value into a record. Shared stored pointers still participate
    // in record-result custody, including discarded-result admission.
    if crate::reference_shell(program, reference).is_some() {
        return shared_borrowed_parts(program, reference).is_some();
    }
    visit(
        program,
        reference,
        &mut Vec::new(),
        &mut DeclarationScopes::default(),
        0,
    ) == Some(true)
}

/// Reconstruct a leaf's exact formal ingress. Structural parameter ordinals
/// exclude scalar and const parameters, just like ordinary call preparation.
/// Authenticate the complete qualified type before publishing the canonical
/// storage identity. Logical loan rosters keep their exact qualified identities;
/// executable arguments must join the storage catalog, whose predicate-only
/// domains contribute no representation.
pub fn initializer_source(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) = stored_loan_parts(program, reference)?;
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Name(_)
    ) {
        return borrowed_initializer_source(program, state, expression, reference);
    }
    let Some((path, name)) = program.expression_table.single_name_member(expression) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.symbol != path.head_symbol
        || program.symbols.get(path.symbol).parent != state.symbol
    {
        return None;
    }

    let found = structural_parameter_position(program, state, path.symbol);
    if let Some((position, parameter)) = found {
        if parameter.name != *name
            || !reference_parameter_can_supply(program, parameter.type_reference, reference)
        {
            return None;
        }
        return Some(checked_trees::CheckedUnitStructuralArgumentPlan {
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(position).ok()?,
            },
            path: Vec::new(),
            type_identity: crate::structural_type_identity(program, referent, &[], &[]),
            access,
        });
    }
    // A reference-typed local forwards its own loan: the binding plans no
    // storage of its own, so the field's ingress is the loan the local's
    // initializer captured, re-derived at the same access. The marker is
    // immutable (a reference binding cannot reseat) and its declared carrier
    // must be able to supply the exact reference the field asks for.
    let (_, local) = program.state_local_declaration(state, path.symbol)?;
    if local.is_mutable
        || local.name != *name
        || !reference_parameter_can_supply(program, local.type_reference, reference)
    {
        return None;
    }
    borrowed_initializer_source(program, state, local.initial_value, reference)
}

/// The filtered formal ordinal + parameter for a non-const, non-primitive
/// state parameter named `symbol` — the structural-parameter lane both
/// borrowed-carrier walks index by.
fn structural_parameter_position<'program>(
    program: &'program dyn crate::CheckingInput,
    state: &'program typed_trees::state::State,
    symbol: SymbolHandle,
) -> Option<(usize, &'program typed_trees::signature::StateParameter)> {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| is_mutable_structural_parameter(program, parameter))
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == symbol)
}

/// Returning an ingress loan can reduce access, never change its referent.
/// Lifetime validity is checked by the borrow checker; this reconstruction
/// retains the exact referent (including qualifications). An outer constraint
/// shell is not silently dropped to make an access conversion succeed.
fn reference_parameter_can_supply(
    program: &dyn crate::CheckingInput,
    source: TypeReferenceHandle,
    result: TypeReferenceHandle,
) -> bool {
    if program.normalized_type_identity(source) == program.normalized_type_identity(result) {
        return true;
    }
    let TypeReferenceNode::Reference {
        referee: source_referee,
        access: language_core::ReferenceAccess::Mutable,
        ..
    } = program.type_reference_table.type_reference(source)
    else {
        return false;
    };
    let TypeReferenceNode::Reference {
        referee: result_referee,
        access: language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::WriteOnly,
        ..
    } = program.type_reference_table.type_reference(result)
    else {
        return false;
    };
    program.normalized_type_identity(*source_referee)
        == program.normalized_type_identity(*result_referee)
}

/// A borrowed initializer names a whole referent or projected storage leaf.
/// The authored address walks member hops and literal index hops from its
/// storage root; the source retains that root and the projected segments.
/// A reference formal already denotes its referent. A final `Referent` step
/// belongs only to a stored reference leaf, never to an ordinary value being
/// borrowed. Indexes bound to locals stay opaque: folding them would need the
/// constant-binding evidence the producer resolves elsewhere.
pub fn borrowed_initializer_source(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) = stored_loan_parts(program, reference)?;
    let Some(borrow) = program.expression_table.borrow_expression(expression) else {
        return None;
    };
    let authored_access = match borrow.access {
        language_core::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        language_core::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        language_core::ReferenceAccess::WriteOnly => CheckedStructuralAccess::WriteOnlyBorrow,
    };
    if authored_access != access {
        return None;
    }
    let (projections, cursor) = crate::expression_traversal::projection_climb(
        program,
        borrow.target,
        MAX_REFERENCE_ORIGIN_DEPTH,
    )?;
    let path = program.expression_table.name_path_node(cursor)?;
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if !path.symbol.is_valid() || path.symbol != path.head_symbol {
        return None;
    }
    let found = structural_parameter_position(program, state, path.symbol);
    // The borrowed carrier roots at a non-self structural formal — the
    // existing ingress family — or at an established local of this state,
    // which mints the local's own storage as the loaned home. A borrowed formal
    // already denotes its referent; a local admits the whole place (`&mut owner`)
    // because its storage is wholly owned by this frame. A mutable or write-only
    // loan needs a mutable referent
    // declaration, the local must not be erased, and a reference-typed
    // local is a silent marker with no storage to name — borrowing through
    // one is the reborrow lane, which this plan does not reconstruct.
    let (source, mut owner_type) = match found {
        Some((position, parameter)) => {
            if parameter.name != *name {
                return None;
            }
            let owner_type = if let Some((referee, parameter_access)) =
                stored_loan_parts(program, parameter.type_reference)
            {
                if !reference_access_can_supply(parameter_access, access) {
                    return None;
                }
                referee
            } else {
                parameter.type_reference
            };
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(position).ok()?,
                },
                owner_type,
            )
        }
        None => {
            let (_, local) = program.state_local_declaration(state, path.symbol)?;
            let exclusive = matches!(
                access,
                CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
            );
            if local.name != *name
                || local.relevance.is_erased()
                || (exclusive && !local.is_mutable)
                || crate::reference_shell(program, local.type_reference).is_some()
            {
                return None;
            }
            // Mutable primitive declarations already establish a primitive
            // home in the operation sequence. Lending that home must retain
            // its storage namespace: calling it a structural local would ask
            // lowering for a second establishment that never occurred.
            // Immutable scalar values and structural carriers keep their
            // existing structural-home projection; reference access remains
            // the independent authored loan checked above.
            let source = if local.is_mutable
                && program
                    .primitive_type_reference(local.type_reference)
                    .is_some()
            {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                    symbol: local.symbol,
                }
            } else {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: local.symbol,
                }
            };
            (source, local.type_reference)
        }
    };
    // Each member hop resolves against the nominal owner its receiver
    // projects: the root carrier's declaration first, then each selected
    // field's (or indexed element's) type.
    let mut segments = Vec::with_capacity(projections.len() + 1);
    for projection in projections.iter().rev() {
        match program.expression_table.expression(*projection) {
            ExpressionNode::Member(member) => {
                let data = nominal_member_owner_type(program, owner_type)?;
                let field = crate::value_custody::places::exact_data_member_field(
                    program,
                    data,
                    member.member_symbol,
                    member.member.as_str(),
                    None,
                )?;
                if field.relevance.is_erased() {
                    return None;
                }
                segments.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
                ));
                owner_type = field.type_reference;
            }
            ExpressionNode::Indexed(indexed) => {
                let index = program.expression_table.integer_node(indexed.index)?;
                segments.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(index.value_i64()?).ok()?,
                ));
                owner_type = loop {
                    match program.type_reference_table.type_reference(owner_type) {
                        TypeReferenceNode::Constrained { base_type, .. } => owner_type = *base_type,
                        TypeReferenceNode::FixedArray { element_type, .. } => break *element_type,
                        _ => return None,
                    }
                };
            }
            _ => return None,
        }
    }
    // Reference access and reference storage are separate facts. Borrowing a
    // scalar produces a reference but does not dereference the scalar itself.
    let leaf = if let Some(shell) = crate::reference_shell(program, owner_type) {
        if !reference_parameter_can_supply(program, owner_type, reference) {
            return None;
        }
        segments.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
        shell.referee
    } else {
        owner_type
    };
    if program.normalized_type_identity(leaf) != program.normalized_type_identity(referent) {
        return None;
    }
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        source,
        path: segments,
        type_identity: crate::structural_type_identity(program, referent, &[], &[]),
        access,
    })
}

/// Peel constraint shells and reference wrappers to the nominal owner one
/// carrier's members live under.
fn nominal_member_owner_type<'p>(
    program: &'p dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<&'p typed_trees::data::DataDefinition> {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. } => {
                return program.closed_data_definition(reference);
            }
            _ => return None,
        }
    }
}

struct AggregateProjection {
    root: typed_trees::expression::ExpressionHandle,
    path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    selected_type: TypeReferenceHandle,
}

/// Resolve aggregate selection once for origin transport and borrowed calls.
/// Each field identity and fixed index is checked against its declared carrier;
/// a stored reference remains an explicit crossing after this selection.
fn aggregate_projection(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<AggregateProjection> {
    use checked_trees::CheckedUnitStructuralPathSegment as Segment;
    let mut projections = Vec::new();
    let mut root = expression;
    loop {
        if projections.len() >= MAX_REFERENCE_ORIGIN_DEPTH || projections.contains(&root) {
            return None;
        }
        match program.expression_table.expression(root) {
            ExpressionNode::Member(member) => {
                projections.push(root);
                root = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                projections.push(root);
                root = indexed.collection;
            }
            _ => break,
        }
    }
    let root_type = match program.expression_table.expression(root) {
        ExpressionNode::Name(name) => program
            .state_local_declaration(state, name.symbol)
            .map(|(_, local)| local.type_reference)
            .or_else(|| {
                program
                    .state_parameter_by_symbol(state, name.symbol)
                    .map(|parameter| parameter.type_reference)
            })?,
        ExpressionNode::Call(call) => {
            let (_, destination) =
                crate::machine_calls::calls::machine_state_by_symbol(program, call.target_symbol)
                    .or_else(|| {
                    let machine = program.machine_by_symbol(call.target_symbol)?;
                    Some((machine, program.machine_states(machine).first()?))
                })?;
            destination.return_type
        }
        _ => return None,
    };
    let root_type = input_carrier_type(program, root_type)?;
    let mut selected = root_type;
    let mut path = Vec::with_capacity(projections.len());
    for projection in projections.into_iter().rev() {
        selected = crate::peel_declared_constraints(program, selected);
        match program.expression_table.expression(projection) {
            ExpressionNode::Member(member) => {
                // Value transport stays in owned storage; crossing a reference
                // requires an authored borrow rather than moving its referent.
                if crate::reference_shell(program, selected).is_some() {
                    return None;
                }
                let data = nominal_member_owner_type(program, selected)?;
                let field = crate::value_custody::places::exact_data_member_field(
                    program,
                    data,
                    member.member_symbol,
                    member.member.as_str(),
                    member.case_variant.as_ref().map(|variant| variant.as_str()),
                )?;
                if field.relevance.is_erased() {
                    return None;
                }
                // A payload projection selects its declared case namespace,
                // not a same-named sibling field. Keep that coordinate when
                // stripping the selected prefix from transported loan paths.
                if let Some(variant) = facts::payload_variant_for_field(program, field.symbol) {
                    let case = program
                        .data_members(data)
                        .iter()
                        .filter_map(|member| member.as_variant())
                        .find(|case| case.symbol == variant)?;
                    path.push(Segment::Case(
                        checked_trees::CheckedMemberIdentity::of_member(
                            case.identity,
                            case.name.clone(),
                        ),
                    ));
                }
                path.push(Segment::Field(
                    checked_trees::CheckedMemberIdentity::of_member(
                        field.identity,
                        field.name.clone(),
                    ),
                ));
                selected = field.type_reference;
            }
            ExpressionNode::Indexed(indexed) => {
                let (machine, _) =
                    crate::machine_calls::calls::machine_state_by_symbol(program, state.symbol)?;
                let index_type =
                    crate::declarations::operators::landed_integer_literal_type_reference(
                        program,
                        indexed.index,
                    );
                if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    machine.symbol,
                    projection,
                    language_core::OperatorSpelling::Index,
                    &[Some(selected), index_type],
                ) {
                    return None;
                }
                let index = program.expression_table.integer_node(indexed.index)?;
                let index = usize::try_from(index.value_i64()?).ok()?;
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(selected)
                else {
                    return None;
                };
                if index >= *length {
                    return None;
                }
                path.push(Segment::FixedIndex(u64::try_from(index).ok()?));
                selected = *element_type;
            }
            _ => return None,
        }
    }
    Some(AggregateProjection {
        root,
        path,
        selected_type: selected,
    })
}

fn reference_access_can_supply(
    source: CheckedStructuralAccess,
    result: CheckedStructuralAccess,
) -> bool {
    source == result
        || matches!(
            (source, result),
            (
                CheckedStructuralAccess::MutableBorrow,
                CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::WriteOnlyBorrow
            )
        )
}

/// Exact source correspondences for descriptor forwarding and static argument
/// transport. Runtime element families are borrowing origins, not a roster of
/// resident descriptors: they stay in the frame relation and captured result
/// publication. Decline them here rather than expanding an array's capacity.
/// Each exact path still checks its reference crossings and returned access.
fn whole_reference_sources(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    let (referent, access) = stored_loan_parts(program, state.return_type)?;
    let prepared_frames;
    let call_frames = match call_frames {
        Some(call_frames) => call_frames,
        None => {
            prepared_frames = crate::CallFrameResolver::new(program)?;
            &prepared_frames
        }
    };
    let origins = call_frames.reference_result_origins(state.symbol)?;
    if origins.is_empty() || origins.len() > MAX_REFERENCE_ORIGIN_LEAVES {
        return None;
    }
    program.machine_holding_state(state.symbol)?;
    let mut sources = Vec::with_capacity(origins.len());
    for origin in origins {
        if !origin.root.is_valid()
            || origin.representation.is_some()
            || !origin.recasts.is_empty()
            || origin.segments.len() > MAX_REFERENCE_ORIGIN_DEPTH
        {
            return None;
        }
        let (parameter_index, parameter) = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| is_mutable_structural_parameter(program, parameter))
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == origin.root)?;
        let segments = &origin.segments;
        let mut selected = parameter.type_reference;
        let mut borrowed = false;
        let mut path = Vec::with_capacity(origin.segments.len() + 1);
        if let Some((referee, parameter_access)) = stored_loan_parts(program, selected) {
            if !reference_access_can_supply(parameter_access, access) {
                return None;
            }
            selected = referee;
            borrowed = true;
        }
        for segment in segments {
            if let Some((referee, leaf_access)) = stored_loan_parts(program, selected) {
                if !reference_access_can_supply(leaf_access, access) {
                    return None;
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                selected = referee;
                borrowed = true;
            }
            match segment {
                facts::PlaceSegment::Field { symbol } => {
                    // The nominal lookup also peels reference shells. Every
                    // crossing here must instead be admitted explicitly above
                    // so an unsupported nested carrier cannot lose its access
                    // check or its Referent step during field lookup.
                    if crate::reference_shell(program, selected).is_some() {
                        return None;
                    }
                    let data = nominal_member_owner_type(program, selected)?;
                    let field =
                        program
                            .data_members(data)
                            .iter()
                            .find_map(|member| match member {
                                typed_trees::data::DataMember::Field(field)
                                    if field.symbol == *symbol && !field.relevance.is_erased() =>
                                {
                                    Some(field)
                                }
                                _ => None,
                            })?;
                    path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ),
                    ));
                    selected = field.type_reference;
                }
                facts::PlaceSegment::FixedIndex { index } => {
                    selected = loop {
                        match program.type_reference_table.type_reference(selected) {
                            TypeReferenceNode::Constrained { base_type, .. } => {
                                selected = *base_type
                            }
                            TypeReferenceNode::FixedArray { element_type, .. } => {
                                break *element_type;
                            }
                            _ => return None,
                        }
                    };
                    path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                        u64::try_from(*index).ok()?,
                    ));
                }
                _ => return None,
            }
        }
        if let Some((referee, leaf_access)) = stored_loan_parts(program, selected) {
            if !reference_access_can_supply(leaf_access, access) {
                return None;
            }
            path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
            selected = referee;
            borrowed = true;
        }
        if !borrowed
            || program.normalized_type_identity(selected)
                != program.normalized_type_identity(referent)
        {
            return None;
        }
        let source = checked_trees::CheckedUnitStructuralArgumentPlan {
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(parameter_index).ok()?,
            },
            path,
            type_identity: program.normalized_type_identity(referent),
            access,
        };
        let source = checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source,
        };
        if !sources.contains(&source) {
            if sources.len() == MAX_REFERENCE_ORIGIN_LEAVES {
                return None;
            }
            sources.push(source);
        }
    }
    Some(sources)
}

/// A terminal invocation forwards its reference directly to the state's return.
/// Its exact flow occurrence has no expression/local holder; the ingress roster
/// is therefore the custody, rather than a fabricated local loan handle.
pub fn terminal_call_reference_sources(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<Vec<checked_trees::CheckedReturnedReferencePlan>> {
    if call.authored_expression.is_valid()
        || call.statement_index != result.statement_index as usize
        || stored_loan_parts(program, state.return_type).is_none()
        || result.type_identity != program.normalized_type_identity(state.return_type)
    {
        return None;
    }
    let transition = program
        .statement_table
        .statement_at(state, call.statement_index)?
        .as_transition()?;
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return None;
    }
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    let target = program.state_by_symbol(call.target_symbol).or_else(|| {
        program
            .machine_by_symbol(call.target_symbol)
            .and_then(|machine| program.machine_states(machine).first())
    })?;
    if path.symbol != target.symbol
        && !program
            .machine_by_symbol(path.symbol)
            .is_some_and(|machine| {
                program
                    .machine_states(machine)
                    .first()
                    .is_some_and(|first| first.symbol == target.symbol)
            })
    {
        return None;
    }
    captured_returns::terminal_call_sources(program, facts, state, call)
}

/// A stored carrier transports its existing borrowed leaves. Reference leaves
/// name a referent; sequence views name the descriptor itself, whose backing
/// is retained by its captured loan. A view is not an explicit `Referent`
/// crossing. Neither source form authorizes creating a fresh loan or moving
/// the backing storage.
pub fn record_carrier_sources(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    carrier: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    declared_carrier_references(program, reference)?
        .into_iter()
        .map(|leaf| {
            let path = leaf
                .path
                .iter()
                .map(|projection| match projection {
                    checked_trees::CheckedReferenceOriginProjection::Exact(segment) => {
                        Some(segment.clone())
                    }
                    checked_trees::CheckedReferenceOriginProjection::Element => None,
                })
                .collect::<Option<Vec<_>>>()?;
            let mut source_path = path.clone();
            if leaf.crosses_reference {
                source_path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
            }
            Some(checked_trees::CheckedReferenceResultSourcePlan {
                path,
                source: checked_trees::CheckedUnitStructuralArgumentPlan {
                    source: carrier.clone(),
                    path: source_path,
                    type_identity: leaf.type_identity,
                    access: leaf.access,
                },
            })
        })
        .collect()
}

/// Declaration-relative descriptor frontier. An unsized collection contributes
/// one element family, not an invented executable selector or an enumeration of
/// runtime storage. Ingress loan identities can retain that family; executable
/// stored-value rosters above still require concrete slots.
pub struct DeclaredCarrierReference {
    pub path: Vec<checked_trees::CheckedReferenceOriginProjection>,
    pub type_identity: typed_trees::type_identity::TypeIdentityId,
    pub access: CheckedStructuralAccess,
    pub crosses_reference: bool,
}

pub fn declared_carrier_references(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<Vec<DeclaredCarrierReference>> {
    use checked_trees::CheckedReferenceOriginProjection::{Element, Exact};
    fn visit(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        path: &mut Vec<checked_trees::CheckedReferenceOriginProjection>,
        output: &mut Vec<DeclaredCarrierReference>,
        scopes: &mut DeclarationScopes,
        scope: usize,
    ) -> Option<()> {
        let (reference, scope) = scopes.head(program, reference, scope);
        // Sequence loans retain the view descriptor at every access mode.
        // Restricting this frontier to shared views loses the source region of
        // an exclusive element returned through a requirement signature.
        if let Some(shell) = crate::reference_shell(program, reference)
            && (matches!(
                program
                    .type_reference_table
                    .type_reference(crate::peel_declared_constraints(program, shell.referee)),
                TypeReferenceNode::Slice { .. }
            ) || crate::byte_sequence_carrier(program, reference, &[])
                .is_some_and(|carrier| carrier.is_borrowed()))
        {
            output.push(DeclaredCarrierReference {
                path: path.clone(),
                type_identity: scopes.type_identity(program, reference, scope),
                access: shell.access.into(),
                crosses_reference: false,
            });
            return Some(());
        }
        if let Some((referent, access)) = stored_loan_parts(program, reference) {
            output.push(DeclaredCarrierReference {
                path: path.clone(),
                type_identity: scopes.type_identity(program, referent, scope),
                access,
                crosses_reference: true,
            });
            return Some(());
        }
        if crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::NumericOwned,
        ) {
            return Some(());
        }
        if let TypeReferenceNode::FixedArray {
            element_type,
            length: typed_trees::types::FixedArrayLength::Literal(length),
        } = program.type_reference_table.type_reference(reference)
        {
            for element_index in 0..*length {
                path.push(Exact(
                    checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                        u64::try_from(element_index).ok()?,
                    ),
                ));
                visit(program, *element_type, path, output, scopes, scope)?;
                path.pop();
            }
            return Some(());
        }
        if let TypeReferenceNode::Slice { element_type } =
            program.type_reference_table.type_reference(reference)
        {
            path.push(Element);
            visit(program, *element_type, path, output, scopes, scope)?;
            path.pop();
            return Some(());
        }
        if let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            return visit(program, *base_type, path, output, scopes, scope);
        }
        let selected = scopes.definition(program, reference, scope);
        let (data, scope) = selected?;
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape {
            return None;
        }
        for member in program.data_members(data) {
            let fields = match member {
                typed_trees::data::DataMember::Field(field) => std::slice::from_ref(field),
                typed_trees::data::DataMember::Variant(case) => {
                    path.push(Exact(
                        checked_trees::CheckedUnitStructuralPathSegment::Case(
                            checked_trees::CheckedMemberIdentity::of_member(
                                case.identity,
                                case.name.clone(),
                            ),
                        ),
                    ));
                    program.data_payload_fields(case)
                }
            };
            for field in fields {
                if field.relevance.is_erased() {
                    continue;
                }
                path.push(Exact(
                    checked_trees::CheckedUnitStructuralPathSegment::Field(
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ),
                    ),
                ));
                visit(program, field.type_reference, path, output, scopes, scope)?;
                path.pop();
            }
            if matches!(member, typed_trees::data::DataMember::Variant(_)) {
                path.pop();
            }
        }
        Some(())
    }
    // The leaf walk owns the stored carrier family, including a whole
    // exclusive reference. The record classifier intentionally excludes that
    // root family and must not act as a second eligibility gate here.
    if !reference_origin_expansion_is_bounded(program, reference) {
        return None;
    }
    let mut output = Vec::new();
    visit(
        program,
        reference,
        &mut Vec::new(),
        &mut output,
        &mut DeclarationScopes::default(),
        0,
    )?;
    Some(output)
}

/// An owned parameter retains every stored borrowed leaf as an opaque ingress
/// origin. A storage classifier alone cannot supply this custody: join the
/// producer's exact owner, state, parameter and declared leaf paths. State
/// arrivals use the same roster as invocation entry; ordinary edge formation
/// still proves the actual descriptor's access and backing lifetime.
pub fn stored_carrier_ingress_is_complete(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameter: SymbolHandle,
    reference: TypeReferenceHandle,
) -> bool {
    let Some(carrier) = input_carrier_type(program, reference) else {
        return false;
    };
    let Some(sources) = record_carrier_sources(
        program,
        carrier,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: parameter,
        },
    ) else {
        return false;
    };
    let incoming = facts
        .borrow
        .incoming_references
        .iter()
        .filter_map(|(_, row)| {
            (row.machine_symbol == machine
                && row.state_symbol == state
                && row.parameter_symbol() == parameter)
                .then_some(row)
        })
        .collect::<Vec<_>>();
    if incoming.len() != sources.len() {
        return false;
    }
    sources.iter().all(|source| {
        let Some(path) = loan_owner_path(program, carrier, &source.path) else {
            return false;
        };
        incoming
            .iter()
            .filter(|row| {
                row.carrier_type == carrier
                    && facts.borrow.owner_segments.span(row.owner_path) == Some(path.as_slice())
            })
            .count()
            == 1
    })
}

/// A readable input exposes its carrier's stored descriptors, not ownership
/// of that carrier. Root-reference permission and contained permissions are
/// different resources; projection and window admission account for the first.
pub fn input_carrier_type(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    let reference = crate::peel_declared_constraints(program, reference);
    match crate::reference_shell(program, reference) {
        Some(shell) if shell.access.is_readable() => Some(shell.referee),
        Some(_) => None,
        None => Some(reference),
    }
}

/// Whether a local of type `reference` must join its stored
/// carrier leaves to captured loans (`local_record_loans`). Record loans
/// belong to a locally constructed record holding reference carriers. A bare
/// `&'a V` shared view is no construction: a selected borrow binds it through
/// the shared-borrow join. `is_reference_record` counts that view for result
/// custody, so this local-construction gate excludes it explicitly. The
/// checked producer and the lowering replay both ask this one question.
pub fn local_owes_record_loans(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    is_reference_record(program, reference)
        && shared_borrowed_parts(program, reference).is_none()
        && crate::stored_view_shell(program, reference, &[]).is_none()
}

/// A descriptor leaf retains a complete group of possible backing origins.
/// The borrow producer already admitted those sources. Join its published
/// occurrences to declared storage paths instead of replaying the initializer:
/// body-selected origins cannot erase declaration-level lifetime obligations.
pub type RecordLeafLoans = (
    checked_trees::CheckedReferenceResultSourcePlan,
    arena::HandleSpan<checked_trees::BorrowLoanFact>,
);

/// Join every declared carrier leaf to its captured formation group. Storage
/// identity and the checked activation distinguish this establishment from a
/// later replacement; similar lifetime spelling alone grants no authority.
pub fn local_record_loans(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
) -> Option<Vec<RecordLeafLoans>> {
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index as usize)?
        .as_local_data()?;
    let result = named_place_loans(
        program,
        facts,
        machine,
        state,
        local.symbol,
        statement_index,
        &[],
    );
    result
}

/// Declared geometry of a named local or formal storage occurrence. A formal
/// can acquire a stored descriptor without owning its incoming referent.
/// Looking up that geometry grants neither access nor a new loan activation.
/// A constructor needs its receiving declaration at the formation itself;
/// ordinary operand lookup deliberately exposes only earlier declarations.
/// Keep that visibility rule unchanged and obtain storage geometry directly.
pub fn storage_owner_type_reference(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    owner: SymbolHandle,
    statement_index: u32,
) -> Option<TypeReferenceHandle> {
    if let Some((ordinal, local)) = program.state_local_declaration(state, owner) {
        return (ordinal <= statement_index as usize).then_some(local.type_reference);
    }
    crate::canonical_place_type_reference(
        program,
        state.symbol,
        statement_index as usize,
        &crate::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(owner),
            segments: Vec::new(),
        },
    )
}

/// Join a named storage occurrence formed by a constructor, call or store.
/// The declared leaf roster and exact activation are independent of the
/// producer. A subtree replacement captures only leaves beneath its destination.
pub fn named_place_loans(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    owner: SymbolHandle,
    statement_index: u32,
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<RecordLeafLoans>> {
    let reference = storage_owner_type_reference(program, state, owner, statement_index)?;
    storage_place_loans(
        program,
        facts,
        machine,
        state,
        facts::PlaceRoot::Symbol(owner),
        reference,
        statement_index,
        selection,
    )
}

/// Join descriptors to the home that actually stores them. A named local and
/// computed backing differ in identity, not leaf geometry or formation proof.
pub fn storage_place_loans(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    owner: facts::PlaceRoot,
    reference: TypeReferenceHandle,
    statement_index: u32,
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<RecordLeafLoans>> {
    let selected_owner_path = loan_owner_path(program, reference, selection)?;
    let mut states = facts.borrow.states.iter().filter_map(|(_, borrowing)| {
        (borrowing.machine_symbol == machine && borrowing.state_symbol == state.symbol)
            .then_some(borrowing)
    });
    let borrowing = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let loans = facts
        .borrow
        .storage_loans(borrowing, owner, statement_index as usize)?;
    let flow = state_flow(facts, machine, state.symbol)?;
    let activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span(flow.borrow_activations)?;
    let mut captured = Vec::new();
    for (handle, loan) in facts.borrow.loan_group(loans)? {
        if !facts
            .borrow
            .loan_owner_path(loan)
            .starts_with(&selected_owner_path)
        {
            continue;
        }
        let mut matching = activations
            .iter()
            .filter(|activation| activation.loan == handle);
        if matching.next()?.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: statement_index as usize,
            })
            || matching.next().is_some()
        {
            return None;
        }
        captured.push(handle);
    }
    record_leaf_loan_groups(program, facts, reference, owner, &captured, selection)
}

/// A borrowed formal has an ingress permission, not an installed owned
/// descriptor. Its first whole-binding replacement can establish a descriptor
/// without disposing an old carrier. Require the new occurrence's complete
/// statement activation and prove that no earlier stored occurrence existed;
/// incoming permissions never become synthetic statement-zero loans.
pub fn initial_parameter_reference_establishment(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    owner: SymbolHandle,
    statement_index: u32,
) -> Option<Vec<RecordLeafLoans>> {
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == owner)?;
    if !parameter.is_mutable
        || crate::reference_shell(program, parameter.type_reference)?.access
            != language_semantics::ReferenceAccess::Shared
        || !crate::stores_reference_shape(program, parameter.type_reference, &[])
    {
        return None;
    }
    let mut states = facts.borrow.states.iter().filter_map(|(_, borrowing)| {
        (borrowing.machine_symbol == machine && borrowing.state_symbol == state.symbol)
            .then_some(borrowing)
    });
    let borrowing = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let owner_path = loan_owner_path(program, parameter.type_reference, &[])?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    if facts.borrow.loan_group(borrowing.loans)?.any(|(_, loan)| {
        loan.storage_owner == facts::PlaceRoot::Symbol(owner)
            && facts.borrow.loan_owner_path(loan) == owner_path.as_slice()
            && order.precedes(loan.statement_index, statement_index as usize)
    }) {
        return None;
    }
    let established =
        named_place_loans(program, facts, machine, state, owner, statement_index, &[])?;
    let [(_, loans)] = established.as_slice() else {
        return None;
    };
    (!loans.is_empty()).then_some(established)
}

/// A stored reference's old occurrence retired by this exact replacement.
/// Loan death is not an owned-referent disposition: it authorizes ending only
/// the selected descriptor. Select the complete historical formation group,
/// including every possible origin, before making that distinction executable.
pub fn replaced_reference_carrier(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    owner: facts::PlaceRoot,
    statement_index: u32,
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<RecordLeafLoans> {
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let reference = crate::canonical_place_type_reference(
        program,
        state.symbol,
        statement_index as usize,
        &crate::CanonicalPlace {
            root: owner,
            segments: Vec::new(),
        },
    )?;
    let owner_path = loan_owner_path(program, reference, selection)?;
    let flow = state_flow(facts, machine, state.symbol)?;
    let mut formation = None;
    for weakening in facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span(flow.borrow_weakenings)?
    {
        if weakening.reason != checked_trees::FlowBorrowWeakeningReason::LocalReassigned
            || weakening.source
                != (checked_trees::FlowInvalidationSource::Statement {
                    statement_index: statement_index as usize,
                })
        {
            continue;
        }
        let loan = facts.borrow.loans.get(weakening.loan);
        if loan.storage_owner != owner || facts.borrow.loan_owner_path(loan) != owner_path {
            continue;
        }
        if !order.precedes(loan.statement_index, statement_index as usize)
            || formation.is_some_and(|previous| previous != loan.statement_index)
        {
            return None;
        }
        formation = Some(loan.statement_index);
    }
    let groups = storage_place_loans(
        program,
        facts,
        machine,
        state,
        owner,
        reference,
        u32::try_from(formation?).ok()?,
        selection,
    )?;
    let [group] = groups.as_slice() else {
        return None;
    };
    // Pointer carriers cross their stored reference shell; a slice leaf names
    // its descriptor directly. Both retire the exact prior carrier occurrence,
    // never the referent or a descendant loan that survives this replacement.
    if group.0.path != selection
        || (group.0.source.path != selection
            && group
                .0
                .source
                .path
                .strip_suffix(&[checked_trees::CheckedUnitStructuralPathSegment::Referent])
                != Some(selection))
        || !facts.borrow.loan_group(group.1)?.all(|(handle, _)| {
            loan_weakening(facts, machine, state.symbol, handle).is_some_and(|weakening| {
                weakening.reason == checked_trees::FlowBorrowWeakeningReason::LocalReassigned
                    && weakening.source
                        == (checked_trees::FlowInvalidationSource::Statement {
                            statement_index: statement_index as usize,
                        })
            })
        })
    {
        return None;
    }
    Some(group.clone())
}

/// Loan rows for one descriptor are contiguous in the producer's arena, even
/// when that descriptor retains several possible origins. The checked captured
/// roster owns residency: declarations enumerate possible slots, not loans for
/// inactive cases. Every captured row must join one declared slot exactly once;
/// enumerating all declared slots must never manufacture extra permissions.
fn record_leaf_loan_groups(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    reference: TypeReferenceHandle,
    owner: facts::PlaceRoot,
    loans: &[arena::Handle<checked_trees::BorrowLoanFact>],
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<RecordLeafLoans>> {
    let source = match owner {
        facts::PlaceRoot::Symbol(symbol) => {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        }
        facts::PlaceRoot::Expression(expression) => {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression,
            }
        }
        _ => return None,
    };
    let sources = record_carrier_sources(program, reference, source)?;
    let mut grouped_count = 0usize;
    let mut output = Vec::with_capacity(sources.len());
    for source in sources
        .into_iter()
        .filter(|source| source.path.starts_with(selection))
    {
        let owner_path = loan_owner_path(program, reference, &source.path)?;
        let access = source.source.access.borrow_access_kind()?;
        let mut group = arena::HandleSpan::empty();
        let mut formation = None;
        for handle in loans.iter().copied() {
            let loan = facts.borrow.loans.get(handle);
            if facts.borrow.loan_owner_path(loan) != owner_path {
                continue;
            }
            if loan.storage_owner != owner
                || loan.kind != access
                || formation.is_some_and(|statement| statement != loan.statement_index)
                || (!group.is_empty() && !group.is_followed_by(handle))
            {
                return None;
            }
            formation = Some(loan.statement_index);
            let start = if group.is_empty() {
                handle
            } else {
                group.start()
            };
            group = arena::HandleSpan::from_parts(start, group.count().checked_add(1)?);
            grouped_count = grouped_count.checked_add(1)?;
        }
        if group.is_empty() {
            continue;
        }
        output.push((source, group));
    }
    (grouped_count == loans.len()).then_some(output)
}

/// The loan segments a borrowed initializer source projects under its
/// parameter's declared type, spelled in the ownership vocabulary the
/// ledger records. Field hops resolve through the nominal owner exactly as
/// the owner-path walk does; literal fixed-array hops keep their ordinal.
/// Case hops retain the declaration guard and select its own payload namespace;
/// they are not a claim that inactive payload storage is live. Runtime-indexed
/// and referent hops use captured loans instead of direct backing geometry.
pub fn loan_place_segments(
    program: &dyn crate::CheckingInput,
    root: TypeReferenceHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<facts::PlaceSegment>> {
    origin_place_segments(
        program,
        root,
        path.iter()
            .cloned()
            .map(checked_trees::CheckedReferenceOriginProjection::Exact),
    )
}

/// Translate a declaration frontier into semantic owner coordinates. Element
/// families remain unknown selectors; they never become executable indices.
pub fn origin_place_segments(
    program: &dyn crate::CheckingInput,
    root: TypeReferenceHandle,
    path: impl IntoIterator<Item = checked_trees::CheckedReferenceOriginProjection>,
) -> Option<Vec<facts::PlaceSegment>> {
    let mut selected = root;
    let mut payload = None;
    let mut segments = Vec::new();
    let mut scopes = DeclarationScopes::default();
    let mut scope = 0;
    for projection in path {
        (selected, scope) = scopes.head(program, selected, scope);
        while let TypeReferenceNode::Constrained { base_type, .. }
        | TypeReferenceNode::Reference {
            referee: base_type, ..
        } = program.type_reference_table.type_reference(selected)
        {
            (selected, scope) = scopes.head(program, *base_type, scope);
        }
        let checked_trees::CheckedReferenceOriginProjection::Exact(segment) = projection else {
            if payload.is_some() {
                return None;
            }
            let (TypeReferenceNode::Slice { element_type }
            | TypeReferenceNode::FixedArray { element_type, .. }) =
                program.type_reference_table.type_reference(selected)
            else {
                return None;
            };
            selected = *element_type;
            segments.push(facts::PlaceSegment::Index {
                expression: ExpressionHandle::invalid(),
            });
            continue;
        };
        let segment = &segment;
        match segment {
            checked_trees::CheckedUnitStructuralPathSegment::Case(identity) => {
                if payload.is_some() {
                    return None;
                }
                let (data, nested_scope) = scopes.definition(program, selected, scope)?;
                scope = nested_scope;
                let case = program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Variant(case)
                            if checked_trees::CheckedMemberIdentity::of_member(
                                case.identity,
                                case.name.clone(),
                            ) == *identity =>
                        {
                            Some(case)
                        }
                        _ => None,
                    })?;
                segments.push(facts::PlaceSegment::Case {
                    variant: case.symbol,
                });
                payload = Some(case);
            }
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                let field = if let Some(case) = payload.take() {
                    program.data_payload_fields(case).iter().find(|field| {
                        checked_trees::CheckedMemberIdentity::of_member(
                            field.identity,
                            field.name.clone(),
                        ) == *identity
                    })?
                } else {
                    let (data, nested_scope) = scopes.definition(program, selected, scope)?;
                    scope = nested_scope;
                    program
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            typed_trees::data::DataMember::Field(field)
                                if checked_trees::CheckedMemberIdentity::of_member(
                                    field.identity,
                                    field.name.clone(),
                                ) == *identity =>
                            {
                                Some(field)
                            }
                            _ => None,
                        })?
                };
                segments.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                selected = field.type_reference;
            }
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
                if payload.is_some() {
                    return None;
                }
                let mut carrier = selected;
                loop {
                    match program.type_reference_table.type_reference(carrier) {
                        TypeReferenceNode::Constrained { base_type, .. } => carrier = *base_type,
                        TypeReferenceNode::FixedArray { element_type, .. }
                        | TypeReferenceNode::Slice { element_type } => {
                            selected = *element_type;
                            break;
                        }
                        _ => return None,
                    }
                }
                segments.push(facts::PlaceSegment::FixedIndex {
                    index: usize::try_from(*index).ok()?,
                });
            }
            _ => return None,
        }
    }
    Some(segments)
}

pub fn loan_owner_path(
    program: &dyn crate::CheckingInput,
    root: TypeReferenceHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<checked_trees::BorrowLoanOwnerSegment>> {
    origin_owner_path(
        program,
        root,
        path.iter()
            .cloned()
            .map(checked_trees::CheckedReferenceOriginProjection::Exact),
    )
}

pub fn origin_owner_path(
    program: &dyn crate::CheckingInput,
    root: TypeReferenceHandle,
    path: impl IntoIterator<Item = checked_trees::CheckedReferenceOriginProjection>,
) -> Option<Vec<checked_trees::BorrowLoanOwnerSegment>> {
    origin_place_segments(program, root, path)?
        .into_iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Field { symbol } => {
                Some(checked_trees::BorrowLoanOwnerSegment::Field(symbol))
            }
            facts::PlaceSegment::FixedIndex { index } => {
                Some(checked_trees::BorrowLoanOwnerSegment::FixedIndex(index))
            }
            facts::PlaceSegment::Case { variant } => {
                Some(checked_trees::BorrowLoanOwnerSegment::Case(variant))
            }
            facts::PlaceSegment::Index { expression } if !expression.is_valid() => {
                Some(checked_trees::BorrowLoanOwnerSegment::DynamicIndex)
            }
            _ => None,
        })
        .collect()
}

/// The checked loan constraints available before this exact statement.
fn statement_entry_constraints(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
) -> Option<&[checked_trees::FlowConstraintRef]> {
    let flow = state_flow(facts, machine, state)?;
    let entries = facts.flow.control.statements.span(flow.statements)?;
    let mut entries = entries
        .iter()
        .filter(|entry| entry.statement_index == statement_index as usize);
    let entry = entries.next()?;
    if entries.next().is_some() {
        return None;
    }
    facts
        .flow
        .contexts
        .constraint_refs
        .span(entry.entry_constraints)
}

/// Join the current carrier's declared leaves to the active captured occurrences.
/// Its declaration identifies storage and type, not the initializer's lifetime:
/// replacement can install a new occurrence in that same home. Operand custody
/// reads the checked prefix once instead of replaying an overwritten initializer.
fn active_record_loans(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    declaration_statement: u32,
) -> Option<Vec<RecordLeafLoans>> {
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    if !order.precedes(declaration_statement as usize, statement_index as usize) {
        return None;
    }
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .get(declaration_statement as usize)?
        .as_local_data()?;
    let constraints = statement_entry_constraints(facts, machine, state.symbol, statement_index)?;
    let mut loans = constraints
        .iter()
        .filter_map(|constraint| {
            let checked_trees::FlowConstraintKind::BorrowLoan { loan } = constraint.kind else {
                return None;
            };
            let captured = facts.borrow.loans.get(loan);
            (captured.storage_owner == facts::PlaceRoot::Symbol(local.symbol)).then_some(loan)
        })
        .collect::<Vec<_>>();
    // Flow contexts preserve their own constraint order. Arena order owns the
    // contiguous descriptor groups; do not make context insertion order part
    // of the custody join.
    loans.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    if loans.iter().any(|handle| {
        !order.precedes(
            facts.borrow.loans.get(*handle).statement_index,
            statement_index as usize,
        )
    }) {
        return None;
    }
    record_leaf_loan_groups(
        program,
        facts,
        local.type_reference,
        facts::PlaceRoot::Symbol(local.symbol),
        &loans,
        &[],
    )
}

/// The call's reference-record destination parameters and its returned leaf
/// sources — `None` when the call does not return a reference record.
/// Callers then require each source's complete active leaf group, filtered per site.
fn reference_record_call_sources<'program>(
    program: &'program dyn crate::CheckingInput,
    facts: &CheckFacts,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<(
    &'program [typed_trees::signature::StateParameter],
    Vec<checked_trees::CheckedReturnedReferencePlan>,
)> {
    let return_type = crate::call_target_return_type(program, call.target_symbol)?;
    let parameters = crate::call_target_parameters(program, call.target_symbol)?;
    if !is_reference_record(program, return_type) {
        return None;
    }
    let sources = captured_returns::call_sources(program, facts, call.target_symbol)?;
    Some((parameters, sources))
}

/// Returned leaves not installed in a local stay in the call's temporary
/// carrier until its consuming edge. Borrowed ingress uses already-admitted
/// operand authority; owned descriptor inputs retain their complete active
/// leaf groups. Neither case extends a loan past the temporary's disposal.
pub fn transient_call_record_loans(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    arguments: &[checked_trees::CheckedUnitStructuralArgumentPlan],
    residual_paths: &[Vec<checked_trees::CheckedUnitStructuralPathSegment>],
) -> bool {
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    let Some((parameters, sources)) = reference_record_call_sources(program, facts, call) else {
        return false;
    };
    let Some(flow) = state_flow(facts, machine, state.symbol) else {
        return false;
    };
    let Some(calls) = facts.flow.control.calls.span(flow.calls) else {
        return false;
    };
    let Some(occurrence) = calls.iter().find(|occurrence| {
        occurrence.authored_expression == expression
            && occurrence.statement_index == statement_index as usize
            && occurrence.target_symbol == call.target_symbol
    }) else {
        return false;
    };
    let Some(paths) = captured_returns::call_result_paths(program, facts, occurrence) else {
        return false;
    };
    !sources.is_empty()
        && residual_paths.iter().all(|path| paths.contains(path))
        && sources
            .iter()
            .filter(|source| residual_paths.contains(&source.path))
            .all(|source| {
                let parameter_index = match source.source.root {
                    checked_trees::CheckedReferenceOriginRoot::Parameter { index } => index,
                    checked_trees::CheckedReferenceOriginRoot::Invocation { .. } => return true,
                };
                let Some(parameter) = parameters
                    .iter()
                    .filter(|parameter| is_mutable_structural_parameter(program, parameter))
                    .nth(parameter_index as usize)
                else {
                    return false;
                };
                if crate::reference_shell(program, parameter.type_reference).is_some() {
                    // Borrowed ingress denotes the referent, not an owned slot
                    // ending in Referent. Operand admission already checked its
                    // exact storage, projection and access. The temporary may
                    // attenuate that access, never promote it; it ends at this
                    // call's consuming value edge rather than a local lifetime.
                    let Some(actual) = arguments.get(parameter_index as usize) else {
                        return false;
                    };
                    let Some(parent) = actual.access.borrow_access_kind() else {
                        return false;
                    };
                    let Some(child) = source.source.access.borrow_access_kind() else {
                        return false;
                    };
                    parent.direct_reborrow_effect(&child).is_some()
                } else {
                    // The retained operand/value graph supplies this exact
                    // capture, including anonymous constructors and projected
                    // values. Source admission owns its lifetime obligations;
                    // requiring a bare-name local here replayed only one
                    // producer family and rejected ordinary composition.
                    // Actuals name structural storage, not the qualified
                    // source type. Use the same identity rule as their
                    // producer; source admission retains domain evidence
                    // independently, without changing this descriptor home.
                    arguments
                        .get(parameter_index as usize)
                        .is_some_and(|actual| {
                            actual.type_identity
                                == crate::structural_type_identity(
                                    program,
                                    parameter.type_reference,
                                    &[],
                                    &[],
                                )
                        })
                }
            })
}

/// A call result's reference field is already a captured descriptor. Forward
/// that leaf when the consuming argument projects it. A named local stays on
/// the construction path below; this does not invent a loan for a shared view.
fn call_result_field_argument(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    result: &CheckedUnitStructuralResultBindingPlan,
    expression: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let projection = aggregate_projection(program, state, expression)?;
    let ExpressionNode::Call(produced) = program.expression_table.expression(projection.root)
    else {
        return None;
    };
    let callee = crate::call_target_state(program, produced.target_symbol)?;
    if result.type_identity != program.normalized_type_identity(callee.return_type) {
        return None;
    }
    let destination_leaves = declared_carrier_references(program, destination)?;
    let [destination_leaf] = destination_leaves.as_slice() else {
        return None;
    };
    if !destination_leaf.path.is_empty() || crate::reference_shell(program, destination).is_none() {
        return None;
    }
    if program.normalized_type_identity(projection.selected_type)
        != program.normalized_type_identity(destination)
    {
        return None;
    }
    let returned = captured_return_sources(program, facts, callee)?;
    let leaf = returned.iter().find(|leaf| leaf.path == projection.path)?;
    if leaf.source.type_identity != destination_leaf.type_identity
        || !reference_access_can_supply(leaf.source.access, destination_leaf.access)
    {
        return None;
    }
    let mut path = projection.path;
    if destination_leaf.crosses_reference {
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
    }
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: crate::structural_type_identity(program, destination, &[], &[]),
        access: destination_leaf.access,
    })
}

/// A projected argument crosses the stored carrier only after its exact local
/// construction and active leaf loan have been reconstructed.
pub fn record_argument(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    destination: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    if let Some(argument) =
        call_result_field_argument(program, facts, state, result, expression, destination)
    {
        return Some(argument);
    }
    let destination_leaves = declared_carrier_references(program, destination)?;
    let [destination_leaf] = destination_leaves.as_slice() else {
        return None;
    };
    if !destination_leaf.path.is_empty() || crate::reference_shell(program, destination).is_none() {
        return None;
    }
    let access = destination_leaf.access;
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?
        .as_local_data()?;
    // A whole view local already has a descriptor-forwarding lane. Aggregate
    // projection peels readable roots to their backing; that backing must not
    // be substituted for the identity of a standalone descriptor.
    if !destination_leaf.crosses_reference
        && crate::stored_view_shell(program, local.type_reference, &[]).is_some()
    {
        return None;
    }
    if result.type_identity
        != program
            .normalized_type_identity(local.type_reference)
        // Source reference uses are reusable, but each materialized descriptor
        // owns affine lifetime custody, including descriptors in fixed arrays.
        || result.multiplicity != result_multiplicity(program, local.type_reference)
    {
        return None;
    }
    let projection = aggregate_projection(program, state, expression)?;
    let name = program.expression_table.name_path_node(projection.root)?;
    if name.symbol != local.symbol
        || name.head_symbol != local.symbol
        || !matches!(program.expression_table.name_path_members(name.members), [name] if *name == local.name)
    {
        return None;
    }
    let mut path = projection.path;
    if program.normalized_type_identity(projection.selected_type)
        != program.normalized_type_identity(destination)
    {
        return None;
    }
    let loans = active_record_loans(
        program,
        facts,
        machine,
        state,
        statement_index,
        result.statement_index,
    )?;
    let (source, _) = loans.iter().find(|(source, _)| source.path == path)?;
    if source.source.type_identity != destination_leaf.type_identity
        || !reference_access_can_supply(source.source.access, access)
    {
        return None;
    }
    // A pointer descriptor selects its referent through an explicit crossing.
    // A sequence descriptor already carries the backing and extent; forwarding
    // it must retain that descriptor rather than invent a pointer-shaped hop.
    if destination_leaf.crosses_reference {
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
    }
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: crate::structural_type_identity(program, destination, &[], &[]),
        access,
    })
}

/// Classify every type reference's borrow custody once, over the whole
/// frozen type table.
///
/// The leaf-family, record and multiplicity questions below are pure
/// functions of the type table: they read no machine, no state and no
/// checked fact. Checking answers them while it validates carriers, and
/// lowering needs the same answers at a few hundred call sites. Walking the
/// table once records them for the whole program, so the classifier's
/// admitted shapes stay behind this one entry point instead of being
/// reachable from a later stage.
///
/// The row vector is positional by `TypeReferenceHandle::arena_index`, and
/// index 0 — the invalid handle — carries the classification every handle
/// the table does not name reads. The table is append-only, so an index is
/// never reused and a recorded row cannot go stale under a later handle.
pub fn classify_program_reference_custody(
    program: &dyn crate::CheckingInput,
) -> ReferenceCustodyFacts {
    crate::classify_type_references(program, reference_custody, ReferenceCustodyFacts::with_rows)
}

/// One type reference's custody row.
fn reference_custody(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> ReferenceCustodyRow {
    let stored = stored_loan_parts(program, reference);
    ReferenceCustodyRow {
        primitive_loan_referent: parts(program, reference)
            .map(|(referent, _)| referent)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        stored_loan_referent: stored
            .map(|(referent, _)| referent)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        stored_loan_access: stored
            .map(|(_, access)| access)
            .unwrap_or(CheckedStructuralAccess::Owned),
        mutable_borrow_referent: mutable_borrowed_parts(program, reference)
            .map(|(referent, _)| referent)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        shared_borrow_referent: shared_borrowed_parts(program, reference)
            .map(|(referent, _)| referent)
            .unwrap_or_else(TypeReferenceHandle::invalid),
        is_reference_record: is_reference_record(program, reference),
        binding_multiplicity: binding_multiplicity(program, reference),
        result_multiplicity: result_multiplicity(program, reference),
    }
}

/// Stored reference classification precedes referent-oriented normalization.
/// An exclusive primitive carrier retains its exact mutable or write-only
/// access. The pointer is transported without reading its referee; operations
/// through it still owe that access and the captured loan's own authority.
pub fn parts(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if !access.is_exclusive()
        || !matches!(
            program.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Named { .. }
        )
        || program.primitive_type_reference(*referee).is_none()
    {
        return None;
    }
    Some((*referee, (*access).into()))
}

/// One stored carrier leaf's referent and exact access. Shared, mutable and
/// write-only loans use the same stored-reference relation; access remains
/// independent of the descriptor's representation. Slice and byte views
/// retain their distinct descriptors, not this pointer carrier.
/// Every consumer of the leaf family — the
/// reference-record classifier, ingress reconstruction, construction-source
/// rosters and formal forwarding — asks this one question so the admitted
/// shape never drifts between them.
pub fn stored_loan_parts(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|_| crate::stores_reference_shape(program, reference, &[]))
        .map(|shell| (shell.referee, shell.access.into()))
}

/// The referee beneath any `&mut T` carrier, with constraint shells peeled.
/// Unlike `parts` this does not require a primitive referent: a
/// `&mut Record` names storage in the caller's frame just as a `&mut u8`
/// does. Callers that need the primitive-only reference-result shape keep
/// using `parts`.
pub fn mutable_borrowed_parts(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| shell.access == language_semantics::ReferenceAccess::Mutable)
        .map(|shell| (shell.referee, CheckedStructuralAccess::MutableBorrow))
}

/// A shared stored pointer's referee, with constraint shells peeled. The
/// referent stays in its owner's frame, regardless of its source type family;
/// registering the pointer never demands transitive pointee storage.
pub fn shared_borrowed_parts(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    stored_loan_parts(program, reference)
        .filter(|(_, access)| *access == CheckedStructuralAccess::SharedBorrow)
}

/// Whether `reference` is a caller-frame borrow carrier. Stored pointers retain
/// shared, mutable or write-only access; mutable slice descriptors retain their
/// distinct carrier. Result-custody consumers ask this common question instead
/// of reconstructing an access whitelist. This grants no executable shape.
pub fn has_borrowed_parts(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    stored_loan_parts(program, reference).is_some()
        || mutable_borrowed_parts(program, reference).is_some()
}

/// The referee beneath any `& T` place-loan carrier, with constraint shells
/// peeled, including byte and slice views with distinct descriptors. A
/// `&[u8; 48]` loan names the array's storage in the caller's frame just as
/// a `&u32` does — the same coverage `mutable_borrowed_parts` gives the
/// exclusive side. Callers that need the shared reference-result shape keep
/// using `shared_borrowed_parts`.
pub fn shared_place_borrowed_parts(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| shell.access == language_semantics::ReferenceAccess::Shared)
        .map(|shell| (shell.referee, CheckedStructuralAccess::SharedBorrow))
}

/// Custody of a structural binding that denotes the referent directly.
/// Shared aliases remain unrestricted observations; exclusive aliases retain
/// affine loan custody. A stored reference value instead uses
/// `result_multiplicity`: its descriptor is an owned lifetime resource even
/// when its referent is shared. Aggregate bindings own their stored elements.
pub fn binding_multiplicity(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Multiplicity {
    match crate::reference_shell(program, reference) {
        Some(shell) if shell.access == language_semantics::ReferenceAccess::Shared => {
            Multiplicity::Unrestricted
        }
        Some(_) => Multiplicity::Affine,
        None => result_multiplicity(program, reference),
    }
}

/// Source reference use multiplicity does not encode the lifetime obligation
/// of a materialized carrier. Reference carrier custody must end exactly once.
pub fn result_multiplicity(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Multiplicity {
    // Aggregate custody follows the stored element, not source use
    // multiplicity. References are reusable source observations, but each
    // materialized loan carrier (also inside nested arrays) ends once.
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            return result_multiplicity(program, *base_type);
        }
        TypeReferenceNode::FixedArray { element_type, .. } => {
            return result_multiplicity(program, *element_type);
        }
        _ => {}
    }
    // Stored pointer custody is independent of shared, mutable or write-only
    // access. Mutable slice/byte views retain their separate affine carrier.
    if stored_loan_parts(program, reference).is_some()
        || mutable_borrowed_parts(program, reference).is_some()
    {
        Multiplicity::Affine
    } else {
        program.type_multiplicity(reference)
    }
}

/// Whole-reference completion retains one formal ingress regardless of whether
/// the body spells a name, reborrow, local alias or internal transition. The
/// complete origin relation proves correspondence; an empty source path keeps
/// this query distinct from a projected referent or an owned reference leaf.
pub fn source_parameter(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
) -> Option<usize> {
    let sources = whole_reference_sources(program, state, None)?;
    if sources.iter().any(|source| !source.source.path.is_empty()) {
        return None;
    }
    let position = reference_sources_parameter(program, state, &sources)?;
    let parameter = program.state_parameters(state).get(position)?;
    stored_loan_parts(program, parameter.type_reference)?;
    Some(position)
}

/// An owned formal's returned descriptor retains its exact ingress origin
/// through both direct projections and local aliases. Consume the complete
/// result relation, not the final expression's spelling. This correspondence
/// does not create a loan or discharge the carrier's residual cleanup.
pub fn source_leaf(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
) -> Option<(usize, checked_trees::CheckedUnitStructuralArgumentPlan)> {
    parts(program, state.return_type)?;
    let sources = whole_reference_sources(program, state, None)?;
    let [source] = sources.as_slice() else {
        return None;
    };
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        source.source.source
    else {
        return None;
    };
    let (position, parameter) = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| is_mutable_structural_parameter(program, parameter))
        .nth(parameter_index as usize)?;
    if stored_loan_parts(program, parameter.type_reference).is_some()
        || !is_reference_record(program, parameter.type_reference)
        || !matches!(
            source.source.path.as_slice(),
            [
                _,
                ..,
                checked_trees::CheckedUnitStructuralPathSegment::Referent
            ]
        )
    {
        return None;
    }
    Some((position, source.source.clone()))
}

/// Every normal completion must retain the same caller ingress at the declared
/// result access. The complete relation owns internal jumps and captured alias
/// substitution; distinct projected paths remain alternatives, not permissions.
fn reference_sources_parameter(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    sources: &[checked_trees::CheckedReferenceResultSourcePlan],
) -> Option<usize> {
    let (_, access) = stored_loan_parts(program, state.return_type)?;
    let mut parameter = None;
    for source in sources {
        if !source.path.is_empty() || source.source.access != access {
            return None;
        }
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            source.source.source
        else {
            return None;
        };
        let (position, _) = program
            .state_parameters(state)
            .iter()
            .enumerate()
            .filter(|(_, formal)| {
                !formal.is_const
                    && program
                        .primitive_type_reference(formal.type_reference)
                        .is_none()
            })
            .nth(parameter_index as usize)?;
        if parameter.is_some_and(|prior| prior != position) {
            return None;
        }
        parameter = Some(position);
    }
    parameter
}

/// Borrow the complete loan group that checking produced for this call's
/// result occurrence. Source admission already established its lifetime relation,
/// captured places and access. Do not replay argument initializers here or
/// choose one possible origin: the published facts own those decisions.
///
/// Receiving syntax is not identity: a call may initialize a local directly,
/// feed a selected arm or be captured inside a construction. The loan producer
/// retains that exact formation expression, independently of the enclosing
/// statement's initializer and call ordinal. Join those records, not a second
/// recognizer for direct immutable `let` calls. The span retains every candidate
/// without a second allocation; interleaved groups fail rather than silently
/// dropping a source or including a different call's loans.
pub fn result_loans(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<arena::HandleSpan<checked_trees::BorrowLoanFact>> {
    // An anonymous reference result is retained by its enclosing operand,
    // rather than installed in a named reference holder. Ordered call access
    // and descriptor origins own that temporary lifetime; there is no local
    // captured group for the reference-retirement scheduler to release.
    // This lifetime is independent of whether the referent is a scalar,
    // record or slice. Structural call custody still admits its exact origins.
    if !result.local.is_named()
        && call.authored_expression.is_valid()
        && call.statement_index == result.statement_index as usize
        && !program
            .statement_table
            .statement_at(state, call.statement_index)
            .and_then(typed_trees::statement::StatementNode::as_local_data)
            .is_some_and(|local| local.initial_value == call.authored_expression)
        && crate::call_target_return_type(program, call.target_symbol)
            .is_some_and(|return_type| crate::reference_shell(program, return_type).is_some())
    {
        return Some(arena::HandleSpan::empty());
    }
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?;
    match statement {
        StatementNode::Call(authored)
            if authored.discards_result
                && !call.authored_expression.is_valid()
                && call.statement_index == result.statement_index as usize
                && call.call_ordinal == 0
                && authored.target_symbol == call.target_symbol =>
        {
            return Some(arena::HandleSpan::empty());
        }
        _ => {}
    }
    if !call.authored_expression.is_valid()
        || call.statement_index != result.statement_index as usize
        || program
            .expression_table
            .call_node(call.authored_expression)?
            .target_symbol
            != call.target_symbol
    {
        return None;
    }
    let return_type = crate::call_target_return_type(program, call.target_symbol)?;
    let shell = crate::reference_shell(program, return_type)?;
    let expected_kind = match shell.access {
        language_semantics::ReferenceAccess::Shared => checked_trees::BorrowAccessKind::Read,
        language_semantics::ReferenceAccess::Mutable => checked_trees::BorrowAccessKind::Mutable,
        language_semantics::ReferenceAccess::WriteOnly => {
            checked_trees::BorrowAccessKind::WriteOnly
        }
    };
    let mut states = facts
        .borrow
        .states
        .iter()
        .map(|(_, borrowing)| borrowing)
        .filter(|borrowing| {
            borrowing.machine_symbol == machine && borrowing.state_symbol == state.symbol
        });
    let borrowing = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let mut group = arena::HandleSpan::empty();
    for (handle, loan) in facts.borrow.loan_group(borrowing.loans)? {
        if loan.statement_index != call.statement_index
            || loan.formation_expression != call.authored_expression
        {
            continue;
        }
        if expected_kind.direct_reborrow_effect(&loan.kind).is_none()
            || (!group.is_empty() && !group.is_followed_by(handle))
        {
            return None;
        }
        let start = if group.is_empty() {
            handle
        } else {
            group.start()
        };
        group = arena::HandleSpan::from_parts(start, group.count().checked_add(1)?);
    }
    (!group.is_empty()).then_some(group)
}

/// Earliest statement entry permitted by this local's own uses and loans.
/// A dead reference leaf does not authorize destroying a live scalar sibling.
/// Missing leaf weakening evidence keeps disposal on the existing exit path.
/// The execution scheduler must also retire holders borrowing this local's
/// storage before using this boundary to dispose of its whole value.
pub fn whole_local_release_boundary(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
) -> Option<u32> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let local = statements.get(statement_index as usize)?.as_local_data()?;
    let loans = local_record_loans(program, facts, machine, state, statement_index)?;
    if !local.symbol.is_valid() {
        return None;
    }
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let mut boundary = order.successor(statement_index as usize)?;
    for (_, group) in loans {
        for (loan, _) in facts.borrow.loan_group(group)? {
            let incoming = release_statement(program, facts, machine, state.symbol, loan)? as usize;
            if order.position(incoming)? > order.position(boundary)? {
                boundary = incoming;
            }
        }
    }
    for index in order
        .indices()
        .iter()
        .copied()
        .skip(order.position(statement_index as usize)? + 1)
    {
        if crate::value_custody::source_liveness::statement_uses_symbol(
            program,
            &statements[index],
            local.symbol,
        ) {
            let incoming = order.successor(index)?;
            if order.position(incoming)? > order.position(boundary)? {
                boundary = incoming;
            }
        }
    }
    let boundary = u32::try_from(boundary).ok()?;
    Some(boundary)
}

/// Loans that keep some part of a local's storage borrowed in this state.
/// The loan root identifies backing storage; `source_owner_symbol` instead
/// records transfer provenance and does not keep the former holder alive.
/// Consumers join each formation occurrence to its actual emitted holder and
/// retirement, rather than treating a leaf's lexical weakening as disposal.
pub fn local_storage_loans(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    root: SymbolHandle,
) -> Option<Vec<arena::Handle<checked_trees::BorrowLoanFact>>> {
    let mut states = facts.borrow.states.iter().filter_map(|(_, candidate)| {
        (candidate.machine_symbol == machine && candidate.state_symbol == state)
            .then_some(candidate)
    });
    let borrowing = states.next()?;
    if states.next().is_some() {
        return None;
    }
    Some(
        facts
            .borrow
            .loans
            .iter()
            .filter_map(|(handle, loan)| {
                (facts.borrow.state_owns_loan(borrowing, handle)
                    && loan.root == facts::PlaceRoot::Symbol(root))
                .then_some(handle)
            })
            .collect(),
    )
}

/// The exact occurrence's checked weakening, including replacement-owned death.
/// Its reason is part of the boundary: replacement runs after RHS evaluation,
/// whereas ordinary expiry runs before statement entry.
pub fn loan_weakening(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<&checked_trees::FlowBorrowWeakeningFact> {
    let flow = state_flow(facts, machine, state)?;
    let mut weakenings = facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings)
        .iter()
        .filter(|weakening| weakening.loan == handle);
    let weakening = weakenings.next()?;
    weakenings.next().is_none().then_some(weakening)
}

pub fn release_statement(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<u32> {
    let weakening = loan_weakening(facts, machine, state, handle)?;
    let checked_trees::FlowInvalidationSource::Statement { statement_index } = weakening.source
    else {
        return None;
    };
    let state = program.state_by_symbol(state)?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    if !matches!(
        weakening.reason,
        checked_trees::FlowBorrowWeakeningReason::LastUseExpired
            | checked_trees::FlowBorrowWeakeningReason::StateExit
    ) || order.successor(facts.borrow.loans.get(handle).last_use_statement_index)
        != Some(statement_index)
    {
        return None;
    }
    u32::try_from(statement_index).ok()
}

/// The weakening statement a borrowed result's loan release lands on, or the
/// frame's own edge when the loan's last use is the final statement and no
/// weakening had to restore access — the state exit retires that custody
/// implicitly. Every release-side query shares this answer so the produced
/// operations, their custody accounting and the lowered validation see one
/// boundary.
pub fn release_boundary(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<u32> {
    if let Some(boundary) = release_statement(program, facts, machine, state, handle) {
        return Some(boundary);
    }
    let state = program.state_by_symbol(state)?;
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let last = facts.borrow.loans.get(handle).last_use_statement_index;
    (order.successor(last) == Some(state.statement_nodes.len()))
        .then(|| u32::try_from(state.statement_nodes.len()).ok())
        .flatten()
}

fn state_flow(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
) -> Option<&checked_trees::FlowStateFact> {
    let mut states = facts
        .flow
        .control
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .filter(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state);
    let flow = states.next()?;
    states.next().is_none().then_some(flow)
}
