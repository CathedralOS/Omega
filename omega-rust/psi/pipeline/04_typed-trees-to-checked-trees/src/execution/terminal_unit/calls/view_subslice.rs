//! One admission for every exclusive `view[start..end]` range over an
//! established immutable view.
//!
//! A call argument, a transition argument and a `let` initializer narrow a
//! view with the same builtin range. They differ only in where the range sits
//! (`CheckedSubsliceSite`) and in what consumes the admitted range: a call
//! argument plan, an edge transfer, or a view-local binding. Byte views
//! (`&[u8]`) and element views (`&[T]`, T != u8) differ only in their carrier
//! check and source variant, so both kinds share this admission as well.
//!
//! The narrowed view must be an established view place: a whole immutable
//! view parameter of the state, or an immutable view local that an earlier
//! statement of the same body declared. Admission names the local by its `let`
//! symbol only; the producer that sequences the body decides whether that
//! local was actually established before this statement, and lowering
//! resolves the symbol to its published place or refuses.
//!
//! A range over a fixed-array field (`let sub: &[T] = self.items[1..hi]`) has
//! no established view to narrow, so the range names the field instead: its
//! root is the structural parameter that owns the storage and its argument
//! `path` is the record-field projection to the array. Lowering first
//! establishes a whole element view over that field under a shared loan and
//! then narrows it through the same subslice operation, so the field's
//! extent, the endpoints' bounds and the narrowed length all come from the
//! ordinary view vocabulary rather than a field-specific range. Only a view
//! local's binding admits this form: its view outlives the statement, while a
//! call argument over field storage keeps its call-scoped `FixedByteRange`
//! window and an edge transfer carries no projection. The checked borrow
//! ledger must already hold the local's shared loan of exactly that field, so
//! later writes to the array while the view is live stay rejected there.
use crate::execution::terminal_unit::CheckFacts;
use crate::execution::terminal_unit::CheckedScalarExpression;
use crate::execution::terminal_unit::CheckedScalarExpressionRole;
use crate::execution::terminal_unit::CheckedStructuralAccess;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
use crate::execution::terminal_unit::CheckedUnitStructuralParameterPlan;
use crate::execution::terminal_unit::ExpressionNode;
use crate::execution::terminal_unit::Multiplicity;
use crate::execution::terminal_unit::PrimitiveType;
use crate::execution::terminal_unit::StatementNode;
use crate::execution::terminal_unit::TypeReferenceHandle;
use crate::execution::terminal_unit::TypeReferenceNode;
use crate::execution::terminal_unit::TypedTrees;
use crate::execution::terminal_unit::types::{
    borrowed_slice_view_element, borrowed_slice_view_type_identity, byte_sequence_carrier,
    byte_sequence_type_identity, structural_access_for_type_reference,
};
use checked_trees::{
    CheckedScalarExpressionPlans, CheckedStorageRoot, CheckedStructuralControlTransferSourcePlan,
    CheckedSubsliceSite, CheckedUnitStructuralPathSegment,
};
use typed_trees::expression::ExpressionHandle;

/// Which borrowed view family a range narrows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::execution) enum ViewKind {
    Bytes,
    Elements,
}

/// The exact shape of one admitted range: its established source and the
/// narrowed view's type identity. Endpoints are attached by `admit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::execution) struct ViewRange {
    pub(in crate::execution) root: CheckedStorageRoot,
    /// Record-field projection from a parameter root to the fixed array the
    /// range views whole before narrowing; empty when the root is itself an
    /// established view.
    pub(in crate::execution) collection: Vec<CheckedUnitStructuralPathSegment>,
    pub(in crate::execution) kind: ViewKind,
    pub(in crate::execution) type_identity: String,
}

/// One admitted range with its present endpoints, copied from the unique
/// source-bound `SubsliceStart`/`SubsliceEnd` facts at its site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::execution) struct ViewSubslice {
    pub(in crate::execution) range: ViewRange,
    pub(in crate::execution) expression: ExpressionHandle,
    pub(in crate::execution) start: Option<CheckedScalarExpression>,
    pub(in crate::execution) end: Option<CheckedScalarExpression>,
}

