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
use crate::execution::terminal_unit::CheckedScalarExpressionRole;
use crate::execution::terminal_unit::CheckedStructuralAccess;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentPlan;
use crate::execution::terminal_unit::CheckedUnitStructuralArgumentSourcePlan;
use crate::execution::terminal_unit::CheckedUnitStructuralParameterPlan;
use crate::execution::terminal_unit::Multiplicity;

use crate::execution::terminal_unit::PrimitiveType;
use crate::execution::terminal_unit::StatementNode;
use crate::execution::terminal_unit::TypeReferenceHandle;
use crate::execution::terminal_unit::TypeReferenceNode;
use crate::execution::terminal_unit::types::{
    borrowed_slice_view_element, byte_sequence_carrier, byte_sequence_type_identity,
    structural_access_for_type_reference,
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
    /// Exact ordered projection to the collection or established descriptor.
    /// Storage kind comes from the selected declaration, never path length.
    pub(in crate::execution) collection: Vec<CheckedUnitStructuralPathSegment>,
    /// Exact Terminal type identity of the collection's storage carrier,
    /// enrolled in the checked structural type table.
    pub(in crate::execution) storage_type_identity: typed_trees::type_identity::TypeIdentityId,
    pub(in crate::execution) kind: ViewKind,
    pub(in crate::execution) access: CheckedStructuralAccess,
    pub(in crate::execution) type_identity: typed_trees::type_identity::TypeIdentityId,
}

/// One admitted range with its present endpoints, copied from the unique
/// source-bound `SubsliceStart`/`SubsliceEnd` facts at its site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ViewSubslice {
    pub(in crate::execution) range: ViewRange,
    pub(in crate::execution) expression: ExpressionHandle,
    pub(in crate::execution) start: Option<checked_trees::CheckedCallScalarArgument>,
    pub(in crate::execution) end: Option<checked_trees::CheckedCallScalarArgument>,
    pub(in crate::execution) end_inclusive: bool,
    pub(in crate::execution) site: CheckedSubsliceSite,
}

impl ViewSubslice {
    /// A bodyless signature cannot prove that a result does not retain its
    /// descriptor argument. Preserve fixed projected windows as call-scoped
    /// places instead; dynamic windows and whole views keep their evaluated
    /// descriptor. This presentation depends on the declared retention
    /// boundary, not whether provider selection reached it through a map.
    /// `None` means an admitted fixed projection, not a failed range admission.
    pub(crate) fn call_argument(
        self,
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        bodyless_signature: bool,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        let fixed_projection = bodyless_signature
            && !(self.range.kind == ViewKind::Bytes && self.range.collection.is_empty())
            && matches!(
                validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    self.expression,
                )
                .and_then(|place| place.segments.last().cloned()),
                Some(facts::PlaceSegment::FixedRange { .. })
            );
        (!fixed_projection).then(|| self.argument())
    }

    /// The access-preserving argument plan a call, a return or a view-local
    /// binding carries for this range.
    pub(crate) fn argument(self) -> CheckedUnitStructuralArgumentPlan {
        let ViewSubslice {
            range,
            expression,
            start,
            end,
            end_inclusive,
            site,
        } = self;
        CheckedUnitStructuralArgumentPlan {
            source: match range.kind {
                ViewKind::Bytes => CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                    root: range.root,
                    expression,
                    start,
                    end,
                    end_inclusive,
                    site,
                    storage_type_identity: range.storage_type_identity,
                },
                ViewKind::Elements => {
                    CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                        root: range.root,
                        expression,
                        start,
                        end,
                        end_inclusive,
                        site,
                        storage_type_identity: range.storage_type_identity,
                    }
                }
            },
            path: range.collection,
            type_identity: range.type_identity,
            access: range.access,
        }
    }

    /// The edge transfer for this range: its argument plan, admitted at the
    /// edge's `TransitionArgument` site, so the endpoints are the edge's own.
    pub(in crate::execution) fn transfer(self) -> CheckedStructuralControlTransferSourcePlan {
        CheckedStructuralControlTransferSourcePlan::ViewSubslice(self.argument())
    }
}

