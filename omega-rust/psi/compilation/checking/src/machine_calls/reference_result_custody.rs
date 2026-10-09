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

use checked_trees::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralResultBindingPlan,
    ReferenceCustodyFacts, ReferenceCustodyRow, StateReferenceCustodyFacts,
    StateReferenceCustodyRow,
};
use language_semantics::Multiplicity;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

mod primitive_locals;
pub use primitive_locals::primitive_local_source;
mod stored_carrier_sources;
pub use stored_carrier_sources::{StoredCarrierSource, stored_carrier_sources};
mod call_reception;
pub use call_reception::{RecordCallReception, record_call_reception};

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
            crate::declarations::data_queries::data_definition_by_symbol(program, symbol)
        {
            for member in program.data_members(data) {
                if let Some(field) = member.as_field() {
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
        seen: &mut Vec<SymbolHandle>,
    ) -> Option<bool> {
        if stored_loan_parts(program, reference).is_some() {
            return Some(true);
        }
        if crate::has_contents(
            program,
            reference,
            crate::value_custody::storage_contents::ContentsRequirement::NumericOwned,
        ) {
            return Some(false);
        }
        if let TypeReferenceNode::FixedArray { element_type, .. } =
            program.type_reference_table.type_reference(reference)
        {
            return visit(program, *element_type, seen);
        }
        let symbol = program
            .type_reference_table
            .unparameterized_type_symbol(reference)?;
        if seen.contains(symbol) {
            return None;
        }
        let data = crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)?;
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
            let field = member.as_field()?;
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
                    field.path_identity(),
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
        type_identity: program.normalized_type_identity(referent).into_string(),
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
            TypeReferenceNode::Named { symbol, .. } => {
                return crate::declarations::data_queries::data_definition_by_symbol(
                    program, *symbol,
                );
            }
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if arguments.count() == 0 => {
                return crate::declarations::data_queries::data_definition_by_symbol(
                    program,
                    *base_symbol,
                );
            }
            _ => return None,
        }
    }
}

/// Completion and call substitution share the complete normal-edge origin
/// relation. Internal state ordinals are substituted through their actuals;
/// a record result is not restricted to a single state's tail expression.
pub fn returned_record_sources(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    if !is_reference_record(program, state.return_type) {
        return None;
    }
    returned_reference_sources(program, state)
}

/// The expression producing a state's returned value: the tail expression
/// statement, or the exit arm of a terminal return transition.
pub fn tail_return_expression(
    program: &dyn crate::CheckingInput,
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

/// Compose the origin relation of a value, independently of how it is returned
/// or stored. Captured local custody comes from the borrow ledger, not this
/// body's possible return relation. Control alternatives union origins at the
/// same result path; aggregate children prefix that path instead.
fn expression_sources_with_calls(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    if stored_loan_parts(program, reference).is_some()
        && let Some(source) = initializer_source(program, state, expression, reference)
    {
        return Some(vec![checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source,
        }]);
    }
    if crate::has_contents(
        program,
        reference,
        crate::value_custody::storage_contents::ContentsRequirement::NumericOwned,
    ) {
        return Some(Vec::new());
    }
    if let Some(dispatch) = program.expression_table.match_node(expression) {
        let arms = program.expression_table.match_arms(dispatch.arms);
        if arms.is_empty() {
            return None;
        }
        let mut sources = Vec::new();
        for arm in arms {
            // Every alternative must supply its complete relation. Selecting
            // an arm does not prove which input loan obligations can end;
            // declaration-level lifetime candidates remain live separately.
            for source in expression_sources_with_calls(
                program,
                state,
                arm.value,
                reference,
                active_origins,
                call_frames,
            )? {
                if !sources.contains(&source) {
                    sources.push(source);
                    if sources.len() > MAX_REFERENCE_ORIGIN_LEAVES {
                        return None;
                    }
                }
            }
        }
        return Some(sources);
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
            call_frames,
        );
    }
    if stored_loan_parts(program, reference).is_none() && !is_reference_record(program, reference) {
        return None;
    }
    if let Some(name) = program.expression_table.name_path_node(expression) {
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
            let sources = expression_sources_with_calls(
                program,
                state,
                local.initial_value,
                reference,
                active_origins,
                call_frames,
            );
            active_origins.pop();
            return sources;
        }
        let (ordinal, parameter) = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| is_mutable_structural_parameter(program, parameter))
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == name.symbol)?;
        if name.head_symbol != name.symbol
            || program.symbols.get(name.symbol).parent != state.symbol
            || parameter.name != *spelling
            || program
                .normalized_type_identity(input_carrier_type(program, parameter.type_reference)?)
                != program.normalized_type_identity(reference)
        {
            return None;
        }
        return record_carrier_sources(
            program,
            reference,
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(ordinal).ok()?,
            },
        )
        .map(|mut sources| {
            if crate::reference_shell(program, parameter.type_reference).is_some() {
                for source in &mut sources {
                    source
                        .source
                        .path
                        .insert(0, checked_trees::CheckedUnitStructuralPathSegment::Referent);
                }
            }
            sources
        });
    }
    if let Some(call) = program.expression_table.call_node(expression) {
        call_sources_with_calls(
            program,
            state,
            call.target_symbol,
            program.expression_table.expression_handles(call.arguments),
            reference,
            active_origins,
            call_frames,
        )
    } else {
        aggregate_construction_sources(
            program,
            state,
            expression,
            reference,
            active_origins,
            call_frames,
        )
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
        root_type,
        path,
        selected_type: selected,
    })
}

