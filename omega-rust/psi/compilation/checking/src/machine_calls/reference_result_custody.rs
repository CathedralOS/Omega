//! Reference values join ordinary result bindings to exact source loan events.
//! Record construction, projected use and cleanup replay the same authored
//! origins, current loan premises and weakening boundaries independently.

use checked_trees::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralResultBindingPlan,
    ReferenceCustodyFacts, ReferenceCustodyRow, StateReferenceCustodyFacts,
    StateReferenceCustodyRow,
};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

mod primitive_locals;
pub use primitive_locals::primitive_local_source;

// An owned formal has a compact type DAG but its returned-origin roster is
// materialized by path. Bound that expansion before semantic classification or
// allocation. These are private production limits, not reference semantics;
// the leaf limit matches the executable ingress roster's current capacity.
const MAX_REFERENCE_ORIGIN_LEAVES: usize = 4096;
const MAX_REFERENCE_ORIGIN_VISITS: usize = MAX_REFERENCE_ORIGIN_LEAVES * 2;
const MAX_REFERENCE_ORIGIN_DEPTH: usize = 128;

fn reference_origin_expansion_is_bounded(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    fn visit(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        depth: usize,
        visits: &mut usize,
        leaves: &mut usize,
    ) -> Option<()> {
        *visits = visits.checked_add(1)?;
        if *visits > MAX_REFERENCE_ORIGIN_VISITS || depth > MAX_REFERENCE_ORIGIN_DEPTH {
            return None;
        }
        let symbol = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { .. } => {
                *leaves = leaves.checked_add(1)?;
                return (*leaves <= MAX_REFERENCE_ORIGIN_LEAVES).then_some(());
            }
            TypeReferenceNode::Named { symbol, .. } => *symbol,
            TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
            TypeReferenceNode::Constrained { base_type, .. } => {
                return visit(program, *base_type, depth + 1, visits, leaves);
            }
            TypeReferenceNode::FixedArray {
                element_type,
                length,
            } => {
                let before = *leaves;
                visit(program, *element_type, depth + 1, visits, leaves)?;
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
                return visit(program, *element_type, depth + 1, visits, leaves);
            }
            _ => return Some(()),
        };
        if let Some(data) =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, symbol)
        {
            for member in program.data_members(data) {
                if let typed_trees::data::DataMember::Field(field) = member {
                    visit(program, field.type_reference, depth + 1, visits, leaves)?;
                }
            }
        }
        Some(())
    }
    visit(program, reference, 0, &mut 0, &mut 0).is_some()
}

/// Closed aggregate construction can contain real reference carriers.
/// Arrays recurse through the same stored-value tree as record fields; the
/// owned aggregate holds each loan descriptor, never the referent's bytes.
/// This classifier grants neither an initializer origin nor a loan lifetime.
pub fn is_reference_record(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    if !reference_origin_expansion_is_bounded(program, reference) {
        return false;
    }
    fn visit(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        seen: &mut Vec<SymbolHandle>,
    ) -> Option<bool> {
        if stored_loan_parts(program, reference).is_some() {
            return Some(true);
        }
        if crate::has_plain_owned_contents_with_numeric_constraints(program, reference) {
            return Some(false);
        }
        if let TypeReferenceNode::FixedArray { element_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            return visit(program, *element_type, seen);
        }
        let symbol = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => symbol,
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if arguments.count() == 0 => base_symbol,
            _ => return None,
        };
        if seen.contains(symbol) {
            return None;
        }
        let data =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
        if data.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            || program.type_multiplicity(reference) != Multiplicity::Affine
            || !program.data_type_parameters(data).is_empty()
            || program.machines().iter().any(|machine| {
                machine.attached_data_symbol == *symbol && machine.name.as_str().ends_with("::drop")
            })
        {
            return None;
        }
        seen.push(*symbol);
        let mut contains_reference = false;
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            if field.relevance.is_erased() {
                return None;
            }
            contains_reference |= visit(program, field.type_reference, seen)?;
        }
        seen.pop();
        Some(contains_reference)
    }
    // A bare exclusive leaf — `&mut` over a primitive or named referent — is
    // its own custody family, not a record, so the classifier stays out.
    // A shared `&T` borrow has no such family: it has always counted as
    // reference-bearing here, and the discarded-result and admission gates
    // rely on that classification.
    parts(program, reference).is_none()
        && stored_mutable_parts(program, reference).is_none()
        && visit(program, reference, &mut Vec::new()) == Some(true)
}

/// Reconstruct a leaf's exact formal ingress. Structural parameter ordinals
/// exclude scalar and const parameters, just like ordinary call preparation.
pub fn initializer_source(
    program: &TypedTrees,
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
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if !path.symbol.is_valid()
        || path.symbol != path.head_symbol
        || program.symbols.get(path.symbol).parent != state.symbol
    {
        return None;
    }
    let found = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == path.symbol);
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
            type_identity: program.normalized_type_identity(referent).into_string(),
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

/// Returning an ingress loan can reduce access, never change its referent.
/// Lifetime validity is checked by the borrow checker; this reconstruction
/// retains the exact referent (including qualifications). An outer constraint
/// shell is not silently dropped to make an access conversion succeed.
fn reference_parameter_can_supply(
    program: &TypedTrees,
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

/// A borrowed initializer may name one projected leaf of a formal carrier —
/// `&receiver.field[i]` — where the whole-carrier arm above only spells a
/// bare parameter. The authored address walks member hops and literal index
/// hops off the receiver; the minted source is that parameter's position and
/// the projected segments. A final `Referent` step belongs only to a stored
/// reference leaf, never to an ordinary value being borrowed. Indexes bound
/// to locals stay opaque: folding them would need the
/// constant-binding evidence the producer resolves elsewhere.
pub fn borrowed_initializer_source(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) = stored_loan_parts(program, reference)?;
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
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
    let ExpressionNode::Name(path) = program.expression_table.expression(cursor) else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if !path.symbol.is_valid() || path.symbol != path.head_symbol {
        return None;
    }
    let found = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == path.symbol);
    // The borrowed carrier roots at a non-self structural formal — the
    // existing ingress family — or at an established local of this state,
    // which mints the local's own storage as the loaned home. A formal still
    // requires a projected leaf (`&mut formal.field`); a local admits the
    // whole place (`&mut owner`) because its storage is wholly owned by this
    // frame. A mutable or write-only loan needs a mutable referent
    // declaration, the local must not be erased, and a reference-typed
    // local is a silent marker with no storage to name — borrowing through
    // one is the reborrow lane, which this plan does not reconstruct.
    let (source, mut owner_type) = match found {
        Some((position, parameter)) => {
            if parameter.name != *name || projections.is_empty() {
                return None;
            }
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(position).ok()?,
                },
                parameter.type_reference,
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
                    field.path_identity(),
                ));
                owner_type = field.type_reference;
            }
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) =
                    program.expression_table.expression(indexed.index)
                else {
                    return None;
                };
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
        type_identity: program.normalized_type_identity(referent).into_string(),
        access,
    })
}

/// Peel constraint shells and reference wrappers to the nominal owner one
/// carrier's members live under.
fn nominal_member_owner_type<'p>(
    program: &'p TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<&'p typed_trees::data::DataDefinition> {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Named { symbol, .. } => {
                return crate::program_scope::data_definitions::data_definition_by_symbol(
                    program, *symbol,
                );
            }
            _ => return None,
        }
    }
}

/// Reconstruct the declaration-ordered reference roster from a complete
/// constructor. Reference leaves retain formal ingress instead of scalar data.
pub fn construction_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    construction_sources_with_calls(
        program,
        state,
        expression,
        reference,
        &mut vec![state.symbol],
    )
    .map(|sources| sources.into_iter().map(|source| source.value).collect())
}

/// Completion and call substitution share one returned-leaf origin query.
/// Earlier ordinary statements do not change which authored value is returned.
pub fn returned_record_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    if !is_reference_record(program, state.return_type) {
        return None;
    }
    let expression = tail_return_expression(program, state)?;
    construction_sources(program, state, expression, state.return_type)
}