/// Admit an exact builtin exclusive range at `site`, vetoing any range whose
/// operator use did not resolve to the builtin meaning.
pub(in crate::execution) fn admit(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    expression: ExpressionHandle,
    statement_index: usize,
    site: CheckedSubsliceSite,
) -> Option<ViewSubslice> {
    admit_resolved(
        program,
        &facts.operators,
        &facts.values.scalar_expressions,
        &facts.values.scalar_computations,
        &facts.borrow,
        machine,
        state,
        parameters,
        target,
        expression,
        statement_index,
        site,
    )
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
    program: &dyn ::validation::CheckingInput,
    operators: &checked_trees::CheckedOperatorFacts,
    expressions: &CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
    borrow: &checked_trees::BorrowFacts,
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
    let subslice = admit_replayed(
        program,
        expressions,
        computations,
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
            borrow,
            machine,
            state,
            statement_index,
            expression,
            subslice.range.access,
        ),
        // Call overlap owns the temporary loan for the invocation.
        CheckedSubsliceSite::CallArgument { .. } | CheckedSubsliceSite::Expression { .. } => true,
        // An incoming borrowed parameter survives the source state, so the
        // edge only narrows it. An owned local fixed array is the same range:
        // the edge forms the window descriptor and lowering keeps that
        // local's storage for the successor. Refusing the local left no way
        // to lend two windows of one occupancy row across a state edge. An
        // owned unrestricted parameter is the same frame storage: the
        // invocation keeps it through every state, so its fixed-array window
        // lends across the edge the same way.
        CheckedSubsliceSite::TransitionArgument { .. } => match subslice.range.root {
            CheckedStorageRoot::Parameter { index } => {
                parameters.get(index as usize).is_some_and(|parameter| {
                    matches!(
                        parameter.access,
                        checked_trees::checked_borrowed_access_pair_pattern!()
                    ) || (parameter.access == CheckedStructuralAccess::Owned
                        && parameter.multiplicity == Multiplicity::Unrestricted
                        && subslice.range.storage == ViewStorage::FixedArray)
                })
            }
            CheckedStorageRoot::OwnedLocal { .. } => {
                subslice.range.storage == ViewStorage::FixedArray
            }
            CheckedStorageRoot::ViewLocal { .. } => false,
        },
    };
    established.then_some(subslice)
}

