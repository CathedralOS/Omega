//! Admit one checked collection range with its exact access and endpoints.
//!
//! Local bindings, calls and state transfers carry the same range operation.
//! Source storage and the requested reference determine authority; a type or
//! endpoint equation cannot amplify a shared loan into a mutable one. Local
//! array ranges additionally rejoin the exact checked loan, including access,
//! formation statement and captured projection. Call-scoped ranges retain
//! their ordinary call-argument overlap judgment.
//!
//! Arrays first lend a whole descriptor; established views narrow directly.
//! This is a carrier distinction, not a local-versus-parameter alternative.
//! Present endpoints retain their checked site and evaluation order. Byte and
//! element counts remain separate. Checking proves the authored call's domain
//! requirements; Terminal independently reconstructs subslice-preserving byte
//! predicates from the exact source. The result never asserts a qualification
//! merely because its consumer wants one.
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

/// A descriptor can be narrowed directly; array storage must first lend one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ViewStorage {
    Established,
    FixedArray,
}

/// The exact shape of one admitted range: its established source and the
/// narrowed view's type identity. Endpoints are attached by `admit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::execution) struct ViewRange {
    pub(in crate::execution) root: CheckedStorageRoot,
    storage: ViewStorage,
    /// Projection to the collection. Whole arrays and established views both
    /// have an empty path; storage kind comes from the declared type.
    pub(in crate::execution) collection: Vec<CheckedUnitStructuralPathSegment>,
    /// Exact Terminal type identity of the collection's storage carrier,
    /// enrolled in the checked structural type table.
    pub(in crate::execution) storage_type_identity: String,
    pub(in crate::execution) kind: ViewKind,
    pub(in crate::execution) access: CheckedStructuralAccess,
    pub(in crate::execution) type_identity: String,
}

/// One admitted range with its present endpoints, copied from the unique
/// source-bound `SubsliceStart`/`SubsliceEnd` facts at its site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewSubslice {
    pub(in crate::execution) range: ViewRange,
    pub(in crate::execution) expression: ExpressionHandle,
    pub(in crate::execution) start: Option<CheckedScalarExpression>,
    pub(in crate::execution) end: Option<CheckedScalarExpression>,
}