/// The expression producing a state's returned value: the tail expression
/// statement, or the exit arm of a terminal return transition.
pub fn tail_return_expression(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let last = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?;
    match last {
        StatementNode::Expression(expression) => Some(*expression),
        StatementNode::Transition(transition)
            if matches!(
                transition.guard,
                typed_trees::statement::TransitionGuardNode::Always
            ) && transition.exit == typed_trees::statement::TransitionExit::Ordinary
                && !transition.continuation.is_valid() =>
        {
            match program.statement_table.transition_target(transition.target) {
                typed_trees::statement::TransitionTargetNode::Value(expression) => {
                    Some(*expression)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// A leaf retains both its ultimate backing and the holder supplying its loan.
/// Flattening these into backing alone loses ordinary local-to-record and
/// record-to-record transfers; a call is not the only operation that moves a loan.
struct ConstructionSource {
    value: checked_trees::CheckedReferenceResultSourcePlan,
    holder: SymbolHandle,
    holder_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
}

fn construction_sources_with_calls(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
) -> Option<Vec<ConstructionSource>> {
    if stored_loan_parts(program, reference).is_some()
        && let Some(source) = initializer_source(program, state, expression, reference)
    {
        let holder = match program.expression_table.expression(expression) {
            ExpressionNode::Name(name)
                if program
                    .state_local_declaration(state, name.symbol)
                    .is_some() =>
            {
                name.symbol
            }
            _ => SymbolHandle::default(),
        };
        return Some(vec![ConstructionSource {
            value: checked_trees::CheckedReferenceResultSourcePlan {
                path: Vec::new(),
                source,
            },
            holder,
            holder_path: Vec::new(),
        }]);
    }
    if crate::has_plain_owned_contents_with_numeric_constraints(program, reference) {
        return Some(Vec::new());
    }
    if matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
    ) {
        return projected_construction_sources(
            program,
            state,
            expression,
            reference,
            active_origins,
        );
    }
    if stored_loan_parts(program, reference).is_none() && !is_reference_record(program, reference) {
        return None;
    }
    if let ExpressionNode::Name(name) = program.expression_table.expression(expression) {
        let [spelling] = program.expression_table.name_path_members(name.members) else {
            return None;
        };
        if let Some((_, local)) = program.state_local_declaration(state, name.symbol) {
            if local.is_mutable
                || name.head_symbol != name.symbol
                || local.name != *spelling
                || program.symbols.get(name.symbol).parent != state.symbol
                || program.normalized_type_identity(local.type_reference)
                    != program.normalized_type_identity(reference)
                || active_origins.contains(&local.symbol)
            {
                return None;
            }
            active_origins.push(local.symbol);
            let sources = construction_sources_with_calls(
                program,
                state,
                local.initial_value,
                reference,
                active_origins,
            );
            active_origins.pop();
            return sources.map(|mut sources| {
                for source in &mut sources {
                    source.holder = local.symbol;
                    source.holder_path = source.value.path.clone();
                }
                sources
            });
        }
        let (ordinal, parameter) = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == name.symbol)?;
        if name.head_symbol != name.symbol
            || program.symbols.get(name.symbol).parent != state.symbol
            || parameter.name != *spelling
            || program.normalized_type_identity(parameter.type_reference)
                != program.normalized_type_identity(reference)
        {
            return None;
        }
        return formal_record_sources(program, reference, u32::try_from(ordinal).ok()?).map(
            |sources| {
                sources
                    .into_iter()
                    .map(|value| ConstructionSource {
                        value,
                        holder: SymbolHandle::default(),
                        holder_path: Vec::new(),
                    })
                    .collect()
            },
        );
    }
    if let ExpressionNode::Call(call) = program.expression_table.expression(expression) {
        call_sources_with_calls(
            program,
            state,
            call.target_symbol,
            program.expression_table.expression_handles(call.arguments),
            reference,
            active_origins,
        )
    } else {
        aggregate_construction_sources(program, state, expression, reference, active_origins)
    }
}

struct AggregateProjection {
    root: typed_trees::expression::ExpressionHandle,
    root_type: TypeReferenceHandle,
    path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    selected_type: TypeReferenceHandle,
}

/// Resolve aggregate selection once for origin transport and borrowed calls.
/// Each field identity and fixed index is checked against its declared carrier;
/// a stored reference remains an explicit crossing after this selection.
fn aggregate_projection(
    program: &TypedTrees,
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
            ExpressionNode::Member(member) if member.case_variant.is_none() => {
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
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.symbol == name.symbol)
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
                    None,
                )?;
                if field.relevance.is_erased() {
                    return None;
                }
                path.push(Segment::Field(field.path_identity()));
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
                let ExpressionNode::Integer(index) =
                    program.expression_table.expression(indexed.index)
                else {
                    return None;
                };
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
        root_type,
        path,
        selected_type: selected,
    })
}

/// Selecting an existing value preserves the backing and immediate holder of
/// every selected descriptor. The projection removes only its owner prefix;
/// the holder path stays relative to the storage actually supplying the loan.
fn projected_construction_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
) -> Option<Vec<ConstructionSource>> {
    let projection = aggregate_projection(program, state, expression)?;
    if program.normalized_type_identity(projection.selected_type)
        != program.normalized_type_identity(reference)
    {
        return None;
    }
    let sources = construction_sources_with_calls(
        program,
        state,
        projection.root,
        projection.root_type,
        active_origins,
    )?;
    Some(
        sources
            .into_iter()
            .filter_map(|mut source| {
                let suffix = source.value.path.strip_prefix(projection.path.as_slice())?;
                source.value.path = suffix.to_vec();
                Some(source)
            })
            .collect(),
    )
}

/// Substitute returned permission leaves through the exact resolved invocation.
/// Expression and terminal calls share this owner; neither creates a local loan.
fn call_sources_with_calls(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    target: SymbolHandle,
    arguments: &[typed_trees::expression::ExpressionHandle],
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
) -> Option<Vec<ConstructionSource>> {
    let (machine, destination) =
        crate::machine_calls::calls::machine_state_by_symbol(program, target).or_else(|| {
            let machine = program.machine_by_symbol(target)?;
            Some((machine, program.machine_states(machine).first()?))
        })?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        return None;
    }
    if active_origins.contains(&destination.symbol)
        || program.normalized_type_identity(destination.return_type)
            != program.normalized_type_identity(reference)
    {
        return None;
    }
    let parameters = program.state_parameters(destination);
    if parameters.len() != arguments.len() || parameters.iter().any(|parameter| parameter.is_const)
    {
        return None;
    }
    active_origins.push(destination.symbol);
    let sources = returned_sources_with_calls(program, destination, active_origins);
    active_origins.pop();
    let mut output = Vec::new();
    for source in sources? {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            source.value.source.source
        else {
            return None;
        };
        let (position, parameter) = parameters
            .iter()
            .enumerate()
            .filter(|(_, parameter)| {
                program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
            })
            .nth(parameter_index as usize)?;
        let actual_sources = construction_sources_with_calls(
            program,
            state,
            *arguments.get(position)?,
            parameter.type_reference,
            active_origins,
        )?;
        let borrowed_formal = stored_loan_parts(program, parameter.type_reference);
        let selected_path = if let Some((referent, access)) = borrowed_formal {
            if !reference_access_can_supply(access, source.value.source.access) {
                return None;
            }
            // The callee's source reconstruction already authenticates each
            // projected declaration and its final referent type. Substitution
            // requires the actual to supply that exact formal referent; its
            // existing backing path then precedes the callee's projection.
            let identity = program.normalized_type_identity(referent);
            if actual_sources.iter().any(|actual| {
                !actual.value.path.is_empty()
                    || actual.value.source.type_identity != identity.as_str()
                    || !reference_access_can_supply(actual.value.source.access, access)
            }) {
                return None;
            }
            &[][..]
        } else {
            let (last, path) = source.value.source.path.split_last()?;
            if *last != checked_trees::CheckedUnitStructuralPathSegment::Referent
                || !is_reference_record(program, parameter.type_reference)
            {
                return None;
            }
            // An owned carrier's stored reference supplies its captured
            // origin. Its field path selects a permission leaf; it is not
            // an address projection to append to that captured origin.
            path
        };
        let before = output.len();
        for mut actual in actual_sources
            .into_iter()
            .filter(|actual| actual.value.path == selected_path)
        {
            if !reference_access_can_supply(actual.value.source.access, source.value.source.access)
            {
                return None;
            }
            if borrowed_formal.is_some() {
                actual
                    .value
                    .source
                    .path
                    .extend_from_slice(&source.value.source.path);
                actual.value.source.type_identity = source.value.source.type_identity.clone();
            } else if actual.value.source.type_identity != source.value.source.type_identity {
                return None;
            }
            actual.value.path = source.value.path.clone();
            actual.value.source.access = source.value.source.access;
            output.push(actual);
            if output.len() > MAX_REFERENCE_ORIGIN_LEAVES {
                return None;
            }
        }
        if output.len() == before {
            return None;
        }
    }
    Some(output)
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

/// Convert the shared complete result relation to executable custody without
/// interpreting its display footprint. Internal-state substitution has already
/// preserved every source and actual prefix. This boundary independently checks
/// the exact declaration selectors, reference crossings and returned access;
/// it neither creates a holder nor grants permission to borrow the backing.
fn whole_reference_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<Vec<ConstructionSource>> {
    let (referent, access) = stored_loan_parts(program, state.return_type)?;
    let origins = crate::CallFrameResolver::new(program)?.reference_result_origins(state.symbol)?;
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
            .filter(|parameter| {
                !parameter.is_const
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == origin.root)?;
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
        for segment in &origin.segments {
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
                        field.path_identity(),
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
            type_identity: program.normalized_type_identity(referent).into_string(),
            access,
        };
        sources.push(ConstructionSource {
            value: checked_trees::CheckedReferenceResultSourcePlan {
                path: Vec::new(),
                source,
            },
            holder: SymbolHandle::invalid(),
            holder_path: Vec::new(),
        });
    }
    Some(sources)
}

fn returned_sources_with_calls(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    active_origins: &mut Vec<SymbolHandle>,
) -> Option<Vec<ConstructionSource>> {
    if stored_loan_parts(program, state.return_type).is_some() {
        return whole_reference_sources(program, state);
    }
    let mut sources = Vec::new();
    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::Transition(transition) = statement else {
            continue;
        };
        // A crash has no normal result and therefore contributes no origin.
        // Ordinary continuations still require their own completion analysis.
        if matches!(transition.exit, TransitionExit::Crash(_)) {
            continue;
        }
        if transition.continuation.is_valid() {
            return None;
        }
        let returned = match program.statement_table.transition_target(transition.target) {
            TransitionTargetNode::Value(expression) => construction_sources_with_calls(
                program,
                state,
                *expression,
                state.return_type,
                active_origins,
            )?,
            TransitionTargetNode::Named {
                path, arguments, ..
            } => call_sources_with_calls(
                program,
                state,
                path.symbol,
                program.statement_table.expression_handles(*arguments),
                state.return_type,
                active_origins,
            )?,
            TransitionTargetNode::Terminal => return None,
        };
        for source in returned {
            if !sources
                .iter()
                .any(|existing: &ConstructionSource| existing.value == source.value)
            {
                sources.push(source);
            }
        }
    }
    if let Some(StatementNode::Expression(expression)) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
    {
        for source in construction_sources_with_calls(
            program,
            state,
            *expression,
            state.return_type,
            active_origins,
        )? {
            if !sources
                .iter()
                .any(|existing| existing.value == source.value)
            {
                sources.push(source);
            }
        }
    }
    (!sources.is_empty()).then_some(sources)
}