/// Selecting an existing value preserves its source-only returned origins.
/// The projection removes the selected owner prefix; captured descriptor
/// custody remains independently available in the borrow ledger.
fn projected_construction_sources(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    let projection = aggregate_projection(program, state, expression)?;
    if program.normalized_type_identity(projection.selected_type)
        != program.normalized_type_identity(reference)
    {
        return None;
    }
    let sources = expression_sources_with_calls(
        program,
        state,
        projection.root,
        projection.root_type,
        active_origins,
        call_frames,
    )?;
    Some(
        sources
            .into_iter()
            .filter_map(|mut source| {
                let suffix = source.path.strip_prefix(projection.path.as_slice())?;
                source.path = suffix.to_vec();
                Some(source)
            })
            .collect(),
    )
}

/// Substitute returned permission leaves through the exact resolved invocation.
/// Expression and terminal calls share this owner; neither creates a local loan.
fn call_sources_with_calls(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    target: SymbolHandle,
    arguments: &[typed_trees::expression::ExpressionHandle],
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
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
    let sources = returned_sources_with_calls(program, destination, active_origins, call_frames);
    active_origins.pop();
    let mut output = Vec::new();
    for source in sources? {
        let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } =
            source.source.source
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
        let actual_sources = expression_sources_with_calls(
            program,
            state,
            *arguments.get(position)?,
            parameter.type_reference,
            active_origins,
            call_frames,
        )?;
        let borrowed_formal = stored_loan_parts(program, parameter.type_reference);
        let selected_path = if let Some((referent, access)) = borrowed_formal {
            if !reference_access_can_supply(access, source.source.access) {
                return None;
            }
            // The callee's source reconstruction already authenticates each
            // projected declaration and its final referent type. Substitution
            // requires the actual to supply that exact formal referent; its
            // existing backing path then precedes the callee's projection.
            let identity = program.normalized_type_identity(referent);
            if actual_sources.iter().any(|actual| {
                !actual.path.is_empty()
                    || actual.source.type_identity != identity.as_str()
                    || !reference_access_can_supply(actual.source.access, access)
            }) {
                return None;
            }
            &[][..]
        } else {
            let (last, path) = source.source.path.split_last()?;
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
            .filter(|actual| actual.path == selected_path)
        {
            if !reference_access_can_supply(actual.source.access, source.source.access) {
                return None;
            }
            if borrowed_formal.is_some() {
                actual.source.path.extend_from_slice(&source.source.path);
                actual.source.type_identity = source.source.type_identity.clone();
            } else if actual.source.type_identity != source.source.type_identity {
                return None;
            }
            actual.path = source.path.clone();
            actual.source.access = source.source.access;
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
        sources.push(checked_trees::CheckedReferenceResultSourcePlan {
            path: Vec::new(),
            source,
        });
    }
    Some(sources)
}

fn returned_sources_with_calls(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    active_origins: &mut Vec<SymbolHandle>,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    if stored_loan_parts(program, state.return_type).is_some() {
        return whole_reference_sources(program, state, call_frames);
    }
    let mut sources = Vec::new();
    for statement in program.statement_table.statements(state.statement_nodes) {
        let Some(transition) = statement.as_transition() else {
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
            TransitionTargetNode::Value(expression) => expression_sources_with_calls(
                program,
                state,
                *expression,
                state.return_type,
                active_origins,
                call_frames,
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
                call_frames,
            )?,
            TransitionTargetNode::Terminal => return None,
        };
        for source in returned {
            if !sources.iter().any(
                |existing: &checked_trees::CheckedReferenceResultSourcePlan| existing == &source,
            ) {
                sources.push(source);
            }
        }
    }
    if let Some(StatementNode::Expression(expression)) = program
        .statement_table
        .statements(state.statement_nodes)
        .last()
    {
        for source in expression_sources_with_calls(
            program,
            state,
            *expression,
            state.return_type,
            active_origins,
            call_frames,
        )? {
            if !sources.iter().any(|existing| existing == &source) {
                sources.push(source);
            }
        }
    }
    (!sources.is_empty()).then_some(sources)
}

/// Every normal completion contributes its exact caller-relative origin.
pub fn returned_reference_sources(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    returned_reference_sources_with_frames(program, state, None)
}

/// Borrow an already prepared compilation resolver through every substituted
/// origin query. Standalone consumers prepare their own resolver; source
/// checks must not rebuild whole-program symbol tables for each returned view.
pub fn returned_reference_sources_with_frames(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    returned_sources_with_calls(program, state, &mut vec![state.symbol], call_frames)
}

/// A terminal invocation forwards its reference directly to the state's return.
/// Its exact flow occurrence has no expression/local holder; the ingress roster
/// is therefore the custody, rather than a fabricated local loan handle.
pub fn terminal_call_reference_sources(
    program: &dyn crate::CheckingInput,
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
    let transition = program
        .statement_table
        .statement_at(state, call.statement_index)?
        .as_transition()?;
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
        None,
    )
}