impl ViewSubslice {
    /// The access-preserving argument plan a call, a return or a view-local
    /// binding carries for this range.
    pub(crate) fn argument(self) -> CheckedUnitStructuralArgumentPlan {
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
                    storage_type_identity: range.storage_type_identity,
                },
                ViewKind::Elements => {
                    CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                        root: range.root,
                        expression,
                        start,
                        end,
                        storage_type_identity: range.storage_type_identity,
                    }
                }
            },
            path: range.collection,
            type_identity: range.type_identity,
            access: range.access,
        }
    }

    /// The edge transfer for this range. Transfers retain only the source and
    /// the authored range; their endpoints are replayed at the edge's site.
    /// A field collection's projection rides along so the edge's emission
    /// establishes the field's whole view exactly as a binding does.
    pub(in crate::execution) fn transfer(&self) -> CheckedStructuralControlTransferSourcePlan {
        match self.range.kind {
            ViewKind::Bytes => CheckedStructuralControlTransferSourcePlan::ByteSequenceSubslice {
                root: self.range.root,
                expression: self.expression,
                storage_type_identity: self.range.storage_type_identity.clone(),
            },
            ViewKind::Elements => CheckedStructuralControlTransferSourcePlan::ElementViewSubslice {
                root: self.range.root,
                path: self.range.collection.clone(),
                expression: self.expression,
                storage_type_identity: self.range.storage_type_identity.clone(),
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
    let subslice = admit_resolved(
        program,
        &facts.operators,
        &facts.values.scalar_expressions,
        machine,
        state,
        parameters,
        target,
        expression,
        statement_index,
        site,
    )?;
    let established = match site {
        _ if subslice.range.storage == ViewStorage::Established => true,
        CheckedSubsliceSite::LocalBinding => lends_collection(
            program,
            facts,
            machine,
            state,
            statement_index,
            expression,
            subslice.range.access,
        ),
        // Call overlap owns the temporary loan for the invocation.
        CheckedSubsliceSite::CallArgument { .. } | CheckedSubsliceSite::Expression { .. } => true,
        // An incoming loan's backing survives the source state. A local or
        // owned array instead needs retained backing transport on the edge;
        // constructing a descriptor does not supply that lifetime evidence.
        CheckedSubsliceSite::TransitionArgument { .. } => match subslice.range.root {
            CheckedStorageRoot::Parameter { index } => {
                parameters.get(index as usize).is_some_and(|parameter| {
                    matches!(
                        parameter.access,
                        CheckedStructuralAccess::SharedBorrow
                            | CheckedStructuralAccess::MutableBorrow
                    )
                })
            }
            _ => false,
        },
    };
    established.then_some(subslice)
}

/// Whether `argument` produces a range descriptor. Its path selects the
/// backing collection; it does not transfer an owned subtree to the callee.
pub(crate) fn is_view_subslice(argument: &CheckedUnitStructuralArgumentPlan) -> bool {
    matches!(
        argument.source,
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice { .. }
            | CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice { .. }
    )
}

/// Shared admission also serves computation construction, before CheckFacts
/// is assembled. Operator meaning and source-bound endpoint facts are both
/// required; a range with omitted endpoints still needs the operator veto.
pub(crate) fn admit_resolved(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    expressions: &CheckedScalarExpressionPlans,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    expression: ExpressionHandle,
    statement_index: usize,
    site: CheckedSubsliceSite,
) -> Option<ViewSubslice> {
    let operation = validation::subslice_expression(program, expression)?;
    let spelling = language_core::OperatorSpelling::Range;
    if operators.expression_use(operation).is_some_and(|selected| {
        selected.spelling != spelling
            || selected.selected_operator_symbol.is_valid()
            || selected.candidate_count != 0
            || !matches!(
                selected.status,
                checked_trees::CheckedOperatorResolutionStatus::Missing
                    | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            )
    }) {
        return None;
    }
    admit_replayed(
        program,
        expressions,
        machine,
        state,
        parameters,
        target,
        expression,
        statement_index,
        site,
    )
}

/// Whether checked borrow admission recorded the `let` at `statement_index`
/// as the sole loan, with matching access, of the fixed-array range `expression`
/// narrows. The view the range establishes is that loan: the ledger, not
/// the authored spelling, is what keeps the array unwritten while it lives.
#[allow(clippy::too_many_arguments)]
fn lends_collection(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    access: CheckedStructuralAccess,
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
    // The loan names the ranged place itself: the backing followed by the
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
        && loan.kind
            == match access {
                CheckedStructuralAccess::SharedBorrow => checked_trees::BorrowAccessKind::Read,
                CheckedStructuralAccess::MutableBorrow => checked_trees::BorrowAccessKind::Mutable,
                _ => return false,
            }
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
    let operation = validation::subslice_expression(program, expression)?;
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(operation) else {
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

/// Range admission without endpoints: the target is an exact readable
/// borrowed view, the authored range is exclusive and builtin, and the
/// collection either holds an established view of the same kind and identity
/// or names readable array storage with exactly the view's element type.
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
    shape_of_kind(
        program,
        machine,
        state,
        parameters,
        target,
        kind,
        expression,
        statement_index,
    )
}

/// `shape` for a target of the view family `kind`.
#[allow(clippy::too_many_arguments)]
fn shape_of_kind(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    kind: ViewKind,
    expression: ExpressionHandle,
    statement_index: usize,
) -> Option<ViewRange> {
    let access = structural_access_for_type_reference(program, target)?;
    if !matches!(
        access,
        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
    ) {
        return None;
    }
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) {
        let authored = match borrow.access {
            language_core::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
            language_core::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
            language_core::ReferenceAccess::WriteOnly => return None,
        };
        if authored != access {
            return None;
        }
    }
    let expression = validation::subslice_expression(program, expression)?;
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
    if let Some((root, collection, storage)) = array_collection(
        program,
        machine,
        state,
        parameters,
        target,
        indexed.collection,
        statement_index,
    ) {
        return validation::has_builtin_subslice_meaning(program, machine, Some(state), expression)
            .then_some(ViewRange {
                root,
                storage: ViewStorage::FixedArray,
                collection,
                storage_type_identity: collection_storage_identity(program, storage),
                kind,
                access,
                type_identity,
            });
    }
    let ExpressionNode::Name(path) = program.expression_table.expression(indexed.collection) else {
        return None;
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
        let parameter_index = parameters
            .iter()
            .position(|parameter| parameter.position as usize == position)?;
        let parameter = &parameters[parameter_index];
        let source_kind = view_kind(program, authored[position].type_reference);
        if parameter.access != access
            || parameter.multiplicity != Multiplicity::Unrestricted
            || !parameter.projected_qualifications.is_empty()
            || parameter.type_identity != type_identity
            || source_kind != Some(kind)
        {
            return None;
        }
        CheckedStorageRoot::Parameter {
            index: u32::try_from(parameter_index).ok()?,
        }
    } else {
        let local = view_local_before(program, state, statement_index, path.symbol)?;
        if structural_access_for_type_reference(program, local.type_reference) != Some(access)
            || view_kind(program, local.type_reference) != Some(kind)
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
        storage: ViewStorage::Established,
        collection: Vec::new(),
        // An established view's own type is its collection's storage
        // carrier: the range narrows a descriptor, not array backing.
        storage_type_identity: type_identity.clone(),
        kind,
        access,
        type_identity,
    })
}

/// The exact Terminal identity of the collection's storage carrier, spelled
/// the way the checked structural type table enrolls the declared type: a
/// byte carrier keeps its reference shell (`ref-mut(array(u8,3))`), while
/// ordinary collections enroll under the peeled carrier spelling — the same
/// carrier spelling lowering's `structural_carrier_type` computes.
fn collection_storage_identity(program: &TypedTrees, storage: TypeReferenceHandle) -> String {
    if byte_sequence_carrier(program, storage, &[]).is_some()
        && let Some(identity) = byte_sequence_type_identity(program, storage, &[], &[])
    {
        return identity;
    }
    let mut peeled = storage;
    loop {
        match program.type_reference_table.type_reference(peeled) {
            TypeReferenceNode::Reference { referee, .. }
            | TypeReferenceNode::Constrained {
                base_type: referee, ..
            } => peeled = *referee,
            _ => break,
        }
    }
    program.normalized_type_identity(peeled).into_string()
}

/// Resolve readable fixed-array storage, including a whole parameter or an
/// earlier owned local and their record-field projections. The array's
/// elements must be exactly the target view's elements. The existing
/// collection-view establishment also lends exact `u8` arrays as byte views;
/// it does not reinterpret other element types or copy the backing. Both
/// families retain the local's exact checked loan and endpoint evidence.
#[allow(clippy::too_many_arguments)]
fn array_collection(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    collection: ExpressionHandle,
    statement_index: usize,
) -> Option<(
    CheckedStorageRoot,
    Vec<CheckedUnitStructuralPathSegment>,
    TypeReferenceHandle,
)> {
    let target_view = validation::slice_view_shell(program, target, &[])?;
    let element = target_view.element;
    let mutable = target_view.access == language_core::ReferenceAccess::Mutable;
    let mut place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        collection,
    )?;
    crate::flow::normalize_attached_place_root(program, machine.symbol, state.symbol, &mut place);
    let facts::PlaceRoot::Symbol(symbol) = place.root else {
        return None;
    };
    let root = if let Some(position) = program
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == symbol)
    {
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
        if mutable && parameters[parameter_index].access == CheckedStructuralAccess::SharedBorrow {
            return None;
        }
        CheckedStorageRoot::Parameter {
            index: u32::try_from(parameter_index).ok()?,
        }
    } else {
        let (_, local) = program
            .state_local_declaration(state, symbol)
            .filter(|&(ordinal, _)| ordinal < statement_index)?;
        // The earlier declaration owns its storage whether its establishment
        // was explicit or zero-initialized. Body sequencing publishes that
        // backing; an initializer expression is not storage authority.
        if (mutable && !local.is_mutable)
            || structural_access_for_type_reference(program, local.type_reference)
                != Some(CheckedStructuralAccess::Owned)
        {
            return None;
        }
        CheckedStorageRoot::OwnedLocal { symbol }
    };
    let (mut projected, path) =
        super::projected_argument_path(program, state.symbol, statement_index, &place)?;
    // The declared reference shell stays on the storage handle: byte
    // carriers enroll in the checked type table under that shell's spelling
    // (`ref-mut(array(u8,3))` for a `&mut [u8;3]` parameter), so peeling
    // here would name an identity no roster carries.
    let storage = projected;
    // Parameter access describes the one root reference already lowered to
    // its referent. The shared projection resolver owns field/index typing;
    // the selected collection must itself be an array. A nested reference
    // cannot silently lose the explicit Referent crossing it requires.
    if path.is_empty()
        && matches!(root, CheckedStorageRoot::Parameter { .. })
        && let Some(reference) = validation::reference_shell(program, projected)
    {
        if !matches!(
            reference.access,
            language_semantics::ReferenceAccess::Shared
                | language_semantics::ReferenceAccess::Mutable
        ) {
            return None;
        }
        projected = reference.referee;
    }
    let array_element = loop {
        match program.type_reference_table.type_reference(projected) {
            TypeReferenceNode::Constrained { base_type, .. } => projected = *base_type,
            TypeReferenceNode::FixedArray { element_type, .. } => break *element_type,
            _ => return None,
        }
    };
    (program.normalized_type_identity(array_element) == program.normalized_type_identity(element))
        .then_some((root, path, storage))
}