/// Every normal completion contributes its exact caller-relative origin.
pub fn returned_reference_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    returned_sources_with_calls(program, state, &mut vec![state.symbol])
        .map(|sources| sources.into_iter().map(|source| source.value).collect())
}

/// A terminal invocation forwards its reference directly to the state's return.
/// Its exact flow occurrence has no expression/local holder; the ingress roster
/// is therefore the custody, rather than a fabricated local loan handle.
pub fn terminal_call_reference_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    if call.authored_expression.is_valid()
        || call.statement_index != result.statement_index as usize
        || stored_loan_parts(program, state.return_type).is_none()
        || result.type_identity != program.normalized_type_identity(state.return_type).as_str()
    {
        return None;
    }
    let StatementNode::Transition(transition) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(call.statement_index)?
    else {
        return None;
    };
    if transition.exit != TransitionExit::Ordinary || transition.continuation.is_valid() {
        return None;
    }
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
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
    call_sources_with_calls(
        program,
        state,
        target.symbol,
        program.statement_table.expression_handles(*arguments),
        state.return_type,
        &mut vec![state.symbol],
    )
    .map(|sources| sources.into_iter().map(|source| source.value).collect())
}

/// An owned formal transports its existing reference leaves. The source path
/// names each borrowed referent; it does not authorize creating a fresh loan
/// or moving storage through the referent boundary.
fn formal_record_sources(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
    parameter_index: u32,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    fn visit(
        program: &TypedTrees,
        reference: TypeReferenceHandle,
        parameter_index: u32,
        path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        output: &mut Vec<checked_trees::CheckedReferenceResultSourcePlan>,
    ) -> Option<()> {
        if let Some((referent, access)) = stored_loan_parts(program, reference) {
            let mut source_path = path.clone();
            source_path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
            output.push(checked_trees::CheckedReferenceResultSourcePlan {
                path: path.clone(),
                source: checked_trees::CheckedUnitStructuralArgumentPlan {
                    source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index,
                    },
                    path: source_path,
                    type_identity: program.normalized_type_identity(referent).into_string(),
                    access,
                },
            });
            return Some(());
        }
        if crate::has_plain_owned_contents_with_numeric_constraints(program, reference) {
            return Some(());
        }
        let symbol = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => symbol,
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if arguments.count() == 0 => base_symbol,
            _ => return None,
        };
        let data =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
        for member in program.data_members(data) {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                field.path_identity(),
            ));
            visit(program, field.type_reference, parameter_index, path, output)?;
            path.pop();
        }
        Some(())
    }
    if !is_reference_record(program, reference) {
        return None;
    }
    let mut output = Vec::new();
    visit(
        program,
        reference,
        parameter_index,
        &mut Vec::new(),
        &mut output,
    )?;
    Some(output)
}

fn aggregate_construction_sources(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
) -> Option<Vec<ConstructionSource>> {
    // Enumerate semantic children once, then compose every child's captured
    // loans through its exact stored path. A field and an array slot differ
    // in their projection, not in the ownership of the reference they hold.
    let mut children = Vec::new();
    if let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(reference)
    {
        let ExpressionNode::ArrayLiteral(literal) = program.expression_table.expression(expression)
        else {
            return None;
        };
        let elements = program.expression_table.expression_handles(*literal);
        if elements.len() != *length {
            return None;
        }
        for (index, expression) in elements.iter().enumerate() {
            children.push((
                checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(index).ok()?,
                ),
                *expression,
                *element_type,
            ));
        }
    } else {
        let symbol = match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => symbol,
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if arguments.count() == 0 => base_symbol,
            _ => return None,
        };
        let ExpressionNode::StructLiteral(literal) =
            program.expression_table.expression(expression)
        else {
            return None;
        };
        if literal.type_symbol != *symbol || literal.case_symbol.is_some() {
            return None;
        }
        let data =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
        let initializers = program.expression_table.struct_fields(literal.fields);
        let fields = program.data_members(data);
        if initializers.len() != fields.len() {
            return None;
        }
        for member in fields {
            let typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            let mut matching = initializers
                .iter()
                .filter(|initializer| initializer.field_symbol == field.symbol);
            let initializer = matching.next()?;
            if matching.next().is_some() {
                return None;
            }
            children.push((
                checked_trees::CheckedUnitStructuralPathSegment::Field(field.path_identity()),
                initializer.value,
                field.type_reference,
            ));
        }
    }
    let mut output = Vec::new();
    for (segment, expression, reference) in children {
        for mut source in
            construction_sources_with_calls(program, state, expression, reference, active_origins)?
        {
            source.value.path.insert(0, segment.clone());
            output.push(source);
        }
    }
    Some(output)
}

/// Whether a local of type `reference` must join its stored
/// carrier leaves to captured loans (`local_record_loans`). Record loans
/// belong to a locally constructed record holding reference carriers. A bare
/// `&'a V` shared view is no construction: a selected borrow binds it through
/// the shared-borrow join. `is_reference_record` counts that view for result
/// custody, so this local-construction gate excludes it explicitly. The
/// checked producer and the lowering replay both ask this one question.
pub fn local_owes_record_loans(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    is_reference_record(program, reference) && shared_borrowed_parts(program, reference).is_none()
}

/// Join each stored carrier leaf to its exact captured loan and weakening.
/// Similar lifetime spelling and UnretainedDerived alone are not authority.
pub fn local_record_loans(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
) -> Option<
    Vec<(
        checked_trees::CheckedReferenceResultSourcePlan,
        arena::Handle<checked_trees::BorrowLoanFact>,
    )>,
> {
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index as usize)?
    else {
        return None;
    };
    if !is_reference_record(program, local.type_reference) {
        return None;
    }
    let sources = construction_sources_with_calls(
        program,
        state,
        local.initial_value,
        local.type_reference,
        &mut vec![state.symbol],
    )?;
    let mut borrowing_states =
        facts
            .borrow
            .states
            .iter()
            .map(|(_, state)| state)
            .filter(|candidate| {
                candidate.machine_symbol == machine && candidate.state_symbol == state.symbol
            });
    let borrowing = borrowing_states.next()?;
    if borrowing_states.next().is_some() {
        return None;
    }
    let loans = facts
        .borrow
        .loans
        .iter()
        .filter(|(handle, loan)| {
            // This query reconstructs one establishment occurrence. A mutable
            // binding may acquire different loans later; neither those later
            // occurrences nor binding mutability change this capture's identity.
            facts.borrow.state_owns_loan(borrowing, *handle)
                && loan.owner_symbol == local.symbol
                && loan.statement_index == statement_index as usize
        })
        .collect::<Vec<_>>();
    if sources.len() != loans.len() {
        return None;
    }
    let flow = state_flow(facts, machine, state.symbol)?;
    let mut output = Vec::new();
    for origin in sources {
        let source = &origin.value;
        // Both sides resolve through the declared aggregate. Owner paths use
        // symbols/ordinals while result paths use declaration identities; the
        // same typed projection walk authenticates fields and fixed slots.
        let owner_path = loan_place_segments(program, local.type_reference, &source.path)?
            .into_iter()
            .map(|segment| match segment {
                facts::PlaceSegment::Field { symbol } => {
                    Some(checked_trees::BorrowLoanOwnerSegment::Field(symbol))
                }
                facts::PlaceSegment::FixedIndex { index } => {
                    Some(checked_trees::BorrowLoanOwnerSegment::FixedIndex(index))
                }
                _ => None,
            })
            .collect::<Option<Vec<_>>>()?;
        let mut matching = loans
            .iter()
            .filter(|(_, loan)| facts.borrow.loan_owner_path(loan) == owner_path);
        let (handle, loan) = *matching.next()?;
        // A stored carrier's loan kind is the access its field declared: a
        // shared `&T` captures a read loan, exclusive carriers capture their
        // mutable or write-only loan -- the same kind derivation
        // `result_loan` applies to shared views.
        let expected_kind = match source.source.access {
            CheckedStructuralAccess::SharedBorrow => checked_trees::BorrowAccessKind::Read,
            CheckedStructuralAccess::MutableBorrow => checked_trees::BorrowAccessKind::Mutable,
            CheckedStructuralAccess::WriteOnlyBorrow => checked_trees::BorrowAccessKind::WriteOnly,
            CheckedStructuralAccess::Owned => return None,
        };
        if matching.next().is_some()
            || loan.statement_index != statement_index as usize
            || loan.kind != expected_kind
        {
            return None;
        }
        if let Some((owner, prior)) =
            construction_source_loan(program, facts, machine, state, statement_index, &origin)?
        {
            let previous = facts.borrow.loans.get(prior);
            if loan.source_owner_symbol != owner
                || loan.root_symbol != previous.root_symbol
                || loan.kind != previous.kind
                || facts.borrow.loan_segments(loan) != facts.borrow.loan_segments(previous)
            {
                return None;
            }
        } else {
            // Direct captures establish backing geometry. A transferred carrier
            // already has that geometry in its exact supplying loan above;
            // its capture may cross a descriptor rather than spell that backing.
            let (root_symbol, root_type) = match source.source.source {
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                } => {
                    let parameter = program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| {
                            !parameter.is_const
                                && program
                                    .primitive_type_reference(parameter.type_reference)
                                    .is_none()
                        })
                        .nth(parameter_index as usize)?;
                    (parameter.symbol, parameter.type_reference)
                }
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                }
                | checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal {
                    symbol,
                } => {
                    let (ordinal, referent) = program.state_local_declaration(state, symbol)?;
                    if ordinal >= statement_index as usize {
                        return None;
                    }
                    (symbol, referent.type_reference)
                }
                _ => return None,
            };
            let expected_segments = loan_place_segments(program, root_type, &source.source.path)?;
            if loan.source_owner_symbol.is_valid()
                || loan.root_symbol != root_symbol
                || facts.borrow.loan_segments(loan) != expected_segments.as_slice()
            {
                return None;
            }
        }
        let mut activations = facts
            .flow
            .borrow_lifetimes
            .activations
            .span_or_empty(flow.borrow_activations)
            .iter()
            .filter(|activation| activation.loan == handle);
        if activations.next()?.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: statement_index as usize,
            })
            || activations.next().is_some()
        {
            return None;
        }
        release_boundary(program, facts, machine, state.symbol, handle)?;
        output.push((origin.value, handle));
    }
    Some(output)
}