/// A stored carrier transports its existing reference leaves. The source path
/// names each borrowed referent; it does not authorize creating a fresh loan
/// or moving storage through the referent boundary.
pub fn record_carrier_sources(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    carrier: checked_trees::CheckedUnitStructuralArgumentSourcePlan,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    fn visit(
        program: &dyn crate::CheckingInput,
        reference: TypeReferenceHandle,
        carrier: &checked_trees::CheckedUnitStructuralArgumentSourcePlan,
        path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        output: &mut Vec<checked_trees::CheckedReferenceResultSourcePlan>,
    ) -> Option<()> {
        if let Some((referent, access)) = stored_loan_parts(program, reference) {
            let mut source_path = path.clone();
            source_path.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
            output.push(checked_trees::CheckedReferenceResultSourcePlan {
                path: path.clone(),
                source: checked_trees::CheckedUnitStructuralArgumentPlan {
                    source: carrier.clone(),
                    path: source_path,
                    type_identity: program.normalized_type_identity(referent).into_string(),
                    access,
                },
            });
            return Some(());
        }
        if crate::has_contents(
            program,
            reference,
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
                path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    u64::try_from(element_index).ok()?,
                ));
                visit(program, *element_type, carrier, path, output)?;
                path.pop();
            }
            return Some(());
        }
        let symbol = program
            .type_reference_table
            .unparameterized_type_symbol(reference)?;
        let data = crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)?;
        for member in program.data_members(data) {
            let field = member.as_field()?;
            path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
                field.path_identity(),
            ));
            visit(program, field.type_reference, carrier, path, output)?;
            path.pop();
        }
        Some(())
    }
    if !is_reference_record(program, reference) {
        return None;
    }
    let mut output = Vec::new();
    visit(program, reference, &carrier, &mut Vec::new(), &mut output)?;
    Some(output)
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