/// Whether checked borrow admission recorded the `let` at `statement_index`
/// as one exact formation, with matching access, of the fixed-array range `expression`
/// narrows. The view the range establishes is that loan: the ledger, not
/// the authored spelling, is what keeps the array unwritten while it lives.
#[allow(clippy::too_many_arguments)]
fn lends_collection(
    program: &dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
    access: CheckedStructuralAccess,
) -> bool {
    let Some(StatementNode::LocalData(local)) =
        program.statement_table.statement_at(state, statement_index)
    else {
        return false;
    };
    if local.initial_value != expression {
        return false;
    }
    // The loan names the ranged place itself: the backing followed by the
    // range's own index segment, spelled by the same canonicalization that
    // produced it — a whole-collection window keeps its backing spelling
    // rather than a redundant full-extent selector.
    let Some(place) = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        expression,
    ) else {
        return false;
    };

    let Some((_, borrow_state)) = borrow.states.iter().find(|(_, borrow_state)| {
        borrow_state.machine_symbol == machine.symbol && borrow_state.state_symbol == state.symbol
    }) else {
        return false;
    };
    let mut loans = borrow
        .loans
        .span_or_empty(borrow_state.loans)
        .iter()
        .filter(|loan| {
            loan.owner_symbol == local.symbol
                && loan.statement_index == statement_index
                && loan.formation_expression == expression
        });
    let (Some(loan), None) = (loans.next(), loans.next()) else {
        return false;
    };
    let lent = validation::CanonicalPlace {
        root: loan.root,
        segments: borrow.access_segments.span_or_empty(loan.segments).to_vec(),
    };

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
    program: &dyn ::validation::CheckingInput,
    expressions: &CheckedScalarExpressionPlans,
    computations: &checked_trees::CheckedScalarComputationPlans,
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
    let indexed = program.expression_table.indexed_node(operation)?;
    let authored = program.expression_table.range_node(indexed.index)?;
    let statement_ordinal = u32::try_from(statement_index).ok()?;
    let endpoint = |endpoint: ExpressionHandle, role| {
        if !endpoint.is_valid() {
            return Some(None);
        }
        if let Some(root) = computations.root_at(state.symbol, statement_ordinal, role) {
            if !computations.nodes.is_valid(root.root)
                || expressions
                    .expression_at(state.symbol, statement_ordinal, role)
                    .is_some()
            {
                return None;
            }
            let value = computations.nodes.get(root.root);
            return (root.machine == machine.symbol
                && value.authored_root == endpoint
                && value.primitive_type.integer_sign_bits().is_some())
            .then_some(Some(checked_trees::CheckedCallScalarArgument::Computation(
                root.root,
            )));
        }
        let (binding, value) =
            expressions.bound_expression_at(state.symbol, statement_ordinal, role)?;
        (binding.expression == endpoint
            && !binding.destination.is_valid()
            && value
                .primitive_type()
                .is_some_and(|primitive| primitive.integer_sign_bits().is_some()))
        .then(|| {
            Some(checked_trees::CheckedCallScalarArgument::Pure(
                value.clone(),
            ))
        })
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
        end_inclusive: authored.end_inclusive,
        site,
    })
}

/// Range admission without endpoints: the target is an exact readable
/// borrowed view, the authored range is builtin, and the
/// collection either holds an established view of the same kind and identity
/// or names readable array storage with exactly the view's element type.
pub(in crate::execution) fn shape(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    parameters: &[CheckedUnitStructuralParameterPlan],
    target: TypeReferenceHandle,
    kind: ViewKind,
    expression: ExpressionHandle,
    statement_index: usize,
) -> Option<ViewRange> {
    let access = borrowed_view_access(program, target, expression)?;
    let expression = validation::subslice_expression(program, expression)?;
    let indexed = program.expression_table.indexed_node(expression)?;
    let type_identity = view_identity(program, kind, target)?;
    let (root, collection, storage, storage_kind) = collection_storage(
        program,
        machine,
        state,
        parameters,
        target,
        indexed.collection,
        statement_index,
    )?;
    validation::has_builtin_subslice_meaning(program, machine, Some(state), expression).then_some(
        ViewRange {
            root,
            storage: storage_kind,
            collection,
            storage_type_identity: validation::structural_type_identity(program, storage, &[], &[]),
            kind,
            access,
            type_identity,
        },
    )
}

/// Resolve the canonical borrowed collection operand once. Its declared
/// carrier decides whether storage must lend a descriptor or an existing
/// descriptor narrows directly; local/formal origins and member projections
/// do not select another range producer. Source access, exact element type,
/// local loan admission and endpoint formation remain independent obligations.
#[allow(clippy::too_many_arguments)]
fn collection_storage(
    program: &dyn ::validation::CheckingInput,
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
    ViewStorage,
)> {
    let target_view = validation::slice_view_shell(program, target, &[])?;
    let element = target_view.element;
    let mutable = target_view.access == language_core::ReferenceAccess::Mutable;
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        collection,
    )?;

    let symbol = place.root.as_symbol()?;
    let root = if let Some(position) =
        typed_trees::signature::state_parameter_position(program.state_parameters(state), symbol)
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
        let source_access = structural_access_for_type_reference(program, local.type_reference)?;
        if !matches!(
            source_access,
            CheckedStructuralAccess::Owned
                | CheckedStructuralAccess::SharedBorrow
                | CheckedStructuralAccess::MutableBorrow
        ) || mutable && source_access == CheckedStructuralAccess::SharedBorrow
        {
            return None;
        }
        if source_access == CheckedStructuralAccess::Owned {
            CheckedStorageRoot::OwnedLocal { symbol }
        } else {
            CheckedStorageRoot::ViewLocal { symbol }
        }
    };
    // Forming a view lends its selected collection; it does not move an
    // array element. Reuse the borrowed projection's runtime selector and
    // bounds obligation instead of requiring the collection index to fold.
    let (mut projected, path) = super::projected_borrowed_path(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        &place,
    )?;
    if let Some(source_view) = validation::slice_view_shell(program, projected, &[]) {
        if source_view.access == language_core::ReferenceAccess::WriteOnly
            || mutable && source_view.access != language_core::ReferenceAccess::Mutable
            || program.normalized_type_identity(source_view.element)
                != program.normalized_type_identity(element)
        {
            return None;
        }
        return Some((root, path, projected, ViewStorage::Established));
    }
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
        .then_some((root, path, storage, ViewStorage::FixedArray))
}