/// The loan segments a borrowed initializer source projects under its
/// parameter's declared type, spelled in the ownership vocabulary the
/// ledger records. Field hops resolve through the nominal owner exactly as
/// the owner-path walk does; literal fixed-array hops keep their ordinal.
/// Runtime-indexed, case and referent hops are not direct backing geometry.
/// Transfers through a stored descriptor use its captured loan instead.
pub fn loan_place_segments(
    program: &TypedTrees,
    root: TypeReferenceHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<facts::PlaceSegment>> {
    let mut selected = root;
    let mut segments = Vec::with_capacity(path.len());
    for segment in path {
        match segment {
            checked_trees::CheckedUnitStructuralPathSegment::Field(identity) => {
                let data = nominal_member_owner_type(program, selected)?;
                let field = program
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        typed_trees::data::DataMember::Field(field)
                            if field.path_identity() == *identity =>
                        {
                            Some(field)
                        }
                        _ => None,
                    })?;
                segments.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                selected = field.type_reference;
            }
            checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(index) => {
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

/// Reconstruct the supplying holder independently of the surrounding operation.
/// Its exact captured leaf must still be live when construction transfers it.
/// This establishes correspondence, not a new reborrow: the owned carrier and
/// its existing permission move together.
fn construction_source_loan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    source: &ConstructionSource,
) -> Option<Option<(SymbolHandle, arena::Handle<checked_trees::BorrowLoanFact>)>> {
    if !source.holder.is_valid() {
        return Some(None);
    }
    let (prior_index, prior_local) = program
        .state_local_declaration(state, source.holder)
        .filter(|&(ordinal, _)| ordinal < statement_index as usize)?;
    let prior = if stored_loan_parts(program, prior_local.type_reference).is_some() {
        // A bare reference has one empty owner path. Join its exact formation
        // occurrence; its captured backing may cross another stored descriptor.
        if !source.holder_path.is_empty() {
            return None;
        }
        let mut candidates = facts.borrow.loans.iter().filter(|(handle, loan)| {
            loan.owner_symbol == prior_local.symbol
                && loan.statement_index == prior_index
                && facts.borrow.loan_owner_path(loan).is_empty()
                && facts.borrow.states.iter().any(|(_, borrowing)| {
                    borrowing.machine_symbol == machine
                        && borrowing.state_symbol == state.symbol
                        && facts.borrow.state_owns_loan(borrowing, *handle)
                })
        });
        let (handle, _) = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        handle
    } else {
        let prior = local_record_loans(
            program,
            facts,
            machine,
            state,
            u32::try_from(prior_index).ok()?,
        )?;
        let mut matching = prior
            .iter()
            .filter(|(origin, _)| origin.path == source.holder_path);
        let (_, handle) = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
        *handle
    };
    if !record_loan_is_active(
        program,
        facts,
        machine,
        state.symbol,
        statement_index,
        prior,
    ) {
        return None;
    }
    Some(Some((prior_local.symbol, prior)))
}

/// Join one leaf of a call's returned-leaf roster through the call's exact
/// actual to a caller-side local's captured loan. This is the operand form of
/// construction's holder join: the producing call is named by its authored
/// expression rather than a `let` statement's initializer, and the consuming
/// statement's activity or last-use discipline is checked by the caller.
fn call_actual_leaf_loan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    call: &typed_trees::expression::TableCallExpression,
    destination: &typed_trees::state::State,
    source: &checked_trees::CheckedReferenceResultSourcePlan,
) -> Option<arena::Handle<checked_trees::BorrowLoanFact>> {
    let [
        path @ ..,
        checked_trees::CheckedUnitStructuralPathSegment::Referent,
    ] = source.source.path.as_slice()
    else {
        return None;
    };
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
        source.source.source
    else {
        return None;
    };
    let (position, parameter) = program
        .state_parameters(destination)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .nth(parameter_index as usize)?;
    if !is_reference_record(program, parameter.type_reference) {
        return None;
    }
    let actual = *program
        .expression_table
        .expression_handles(call.arguments)
        .get(position)?;
    let ExpressionNode::Name(name) = program.expression_table.expression(actual) else {
        return None;
    };
    if name.head_symbol != name.symbol
        || program
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return None;
    }
    let (prior_index, prior_local) = program
        .state_local_declaration(state, name.symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index as usize)?;
    if program.normalized_type_identity(prior_local.type_reference)
        != program.normalized_type_identity(parameter.type_reference)
    {
        return None;
    }
    let prior = local_record_loans(
        program,
        facts,
        machine,
        state,
        u32::try_from(prior_index).ok()?,
    )?;
    let mut matching = prior.iter().filter(|(source, _)| source.path == path);
    let (_, handle) = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(*handle)
}

fn record_loan_is_active(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    loan: arena::Handle<checked_trees::BorrowLoanFact>,
) -> bool {
    let Some(flow) = state_flow(facts, machine, state) else {
        return false;
    };
    let Some(entries) = facts.flow.control.statements.span(flow.statements) else {
        return false;
    };
    let mut entries = entries
        .iter()
        .filter(|entry| entry.statement_index == statement_index as usize);
    let Some(entry) = entries.next() else {
        return false;
    };
    entries.next().is_none()
        && facts.flow.contexts.constraint_refs.span(entry.entry_constraints).is_some_and(|constraints|
            constraints.iter().filter(|constraint| matches!(constraint.kind, checked_trees::FlowConstraintKind::BorrowLoan { loan: active } if active == loan)).count() == 1)
        && facts.borrow.loans.get(loan).statement_index < statement_index as usize
        && release_boundary(program, facts, machine, state, loan).is_some_and(|end| statement_index < end)
}

/// Whole owned operands retain every captured leaf until their consuming call.
pub fn owned_record_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    source_statement: u32,
) -> bool {
    source_statement < statement_index
        && local_record_loans(program, facts, machine, state, source_statement).is_some_and(
            |loans| {
                !loans.is_empty()
                    && loans.iter().all(|(_, loan)| {
                        record_loan_is_active(
                            program,
                            facts,
                            machine,
                            state.symbol,
                            statement_index,
                            *loan,
                        )
                    })
            },
        )
}

/// Resolve an authored member projection against a record type, yielding the
/// field-identity path and the projected type. The spelling matches the leaf
/// paths `formal_record_sources` and `local_record_loans` reconstruct, so one
/// resolver keeps those joins from drifting.
pub(crate) fn declared_field_path<'a>(
    program: &TypedTrees,
    mut selected: TypeReferenceHandle,
    members: impl Iterator<Item = &'a typed_trees::expression::TableMemberExpression>,
) -> Option<(
    Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    TypeReferenceHandle,
)> {
    let mut path = Vec::new();
    for member in members {
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(selected)
        else {
            return None;
        };
        let data =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
        let field = crate::value_custody::places::exact_data_member_field(
            program,
            data,
            member.member_symbol,
            member.member.as_str(),
            member.case_variant.as_ref().map(|case| case.as_str()),
        )?;
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            field.path_identity(),
        ));
        selected = field.type_reference;
    }
    Some((path, selected))
}

/// A projected owned-record argument moves the projected subtree's captured
/// leaves with the call. Every leaf under the projected edge must stay live
/// until this consuming statement, and the edge must carry the record's whole
/// leaf roster so no sibling custody strands under a consumed residual shell.
/// `path` is the projected field-identity path exactly as
/// `projected_argument_path` spells it.
pub fn projected_record_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    source_statement: u32,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> bool {
    !path.is_empty()
        && source_statement < statement_index
        && local_record_loans(program, facts, machine, state, source_statement).is_some_and(
            |loans| {
                !loans.is_empty()
                    && loans.iter().all(|(source, loan)| {
                        source.path.starts_with(path)
                            && record_loan_is_active(
                                program,
                                facts,
                                machine,
                                state.symbol,
                                statement_index,
                                *loan,
                            )
                    })
            },
        )
}

/// A projected operand may root at a nested call's anonymous result instead
/// of a named local: `select(forward_outer(outer).inner)`. Every leaf the
/// nested result publishes under the projected edge must replay one
/// caller-side local's captured leaf loan through the call's exact actual,
/// and each such loan must stay live until the consuming call — the same
/// custody `projected_record_argument` proves for a stored carrier. The
/// anonymous carrier itself is consumed whole by the projection, so its
/// entire returned leaf roster must live under the edge.
pub fn nested_call_record_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some(destination) = program.state_by_symbol(call.target_symbol) else {
        return false;
    };
    if !is_reference_record(program, destination.return_type) {
        return false;
    }
    let Some(sources) = returned_record_sources(program, destination) else {
        return false;
    };
    !path.is_empty()
        && !sources.is_empty()
        && sources.iter().all(|source| {
            source.path.starts_with(path)
                && call_actual_leaf_loan(
                    program,
                    facts,
                    machine,
                    state,
                    statement_index,
                    call,
                    destination,
                    source,
                )
                .is_some_and(|loan| {
                    record_loan_is_active(
                        program,
                        facts,
                        machine,
                        state.symbol,
                        statement_index,
                        loan,
                    )
                })
        })
}