fn aggregate_construction_sources(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    expression: typed_trees::expression::ExpressionHandle,
    reference: TypeReferenceHandle,
    active_origins: &mut Vec<SymbolHandle>,
    call_frames: Option<&crate::CallFrameResolver<'_>>,
) -> Option<Vec<checked_trees::CheckedReferenceResultSourcePlan>> {
    // Enumerate semantic children once, then compose every child's captured
    // loans through its exact stored path. A field and an array slot differ
    // in their projection, not in the ownership of the reference they hold.
    let mut children = Vec::new();
    if let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(reference)
    {
        let literal = program.expression_table.array_literal_node(expression)?;
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
        let symbol = program
            .type_reference_table
            .unparameterized_type_symbol(reference)?;
        let literal = program.expression_table.struct_literal_node(expression)?;
        if literal.type_symbol != *symbol || literal.case_symbol.is_some() {
            return None;
        }
        let data = crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)?;
        let initializers = program.expression_table.struct_fields(literal.fields);
        let fields = program.data_members(data);
        if initializers.len() != fields.len() {
            return None;
        }
        for member in fields {
            let field = member.as_field()?;
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
        for mut source in expression_sources_with_calls(
            program,
            state,
            expression,
            reference,
            active_origins,
            call_frames,
        )? {
            source.path.insert(0, segment.clone());
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
pub fn local_owes_record_loans(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
    is_reference_record(program, reference) && shared_borrowed_parts(program, reference).is_none()
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
    local_place_loans(
        program,
        facts,
        machine,
        state,
        local.symbol,
        statement_index,
        &[],
    )
}

/// Join a local storage occurrence formed by a constructor, call or store.
/// The declared leaf roster and exact activation are independent of the
/// producer. A subtree replacement captures only leaves beneath its destination.
pub fn local_place_loans(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    owner: SymbolHandle,
    statement_index: u32,
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<RecordLeafLoans>> {
    let (_, local) = program.state_local_declaration(state, owner)?;
    let selected_owner_path = loan_owner_path(program, local.type_reference, selection)?;
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
        .local_loans(borrowing, local.symbol, statement_index as usize)?;
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
    record_leaf_loan_groups(
        program,
        facts,
        local.type_reference,
        local.symbol,
        &captured,
        selection,
    )
}

/// Loan rows for one descriptor are contiguous in the producer's arena, even
/// when that descriptor retains several possible origins. Every captured row
/// belongs to one declared leaf; neither an omitted leaf nor an extra path is
/// permission to dispose of only a convenient subset of the custody.
fn record_leaf_loan_groups(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    reference: TypeReferenceHandle,
    owner: SymbolHandle,
    loans: &[arena::Handle<checked_trees::BorrowLoanFact>],
    selection: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<RecordLeafLoans>> {
    let sources = record_carrier_sources(
        program,
        reference,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: owner },
    )?;
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
            if loan.owner_symbol != owner
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
            return None;
        }
        output.push((source, group));
    }
    (grouped_count == loans.len()).then_some(output)
}

/// The loan segments a borrowed initializer source projects under its
/// parameter's declared type, spelled in the ownership vocabulary the
/// ledger records. Field hops resolve through the nominal owner exactly as
/// the owner-path walk does; literal fixed-array hops keep their ordinal.
/// Runtime-indexed, case and referent hops are not direct backing geometry.
/// Transfers through a stored descriptor use its captured loan instead.
pub fn loan_place_segments(
    program: &dyn crate::CheckingInput,
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

pub fn loan_owner_path(
    program: &dyn crate::CheckingInput,
    root: TypeReferenceHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> Option<Vec<checked_trees::BorrowLoanOwnerSegment>> {
    loan_place_segments(program, root, path)?
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
        .collect()
}

/// Join one leaf of a call's returned-leaf roster through the call's exact
/// actual to a caller-side local's captured loan group. This is the operand form of
/// construction's holder join: the producing call is named by its authored
/// expression rather than a `let` statement's initializer, and the consuming
/// statement's activity or last-use discipline is checked by the caller.
fn call_actual_leaf_loans(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    call: &typed_trees::expression::TableCallExpression,
    destination: &typed_trees::state::State,
    source: &checked_trees::CheckedReferenceResultSourcePlan,
) -> Option<arena::HandleSpan<checked_trees::BorrowLoanFact>> {
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
        .filter(|(_, parameter)| is_mutable_structural_parameter(program, parameter))
        .nth(parameter_index as usize)?;
    if !is_reference_record(program, parameter.type_reference) {
        return None;
    }
    let actual = *program
        .expression_table
        .expression_handles(call.arguments)
        .get(position)?;
    let name = program.expression_table.name_path_node(actual)?;
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
    let prior = active_record_loans(
        program,
        facts,
        machine,
        state,
        statement_index,
        u32::try_from(prior_index).ok()?,
    )?;
    let mut matching = prior.iter().filter(|(source, _)| source.path == path);
    let (_, handle) = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    Some(*handle)
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
    if declaration_statement >= statement_index {
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
            (captured.owner_symbol == local.symbol).then_some(loan)
        })
        .collect::<Vec<_>>();
    // Flow contexts preserve their own constraint order. Arena order owns the
    // contiguous descriptor groups; do not make context insertion order part
    // of the custody join.
    loans.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
    if loans
        .iter()
        .any(|handle| facts.borrow.loans.get(*handle).statement_index >= statement_index as usize)
    {
        return None;
    }
    record_leaf_loan_groups(
        program,
        facts,
        local.type_reference,
        local.symbol,
        &loans,
        &[],
    )
}

/// Whole owned operands retain every captured leaf until their consuming call.
pub fn owned_record_argument(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    source_statement: u32,
) -> bool {
    source_statement < statement_index
        && active_record_loans(
            program,
            facts,
            machine,
            state,
            statement_index,
            source_statement,
        )
        .is_some_and(|loans| !loans.is_empty())
}

/// Resolve an authored member projection against a record type, yielding the
/// field-identity path and the projected type. The spelling matches the leaf
/// paths `record_carrier_sources` and `local_record_loans` reconstruct, so one
/// resolver keeps those joins from drifting.
pub(crate) fn declared_field_path<'a>(
    program: &dyn crate::CheckingInput,
    mut selected: TypeReferenceHandle,
    members: impl Iterator<Item = &'a typed_trees::expression::TableMemberExpression>,
) -> Option<(
    Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    TypeReferenceHandle,
)> {
    let mut path = Vec::new();
    for member in members {
        let symbol = match program.type_reference_table.type_reference(selected) {
            TypeReferenceNode::Named { symbol, .. } => symbol,
            // Lifetimes do not substitute stored field types. Real type
            // arguments still require an instantiated field declaration.
            TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } if arguments.count() == 0 => base_symbol,
            _ => return None,
        };
        let data = crate::declarations::data_queries::data_definition_by_symbol(program, *symbol)?;
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
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    source_statement: u32,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> bool {
    !path.is_empty()
        && source_statement < statement_index
        && active_record_loans(
            program,
            facts,
            machine,
            state,
            statement_index,
            source_statement,
        )
        .is_some_and(|loans| {
            !loans.is_empty()
                && loans
                    .iter()
                    .all(|(source, _)| source.path.starts_with(path))
        })
}

/// A projected operand may root at a nested call's anonymous result instead
/// of a named local: `select(forward_outer(outer).inner)`. Every leaf the
/// nested result publishes under the projected edge must retain the complete
/// caller-side captured leaf group through the call's exact actual,
/// and every candidate must stay live until the consuming call — the same
/// custody `projected_record_argument` proves for a stored carrier. The
/// anonymous carrier itself is consumed whole by the projection, so its
/// entire returned leaf roster must live under the edge.
pub fn nested_call_record_argument(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    path: &[checked_trees::CheckedUnitStructuralPathSegment],
) -> bool {
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    let Some((destination, sources)) = reference_record_call_sources(program, call) else {
        return false;
    };
    !path.is_empty()
        && sources_replay_active_leaf_loans(
            program,
            facts,
            machine,
            state,
            statement_index,
            call,
            destination,
            &sources,
            |source| source.path.starts_with(path),
        )
}

/// Every admitted returned leaf source replays through the call's exact
/// actual to the complete active leaf group at this statement.
#[allow(clippy::too_many_arguments)]
fn sources_replay_active_leaf_loans(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    statement_index: u32,
    call: &typed_trees::expression::TableCallExpression,
    destination: &typed_trees::state::State,
    sources: &[checked_trees::CheckedReferenceResultSourcePlan],
    admits: impl Fn(&checked_trees::CheckedReferenceResultSourcePlan) -> bool,
) -> bool {
    !sources.is_empty()
        && sources.iter().all(|source| {
            admits(source)
                && call_actual_leaf_loans(
                    program,
                    facts,
                    machine,
                    state,
                    statement_index,
                    call,
                    destination,
                    source,
                )
                .is_some()
        })
}

/// The call's reference-record destination state + its returned leaf
/// sources — `None` when the call does not return a reference record.
/// Callers then require each source's complete active leaf group, filtered per site.
fn reference_record_call_sources<'program>(
    program: &'program dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<(
    &'program typed_trees::state::State,
    Vec<checked_trees::CheckedReferenceResultSourcePlan>,
)> {
    let destination = program.state_by_symbol(call.target_symbol)?;
    if !is_reference_record(program, destination.return_type) {
        return None;
    }
    let sources = returned_record_sources(program, destination)?;
    Some((destination, sources))
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
    let Some((destination, sources)) = reference_record_call_sources(program, call) else {
        return false;
    };
    !sources.is_empty()
        && residual_paths
            .iter()
            .all(|path| sources.iter().any(|source| &source.path == path))
        && sources
            .iter()
            .filter(|source| residual_paths.contains(&source.path))
            .all(|source| {
                let checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index,
                } = source.source.source
                else {
                    return false;
                };
                let Some(parameter) = program
                    .state_parameters(destination)
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
                    call_actual_leaf_loans(
                        program,
                        facts,
                        machine,
                        state,
                        statement_index,
                        call,
                        destination,
                        source,
                    )
                    .is_some()
                }
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
    let (referent, access) = stored_loan_parts(program, destination)?;
    let local = program
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)?
        .as_local_data()?;
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
    if source.source.type_identity != program.normalized_type_identity(referent).as_str() {
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
pub fn classify_program_reference_custody(
    program: &dyn crate::CheckingInput,
) -> ReferenceCustodyFacts {
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
/// The first executable carrier retains a mutable, unqualified primitive loan.
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    crate::reference_shell(program, reference)
        .filter(|shell| shell.access == language_semantics::ReferenceAccess::Mutable)
        .map(|shell| (shell.referee, CheckedStructuralAccess::MutableBorrow))
}

/// The named referee beneath a `&'a V` shared-borrowed view, with constraint
/// shells peeled. The referent's storage lives in the caller's frame like
/// `parts`' loan — it just cannot be written through. The referee must be
/// spelled as a `Named` type: this is the shared reference-result shape.
pub fn shared_borrowed_parts(
    program: &dyn crate::CheckingInput,
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
    if parts(program, reference).is_some()
        || shared_borrowed_parts(program, reference).is_some()
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
/// local result. Source admission already established its lifetime relation,
/// captured places and access. Do not replay argument initializers here or
/// choose one possible origin: the published facts own those decisions.
///
/// Borrow production appends each statement's local loans contiguously. The
/// span retains every candidate without a second allocation; malformed or
/// incomplete grouping fails closed rather than silently dropping a source.
pub fn result_loans(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    result: &CheckedUnitStructuralResultBindingPlan,
) -> Option<arena::HandleSpan<checked_trees::BorrowLoanFact>> {
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
            return Some(arena::HandleSpan::empty());
        }
        _ => return None,
    };
    let shell = crate::reference_shell(program, local.type_reference)?;
    let expected_kind = match shell.access {
        language_semantics::ReferenceAccess::Shared => checked_trees::BorrowAccessKind::Read,
        language_semantics::ReferenceAccess::Mutable => checked_trees::BorrowAccessKind::Mutable,
        language_semantics::ReferenceAccess::WriteOnly => {
            checked_trees::BorrowAccessKind::WriteOnly
        }
    };
    let Some(expression) = program.expression_table.call_node(local.initial_value) else {
        return None;
    };
    if local.is_mutable
        || expression.target_symbol != call.target_symbol
        || call.authored_expression != local.initial_value
        || call.statement_index != result.statement_index as usize
        || call.call_ordinal != 0
    {
        return None;
    }
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
    let group = facts
        .borrow
        .local_loans(borrowing, local.symbol, call.statement_index)?;
    if !facts.borrow.loan_group(group)?.all(|(_, loan)| {
        loan.kind == expected_kind && facts.borrow.loan_owner_path(loan).is_empty()
    }) {
        return None;
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
    if loans.is_empty() || !local.symbol.is_valid() {
        return None;
    }
    let mut boundary = statement_index.checked_add(1)?;
    for (_, group) in loans {
        for (loan, _) in facts.borrow.loan_group(group)? {
            boundary = boundary.max(release_statement(facts, machine, state.symbol, loan)?);
        }
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
    program: &dyn crate::CheckingInput,
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