/// The whole-extent spelling of the same range: `let v: &T = coll` lends
/// `coll`'s entire backing as the `coll[..]` range — no `..` operator is
/// spelled, so its endpoints stay omitted and the backing's own extent
/// names the end.
pub(in crate::execution) fn whole_extent(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    expression: ExpressionHandle,
    statement_index: usize,
) -> Option<(ViewSubslice, TypeReferenceHandle)> {
    let kind = view_kind(program, target).or_else(|| {
        borrowed_slice_view_element(program, target, &[]).map(|_| ViewKind::Elements)
    })?;
    let access = structural_access_for_type_reference(program, target)?;
    if !matches!(
        access,
        CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
    ) {
        return None;
    }
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) {
        let authored = match borrow.access {
            language_core::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
            language_core::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
            language_core::ReferenceAccess::WriteOnly => return None,
        };
        if authored != access {
            return None;
        }
    }
    let type_identity = match kind {
        ViewKind::Bytes => byte_sequence_type_identity(program, target, &[], &[])?,
        ViewKind::Elements => borrowed_slice_view_type_identity(program, target, &[], &[]),
    };
    let (root, collection, storage) = array_collection(
        program,
        machine,
        state,
        parameters,
        target,
        expression,
        statement_index,
    )?;
    Some((
        ViewSubslice {
            range: ViewRange {
                root,
                storage: ViewStorage::FixedArray,
                collection,
                storage_type_identity: collection_storage_identity(program, storage),
                kind,
                access,
                type_identity,
            },
            expression,
            start: None,
            end: None,
        },
        storage,
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
    let (_, local) = program
        .state_local_declaration(state, symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index)?;
    (!local.is_mutable
        && local.initial_value.is_valid()
        && view_kind(program, local.type_reference).is_some())
    .then_some(local)
}

/// Whether `reference` is an immutable byte view under domain constraints
/// only (`&[u8] in Path`): an exact shared `&[u8]` whose reference or slice
/// carries one or more constraint shells, each naming only domains.
fn domain_qualified_byte_view(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    let mut constrained = false;
    let mut peel = |mut reference: TypeReferenceHandle| {
        while let TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = program.type_reference_table.type_reference(reference)
        {
            if !program
                .type_reference_table
                .constraints(*constraints)
                .iter()
                .all(|constraint| {
                    matches!(
                        constraint,
                        typed_trees::types::TypeConstraintNode::Domain(_)
                    )
                })
            {
                return None;
            }
            constrained = true;
            reference = *base_type;
        }
        Some(reference)
    };
    let Some(TypeReferenceNode::Reference {
        referee,
        access: language_semantics::ReferenceAccess::Shared,
        ..
    }) = peel(reference).map(|outer| program.type_reference_table.type_reference(outer))
    else {
        return false;
    };
    let Some(TypeReferenceNode::Slice { element_type }) =
        peel(*referee).map(|inner| program.type_reference_table.type_reference(inner))
    else {
        return false;
    };
    constrained
        && matches!(
            program.type_reference_table.type_reference(*element_type),
            TypeReferenceNode::Named { .. }
        )
        && program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
}

/// The borrowed view family of an exact shared or mutable slice type.
pub(in crate::execution) fn view_kind(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<ViewKind> {
    if domain_qualified_byte_view(program, reference) {
        return Some(ViewKind::Bytes);
    }
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
    if !matches!(
        structural_access_for_type_reference(program, reference),
        Some(CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow)
    ) {
        return None;
    }
    if matches!(
        program.type_reference_table.type_reference(*element_type),
        TypeReferenceNode::Named { .. }
    ) && checked_trees::is_borrowed_view(byte_sequence_carrier(program, reference, &[]))
    {
        return Some(ViewKind::Bytes);
    }
    borrowed_slice_view_element(program, reference, &[]).map(|_| ViewKind::Elements)
}

/// The type identity of a view of `kind`, as the view's own family spells it.
pub(in crate::execution) fn view_identity(
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

#[cfg(test)]
mod tests;