impl ViewSubslice {
    /// The shared-borrow argument plan a call, a return or a view-local
    /// binding carries for this range.
    pub(in crate::execution) fn argument(self) -> CheckedUnitStructuralArgumentPlan {
        let ViewSubslice {
            range,
            expression,
            start,
            end,
        } = self;
        CheckedUnitStructuralArgumentPlan {
            source: match range.kind {
                ViewKind::Bytes => CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    root: range.root,
                    expression,
                    start,
                    end,
                },
                ViewKind::Elements => {
                    CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                        root: range.root,
                        expression,
                        start,
                        end,
                    }
                }
            },
            path: range.collection,
            type_identity: range.type_identity,
            access: CheckedStructuralAccess::SharedBorrow,
        }
    }

    /// The edge transfer for this range. Transfers retain only the source and
    /// the authored range; their endpoints are replayed at the edge's site.
    pub(in crate::execution) fn transfer(&self) -> CheckedStructuralControlTransferSourcePlan {
        match self.range.kind {
            ViewKind::Bytes => CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                root: self.range.root,
                expression: self.expression,
            },
            ViewKind::Elements => CheckedStructuralControlTransferSourcePlan::ElementViewSubslice {
                root: self.range.root,
                expression: self.expression,
            },
        }
    }
}

/// Admit an exact builtin exclusive range at `site`, vetoing any range whose
/// operator use did not resolve to the builtin meaning.
pub(in crate::execution) fn admit(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    expression: ExpressionHandle,
    statement_index: usize,
    site: CheckedSubsliceSite,
) -> Option<ViewSubslice> {
    let spelling = language_core::OperatorSpelling::Range;
    if facts
        .operators
        .expression_use(expression)
        .is_some_and(|selected| {
            selected.spelling != spelling
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                )
        })
    {
        return None;
    }
    let subslice = admit_replayed(
        program,
        &facts.values.scalar_expressions,
        machine,
        state,
        parameters,
        target,
        expression,
        statement_index,
        site,
    )?;
    (subslice.range.collection.is_empty()
        || lends_field_collection(program, facts, machine, state, statement_index, expression))
    .then_some(subslice)
}

/// Whether checked borrow admission recorded the `let` at `statement_index`
/// as the sole shared loan of exactly the fixed-array field `expression`
/// narrows. The view a field range establishes is that loan: the ledger, not
/// the authored spelling, is what keeps the array unwritten while it lives.
fn lends_field_collection(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> bool {
    let Some(StatementNode::LocalData(local)) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return false;
    };
    if local.initial_value != expression {
        return false;
    }
    // The loan names the ranged place itself: the field followed by the
    // range's own index segment, spelled by the same canonicalization.
    let Some(mut place) = crate::flow::canonical_place_from_expression(program, expression) else {
        return false;
    };
    crate::flow::normalize_attached_place_root(program, machine.symbol, state.symbol, &mut place);
    let Some((_, borrow_state)) = facts.borrow.states.iter().find(|(_, borrow_state)| {
        borrow_state.machine_symbol == machine.symbol && borrow_state.state_symbol == state.symbol
    }) else {
        return false;
    };
    let mut loans = facts
        .borrow
        .loans
        .span_or_empty(borrow_state.loans)
        .iter()
        .filter(|loan| loan.owner_symbol == local.symbol);
    let (Some(loan), None) = (loans.next(), loans.next()) else {
        return false;
    };
    let mut lent = crate::flow::CanonicalPlace {
        root: facts::PlaceRoot::Symbol(loan.root_symbol),
        segments: facts
            .borrow
            .access_segments
            .span_or_empty(loan.segments)
            .to_vec(),
    };
    crate::flow::normalize_attached_place_root(program, machine.symbol, state.symbol, &mut lent);
    loan.statement_index == statement_index
        && loan.kind == checked_trees::BorrowAccessKind::Read
        && lent == place
}