/// A nested call's anonymous reference-record result has no `let` destination:
/// `select(forward_outer(outer).inner)` captures `forward_outer`'s returned
/// leaves directly into the consuming call. Each returned leaf must replay
/// through the call's exact actual to one caller-side local's captured loan
/// that stays live at the containing statement — the same leaf join
/// `nested_call_record_argument` proves under the consumer's projected edge,
/// here required of the complete returned roster.
pub fn nested_call_result_loans(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some(destination) = program.state_by_symbol(call.target_symbol) else {
        return false;
    };
    if !is_reference_record(program, destination.return_type) {
        return false;
    }
    let Some(sources) = returned_record_sources(program, destination) else {
        return false;
    };
    !sources.is_empty()
        && sources.iter().all(|source| {
            call_actual_leaf_loan(
                program,
                facts,
                machine,
                state,
                statement_index,
                call,
                destination,
                source,
            )
            .is_some_and(|loan| {
                record_loan_is_active(program, facts, machine, state.symbol, statement_index, loan)
            })
        })
}

/// A projected argument crosses the stored carrier only after its exact local
/// construction and active leaf loan have been reconstructed.
pub fn record_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    result: &CheckedUnitStructuralResultBindingPlan,
    destination: TypeReferenceHandle,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let (referent, access) = stored_loan_parts(program, destination)?;
    let StatementNode::LocalData(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?
    else {
        return None;
    };
    if result.type_identity
        != program
            .normalized_type_identity(local.type_reference)
            .as_str()
        // Source reference uses are reusable, but each materialized descriptor
        // owns affine lifetime custody, including descriptors in fixed arrays.
        || result.multiplicity != result_multiplicity(program, local.type_reference)
    {
        return None;
    }
    let projection = aggregate_projection(program, state, expression)?;
    let ExpressionNode::Name(name) = program.expression_table.expression(projection.root) else {
        return None;
    };
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
    let loans = local_record_loans(program, facts, machine, state, result.statement_index)?;
    let (source, loan) = loans.iter().find(|(source, _)| source.path == path)?;
    let flow = state_flow(facts, machine, state.symbol)?;
    let mut entries = facts
        .flow
        .control
        .statements
        .span(flow.statements)?
        .iter()
        .filter(|entry| entry.statement_index == statement_index as usize);
    let entry = entries.next()?;
    if entries.next().is_some()
        || facts.flow.contexts.constraint_refs.span(entry.entry_constraints)?.iter()
            .filter(|constraint| matches!(constraint.kind, checked_trees::FlowConstraintKind::BorrowLoan { loan: active } if active == *loan))
            .count() != 1
    {
        return None;
    }
    if statement_index <= result.statement_index
        || statement_index >= release_boundary(program, facts, machine, state.symbol, *loan)?
        || source.source.type_identity != program.normalized_type_identity(referent).as_str()
    {
        return None;
    }
    path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: source.source.type_identity.clone(),
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
pub fn classify_program_reference_custody(program: &TypedTrees) -> ReferenceCustodyFacts {
    let table = &program.type_reference_table;
    let highest = table
        .type_reference_rows()
        .map(|(handle, _)| handle.arena_index())
        .max()
        .unwrap_or(0);
    let mut rows =
        vec![reference_custody(program, TypeReferenceHandle::invalid()); highest as usize + 1];
    for (handle, _) in table.type_reference_rows() {
        rows[handle.arena_index() as usize] = reference_custody(program, handle);
    }
    ReferenceCustodyFacts::with_rows(rows)
}

/// Classify every declared machine state's reference-result custody over the
/// frozen typed program.
///
/// The tail-expression, returned-origin and ingress-shape questions below
/// are pure functions of the typed program: they read only the state's own
/// statements, parameters and return type, no checked fact. Walking the
/// declared states once records them for the whole program, so the
/// classifier's admitted completion shapes stay behind this one entry point
/// instead of being reachable from a later stage.
///
/// The walk is `machines()` by `machine_states()` — the declaration
/// structure every reader resolves a state symbol through, rather than the
/// states checking's own traversal happened to visit — so which completions
/// checking validated cannot leave a hole. The row vector is positional by
/// the state symbol's arena index, and index 0 keeps the invalid symbol's
/// absence row: a state carrying an invalid symbol names no declaration to
/// look up, so it must not take the row every unnamed symbol reads.
pub fn classify_program_state_reference_custody(
    program: &TypedTrees,
) -> StateReferenceCustodyFacts {
    let declared_states = || {
        program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine).iter())
            .filter(|state| state.symbol.is_valid())
    };
    let highest = declared_states()
        .map(|state| state.symbol.arena_index())
        .max()
        .unwrap_or(0);
    let mut rows = vec![StateReferenceCustodyRow::default(); highest as usize + 1];
    for state in declared_states() {
        rows[state.symbol.arena_index() as usize] = state_reference_custody(program, state);
    }
    StateReferenceCustodyFacts::with_rows(rows)
}

/// One state's reference-result custody row.
fn state_reference_custody(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> StateReferenceCustodyRow {
    StateReferenceCustodyRow {
        tail_return_expression: tail_return_expression(program, state).unwrap_or_default(),
        returns_complete_reference_origins: returned_reference_sources(program, state).is_some(),
        tail_names_source_parameter: source_parameter(program, state).is_some(),
        tail_selects_source_leaf: source_leaf(program, state).is_some(),
    }
}

/// One type reference's custody row.
fn reference_custody(program: &TypedTrees, reference: TypeReferenceHandle) -> ReferenceCustodyRow {
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
/// The first executable carrier retains a mutable, unqualified primitive loan.
pub fn parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if *access != language_semantics::ReferenceAccess::Mutable
        || !matches!(
            program.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Named { .. }
        )
        || program.primitive_type_reference(*referee).is_none()
    {
        return None;
    }
    Some((*referee, CheckedStructuralAccess::MutableBorrow))
}

/// The referee beneath an exclusive `&mut T` stored leaf whose referent is a
/// `Named` declaration — primitive or record. `parts` keeps its
/// primitive-only reference-result shape for the whole-return family; a
/// record member or stored leaf admits a record referent under the same
/// exclusive loan because a `&mut Record` names caller-frame storage just
/// as a `&mut u8` does. Slice views and unclassified referees stay out:
/// `&mut [T]` is a view descriptor, not a stored loan leaf.
fn stored_mutable_parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    mutable_borrowed_parts(program, reference).filter(|(referee, _)| {
        matches!(
            program.type_reference_table.type_reference(*referee),
            TypeReferenceNode::Named { .. }
        )
    })
}

/// One stored carrier leaf's (referent, access): an exclusive `&mut` loan
/// over a named referent or a shared `&` borrow — the two leaf families a
/// reference record carries. Every consumer of the leaf family — the
/// reference-record classifier, ingress reconstruction, construction-source
/// rosters and formal forwarding — asks this one question so the admitted
/// shape never drifts between them.
pub fn stored_loan_parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    parts(program, reference)
        .or_else(|| shared_borrowed_parts(program, reference))
        .or_else(|| stored_mutable_parts(program, reference))
}

/// The referee beneath any `&mut T` carrier, with constraint shells peeled.
/// Unlike `parts` this does not require a primitive referent: a
/// `&mut Record` names storage in the caller's frame just as a `&mut u8`
/// does. Callers that need the primitive-only reference-result shape keep
/// using `parts`.
pub fn mutable_borrowed_parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| shell.access == language_semantics::ReferenceAccess::Mutable)
        .map(|shell| (shell.referee, CheckedStructuralAccess::MutableBorrow))
}

/// The referee beneath a `&write T` carrier, with constraint shells peeled —
/// the write-only analog of `mutable_borrowed_parts`. A write-only loan names
/// the same caller-frame storage an exclusive borrow does; only the access
/// authority differs (writes admitted, reads fenced).
pub fn write_only_borrowed_parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| shell.access == language_semantics::ReferenceAccess::WriteOnly)
        .map(|shell| (shell.referee, CheckedStructuralAccess::WriteOnlyBorrow))
}

/// The named referee beneath a `&'a V` shared-borrowed view, with constraint
/// shells peeled. The referent's storage lives in the caller's frame like
/// `parts`' loan — it just cannot be written through. The referee must be
/// spelled as a `Named` type: this is the shared reference-result shape.
pub fn shared_borrowed_parts(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| {
            shell.access == language_semantics::ReferenceAccess::Shared
                && matches!(
                    program.type_reference_table.type_reference(shell.referee),
                    TypeReferenceNode::Named { .. }
                )
        })
        .map(|shell| (shell.referee, CheckedStructuralAccess::SharedBorrow))
}