/// The whole-extent spelling of the same range: `let v: &T = coll` lends
/// `coll`'s entire backing as the `coll[..]` range — no `..` operator is
/// spelled, so its endpoints stay omitted and the backing's own extent
/// names the end.
pub(in crate::execution) fn whole_extent(
    program: &dyn ::validation::CheckingInput,
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
    let access = borrowed_view_access(program, target, expression)?;
    let type_identity = view_identity(program, kind, target)?;
    let (root, collection, storage, storage_kind) = collection_storage(
        program,
        machine,
        state,
        parameters,
        target,
        expression,
        statement_index,
    )?;
    if storage_kind != ViewStorage::FixedArray {
        return None;
    }
    Some((
        ViewSubslice {
            range: ViewRange {
                root,
                storage: ViewStorage::FixedArray,
                collection,
                // The collection's storage carrier, as the structural type
                // table enrolls it.
                storage_type_identity: validation::structural_type_identity(
                    program,
                    storage,
                    &[],
                    &[],
                ),
                kind,
                access,
                type_identity,
            },
            expression,
            start: None,
            end: None,
            end_inclusive: false,
            // No `..` is spelled, so no endpoint facts exist at this site.
            site: CheckedSubsliceSite::LocalBinding,
        },
        storage,
    ))
}

/// Whether `reference` is an immutable byte view under domain constraints
/// only (`&[u8] in Path`): an exact shared `&[u8]` whose reference or slice
/// carries one or more constraint shells, each naming only domains.
fn domain_qualified_byte_view(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> bool {
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
    program: &dyn ::validation::CheckingInput,
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
        Some(checked_trees::checked_borrowed_access_pair_pattern!())
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
    program: &dyn ::validation::CheckingInput,
    kind: ViewKind,
    reference: TypeReferenceHandle,
) -> Option<typed_trees::type_identity::TypeIdentityId> {
    match kind {
        ViewKind::Bytes => byte_sequence_type_identity(program, reference, &[], &[]),
        ViewKind::Elements => Some(validation::structural_type_identity(
            program,
            reference,
            &[],
            &[],
        )),
    }
}

/// A view root's declared access must itself be a borrow, and when the view
/// expression is spelled `&`/`&mut` its authored flavor must match exactly —
/// a mismatched spelling selects no subslice.
fn borrowed_view_access(
    program: &dyn ::validation::CheckingInput,
    target: TypeReferenceHandle,
    expression: ExpressionHandle,
) -> Option<CheckedStructuralAccess> {
    let access = structural_access_for_type_reference(program, target)?;
    if !matches!(
        access,
        checked_trees::checked_borrowed_access_pair_pattern!()
    ) {
        return None;
    }
    if let Some(borrow) = program.expression_table.borrow_expression(expression) {
        let authored = match borrow.access {
            language_core::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
            language_core::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
            language_core::ReferenceAccess::WriteOnly => return None,
        };
        if authored != access {
            return None;
        }
    }
    Some(access)
}