/// Admission without the operator-resolution veto. Lanes that replay emitted
/// endpoint bindings inherit that veto through the bindings' presence: no
/// builtin range means no endpoints to rebind.
pub(in crate::execution) fn admit_replayed(
    program: &TypedTrees,
    expressions: &CheckedScalarExpressionPlans,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    expression: ExpressionHandle,
    statement_index: usize,
    site: CheckedSubsliceSite,
) -> Option<ViewSubslice> {
    let range = shape(
        program,
        machine,
        state,
        parameters,
        target,
        expression,
        statement_index,
    )?;
    if !range.collection.is_empty() && site != CheckedSubsliceSite::LocalBinding {
        return None;
    }
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Range(authored) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    let statement_ordinal = u32::try_from(statement_index).ok()?;
    let endpoint = |endpoint: ExpressionHandle, role| {
        if !endpoint.is_valid() {
            return Some(None);
        }
        let (binding, value) =
            expressions.bound_expression_at(state.symbol, statement_ordinal, role)?;
        (binding.expression == endpoint
            && !binding.destination.is_valid()
            && value.primitive_type() == Some(PrimitiveType::U64))
        .then(|| Some(value.clone()))
    };
    let start = endpoint(
        authored.start,
        CheckedScalarExpressionRole::SubsliceStart { site },
    )?;
    let end = endpoint(
        authored.end,
        CheckedScalarExpressionRole::SubsliceEnd { site },
    )?;
    Some(ViewSubslice {
        range,
        expression,
        start,
        end,
    })
}

/// Range admission without endpoints: the target is an exact immutable
/// borrowed view, the authored range is exclusive and builtin, and the
/// narrowed collection is a bare name of an established view of the same
/// kind and identity.
pub(in crate::execution) fn shape(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    expression: ExpressionHandle,
    statement_index: usize,
) -> Option<ViewRange> {
    // A target keeps the looser element-view peel its call and edge lanes
    // always accepted; the narrowed source must be an exact view.
    let kind = view_kind(program, target).or_else(|| {
        borrowed_slice_view_element(program, target, &[]).map(|_| ViewKind::Elements)
    })?;
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(expression) else {
        return None;
    };
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return None;
    };
    if range.end_inclusive {
        return None;
    }
    let type_identity = match kind {
        ViewKind::Bytes => byte_sequence_type_identity(program, target, &[], &[])?,
        ViewKind::Elements => borrowed_slice_view_type_identity(program, target, &[], &[]),
    };
    let ExpressionNode::Name(path) = program.expression_table.expression(indexed.collection) else {
        let (root, collection) = field_collection(
            program,
            machine,
            state,
            parameters,
            target,
            kind,
            indexed.collection,
            statement_index,
        )?;
        return validation::has_builtin_subslice_meaning(program, machine, Some(state), expression)
            .then_some(ViewRange {
                root,
                collection,
                kind,
                type_identity,
            });
    };
    if !path.symbol.is_valid()
        || path.head_symbol != path.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .len()
            != 1
    {
        return None;
    }
    let authored = program.state_parameters(state);
    let root = if let Some(position) = authored
        .iter()
        .position(|parameter| parameter.symbol == path.symbol)
    {
        if authored[position].is_mutable {
            return None;
        }
        let parameter_index = parameters
            .iter()
            .position(|parameter| parameter.position as usize == position)?;
        let parameter = &parameters[parameter_index];
        if parameter.access != CheckedStructuralAccess::SharedBorrow
            || parameter.multiplicity != Multiplicity::Unrestricted
            || !parameter.qualifications.is_empty()
            || parameter.type_identity != type_identity
            || view_kind(program, authored[position].type_reference) != Some(kind)
        {
            return None;
        }
        CheckedStorageRoot::Parameter {
            index: u32::try_from(parameter_index).ok()?,
        }
    } else {
        let local = view_local_before(program, state, statement_index, path.symbol)?;
        if view_kind(program, local.type_reference) != Some(kind)
            || view_identity(program, kind, local.type_reference)? != type_identity
        {
            return None;
        }
        CheckedStorageRoot::ViewLocal {
            symbol: local.symbol,
        }
    };
    if !validation::has_builtin_subslice_meaning(program, machine, Some(state), expression) {
        return None;
    }
    Some(ViewRange {
        root,
        collection: Vec::new(),
        kind,
        type_identity,
    })
}