/// The referee beneath any `& T` place-loan carrier, with constraint shells
/// peeled but without `shared_borrowed_parts`' named-referent restriction. A
/// `&[u8; 48]` loan names the array's storage in the caller's frame just as
/// a `&u32` does — the same coverage `mutable_borrowed_parts` gives the
/// exclusive side. Callers that need the shared reference-result shape keep
/// using `shared_borrowed_parts`.
pub fn shared_place_borrowed_parts(
    program: &TypedTrees,
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
pub fn binding_multiplicity(program: &TypedTrees, reference: TypeReferenceHandle) -> Multiplicity {
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
pub fn result_multiplicity(program: &TypedTrees, reference: TypeReferenceHandle) -> Multiplicity {
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
    if parts(program, reference).is_some()
        || shared_borrowed_parts(program, reference).is_some()
        || mutable_borrowed_parts(program, reference).is_some()
    {
        Multiplicity::Affine
    } else {
        program.type_multiplicity(reference)
    }
}

pub fn source_parameter(program: &TypedTrees, state: &typed_trees::state::State) -> Option<usize> {
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?
    else {
        return None;
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(*expression) else {
        return None;
    };
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    // The same ingress reconstruction serves bare returns and stored reference
    // initializers. Parameter identity is explicit; attached-machine identity
    // cannot stand in for an authored argument.
    initializer_source(program, state, *expression, state.return_type)?;
    program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == path.symbol && !parameter.is_const)
}

/// A bare reference result may also name one projected leaf of an owned
/// record formal: `value.body` selects the carrier leaf without minting a
/// fresh source. The returned position indexes the callee's authored
/// parameters (call arguments share that order); the argument plan is the
/// leaf's borrowed-referent ingress exactly as `formal_record_sources`
/// encodes it. Only declared `Member` projections through reference records
/// qualify — indexes, case payloads, and non-record receivers stay opaque.
pub fn source_leaf(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<(usize, checked_trees::CheckedUnitStructuralArgumentPlan)> {
    parts(program, state.return_type)?;
    let StatementNode::Expression(expression) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()?
    else {
        return None;
    };
    expression_source_leaf(program, state, *expression, state.return_type)
}

/// Reconstruct a stored permission selected from an exact formal declaration.
/// Completion admission and ordinary result-origin substitution share the same
/// projection and whole-carrier transfer checks.
fn expression_source_leaf(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
) -> Option<(usize, checked_trees::CheckedUnitStructuralArgumentPlan)> {
    stored_loan_parts(program, reference)?;
    let mut members = Vec::new();
    let mut current = expression;
    while let ExpressionNode::Member(member) = program.expression_table.expression(current) {
        if member.case_variant.is_some() {
            return None;
        }
        members.push(member);
        current = member.receiver;
    }
    if members.is_empty() {
        return None;
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(current) else {
        return None;
    };
    if path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let (position, parameter) = program
        .state_parameters(state)
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.symbol == path.symbol)?;
    if parameter.is_const
        || program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
    {
        return None;
    }
    // Every projection must stay inside a checked reference record and the
    // selected leaf must be the declared reference result itself.
    let mut selected = parameter.type_reference;
    let mut leaf_path = Vec::new();
    for member in members.iter().rev() {
        if !is_reference_record(program, selected) {
            return None;
        }
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(selected)
        else {
            return None;
        };
        let data =
            crate::program_scope::data_definitions::data_definition_by_symbol(program, *symbol)?;
        let member_symbol = member.member_symbol;
        let field = program
            .data_members(data)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == member_symbol => {
                    Some(field)
                }
                _ => None,
            })?;
        leaf_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            field.path_identity(),
        ));
        selected = field.type_reference;
    }
    if program.normalized_type_identity(selected) != program.normalized_type_identity(reference) {
        return None;
    }
    let (ordinal, _) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            !parameter.is_const
                && program
                    .primitive_type_reference(parameter.type_reference)
                    .is_none()
        })
        .enumerate()
        .find(|(_, candidate)| candidate.symbol == parameter.symbol)?;
    let sources = formal_record_sources(
        program,
        parameter.type_reference,
        u32::try_from(ordinal).ok()?,
    )?;
    // The establishment moves the carrier's leaf into the bare reference
    // result and consumes the carrier whole, so the selected leaf must be the
    // record's entire declared reference roster. A sibling leaf would strand
    // live custody under a consumed place.
    if sources.len() != 1 {
        return None;
    }
    sources
        .into_iter()
        .find(|source| source.path == leaf_path)
        .map(|source| (position, source.source))
}