/// The structural parameter and record-field path of a fixed-array field a
/// range views whole: `collection` is a place below a readable structural
/// parameter of the state (`self` included), every step is a record field,
/// and the array's elements are the target view's elements. Byte views have
/// no Terminal establishment over fixed byte storage, so only element views
/// take this form.
#[allow(clippy::too_many_arguments)]
fn field_collection(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    kind: ViewKind,
    collection: ExpressionHandle,
    statement_index: usize,
) -> Option<(CheckedStorageRoot, Vec<CheckedUnitStructuralPathSegment>)> {
    if kind != ViewKind::Elements {
        return None;
    }
    let element = borrowed_slice_view_element(program, target, &[])?;
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        collection,
    )?;
    crate::flow::normalize_attached_place_root(program, machine.symbol, state.symbol, &mut place);
    if place.segments.is_empty() {
        return None;
    }
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let position = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == symbol)?;
    let parameter_index = parameters
        .iter()
        .position(|parameter| parameter.position as usize == position)?;
    // A shared view only reads the array; any readable loan or owned root
    // lends it, while a write-only loan grants no read.
    if !matches!(
        parameters[parameter_index].access,
        CheckedStructuralAccess::SharedBorrow
            | CheckedStructuralAccess::MutableBorrow
            | CheckedStructuralAccess::Owned
    ) {
        return None;
    }
    let (mut projected, path) =
        super::projected_argument_path(program, state.symbol, statement_index, &place)?;
    if !path
        .iter()
        .all(|segment| matches!(segment, CheckedUnitStructuralPathSegment::Field(_)))
    {
        return None;
    }
    let array_element = loop {
        match program.type_reference_table.type_reference(projected) {
            TypeReferenceNode::Constrained { base_type, .. } => projected = *base_type,
            TypeReferenceNode::FixedArray { element_type, .. } => break *element_type,
            _ => return None,
        }
    };
    (program.normalized_type_identity(array_element) == program.normalized_type_identity(element))
        .then_some((
            CheckedStorageRoot::Parameter {
                index: u32::try_from(parameter_index).ok()?,
            },
            path,
        ))
}

/// The immutable borrowed-view `let` that `symbol` names, declared at an
/// earlier statement of `state`. Parameters and later or mutable locals are
/// not established view places at `statement_index`.
pub(in crate::execution) fn view_local_before<'a>(
    program: &'a TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    symbol: symbols::SymbolHandle,
) -> Option<&'a typed_trees::statement::TableLocalData> {
    let mut locals = program
        .statement_table
        .statements(state.statement_nodes)
        .get(..statement_index)?
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local),
            _ => None,
        });
    let local = locals.next()?;
    (locals.next().is_none()
        && !local.is_mutable
        && local.initial_value.is_valid()
        && view_kind(program, local.type_reference).is_some())
    .then_some(local)
}

/// The borrowed view family of an exact immutable `&[u8]` or `&[T]` type.
pub(in crate::execution) fn view_kind(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<ViewKind> {
    let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    if structural_access_for_type_reference(program, reference)
        != Some(CheckedStructuralAccess::SharedBorrow)
    {
        return None;
    }
    if matches!(
        program.type_reference_table.type_reference(*element_type),
        TypeReferenceNode::Named { .. }
    ) && byte_sequence_carrier(program, reference, &[])
        == Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView)
    {
        return Some(ViewKind::Bytes);
    }
    borrowed_slice_view_element(program, reference, &[]).map(|_| ViewKind::Elements)
}

fn view_identity(
    program: &TypedTrees,
    kind: ViewKind,
    reference: TypeReferenceHandle,
) -> Option<String> {
    match kind {
        ViewKind::Bytes => byte_sequence_type_identity(program, reference, &[], &[]),
        ViewKind::Elements => Some(borrowed_slice_view_type_identity(
            program,
            reference,
            &[],
            &[],
        )),
    }
}