/// Every normal completion must retain the same caller ingress at the declared
/// result access. The complete relation owns internal jumps and captured alias
/// substitution; distinct projected paths remain alternatives, not permissions.
fn reference_result_source_parameter(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<usize> {
    let (_, access) = stored_loan_parts(program, state.return_type)?;
    let sources = whole_reference_sources(program, state)?;
    let mut parameter = None;
    for source in sources {
        if !source.value.path.is_empty() || source.value.source.access != access {
            return None;
        }
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            source.value.source.source
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

/// The callee-side root a shared `&` view result denotes: any parameter a
/// shared read can reborrow from — `&`/`&mut` referents and owned or mutable
/// storage. A write-only carrier fences reads, so it is the one authority a
/// shared result cannot borrow through.
fn view_source_parameter_is_readable(
    program: &TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    write_only_borrowed_parts(program, parameter.type_reference).is_none()
}

/// The callee-side root a `&mut` view result denotes: a `&mut` parameter
/// symbol, the machine `*self` authority, or a `self` member — the member only
/// while `self` is mutably borrowed.
fn view_source_parameter_is_mutable(
    program: &TypedTrees,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    parameter.is_mutable || mutable_borrowed_parts(program, parameter.type_reference).is_some()
}

/// Reconstruct actual call substitution, not only the return lifetime annotation.
/// The callee's ordinary completion independently verifies this ingress source.
pub fn result_loan(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<arena::Handle<checked_trees::BorrowLoanFact>> {
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?;
    let local = match statement {
        StatementNode::LocalData(local) => local,
        StatementNode::Call(authored)
            if authored.discards_result
                && !call.authored_expression.is_valid()
                && call.statement_index == result.statement_index as usize
                && call.call_ordinal == 0
                && authored.target_symbol == call.target_symbol =>
        {
            // An explicitly discarded result has no binding lifetime. Its
            // materialized carrier still owes ordinary completion cleanup;
            // no fabricated named-local loan may stand in for that debt.
            // The call/result custody replay separately authenticates operands,
            // result type and every callee completion's exact returned origins.
            return Some(arena::Handle::invalid());
        }
        _ => return None,
    };
    // `mutable_borrowed_parts` peels every `&mut`, including the whole
    // primitive references `parts` already owns; those keep the ingress and
    // leaf rosters. Shared and mutable views use the same complete result
    // relation; their declared access still determines the independent caller
    // loan checked below. A materialized shared primitive carrier retains this
    // loan obligation even though source observations do not consume it.
    let mutable_view = parts(program, local.type_reference).is_none()
        && mutable_borrowed_parts(program, local.type_reference).is_some();
    let shared_view = parts(program, local.type_reference).is_none()
        && shared_borrowed_parts(program, local.type_reference).is_some();
    if !mutable_view && !shared_view {
        parts(program, local.type_reference)?;
    }
    if local.is_mutable {
        return None;
    }
    let ExpressionNode::Call(expression) = program.expression_table.expression(local.initial_value)
    else {
        return None;
    };
    if expression.target_symbol != call.target_symbol
        || call.authored_expression != local.initial_value
        || call.statement_index != result.statement_index as usize
        || call.call_ordinal != 0
    {
        return None;
    }
    let callee = program.state_by_symbol(call.target_symbol)?;
    // The returned carrier either forwards a whole `&mut` ingress argument or
    // selects one declared leaf out of an owned record argument. Both lanes
    // must end in the same single result loan below.
    // A shared or mutable view may select different projections across
    // internal states, but every normal result must retain one caller ingress.
    // Primitive reference results keep their exact whole/owned-leaf transfer.
    let leaf = if mutable_view || shared_view {
        None
    } else {
        source_parameter(program, callee)
            .is_none()
            .then(|| source_leaf(program, callee))
            .flatten()
    };
    let position = if mutable_view || shared_view {
        reference_result_source_parameter(program, callee)?
    } else {
        leaf.as_ref()
            .map(|(position, _)| *position)
            .or_else(|| source_parameter(program, callee))?
    };
    let argument = *program
        .expression_table
        .expression_handles(expression.arguments)
        .get(position)?;
    // A declared-field projection may select the argument carrier out of an
    // owned record local or out of a nested call's anonymous result:
    // `input.inner` and `forward_outer(outer).inner` both move the whole
    // `inner` subtree. Index, case-payload and computed roots stay opaque to
    // leaf custody.
    let mut members = Vec::new();
    let mut root = argument;
    if mutable_view || shared_view {
        // An explicit borrow passes the same captured place as an implicit
        // borrowed argument. Peel only the requested access, then rejoin the
        // authored root and the exact checked loan below; the wrapper itself
        // neither creates result custody nor changes its release boundary.
        if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(root) {
            let parameter = program.state_parameters(callee).get(position)?;
            let expected_access = if view_source_parameter_is_mutable(program, parameter) {
                language_semantics::ReferenceAccess::Mutable
            } else if view_source_parameter_is_readable(program, parameter) {
                language_semantics::ReferenceAccess::Shared
            } else {
                return None;
            };
            if borrow.access != expected_access {
                return None;
            }
            root = borrow.target;
        }
    }
    while let ExpressionNode::Member(member) = program.expression_table.expression(root) {
        members.push(member);
        root = member.receiver;
    }
    let expected_places = if let Some((_, leaf_source)) = leaf {
        // The argument is an owned record local, possibly through one
        // declared-field projection; the returned leaf's loan is the exact
        // captured leaf loan under that projection, and this call is that
        // loan's last use.
        let [
            path @ ..,
            checked_trees::CheckedUnitStructuralPathSegment::Referent,
        ] = leaf_source.path.as_slice()
        else {
            return None;
        };
        match program.expression_table.expression(root) {
            ExpressionNode::Name(actual) => {
                if actual.head_symbol != actual.symbol
                    || program
                        .expression_table
                        .name_path_members(actual.members)
                        .len()
                        != 1
                {
                    return None;
                }
                let (index, input) = program.state_local_declaration(state, actual.symbol)?;
                if input.is_mutable
                    || index >= call.statement_index
                    || !is_reference_record(program, input.type_reference)
                {
                    return None;
                }
                let (mut selected_path, projected) = declared_field_path(
                    program,
                    input.type_reference,
                    members.iter().rev().copied(),
                )?;
                if program
                    .state_parameters(callee)
                    .get(position)
                    .is_none_or(|parameter| {
                        program.normalized_type_identity(parameter.type_reference)
                            != program.normalized_type_identity(projected)
                    })
                {
                    return None;
                }
                selected_path.extend(path.iter().cloned());
                let loans =
                    local_record_loans(program, facts, machine, state, u32::try_from(index).ok()?)?;
                let mut matching = loans
                    .iter()
                    .filter(|(source, _)| source.path == selected_path);
                let (_, leaf_loan) = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                let leaf_loan = facts.borrow.loans.get(*leaf_loan);
                if leaf_loan.last_use_statement_index != call.statement_index {
                    return None;
                }
                vec![(
                    leaf_loan.root_symbol,
                    facts.borrow.loan_segments(leaf_loan).to_vec(),
                )]
            }
            ExpressionNode::Call(nested) => {
                // The carrier is the nested call's anonymous result: its
                // returned-leaf roster names which caller-side captured loan
                // the projected leaf replays through the nested actual.
                let nested_destination = program.state_by_symbol(nested.target_symbol)?;
                if !is_reference_record(program, nested_destination.return_type) {
                    return None;
                }
                let (mut selected_path, projected) = declared_field_path(
                    program,
                    nested_destination.return_type,
                    members.iter().rev().copied(),
                )?;
                if program
                    .state_parameters(callee)
                    .get(position)
                    .is_none_or(|parameter| {
                        program.normalized_type_identity(parameter.type_reference)
                            != program.normalized_type_identity(projected)
                    })
                {
                    return None;
                }
                selected_path.extend(path.iter().cloned());
                let sources = returned_record_sources(program, nested_destination)?;
                let mut matching = sources.iter().filter(|source| source.path == selected_path);
                let nested_leaf = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                let leaf_loan = call_actual_leaf_loan(
                    program,
                    facts,
                    machine,
                    state,
                    u32::try_from(call.statement_index).ok()?,
                    nested,
                    nested_destination,
                    nested_leaf,
                )?;
                let leaf_loan = facts.borrow.loans.get(leaf_loan);
                if leaf_loan.last_use_statement_index != call.statement_index {
                    return None;
                }
                vec![(
                    leaf_loan.root_symbol,
                    facts.borrow.loan_segments(leaf_loan).to_vec(),
                )]
            }
            _ => return None,
        }
    } else if mutable_view || shared_view {
        // A returned loan retains the actual argument's exact backing, whether
        // that backing is a parameter, an invocation-local owner, or a view of
        // either. Reconstruct the entire captured path instead of admitting only
        // parameter roots or collapsing a field to its declaration symbol.
        // Borrow checking owns availability/access; activation and release below
        // must still match this particular call and result binding.
        let (root, segments) =
            mutable_source_place(program, machine, state, call.statement_index, argument)?;
        rebase_place_through_local_loans(
            program,
            facts,
            machine,
            state.symbol,
            call.statement_index,
            root,
            segments,
        )
    } else {
        let ExpressionNode::Name(actual) = program.expression_table.expression(root) else {
            return None;
        };
        if actual.head_symbol != actual.symbol
            || program
                .expression_table
                .name_path_members(actual.members)
                .len()
                != 1
            || !members.is_empty()
            || !program.state_parameters(state).iter().any(|parameter| {
                parameter.symbol == actual.symbol
                    && !parameter.is_const
                    && parts(program, parameter.type_reference).is_some()
                    && program.normalized_type_identity(parameter.type_reference)
                        == program.normalized_type_identity(local.type_reference)
            })
        {
            return None;
        }
        vec![(actual.symbol, Vec::new())]
    };
    let mut states = facts
        .borrow
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .filter(|candidate| {
            candidate.machine_symbol == machine && candidate.state_symbol == state.symbol
        });
    let Some(borrowing) = states.next() else {
        return None;
    };
    if states.next().is_some() {
        return None;
    }
    let mut loans = facts.borrow.loans.iter().filter(|(handle, loan)| {
        facts.borrow.state_owns_loan(borrowing, *handle) && loan.owner_symbol == local.symbol
    });
    let Some((handle, loan)) = loans.next() else {
        return None;
    };
    let expected_kind = if shared_view {
        checked_trees::BorrowAccessKind::Read
    } else {
        checked_trees::BorrowAccessKind::Mutable
    };
    if loans.next().is_some()
        || loan.statement_index != call.statement_index
        || !expected_places.iter().any(|(root, segments)| {
            *root == loan.root_symbol && segments.as_slice() == facts.borrow.loan_segments(loan)
        })
        || loan.kind != expected_kind
        || !facts.borrow.loan_owner_path(loan).is_empty()
    {
        return None;
    }
    let Some(flow) = state_flow(facts, machine, state.symbol) else {
        return None;
    };
    let mut activations = facts
        .flow
        .borrow_lifetimes
        .activations
        .span_or_empty(flow.borrow_activations)
        .iter()
        .filter(|activation| activation.loan == handle);
    let Some(activation) = activations.next() else {
        return None;
    };
    if activations.next().is_some()
        || activation.source
            != (checked_trees::FlowInvalidationSource::Statement {
                statement_index: loan.statement_index,
            })
    {
        return None;
    }
    release_boundary(program, facts, machine, state.symbol, handle)?;
    Some(handle)
}

fn mutable_source_place(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            mutable_source_place(program, machine, state, statement_index, inner.target)
        }
        ExpressionNode::Indexed(indexed) => {
            let (root, mut segments) =
                mutable_source_place(program, machine, state, statement_index, indexed.collection)?;
            segments.push(index_place_segment(program, indexed.index));
            Some((root, segments))
        }
        ExpressionNode::Member(member) => {
            let member_symbol = mutable_effective_member_symbol(
                program,
                machine,
                state,
                statement_index,
                member.receiver,
                member,
            );
            if !member_symbol.is_valid() {
                return None;
            }
            match program.expression_table.expression(member.receiver) {
                ExpressionNode::Name(path)
                    if path.members.count() == 1
                        && mutable_name_root(
                            program,
                            state,
                            statement_index,
                            member.receiver,
                            path,
                        ) == Some(machine) =>
                {
                    Some((member_symbol, Vec::new()))
                }
                _ => {
                    let (root, mut segments) = mutable_source_place(
                        program,
                        machine,
                        state,
                        statement_index,
                        member.receiver,
                    )?;
                    push_field_place_segments(program, &mut segments, member_symbol);
                    Some((root, segments))
                }
            }
        }
        ExpressionNode::Name(path) => {
            if crate::proof_contracts::bound_expression_meaning::exact_case_reference_owner(
                program, expression,
            )
            .is_some()
            {
                return None;
            }
            let root = mutable_name_root(program, state, statement_index, expression, path)?;
            let member_symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            let skip = member_symbols
                .iter()
                .position(|member_symbol| *member_symbol == root)
                .map(|index| index + 1)
                .unwrap_or(1);
            let mut segments = Vec::new();
            for symbol in member_symbols.iter().skip(skip).copied() {
                push_field_place_segments(program, &mut segments, symbol);
            }
            Some((root, segments))
        }
        _ => None,
    }
}

/// The root symbol one `Name` resolves to: the typed path's first retained
/// symbol, else the self receiver's parameter, a same-named parameter, or the
/// latest local spelling the name before this statement.
fn mutable_name_root(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
    path: &typed_trees::expression::TableNamePath,
) -> Option<SymbolHandle> {
    if let Some(symbol) = program
        .expression_table
        .name_path_member_symbols(path.member_symbols)
        .first()
        .copied()
        .filter(|symbol| symbol.is_valid())
        .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
        .or_else(|| path.symbol.is_valid().then_some(path.symbol))
    {
        return Some(symbol);
    }
    let name = program.expression_table.display_name(expression);
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == name)
    {
        return Some(parameter.symbol);
    }
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.name.as_str() == name => Some(local.symbol),
            _ => None,
        })
}

/// An index step's normalized segment: a literal or constant-valued index is
/// its fixed ordinal, a half-open literal range is a window, and anything
/// else stays a runtime `Index` expression.
fn index_place_segment(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> facts::PlaceSegment {
    if let ExpressionNode::Range(range) = program.expression_table.expression(expression)
        && let Some((start, end)) = fixed_half_open_range(program, range)
    {
        return facts::PlaceSegment::FixedRange { start, end };
    }
    program
        .expression_table
        .constant_integer_value(expression)
        .and_then(|value| usize::try_from(value).ok())
        .map(|index| facts::PlaceSegment::FixedIndex { index })
        .unwrap_or(facts::PlaceSegment::Index { expression })
}

fn fixed_half_open_range(
    program: &TypedTrees,
    range: &typed_trees::expression::TableRangeExpression,
) -> Option<(usize, usize)> {
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
    if !range.end.is_valid() {
        return None;
    }
    let end = usize::try_from(program.expression_table.constant_integer_value(range.end)?).ok()?;
    let end = if range.end_inclusive {
        end.checked_add(1)?
    } else {
        end
    };
    Some((start, end))
}

/// `borrow_access_place`'s member spelling: the member resolved through the
/// receiver's own type — an attached machine's `self` data, a parameter's
/// named type, or the data a previous hop landed in — before falling back to
/// the expression's retained symbol. `self.bag` in an attached machine
/// spells the data field `Main::bag`, not the machine's `Main::main::bag`
/// state spelling, so both resolvers must answer the same symbol.
fn mutable_effective_member_symbol(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    receiver: typed_trees::expression::ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    if let Some(type_symbol) =
        mutable_receiver_type_symbol(program, machine, state, statement_index, receiver)
        && let Some(symbol) =
            mutable_member_from_type_symbol(program, type_symbol, member.member.as_str())
    {
        return symbol;
    }
    member.member_symbol
}

/// The named type one receiver expression's value inhabits: borrows unwrap,
/// indexed collections keep their element's named owner, and names and
/// members resolve through the symbols their roots already spell.
fn mutable_receiver_type_symbol(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            mutable_receiver_type_symbol(program, machine, state, statement_index, inner.target)
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = mutable_receiver_type_symbol(
                program,
                machine,
                state,
                statement_index,
                indexed.collection,
            )?;
            let definition = crate::program_scope::data_definitions::data_definition_by_symbol(
                program, collection,
            )?;
            Some(definition.symbol)
        }
        ExpressionNode::Member(member) => {
            let symbol = mutable_effective_member_symbol(
                program,
                machine,
                state,
                statement_index,
                member.receiver,
                member,
            );
            mutable_symbol_type_symbol(program, machine, state, statement_index, symbol)
        }
        ExpressionNode::Name(path) => {
            let root = mutable_name_root(program, state, statement_index, expression, path)?;
            mutable_symbol_type_symbol(program, machine, state, statement_index, root)
        }
        _ => None,
    }
}

/// The named type a resolved root or member symbol's value inhabits: the
/// attached machine's own data for its `self` root, the declared type of a
/// parameter or earlier local, or the named owner of a data field's type.
fn mutable_symbol_type_symbol(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: usize,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if symbol == machine {
        let owner = program.machine_by_symbol(machine)?;
        return program
            .attached_data_definition(owner)
            .map(|definition| definition.symbol);
    }
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
    {
        let type_symbol = program
            .type_reference_table
            .type_symbol(parameter.type_reference);
        return type_symbol.is_valid().then_some(type_symbol);
    }
    if let Some(local) = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
            _ => None,
        })
    {
        let type_symbol = program
            .type_reference_table
            .type_symbol(local.type_reference);
        return type_symbol.is_valid().then_some(type_symbol);
    }
    program
        .data_definitions()
        .iter()
        .flat_map(|definition| program.data_members(definition))
        .find_map(|member| match member {
            typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                let type_symbol = program
                    .type_reference_table
                    .type_symbol(field.type_reference);
                type_symbol.is_valid().then_some(type_symbol)
            }
            typed_trees::data::DataMember::Variant(variant) => program
                .data_payload_fields(variant)
                .iter()
                .find(|field| field.symbol == symbol)
                .and_then(|field| {
                    let type_symbol = program
                        .type_reference_table
                        .type_symbol(field.type_reference);
                    type_symbol.is_valid().then_some(type_symbol)
                }),
            _ => None,
        })
}

/// A member name resolved inside one named type — `Main::bag` for `bag` in
/// `Main` — covering declared fields, variants and payload fields of a data
/// definition, an attached machine's data, and machine-owned data, exactly
/// as `resolve_member_symbol_from_type_symbol` spells them.
fn mutable_member_from_type_symbol(
    program: &TypedTrees,
    type_symbol: SymbolHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let resolved = |definition: &typed_trees::data::DataDefinition| {
        program
            .data_members(definition)
            .iter()
            .find_map(|member| match member {
                typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    Some(field.symbol)
                }
                typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    Some(variant.symbol)
                }
                typed_trees::data::DataMember::Variant(variant) => program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.name.as_str() == member_name)
                    .map(|field| field.symbol),
                _ => None,
            })
    };
    if let Some(definition) =
        crate::program_scope::data_definitions::data_definition_by_symbol(program, type_symbol)
        && let Some(symbol) = resolved(definition)
    {
        return Some(symbol);
    }
    if let Some(owner) = program.machine_by_symbol(type_symbol) {
        if let Some(definition) = program.attached_data_definition(owner)
            && let Some(symbol) = resolved(definition)
        {
            return Some(symbol);
        }
        if let Some(owned) = program
            .machine_owned_data(owner)
            .iter()
            .find(|owned| owned.name.as_str() == member_name)
        {
            return Some(owned.symbol);
        }
    }
    None
}

/// A field step pushes the payload's `Case` segment first when the field
/// lives under a sum case, then the field itself — the spelling
/// `push_field_place_segments` shares for place construction.
fn push_field_place_segments(
    program: &TypedTrees,
    segments: &mut Vec<facts::PlaceSegment>,
    symbol: SymbolHandle,
) {
    if let Some(variant) = facts::payload_variant_for_field(program, symbol) {
        segments.push(facts::PlaceSegment::Case { variant });
    }
    segments.push(facts::PlaceSegment::Field { symbol });
}

/// Rebase one actual place through the loans the actual's root already
/// carries: a view local's own loan names the loaned storage this new place
/// joins. Each qualifying earlier loan yields one candidate; none means the
/// root names the place directly (a parameter's own storage).
fn rebase_place_through_local_loans(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    root: SymbolHandle,
    segments: Vec<facts::PlaceSegment>,
) -> Vec<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let mut states = facts
        .borrow
        .states
        .iter()
        .map(|(_, candidate)| candidate)
        .filter(|candidate| candidate.machine_symbol == machine && candidate.state_symbol == state);
    let Some(borrowing) = states.next().filter(|_| states.next().is_none()) else {
        return Vec::new();
    };
    let rebased: Vec<(SymbolHandle, Vec<facts::PlaceSegment>)> = facts
        .borrow
        .loans
        .iter()
        .filter(|(handle, loan)| {
            facts.borrow.state_owns_loan(borrowing, *handle)
                && loan.owner_symbol == root
                && loan.statement_index < statement_index
                && loan_owner_path_is_prefix(program, facts.borrow.loan_owner_path(loan), &segments)
        })
        .map(|(_, loan)| {
            let mut rebased = facts.borrow.loan_segments(loan).to_vec();
            rebased.extend_from_slice(&segments[facts.borrow.loan_owner_path(loan).len()..]);
            (loan.root_symbol, rebased)
        })
        .collect();
    if rebased.is_empty() {
        vec![(root, segments)]
    } else {
        rebased
    }
}

/// Whether a recorded loan's owner path is a prefix of one actual's place
/// segments, so the loan's own (root, segments) rebase the actual's
/// remainder. Dynamic owners overlap any index-shaped place step.
fn loan_owner_path_is_prefix(
    program: &TypedTrees,
    owner_path: &[checked_trees::BorrowLoanOwnerSegment],
    segments: &[facts::PlaceSegment],
) -> bool {
    owner_path.len() <= segments.len()
        && owner_path
            .iter()
            .zip(segments.iter())
            .all(|(owner, place)| {
                use checked_trees::BorrowLoanOwnerSegment as Owner;
                match (owner, place) {
                    (Owner::Field(owner_symbol), facts::PlaceSegment::Field { symbol }) => {
                        !symbol.is_valid() || owner_symbol == symbol
                    }
                    (Owner::Case(owner_variant), facts::PlaceSegment::Case { variant }) => {
                        owner_variant == variant
                    }
                    (Owner::FixedIndex(owner_index), facts::PlaceSegment::FixedIndex { index }) => {
                        owner_index == index
                    }
                    (Owner::FixedIndex(owner_index), facts::PlaceSegment::Index { expression }) => {
                        program
                            .expression_table
                            .constant_integer_value(*expression)
                            .and_then(|value| usize::try_from(value).ok())
                            .is_none_or(|place_index| *owner_index == place_index)
                    }
                    (
                        Owner::DynamicIndex,
                        facts::PlaceSegment::FixedIndex { .. }
                        | facts::PlaceSegment::FixedRange { .. }
                        | facts::PlaceSegment::Index { .. },
                    ) => true,
                    _ => false,
                }
            })
}

/// Earliest statement entry permitted by this local's own uses and loans.
/// A dead reference leaf does not authorize destroying a live scalar sibling.
/// Missing leaf weakening evidence keeps disposal on the existing exit path.
/// The execution scheduler must also retire holders borrowing this local's
/// storage before using this boundary to dispose of its whole value.
pub fn whole_local_release_boundary(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
) -> Option<u32> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(local) = statements.get(statement_index as usize)? else {
        return None;
    };
    let loans = local_record_loans(program, facts, machine, state, statement_index)?;
    if loans.is_empty() || !local.symbol.is_valid() {
        return None;
    }
    let mut boundary = statement_index.checked_add(1)?;
    for (_, loan) in loans {
        boundary = boundary.max(release_statement(facts, machine, state.symbol, loan)?);
    }
    for (index, statement) in statements
        .iter()
        .enumerate()
        .skip(statement_index as usize + 1)
    {
        if crate::value_custody::source_liveness::statement_uses_symbol(
            program,
            statement,
            local.symbol,
        ) {
            boundary = boundary.max(u32::try_from(index).ok()?.checked_add(1)?);
        }
    }
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
                (facts.borrow.state_owns_loan(borrowing, handle) && loan.root_symbol == root)
                    .then_some(handle)
            })
            .collect(),
    )
}

pub fn release_statement(
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<u32> {
    let flow = state_flow(facts, machine, state)?;
    let mut weakenings = facts
        .flow
        .borrow_lifetimes
        .weakenings
        .span_or_empty(flow.borrow_weakenings)
        .iter()
        .filter(|weakening| weakening.loan == handle);
    let weakening = weakenings.next()?;
    let checked_trees::FlowInvalidationSource::Statement { statement_index } = weakening.source
    else {
        return None;
    };
    if weakenings.next().is_some()
        || !matches!(
            weakening.reason,
            checked_trees::FlowBorrowWeakeningReason::LastUseExpired
                | checked_trees::FlowBorrowWeakeningReason::StateExit
        )
        || statement_index
            != facts
                .borrow
                .loans
                .get(handle)
                .last_use_statement_index
                .checked_add(1)?
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
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    handle: arena::Handle<checked_trees::BorrowLoanFact>,
) -> Option<u32> {
    if let Some(boundary) = release_statement(facts, machine, state, handle) {
        return Some(boundary);
    }
    let state = program.state_by_symbol(state)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    (facts
        .borrow
        .loans
        .get(handle)
        .last_use_statement_index
        .checked_add(1)?
        >= statements.len())
    .then(|| u32::try_from(statements.len()).ok())
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
