/*
    Executable Unit plans from checked ownership, control and call facts.

    We build executable source plans from already-checked ownership, control, and
    call facts. A typed body can be legal without fitting a Terminal producer yet;
    these catalogs describe implementation coverage, not additional language rules.
    The lowerer must receive a complete plan, never a partly recognized body.

    Start at `build_checked_unit_effect_plans`, the checking
    stage's entry into this module (`build_checked_unit_effect_plans` is its
    test-only form without call frames). It also publishes the partial-affine
    roster: an ordinary body whose return edge owes a projected owned
    parameter's residual complement is the same candidate kept beside those
    rows, and a callee like any other. Nominal caller metadata is built after
    body publication; it cannot replace that body's cleanup coordinates.
    We first collect boundary
    signatures and ordinary single-state candidates through control.rs. A
    single-state body has one producer: control/statement_sequence.rs walks the
    authored statements in order and emits each one's operations — stores,
    borrowed-window moves and repairs, selected operator and FMA applications,
    structural and scalar bindings, calls and completion — after the prefix of
    trivial affine locals that calls/affine_locals.rs establishes. Machine input
    storage arrives through explicit parameters. We build composed candidates through
    composed_control/assembly.rs, and reconcile composed calls against the completed
    signatures. The composed builders include specialized control shapes and the
    general state_graph.rs route; their current coverage differs, so a failed
    candidate is not permission to omit its unsupported statements.

    Local construction is only the first gate. Ordinary and composed entries share
    one immutable availability roster. We check operations once, then propagate
    unavailable targets through reverse call edges. If A calls B and B calls an
    unavailable C, rejecting B also rejects A without rechecking A's body. Checking
    only A's own statements, or
    pruning each catalog independently, would leave a seemingly complete root with
    an unlowerable transitive call. Scalar calls can borrow an ordinary scalar-result
    body from this same roster; their argument, claim and contract checks still use
    the exact scalar signature. Boundary and structural-result calls retain their
    own availability checks. Only after
    pruning do we retain the referenced structural types and their transitive shapes.

    Every checked-body machine left without a plan carries one gap
    (`plan_gaps.rs`): the construct its builder could not lower, the cleanup a
    state owed, or the call whose callee has no body, at its authored span.
    Lowering and entry establishment report the gap of a body they demand,
    following callee gaps to the construct that removed the chain. The
    builders still return `Option`; the construction trace names the phase
    and statement where they stopped.

    Partial cleanup retains its residual body owner. Nominal actions remain
    distinct from trivial discards in the shared normal-exit transaction;
    their caller metadata owns no second executable body or disposal schedule.
    Replacement joins those same selected dispositions into the statement's
    operation stream: evaluate the RHS, dispose the old occurrence, then store
    or rebind. `cleanup/replacements.rs` completes the death marker from source
    custody once the hook catalog exists; the lowerer never recognizes an
    initializer/call/store pattern to manufacture replacement semantics.
    An owned local changes its current value home. Borrowed storage instead
    retains its lender's home and exact restoration obligation. Treating these
    as one extraction-and-repair operation invents debt for ordinary locals.

    # Executable companions

    Existing source examples are the executable companions to this explanation:
    src/tests/flow/terminal_unit/calls.rs checks mixed case payload/view edges and
    affine return disposal. In the sibling checked-trees-to-lowered-psi crate,
    src/tests/composed_unit_transitive_internal_calls.rs follows Root::enter through
    its helpers and rejects substituted targets and missing plans;
    src/tests/byte_write_loop.rs exercises borrowed buffers and scalar-case returns.
    Read those controls when widening a route. The owning source/Terminal coverage
    contract is ../../terminal-production/README.md relative to this crate.

    # Erased parameters

    An `[erased]` binding occurrence
    (contracts.md#explicit-erased-bindings) stays in the typed signature and in
    proof identity but owns no ABI position. Every signature builder here
    (calls/signatures.rs, calls/call_operations.rs, ../scalar/mod.rs)
    skips it, and every caller-side argument producer
    (values/scalar/computations.rs, values/scalar/call_lowering.rs,
    flow/transfers/scalar_values/calls.rs) numbers its dense argument ordinals over
    the retained positions only, so the two sides agree without a shared table.
    `position` and `source_position` stay the authored typed-signature index and
    therefore become sparse after a strip: the Terminal consumer rejoins each
    retained parameter to `state_parameters(state)[position]` for its type, symbol
    and custody, so renumbering densely would bind the wrong source parameter.
    `strips_erased_parameter` decides which bindings may be stripped at all.
    The dense scalar value namespace that `CheckedScalarExpression::Parameter`
    indexes is the same partition: `values::scalar::occupies_scalar_position`
    excludes erased bindings there, so a body reading a retained parameter after
    an erased one gets the shifted position and a runtime read of the erased one
    finds no position. The consumer, checked-trees-to-lowered-psi, reconstructs
    every one of these partitions independently from the typed relevance
    (unit/attached_unit/parameters.rs, expression_preparation/qualifications.rs,
    scalar_lowering/computations/calls.rs)
    and never trusts the plan's counts. What remains open is a contract that
    names an erased binding (`requires n < bound`): Terminal contract
    propositions have no proof-only value for it, so
    pass/relevance/erased_parameter_proof_only stays a checked-only canary while
    its two RUN siblings execute natively (canary_suite layouts_and_pending
    erased_parameter_* tests).
*/

use crate::execution::terminal_unit::cleanup::build_nominal_affine_unit_cleanup_machine;
use crate::execution::terminal_unit::control::{
    LocalConstructionTrace, build_boundary_machine, build_checked_machine_residual_parts,
    build_static_boundary_requirements,
};
use crate::execution::terminal_unit::plan_gaps::{GapSite, plan_gap};
use crate::execution::terminal_unit::types::{ChangeLog, ShapeCollector};
use std::collections::{BTreeMap, BTreeSet};

use checked_trees::{
    CheckFacts, CheckedAffineConstructionElementPlan, CheckedBoundaryMachinePlan,
    CheckedBoundaryMachineResultPlan, CheckedClosedSumCaseOutcomePlan, CheckedClosedSumCasePlan,
    CheckedClosedSumPayloadTransferPlan, CheckedComposedUnitControlMachinePlan,
    CheckedComposedUnitControlStatePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedGuardedJumpPlan, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedNominalAffineUnitCleanupPlans, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedPartialAffineUnitCleanupPlans, CheckedScalarExpression, CheckedScalarExpressionRole,
    CheckedStructuralAccess, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralResultPlan,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarParameterPlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitCallCoordinate, CheckedUnitCallResult,
    CheckedUnitCallRoute, CheckedUnitClaimTransferPlan, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans as EffectPlans,
    CheckedUnitEntryClaimPlan, CheckedUnitNominalAffineCallerRequirementPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitPlanGapReason as GapReason, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralDomainPlan, CheckedUnitStructuralDomainRequirementPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralResultBindingPlan, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, ContractProofFactKind, ContractProofFactOwner,
};
use diagnostics::Diagnostic;
use language_semantics::{
    CarryPolicy, MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, SemanticDomainId,
};
use symbols::SymbolHandle;
use typed_trees::{
    data::{DataMember, DataShapeKind},
    domain::ProofFact,
    expression::ExpressionNode,
    signature::{SignatureContractKind, StateParameter},
    statement::{StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode},
    types::{PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode},
};

mod atomic_operations {
    /*
        Atomic events: one `AccessPlan` per source atomic operation.

        The parser's carrier (`validation::atomic_assignment_carrier`) spells a
        fetch, swap, or compare-exchange as two statements — a result placeholder
        `let prior: T = 0;` and the carrier assignment `place = Atomic { model }` —
        and a store as the carrier assignment alone. Once instead establishes a
        complete structural local. The sequence plans these events as follows:

        - the placeholder local reserves the next dense scalar binding and plans
          nothing ([`result_placeholder`]); its literal initializer is not a value
          the program observes;
        - the carrier assignment plans the event ([`writing_event`]), binding the
          reserved ordinal to the instruction-observed prior;
        - loads are ordered scalar computations, whether inline or local-bound.

        The place retains its actual atomic home and stored-reference crossings.
        Whole cells have an empty projection; record cells retain their real member
        identity. Static coordinates select one exact declared atomic endpoint, so
        shared access authorizes the event without granting ordinary shared mutation.
        Independent Terminal verification checks that identity.
        Operands retain `AtomicOperand` roles, either as pure scalar rows or ordinary
        computation roots. The latter own their nested calls and ordered effects;
        neither the generated update model nor a separate statement call executes.
        Both forms bind the decoder's exact authored expression and cell value type.
    */
    use super::calls::projected_observed_path;
    use super::receiver_aliases::aliases;
    use super::structural_scalar_store::scalar_custody_is_exact;
    use super::types::ShapeCollector;
    use super::types::machine_binders;
    use super::{
        CheckFacts, CheckedScalarExpressionRole, CheckedUnitScalarResultBindingPlan,
        CheckedUnitStructuralPathSegment as PathSegment, PrimitiveType,
    };
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::published::places::place_at;
    use checked_trees::{
        CheckedAtomicAccessPlan as AccessPlan, CheckedAtomicEvent,
        CheckedAtomicReadModifyWrite as ModifyWrite, CheckedCallScalarArgument as ScalarArgument,
        CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan,
    };
    use validation::AtomicAccessOperation;

    /*
        The reserved result of a writing event whose carrier is the next
        statement: `local` is the placeholder when that carrier's result names it.
    */
    pub(super) fn result_placeholder(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        local: &typed_trees::statement::TableLocalData,
    ) -> bool {
        program
            .state_local_declaration(state, local.symbol)
            .is_some_and(|(ordinal, _)| ordinal == statement_index)
            && validation::atomic_result_placeholder(program, state, local).is_some()
    }

    /*
        The writing event the carrier assignment at `statement_index` denotes.
        `reserved` is the placeholder binding an observing event binds; a store
        takes none.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn writing_event(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: u32,
        assignment: &typed_trees::statement::TableAssignment,
        reserved: Option<CheckedUnitScalarResultBindingPlan>,
        trace: &LocalConstructionTrace,
        shapes: &mut ShapeCollector<'_>,
    ) -> Option<AccessPlan> {
        let carrier = validation::atomic_assignment_carrier(program, assignment)?;
        trace.phase("atomic access: writing place");
        let place = place(
            program,
            facts,
            machine,
            state,
            statement_index,
            carrier.place,
            trace,
            shapes,
        )?;
        let primitive_type = place.primitive_type;
        trace.phase("atomic access: operands");
        let operands = carrier
            .operands
            .iter()
            .enumerate()
            .map(|(ordinal, operand)| {
                operand_row(
                    program,
                    facts,
                    machine.symbol,
                    state,
                    statement_index,
                    u32::try_from(ordinal).ok()?,
                    *operand,
                    primitive_type,
                )
            })
            .collect::<Option<Vec<_>>>()?;
        let fetch = |operation, ordering| {
            let [operand] = <[ScalarArgument; 1]>::try_from(operands.clone()).ok()?;
            Some(CheckedAtomicEvent::ReadModifyWrite {
                operation,
                ordering,
                operand,
            })
        };
        let event = match carrier.operation {
            AtomicAccessOperation::Store(ordering) => {
                let [value] = <[ScalarArgument; 1]>::try_from(operands.clone()).ok()?;
                CheckedAtomicEvent::Store { ordering, value }
            }
            AtomicAccessOperation::Swap(ordering) => {
                let [value] = <[ScalarArgument; 1]>::try_from(operands.clone()).ok()?;
                CheckedAtomicEvent::Swap { ordering, value }
            }
            AtomicAccessOperation::FetchAdd(ordering) => fetch(ModifyWrite::FetchAdd, ordering)?,
            AtomicAccessOperation::FetchSub(ordering) => fetch(ModifyWrite::FetchSub, ordering)?,
            AtomicAccessOperation::FetchAnd(ordering) => fetch(ModifyWrite::FetchAnd, ordering)?,
            AtomicAccessOperation::FetchOr(ordering) => fetch(ModifyWrite::FetchOr, ordering)?,
            AtomicAccessOperation::FetchXor(ordering) => fetch(ModifyWrite::FetchXor, ordering)?,
            AtomicAccessOperation::CompareExchange { success, failure } => {
                let [expected, replacement] =
                    <[ScalarArgument; 2]>::try_from(operands.clone()).ok()?;
                CheckedAtomicEvent::CompareExchange {
                    success,
                    failure,
                    expected,
                    replacement,
                }
            }
            AtomicAccessOperation::Load(_)
            | AtomicAccessOperation::CompareExchangeOnce { .. }
            | AtomicAccessOperation::TryExchangeOnce { .. } => {
                return None;
            }
        };
        trace.phase("atomic access: result binding");
        let result = match (event.observes_resident(), reserved) {
            (true, Some(result))
                if result.primitive_type == primitive_type
                    && result.statement_index.checked_add(1) == Some(statement_index) =>
            {
                Some(result)
            }
            (false, None) => None,
            _ => return None,
        };
        Some(AccessPlan {
            statement_index,
            resident_range_decision: facts.proof.integer_assignment_at(
                machine.symbol,
                state.symbol,
                statement_index as usize,
                assignment.target,
                assignment.value,
            ),
            receiver: place.receiver,
            receiver_root_type_identity: place.receiver_root_type_identity,
            field_case: place.field_case,
            field_identity: place.field_identity,
            primitive_type,
            event,
            result,
            structural_result: None,
            discard_result_on_return: false,
        })
    }

    /*
        Plan a whole-outcome local using the same exact-cell and operand joins as
        scalar atomic events. Its receiving type is independently checked against
        the sealed core family before any result storage is published.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn once_event(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: u32,
        local: &typed_trees::statement::TableLocalData,
        result: checked_trees::CheckedUnitStructuralResultBindingPlan,
        trace: &LocalConstructionTrace,
        shapes: &mut ShapeCollector<'_>,
    ) -> Option<AccessPlan> {
        let carrier = validation::atomic_once_carrier(program, local.initial_value)?;
        let custody = carrier.operation.single_attempt_custody()?;
        let language_core::atomic::AtomicOrderingPlan::CompareExchangeOnce { success, failure } =
            carrier.operation.ordering_plan()
        else {
            return None;
        };
        let place = place(
            program,
            facts,
            machine,
            state,
            statement_index,
            carrier.place,
            trace,
            shapes,
        )?;
        if !validation::atomic_once_result_type(
            program,
            local.type_reference,
            place.primitive_type,
            custody,
        ) {
            return None;
        }
        let [expected, replacement] =
            <[typed_trees::expression::ExpressionHandle; 2]>::try_from(carrier.operands).ok()?;
        let expected = operand_row(
            program,
            facts,
            machine.symbol,
            state,
            statement_index,
            0,
            expected,
            place.primitive_type,
        )?;
        let replacement = operand_row(
            program,
            facts,
            machine.symbol,
            state,
            statement_index,
            1,
            replacement,
            place.primitive_type,
        )?;
        Some(AccessPlan {
            statement_index,
            resident_range_decision: facts.proof.integer_assignment_at(
                machine.symbol,
                state.symbol,
                statement_index as usize,
                carrier.place,
                local.initial_value,
            ),
            receiver: place.receiver,
            receiver_root_type_identity: place.receiver_root_type_identity,
            field_case: place.field_case,
            field_identity: place.field_identity,
            primitive_type: place.primitive_type,
            event: CheckedAtomicEvent::CompareExchangeOnce {
                custody,
                success,
                failure,
                expected,
                replacement,
            },
            result: None,
            discard_result_on_return: result.multiplicity
                == language_semantics::Multiplicity::Affine,
            structural_result: Some(result),
        })
    }

    /*
        The retained `AtomicOperand` source for one authored operand, exact for the
        leaf carrier. The event consumes the ordinary computation when it owns
        calls; otherwise the bound pure row supplies the same value coordinate.
    */
    fn operand_row(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: symbols::SymbolHandle,
        state: &typed_trees::state::State,
        statement_index: u32,
        operand_ordinal: u32,
        operand: typed_trees::expression::ExpressionHandle,
        primitive_type: PrimitiveType,
    ) -> Option<ScalarArgument> {
        let role = CheckedScalarExpressionRole::AtomicOperand { operand_ordinal };
        let computations = &facts.values.scalar_computations;
        let mut roots = computations.roots_for_statement_role(state.symbol, statement_index, role);
        if let Some(root) = roots.next() {
            if roots.next().is_some()
                || !computations.is_authored_root(root, machine, operand, primitive_type)
                || facts
                    .values
                    .scalar_expressions
                    .expression_at(state.symbol, statement_index, role)
                    .is_some()
            {
                return None;
            }
            return Some(ScalarArgument::Computation(root.root));
        }
        let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_index,
            role,
        )?;
        (binding.expression == operand
            && crate::values::scalar_expression_type(value) == Some(primitive_type)
            && scalar_custody_is_exact(program, facts, state, binding, value, primitive_type))
        .then(|| ScalarArgument::Pure(value.clone()))
    }

    /* The atomic field `expression` names at `statement_index`. */
    struct AtomicPlace {
        receiver_root_type_identity: typed_trees::type_identity::TypeIdentityId,
        receiver: CheckedUnitStructuralArgumentPlan,
        field_case: Option<checked_trees::CheckedMemberIdentity>,
        field_identity: checked_trees::CheckedMemberIdentity,
        primitive_type: PrimitiveType,
    }

    /*
        The atomic field `expression` names at `statement_index`: the shared
        receiver of its containing record, the field, and its scalar carrier.
    */
    #[allow(clippy::too_many_arguments)]
    fn place(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: u32,
        expression: typed_trees::expression::ExpressionHandle,
        trace: &LocalConstructionTrace,
        shapes: &mut ShapeCollector<'_>,
    ) -> Option<AtomicPlace> {
        trace.phase("atomic access: place: atomic storage");
        if !validation::place_is_atomic_storage(program, machine, Some(state), expression) {
            return None;
        }
        let primitive_type =
            validation::declared_place_type_raw(program, machine, Some(state), expression)
                .and_then(|declared| program.primitive_type_reference(declared))?;
        let ordinal = usize::try_from(statement_index).ok()?;
        let canonical = place_at(program, state.symbol, ordinal, expression)?;
        let symbol = canonical.root.as_symbol()?;
        let (symbol, segments) = match aliases(program, facts, machine, state)
            .unwrap_or_default()
            .iter()
            .find(|alias| alias.owner == symbol)
        {
            Some(alias) => {
                let mut segments = alias.segments.clone();
                segments.extend_from_slice(&canonical.segments);
                (alias.root, segments)
            }
            None => (symbol, canonical.segments.clone()),
        };
        trace.phase("atomic access: place: static field path");
        if !validation::place_has_builtin_coordinates(program, machine, Some(state), expression)
            || !segments.iter().all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. }
                        | facts::PlaceSegment::FixedIndex { .. }
                        | facts::PlaceSegment::Case { .. }
                )
            })
        {
            return None;
        }
        let mut carrier = validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(symbol),
            segments,
        };
        let field_identity = if let Some(facts::PlaceSegment::Field { symbol: field }) =
            carrier.segments.last().copied()
        {
            carrier.segments.pop();
            crate::execution::terminal_unit::types::terminal_field_identity(program, field)?
        } else {
            Default::default()
        };
        let field_case = if field_identity.is_valid()
            && let Some(facts::PlaceSegment::Case { variant }) = carrier.segments.last()
        {
            let mut cases = program
                .data_definitions()
                .iter()
                .flat_map(|owner| program.data_members(owner))
                .filter_map(|member| match member {
                    typed_trees::data::DataMember::Variant(case) if case.symbol == *variant => {
                        Some(case)
                    }
                    _ => None,
                });
            let case = cases.next()?;
            if cases.next().is_some() {
                return None;
            }
            let identity =
                checked_trees::CheckedMemberIdentity::of_member(case.identity, case.name.clone());
            carrier.segments.pop();
            Some(identity)
        } else {
            None
        };

        let binders = machine_binders(program, machine);
        let mut root_identity = None;
        for position in 0..=carrier.segments.len() {
            let reference = validation::canonical_place_type_reference(
                program,
                state.symbol,
                ordinal,
                &validation::CanonicalPlace {
                    root: carrier.root,
                    segments: carrier.segments[..position].to_vec(),
                },
            )?;
            let identity =
                if position == 0 && program.state_local_declaration(state, symbol).is_some() {
                    shapes.add_stored_type(reference, &binders, &[])?
                } else {
                    shapes.add_type(reference, &binders, &[])?
                };
            if position == 0 {
                root_identity = Some(identity);
            }
        }
        let receiver_root_type_identity = root_identity?;
        trace.phase("atomic access: place: shared receiver");
        let ProjectedArgumentPath {
            storage_type: reference,
            path,
            ..
        } = projected_observed_path(
            program,
            machine.symbol,
            state.symbol,
            ordinal,
            &carrier,
            &|_| None,
        )?;
        if path.iter().any(|segment| {
            matches!(
                segment,
                PathSegment::RuntimeIndex(_)
                    | PathSegment::ComputedIndex(_)
                    | PathSegment::FixedRange { .. }
            )
        }) {
            return None;
        }
        let receiver = CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
            path,
            type_identity: program.normalized_type_identity(reference),
            access: CheckedStructuralAccess::SharedBorrow,
        };
        Some(AtomicPlace {
            receiver,
            receiver_root_type_identity,
            field_case,
            field_identity,
            primitive_type,
        })
    }
}
mod borrowed_windows {
    /*
        Detach structural children and reseat their exact storage. A
        `let local: T = root.field...;` move creates independent owned value custody;
        an owned store to that exact place closes its hole. Only extraction through
        exclusive borrowed storage owes restoration at every normal exit. An owned
        root may remain partially moved under its checked residual disposal plan.
        The replacement may be the removed value, a
        construction, or a call result: its custody belongs to the ordinary result
        bindings, not to the hole. An occupied destination displaces conserved old
        custody before installation; a freely discardable replacement needs no old-value
        observation. An already-open hole must not be read or disposed again. The lowered
        emitter feeds these operations through `BorrowedWindowLedger`, and independent
        verification reconstructs both restoration debt and the values' custody.
        An empty path selects the exclusive parameter's whole referent. It shares
        extraction and repair sequencing with a field; a separate whole-root
        admission mode would make the value's custody depend on its spelling.

        A structural assignment captures its destination before the RHS's first
        producer. The same retained assignment selectors describe construction,
        call, and already-owned replacement values. Cleanup still selects the
        original storage's death occurrence; capture changes when coordinates are
        read, not permission, restoration debt, or the old value's custody.
        Source admission and this planner share stable spatial comparison. The
        resulting store retains the opening extraction's result coordinate, so
        lowering never has to rediscover equality between value-read operands and
        pre-RHS assignment captures. Mutable selector spelling alone cannot prove
        agreement with an earlier captured selection.
    */
    use super::calls::projected_evaluated_path;
    use super::calls::value_index_segment;
    use super::cleanup;
    use super::control::LocalConstructionTrace;
    use super::control::statement_sequence::consume_result;
    use super::structural_scalar_store::scalar_capture_destination;
    use super::types::byte_sequence_carrier;
    use super::types::machine_binders;
    use crate::checks;
    use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
    use crate::execution::terminal_unit::initialized_locals::initialized_local;
    use crate::flow::reference_route_source;
    use crate::flow::reference_source_place;
    use language_semantics::PermissionEventSource;
    use language_semantics::ReferenceAccess;
    use typed_trees::machine::Machine;
    use typed_trees::signature::state_parameter_position;
    use typed_trees::state::State;
    use typed_trees::statement::TableAssignment;
    use typed_trees::statement::TableLocalData;
    use typed_trees::type_identity::TypeIdentityId;
    use typed_trees::types::TypeReferenceHandle;
    use validation::reference_result_custody::initial_reference_establishment;
    use validation::reference_result_custody::replaced_reference_carrier;
    use validation::{
        CallFrameResolver, CanonicalPlace, ImmutableBoundLookup, array_window_shape,
        assignment_stored_value_type, assignment_value_type, canonical_place_type_reference,
        declared_place_type_raw, find_state, reference_shell, stored_structural_type_identity,
        stores_reference_shape,
    };

    use super::{
        CheckedStructuralAccess as StructuralAccess,
        CheckedUnitEffectOperationPlan as OperationPlan, CheckedUnitPartialAffineDiscardPlan,
        CheckedUnitStructuralArgumentPlan as ArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan as SourcePlan,
        CheckedUnitStructuralParameterPlan as ParameterPlan,
        CheckedUnitStructuralPathSegment as PathSegment,
        CheckedUnitStructuralResultBindingPlan as BindingPlan, Multiplicity, StatementNode,
        SymbolHandle,
    };
    use crate::execution::terminal_unit::structural_scalar_store::TargetSelectors;
    use crate::execution::terminal_unit::types::ShapeCollector;
    use crate::flow::expressions;
    use crate::published::places::place_at;

    use checked_trees::{
        CapturedPlaceContainment, CheckFacts, CheckedNormalCleanupAction, CheckedStoreLeaf,
        CheckedStoreRoot, CheckedStoreValue, CheckedUnitCleanupBoundary,
        CheckedUnitParameterStoragePlan, CheckedUnitStructuralTypeShape,
    };
    use facts::{PlaceRoot, PlaceSegment};
    use typed_trees::expression::ExpressionHandle;

    /*
        The planning ledger and source admission must agree on the captured value
        selecting a hole. Re-spelled immutable aliases can denote one index;
        mutable storage spelling alone cannot identify an earlier selection.
    */
    fn same_storage_place(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        left: &CanonicalPlace,
        right: &CanonicalPlace,
    ) -> bool {
        left.root == right.root
            && checks::storage_path_compatibility(
                program,
                checks::StorageSelectorFacts {
                    flow: facts.flow.view(),
                    semantic: &facts.semantic,
                },
                &left.segments,
                &right.segments,
            )
            .1 == CapturedPlaceContainment::Same
    }

    /*
        Assignment owns selector evaluation, even when the selector spells a
        parameter or field that ordinary call arguments can read directly. Its
        retained operand belongs to the pre-RHS occurrence, not a later snapshot.
    */
    fn assignment_path(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &Machine,
        state: &State,
        statement_index: u32,
        target: ExpressionHandle,
        place: &CanonicalPlace,
    ) -> Option<Vec<PathSegment>> {
        let selectors =
            TargetSelectors::resolve(program, facts, machine, state, statement_index, target)?;
        projected_evaluated_path(
            program,
            machine.symbol,
            state.symbol,
            usize::try_from(statement_index).ok()?,
            place,
            Some(&|expression| selectors.runtime_segment(expression)),
        )
        .map(|projection| projection.path)
    }

    /*
        Descriptor retirement consumes the completed store's semantic coordinate.
        Keep it rooted at the executable owner; a resolved backing belonging to a
        different root cannot identify a carrier in this owner's declaration.
    */
    pub(super) fn captured_assignment_path(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        machine: &Machine,
        state: &State,
        statement_index: u32,
        target: ExpressionHandle,
        owner: &CanonicalPlace,
    ) -> Option<Vec<PathSegment>> {
        let control = &facts.flow.control;
        let flow = control.state(machine.symbol, state.symbol)?;
        let statement = facts
            .flow
            .state_statement(flow, usize::try_from(statement_index).ok()?)?;
        let authored = program
            .statement_table
            .statement_at(state, statement_index as usize)?;
        if authored.as_assignment()?.target != target {
            return None;
        }
        let operand = control.operands.get(
            control
                .operands
                .is_valid(statement.store_target)
                .then_some(statement.store_target)?,
        );
        if operand.state_symbol != state.symbol
            || operand.statement_index != statement_index as usize
            || operand.expression != target
        {
            return None;
        }
        let [destination] = control.operand_referents.span(operand.referents)? else {
            return None;
        };
        if destination.root != owner.root {
            return None;
        }
        let captured = CanonicalPlace {
            root: destination.root,
            segments: control
                .operand_referent_segments
                .span(destination.segments)?
                .to_vec(),
        };
        if place_at(program, state.symbol, statement_index as usize, target).as_ref() != Some(owner)
            || !CallFrameResolver::new(program)?
                .captured_assignment_destination_remains_current(machine, authored, &captured)
        {
            return None;
        }
        assignment_path(
            program,
            facts,
            machine,
            state,
            statement_index,
            target,
            &captured,
        )
    }

    /*
        Stored references retain their physical crossing in both destination
        capture and displacement. A structural local contains the descriptor;
        borrowed ingress already names its referent. Views have their own carrier
        geometry and never gain a pointer crossing merely from reference syntax.
    */
    fn retain_assignment_referent(
        program: &dyn validation::CheckingInput,
        source: &SourcePlan,
        path: &mut Vec<PathSegment>,
        declared: TypeReferenceHandle,
        receiving: TypeIdentityId,
    ) {
        let stored_descriptor =
            !path.is_empty() || matches!(source, SourcePlan::StructuralLocal { .. });
        if stored_descriptor
            && stores_reference_shape(program, declared, &[])
            && stored_structural_type_identity(program, declared, &[], &[]) != receiving
        {
            path.push(PathSegment::Referent);
        }
    }

    /*
        Owned storage has no borrowed restoration debt. A formal and a
        local have different physical coordinates, but replacing either retires
        only the old custody selected by source checking. A whole reference-valued
        replacement owns its carrier, not the borrowed contents. Its checked result
        type must equal that carrier's declaration; referent writes still use the
        loan/window ledger below. Mutability is source admission, not storage
        identity. A deferred local's first initialization has no old value to
        displace or borrowed hole to restore.
    */
    pub(super) fn owned_assignment_source(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        parameters: &[ParameterPlan],
        statement_index: usize,
        place: &CanonicalPlace,
        replacement_type: Option<TypeIdentityId>,
    ) -> Option<SourcePlan> {
        for depth in 0..=place.segments.len() {
            let reference = canonical_place_type_reference(
                program,
                state.symbol,
                statement_index,
                &CanonicalPlace {
                    root: place.root,
                    segments: place.segments[..depth].to_vec(),
                },
            )?;
            if reference_shell(program, reference).is_some()
                && (depth < place.segments.len()
                    || replacement_type
                        != Some(stored_structural_type_identity(
                            program,
                            reference,
                            &[],
                            &[],
                        )))
            {
                return None;
            }
        }
        let symbol = place.root.as_symbol()?;
        if let Some((ordinal, local)) = program.state_local_declaration(state, symbol) {
            return (ordinal < statement_index
                && (!checks::type_reference_is_reference(program, local.type_reference)
                    || (place.segments.is_empty()
                        && replacement_type
                            == Some(stored_structural_type_identity(
                                program,
                                local.type_reference,
                                &[],
                                &[],
                            )))))
            .then_some(SourcePlan::StructuralLocal { symbol });
        }
        let position = state_parameter_position(program.state_parameters(state), symbol)?;
        let formal = program.state_parameters(state).get(position)?;
        let (parameter_index, _) = parameters.iter().enumerate().find(|(_, parameter)| {
            parameter.position as usize == position
                && (parameter.access == StructuralAccess::Owned
                    || (parameter.access == StructuralAccess::SharedBorrow
                        && formal.is_mutable
                        && place.segments.is_empty()
                        && reference_shell(program, formal.type_reference)
                            .is_some_and(|shell| shell.access == ReferenceAccess::Shared)
                        && replacement_type
                            == Some(stored_structural_type_identity(
                                program,
                                formal.type_reference,
                                &[],
                                &[],
                            ))))
        })?;
        Some(SourcePlan::Parameter {
            parameter_index: u32::try_from(parameter_index).ok()?,
        })
    }

    /*
        Retain the assignment target once, before evaluating its RHS.
        The target is independent of its replacement's producer and of the fixed
        leaves a whole-array replacement will store beneath it.
    */
    pub(super) fn capture_assignment_destination(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        machine: &Machine,
        state: &State,
        parameters: &[ParameterPlan],
        scalar_parameters: &[super::CheckedStructuralScalarParameterPlan],
        parameter_storage: &[CheckedUnitParameterStoragePlan],
        binders: &[(SymbolHandle, String)],
        statement_index: u32,
        assignment: &TableAssignment,
        trace: &LocalConstructionTrace,
    ) -> Option<OperationPlan> {
        let ordinal = usize::try_from(statement_index).ok()?;
        let collection = expressions(program)
            .indexed_node(assignment.target)
            .filter(|indexed| program.expression_table.range_node(indexed.index).is_some())
            .map(|indexed| indexed.collection);
        let target = collection.unwrap_or(assignment.target);
        let access = place_at(program, state.symbol, ordinal, target)?;
        let owned_source = owned_assignment_source(
            program,
            state,
            parameters,
            ordinal,
            &access,
            facts
                .values
                .structural_values
                .root_for_expression(state.symbol, statement_index, assignment.value)
                .map(|value| {
                    stored_structural_type_identity(program, value.type_reference, &[], &[])
                }),
        );
        if collection.is_none()
            && matches!(
                owned_source,
                Some(SourcePlan::StructuralLocal { .. } | SourcePlan::Parameter { .. })
            )
            && access.segments.is_empty()
        {
            return None;
        }
        if collection.is_none()
            && let Some(destination) = scalar_capture_destination(
                program,
                facts,
                shapes,
                machine,
                state,
                parameters,
                scalar_parameters,
                parameter_storage,
                binders,
                statement_index,
                assignment,
                trace,
            )
        {
            return Some(OperationPlan::CaptureAssignmentDestination {
                statement_index,
                destination_expression: target,
                destination,
            });
        }
        let root = access.root.as_symbol()?;
        let (root, segments) = resolve_storage_place(
            program,
            state,
            program.statement_table.statements(state.statement_nodes),
            ordinal,
            root,
            &access.segments,
        );
        let place = CanonicalPlace { root, segments };
        let (source, mut path) = if let Some(source) = owned_source {
            (
                source,
                assignment_path(
                    program,
                    facts,
                    machine,
                    state,
                    statement_index,
                    target,
                    &place,
                )?,
            )
        } else {
            let selectors =
                TargetSelectors::resolve(program, facts, machine, state, statement_index, target)?;
            let place = window_place(
                program,
                state,
                program.statement_table.statements(state.statement_nodes),
                ordinal,
                parameters,
                &access,
                ReferenceAccess::WriteOnly,
                &|expression| selectors.runtime_segment(expression),
            )?;
            (place.source, place.path)
        };
        let declared = declared_place_type_raw(program, machine, Some(state), target)?;
        let stored = if collection.is_some() {
            assignment_value_type(program, declared)
        } else {
            assignment_stored_value_type(program, machine, state, assignment)?
        };
        retain_assignment_referent(
            program,
            &source,
            &mut path,
            declared,
            stored_structural_type_identity(program, stored, &[], &[]),
        );
        let declared = stored;
        if access.segments.is_empty()
            && collection.is_none()
            && (program.primitive_type_reference(declared).is_some()
                || byte_sequence_carrier(program, declared, &[]).is_some())
        {
            return None;
        }
        Some(OperationPlan::CaptureAssignmentDestination {
            statement_index,
            destination_expression: target,
            destination: ArgumentPlan {
                projection_evaluations: Default::default(),
                source,
                path,
                type_identity: shapes.add_stored_type(declared, binders, &[])?,
                access: StructuralAccess::Owned,
            },
        })
    }

    /*
        Restoration debt and the selected access route are separate coordinates.
        Canonical storage identifies the hole even through forwarding; the emitted
        operand must still use the retained reference loan whose child suspends that
        storage's original parameter. Replacing the route by the debt root loses
        authority or a narrowed view's captured offset.
    */
    pub(in crate::execution::terminal_unit) struct WindowPlace {
        storage: CanonicalPlace,
        source: SourcePlan,
        path: Vec<PathSegment>,
        restoration_required: bool,
    }

    /*
        The exact field chain beneath an exclusive structural loan.
        An empty path pins the whole referent, not its loan descriptor; a fixed or
        retained runtime element step selects the element the window pins. Below
        a `&mut [T]` loan that element is one the view selects in storage it
        does not own; lowering bounds it by the view's runtime length.

        The place is first keyed on the resolved *storage* it names — the same
        rebase the checker performs: a route through a reference-typed local
        (`let r = &mut self; r.f`) names the same hole as the owner path
        (`self.f`), so both must land on one window. The access's evaluation owner supplies
        runtime coordinates: value reads consume checked scalar operands, while
        assignment targets consume their retained pre-RHS selector slots.
    */
    pub(in crate::execution::terminal_unit) fn window_place(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statements: &[StatementNode],
        statement_index: usize,
        structural_parameters: &[ParameterPlan],
        place: &CanonicalPlace,
        required_access: ReferenceAccess,
        selector: &dyn Fn(ExpressionHandle) -> Option<PathSegment>,
    ) -> Option<WindowPlace> {
        let root_symbol = place.root.as_symbol()?;
        let (root, segments) = resolve_storage_place(
            program,
            state,
            statements,
            statement_index,
            root_symbol,
            &place.segments,
        );
        let storage = CanonicalPlace { root, segments };
        let local = program
            .state_local_declaration(state, root_symbol)
            .filter(|(ordinal, _)| *ordinal < statement_index);
        let (source, path, restoration_required) = if let Some((_, local)) = local {
            let borrowed = checks::type_reference_is_reference(program, local.type_reference);
            if borrowed
                && !matches!(
                    (
                        required_access,
                        reference_shell(program, local.type_reference)?.access
                    ),
                    (_, ReferenceAccess::Mutable)
                        | (ReferenceAccess::WriteOnly, ReferenceAccess::WriteOnly)
                )
            {
                return None;
            }
            if !borrowed && place.segments.is_empty() {
                return None;
            }
            (
                SourcePlan::StructuralLocal {
                    symbol: root_symbol,
                },
                place.segments.as_slice(),
                borrowed,
            )
        } else {
            let symbol = root.as_symbol()?;
            let position = state_parameter_position(program.state_parameters(state), symbol)?;
            let position = u32::try_from(position).ok()?;
            let (parameter_index, parameter) = structural_parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.position == position)?;
            if !matches!(
                parameter.access,
                StructuralAccess::Owned | StructuralAccess::MutableBorrow
            ) && !(required_access == ReferenceAccess::WriteOnly
                && parameter.access == StructuralAccess::WriteOnlyBorrow)
                || (parameter.access == StructuralAccess::MutableBorrow
                    && parameter.multiplicity == Multiplicity::Linear)
                || (parameter.access == StructuralAccess::Owned && storage.segments.is_empty())
            {
                return None;
            }
            (
                SourcePlan::Parameter {
                    parameter_index: u32::try_from(parameter_index).ok()?,
                },
                storage.segments.as_slice(),
                parameter.access != StructuralAccess::Owned,
            )
        };
        let (machine, _) = find_state(program, state.symbol)?;
        let path_root = match source {
            SourcePlan::StructuralLocal { symbol } => PlaceRoot::Symbol(symbol),
            _ => storage.root,
        };
        let ProjectedArgumentPath { path, .. } = projected_evaluated_path(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            &CanonicalPlace {
                root: path_root,
                segments: path.to_vec(),
            },
            Some(selector),
        )?;
        Some(WindowPlace {
            storage,
            source,
            path,
            restoration_required,
        })
    }

    /*
        Rebase a place written through a reference-typed local onto the referent
        storage it borrows, following plain reference copies (`let s = r`),
        recast chains, and whole-local reassignments, until the root names a
        parameter or machine rather than a local. Mirrors the checker rule that
        windows key on the resolved storage place, not the access route.
    */
    pub(in crate::execution::terminal_unit) fn resolve_storage_place(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statements: &[StatementNode],
        statement_index: usize,
        root_symbol: SymbolHandle,
        path: &[PlaceSegment],
    ) -> (PlaceRoot, Vec<PlaceSegment>) {
        let mut root = PlaceRoot::Symbol(root_symbol);
        let mut segments = path.to_vec();
        for _ in 0..16 {
            let Some(symbol) = root.as_symbol() else {
                break;
            };
            let Some(source) =
                reference_route_source(program, state, statements, statement_index, symbol)
            else {
                break;
            };
            let Some(source) =
                reference_source_place(program, state.symbol, statement_index, source)
            else {
                break;
            };
            let PlaceRoot::Symbol(..) = source.root else {
                break;
            };
            let mut rebased = source.segments;
            rebased.extend_from_slice(&segments);
            root = source.root;
            let rebased_place = CanonicalPlace {
                root,
                segments: rebased,
            };

            root = rebased_place.root;
            segments = rebased_place.segments;
        }
        (root, segments)
    }

    /*
        Exact places a statement sequence has opened and not yet repaired.
        Detached values live in the sequence's ordinary structural result table;
        consuming a value does not discharge the original place's restoration debt.
    */
    #[derive(Default)]
    pub(super) struct OpenWindows {
        /*
            Invocation scratch, keyed by the same canonical storage subject the
            source checker uses. A dense parameter coordinate cannot name local
            loans, and executable selector roles change at each statement.
        */
        open: Vec<OpenWindow>,
    }

    struct OpenWindow {
        storage: CanonicalPlace,
        opening_binding: u32,
        restoration_required: bool,
    }

    /*
        The exact storage place an immutable, non-reference, ownership-carrying
        local reads whole from an existing place. Such a local has the window
        route by kind: the sequence admits it and then decides whether the place is
        exclusive borrowed storage it can move out of. Copy locals never open a
        window, and a mutable or reference-typed local binds nothing to restore.
    */
    pub(in crate::execution::terminal_unit) fn move_out_candidate(
        program: &dyn ::validation::CheckingInput,
        state: &State,
        statement_index: usize,
        local: &TableLocalData,
    ) -> Option<validation::CanonicalPlace> {
        if local.is_mutable
            || checks::type_reference_is_reference(program, local.type_reference)
            || program.type_multiplicity(local.type_reference) == Multiplicity::Unrestricted
        {
            return None;
        }
        place_at(program, state.symbol, statement_index, local.initial_value)
    }

    impl OpenWindows {
        /*
            `let local: T = root.field...;` moving an affine or linear value out
            of exclusive borrowed storage. The local binds the moved value at
            `binding_ordinal`; a copy value or a place outside a pinned window is
            not a move-out and returns None so the ordinary local-data route
            plans it.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn move_out(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            shapes: &mut ShapeCollector<'_>,
            machine: &Machine,
            state: &State,
            structural_parameters: &[ParameterPlan],
            binders: &[(SymbolHandle, String)],
            statement_index: u32,
            local: &TableLocalData,
            binding_ordinal: u32,
        ) -> Option<(OperationPlan, BindingPlan)> {
            let index = usize::try_from(statement_index).ok()?;
            let place = move_out_candidate(program, state, index, local)?;
            let multiplicity = program.type_multiplicity(local.type_reference);
            let statements = program.statement_table.statements(state.statement_nodes);
            let place = window_place(
                program,
                state,
                statements,
                index,
                structural_parameters,
                &place,
                ReferenceAccess::Mutable,
                &|expression| {
                    value_index_segment(program, &facts.operators, state, index, expression)
                },
            )?;
            let type_identity = shapes.add_type(local.type_reference, binders, &[])?;
            let result = BindingPlan {
                statement_index,
                binding_ordinal,
                type_identity,
                multiplicity,
                local: initialized_local(program, machine, state, local),
            };
            self.bind_extraction(program, facts, place, result, local.initial_value)
        }

        /*
            Materialize an owned argument by vacating its borrowed storage. This is
            the same move as a named extraction, at the argument's actual evaluation
            point. Owned homes keep their ordinary call transfer; only an exclusive
            borrowed referent needs a separate restoration debt.
        */
        #[allow(clippy::too_many_arguments)]
        pub(in crate::execution::terminal_unit) fn move_call_operand(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            shapes: &mut ShapeCollector<'_>,
            machine: &Machine,
            state: &State,
            structural_parameters: &[ParameterPlan],
            statement_index: u32,
            expression: ExpressionHandle,
            type_reference: TypeReferenceHandle,
            binding_ordinal: u32,
        ) -> Option<(OperationPlan, BindingPlan)> {
            let ordinal = usize::try_from(statement_index).ok()?;
            let place = place_at(program, state.symbol, ordinal, expression)?;
            let place = window_place(
                program,
                state,
                program.statement_table.statements(state.statement_nodes),
                ordinal,
                structural_parameters,
                &place,
                ReferenceAccess::Mutable,
                &|selector| {
                    value_index_segment(program, &facts.operators, state, ordinal, selector)
                },
            )?;
            let result = BindingPlan {
                statement_index,
                binding_ordinal,
                type_identity: shapes.add_type(
                    type_reference,
                    &machine_binders(program, machine),
                    &[],
                )?,
                multiplicity: program.type_multiplicity(type_reference),
                local: Default::default(),
            };
            if !place.restoration_required {
                return None;
            }
            self.bind_extraction(program, facts, place, result, expression)
        }

        fn bind_extraction(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            place: WindowPlace,
            result: BindingPlan,
            expression: ExpressionHandle,
        ) -> Option<(OperationPlan, BindingPlan)> {
            let type_identity = result.type_identity;
            let binding_ordinal = result.binding_ordinal;
            if self
                .open
                .iter()
                .any(|open| same_storage_place(program, facts, &open.storage, &place.storage))
            {
                return None;
            }
            self.open.push(OpenWindow {
                storage: place.storage,
                opening_binding: binding_ordinal,
                restoration_required: place.restoration_required,
            });
            Some((
                OperationPlan::MoveStructuralField {
                    result: result.clone(),
                    expression,
                    source: ArgumentPlan {
                        projection_evaluations: Default::default(),
                        source: place.source,
                        path: place.path,
                        type_identity,
                        access: StructuralAccess::Owned,
                    },
                },
                result,
            ))
        }

        /*
            Store `produced` into an exclusive borrowed field. An existing hole
            needs only the store; otherwise first displace the old value into
            `displaced_ordinal` and dispose it on the statement's continuation.
            Neither route depends on how the owned replacement was produced.
            A linear replacement may repair a vacancy, but cannot overwrite a
            live obligation. Decide that from the exact place's checked custody,
            not from the replacement's multiplicity alone.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn replace(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            shapes: &super::types::ShapeCollector<'_>,
            machine: &Machine,
            state: &State,
            structural_parameters: &[ParameterPlan],
            statement_index: u32,
            assignment: &TableAssignment,
            produced: &BindingPlan,
            displaced_ordinal: u32,
            value: ArgumentPlan,
            trace: &LocalConstructionTrace,
        ) -> Option<ValueReplacement> {
            trace.phase("structural replacement: destination identity");
            let ordinal = usize::try_from(statement_index).ok()?;
            let mut parts = Vec::new();
            let range = expressions(program)
                .indexed_node(assignment.target)
                .filter(|indexed| program.expression_table.range_node(indexed.index).is_some());
            if let Some(indexed) = range {
                if !assignment.receiving_type.is_valid() {
                    return None;
                }
                let shape = array_window_shape(
                    program,
                    machine,
                    state,
                    assignment.target,
                    &mut ImmutableBoundLookup::new(program),
                )?;
                let length = u64::try_from(shape.length()?).ok()?;
                let start = shape.start?;
                let declared = shapes.types.get(&produced.type_identity)?;
                let CheckedUnitStructuralTypeShape::FixedArray {
                    element_type_identity,
                    length: source_length,
                } = declared.shape
                else {
                    return None;
                };
                if source_length.literal() != Some(length)
                    || stored_structural_type_identity(program, assignment.receiving_type, &[], &[])
                        != produced.type_identity
                    || stored_structural_type_identity(program, shape.element_type, &[], &[])
                        != element_type_identity
                {
                    return None;
                }
                let collection = place_at(program, state.symbol, ordinal, indexed.collection)?;
                let mut next_displacement = displaced_ordinal;
                for element_ordinal in 0..length {
                    let mut destination = collection.clone();
                    destination.segments.push(PlaceSegment::FixedIndex {
                        index: start.checked_add(usize::try_from(element_ordinal).ok()?)?,
                    });
                    let mut element = value.clone();
                    element.path.push(PathSegment::FixedIndex(element_ordinal));
                    element.type_identity = element_type_identity;
                    let produced_element = BindingPlan {
                        type_identity: element_type_identity,
                        multiplicity: checks::type_multiplicity(program, shape.element_type),
                        ..produced.clone()
                    };
                    let replacement = self.replace_place(
                        program,
                        facts,
                        shapes,
                        machine,
                        state,
                        structural_parameters,
                        statement_index,
                        indexed.collection,
                        destination,
                        &produced_element,
                        next_displacement,
                        element,
                        vec![PathSegment::FixedIndex(element_ordinal)],
                        trace,
                    )?;
                    next_displacement =
                        next_displacement.checked_add(u32::from(replacement.move_out.is_some()))?;
                    parts.push(replacement);
                }
            } else {
                let destination = place_at(program, state.symbol, ordinal, assignment.target)?;
                parts.push(self.replace_place(
                    program,
                    facts,
                    shapes,
                    machine,
                    state,
                    structural_parameters,
                    statement_index,
                    assignment.target,
                    destination,
                    produced,
                    displaced_ordinal,
                    value.clone(),
                    Vec::new(),
                    trace,
                )?);
            }
            self.open.retain(|open| {
                !parts.iter().any(|part| {
                    part.store.store().is_some_and(|store| {
                        matches!(
                            store.destination.leaf,
                            CheckedStoreLeaf::StructuralStore {
                                opening_binding: Some(binding), ..
                            } if binding == open.opening_binding
                        )
                    })
                })
            });
            Some(ValueReplacement {
                source: Some(value),
                parts,
            })
        }

        /*
            Replace one selected subtree. Selection comes from the destination
            region, never from a constructor leaf or a synthetic source expression.
        */
        #[allow(clippy::too_many_arguments)]
        fn replace_place(
            &self,
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            shapes: &super::types::ShapeCollector<'_>,
            machine: &Machine,
            state: &State,
            structural_parameters: &[ParameterPlan],
            statement_index: u32,
            selector_target: ExpressionHandle,
            destination_place: CanonicalPlace,
            produced: &BindingPlan,
            displaced_ordinal: u32,
            value: ArgumentPlan,
            receiver_path: Vec<PathSegment>,
            trace: &LocalConstructionTrace,
        ) -> Option<FieldReplacement> {
            trace.phase("structural replacement: owned destination");
            let index = usize::try_from(statement_index).ok()?;
            let access_place = destination_place.clone();
            let owned_source = owned_assignment_source(
                program,
                state,
                structural_parameters,
                index,
                &access_place,
                Some(produced.type_identity),
            );
            let (resolved_root, resolved_segments) = if owned_source.is_some() {
                (access_place.root, access_place.segments.clone())
            } else {
                resolve_storage_place(
                    program,
                    state,
                    program.statement_table.statements(state.statement_nodes),
                    index,
                    match destination_place.root {
                        PlaceRoot::Symbol(symbol) => symbol,
                        _ => return None,
                    },
                    &destination_place.segments,
                )
            };
            let destination_place = CanonicalPlace {
                root: resolved_root,
                segments: resolved_segments,
            };
            let owned_rebind = owned_source.is_some();
            let (source, mut path, opening_binding, needs_displacement, needs_cleanup) =
                if let Some(source) = owned_source {
                    trace.phase("structural replacement: owned assignment path");
                    let path = assignment_path(
                        program,
                        facts,
                        machine,
                        state,
                        statement_index,
                        selector_target,
                        &destination_place,
                    )?;
                    let captured_path = path
                        .iter()
                        .any(|segment| matches!(segment, PathSegment::RuntimeIndex(_)))
                        .then(|| {
                            captured_assignment_path(
                                program,
                                facts,
                                machine,
                                state,
                                statement_index,
                                selector_target,
                                &destination_place,
                            )
                        })
                        .flatten();
                    let retirement_path = captured_path.as_deref().unwrap_or(&path);
                    let transferred = facts.contextual_cleanup.replaces_transferred_storage(
                        cleanup::owning_cleanup_machine(program, state.symbol),
                        state.symbol,
                        PermissionEventSource::Statement {
                            statement_index: index,
                        },
                        destination_place.root,
                        &destination_place.segments,
                    );
                    let occupied = slot_occupied(facts, machine, state, index, &destination_place)
                        || (!transferred
                            && replaced_reference_carrier(
                                program,
                                facts,
                                machine.symbol,
                                state,
                                destination_place.root,
                                statement_index,
                                retirement_path,
                            )
                            .is_some());
                    let opening = self
                        .open
                        .iter()
                        .find(|open| {
                            same_storage_place(program, facts, &open.storage, &destination_place)
                        })
                        .map(|open| open.opening_binding);
                    let establishes_descriptor = !occupied
                        && path.is_empty()
                        && destination_place.root.as_symbol().is_some_and(|owner| {
                            initial_reference_establishment(
                                program,
                                facts,
                                machine.symbol,
                                state,
                                owner,
                                statement_index,
                            )
                            .is_some()
                        });
                    trace.phase("structural replacement: exact receiving establishment");
                    if !occupied
                        && opening.is_none()
                        && produced.multiplicity != Multiplicity::Unrestricted
                        && !establishes_descriptor
                        && !facts.contextual_cleanup.storage_establishments.iter().any(
                            |(_, establishment)| {
                                establishment.machine == machine.symbol
                                    && establishment.state == state.symbol
                                    && establishment.source
                                        == PermissionEventSource::Statement {
                                            statement_index: index,
                                        }
                                    && establishment.destination.root == destination_place.root
                                    && facts
                                        .contextual_cleanup
                                        .segments
                                        .span_or_empty(establishment.destination.segments)
                                        == destination_place.segments
                            },
                        )
                    {
                        return None;
                    }
                    (source, path, opening, false, occupied)
                } else {
                    trace.phase("structural replacement: borrowed selectors");
                    let selectors = TargetSelectors::resolve(
                        program,
                        facts,
                        machine,
                        state,
                        statement_index,
                        selector_target,
                    )?;
                    trace.phase("structural replacement: borrowed window");
                    let place = window_place(
                        program,
                        state,
                        program.statement_table.statements(state.statement_nodes),
                        index,
                        structural_parameters,
                        &access_place,
                        ReferenceAccess::WriteOnly,
                        &|expression| selectors.runtime_segment(expression),
                    )?;
                    let opening_binding = self
                        .open
                        .iter()
                        .find(|open| {
                            same_storage_place(program, facts, &open.storage, &place.storage)
                        })
                        .map(|open| open.opening_binding);
                    let freely_discardable = produced.multiplicity == Multiplicity::Unrestricted
                        || produced.multiplicity == Multiplicity::Affine
                            && shapes.types.get(&produced.type_identity).is_some_and(
                                |declaration| {
                                    declaration.no_code_affine_cleanup
                                        || declaration.descriptor_cleanup
                                },
                            );
                    let needs_displacement = opening_binding.is_none() && !freely_discardable;
                    (
                        place.source,
                        place.path,
                        opening_binding.or(needs_displacement.then_some(displaced_ordinal)),
                        needs_displacement,
                        needs_displacement && produced.multiplicity == Multiplicity::Affine,
                    )
                };
            trace.phase("structural replacement: receiving carrier");
            let declared =
                canonical_place_type_reference(program, state.symbol, index, &access_place)?;
            retain_assignment_referent(
                program,
                &source,
                &mut path,
                declared,
                produced.type_identity,
            );
            if produced.multiplicity == Multiplicity::Linear && needs_displacement {
                return None;
            }
            let place = ArgumentPlan {
                projection_evaluations: Default::default(),
                source,
                path,
                type_identity: produced.type_identity,
                access: StructuralAccess::Owned,
            };
            let old_value = if needs_displacement {
                ArgumentPlan::structural_result(
                    displaced_ordinal,
                    produced.type_identity,
                    StructuralAccess::Owned,
                )
            } else {
                place.clone()
            };
            let root = match place.source {
                SourcePlan::Parameter { parameter_index } => CheckedStoreRoot::Parameter {
                    position: structural_parameters
                        .get(parameter_index as usize)?
                        .position,
                },
                SourcePlan::StructuralLocal { symbol } => CheckedStoreRoot::Local { symbol },
                _ => return None,
            };
            let type_identity = produced.type_identity;
            let leaf = if owned_rebind && place.path.is_empty() {
                CheckedStoreLeaf::OwnedRebind { type_identity }
            } else {
                CheckedStoreLeaf::StructuralStore {
                    type_identity,
                    multiplicity: produced.multiplicity,
                    opening_binding,
                }
            };
            let mut store = OperationPlan::new_store(
                statement_index,
                root,
                place.path.clone(),
                leaf,
                CheckedStoreValue::Place(value),
            );
            if let OperationPlan::Store(store) = &mut store {
                store.captured_selection = receiver_path.clone();
            }
            Some(FieldReplacement {
                move_out: needs_displacement.then(|| OperationPlan::MoveStructuralField {
                    result: BindingPlan {
                        statement_index,
                        binding_ordinal: displaced_ordinal,
                        type_identity: produced.type_identity,
                        multiplicity: produced.multiplicity,
                        local: Default::default(),
                    },
                    source: place.clone(),
                    expression: selector_target,
                }),
                cleanup: needs_cleanup.then(|| OperationPlan::ContinuationCleanup {
                    partitions: Default::default(),
                    selected_regions: Vec::new(),
                    boundary: CheckedUnitCleanupBoundary::Replacement {
                        statement_index,
                        destination: place.clone(),
                        old_value: old_value.clone(),
                        receiver_path,
                    },
                    cleanup_actions: vec![CheckedNormalCleanupAction::Discard(
                        CheckedUnitPartialAffineDiscardPlan {
                            partition_leaf: Default::default(),
                            source: old_value.source,
                            path: old_value.path,
                            type_identity: produced.type_identity,
                            case_conditions: Default::default(),
                        },
                    )],
                }),
                store,
            })
        }

        /*
            Borrowed holes must be repaired before the body completes. Owned
            projected moves may leave partial custody; the source checker owns
            their residual disposal. A borrowed
            leftover is a sequencing fault rather than a language decision.
        */
        pub(super) fn is_closed(&self) -> bool {
            self.open.iter().all(|open| !open.restoration_required)
        }
    }

    /*
        One store, with displacement and disposal only when its destination was
        occupied. A vacant destination is repaired without inventing an old value.
    */
    struct FieldReplacement {
        move_out: Option<OperationPlan>,
        store: OperationPlan,
        cleanup: Option<OperationPlan>,
    }

    /*
        One complete replacement value, distributed over the selected destination
        region. Its producer's disposal debt is consumed once, even when several
        projected stores install its elements. This plan owns no source evaluation.
    */
    pub(super) struct ValueReplacement {
        pub(super) source: Option<ArgumentPlan>,
        parts: Vec<FieldReplacement>,
    }

    impl ValueReplacement {
        pub(super) fn from_store(store: OperationPlan) -> Self {
            Self {
                source: store.installed_value().cloned(),
                parts: vec![FieldReplacement {
                    move_out: None,
                    cleanup: None,
                    store,
                }],
            }
        }

        pub(super) fn displaced_bindings(&self) -> usize {
            self.parts
                .iter()
                .filter(|part| part.move_out.is_some())
                .count()
        }

        pub(super) fn consume_source(&self, operations: &mut [OperationPlan]) -> Option<()> {
            if let Some(source) = &self.source
                && let Some(binding) = source.source_structural_result_binding_ordinal()
            {
                consume_result(operations, binding, source.access, !source.path.is_empty())?;
            }
            Some(())
        }

        /*
            All RHS values already exist. Displace, clean up every old value, then
            install: no later element may observe an earlier installation's effects.
        */
        pub(super) fn into_operations(self) -> impl Iterator<Item = OperationPlan> {
            let mut displacements = Vec::new();
            let mut cleanup = Vec::new();
            let mut stores = Vec::new();
            for part in self.parts {
                displacements.extend(part.move_out);
                cleanup.extend(part.cleanup);
                stores.push(part.store);
            }
            displacements
                .into_iter()
                .chain(cleanup.into_iter().rev())
                .chain(stores)
        }
    }

    /*
        Whether the permission producer selected an old-value death for the store
        at `statement_index` over `destination`: the slot holds a value the store
        replaces. An absent construction slot (a declared local's field before its
        first store) creates no displaced value and owes no disposal. This follows
        checked custody, not the initializer's spelling.
    */
    fn slot_occupied(
        facts: &CheckFacts,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        destination: &CanonicalPlace,
    ) -> bool {
        facts
            .contextual_cleanup
            .dispositions
            .iter()
            .any(|(_, disposition)| {
                disposition.machine == machine.symbol
                    && disposition.state == state.symbol
                    && disposition.source == PermissionEventSource::Statement { statement_index }
                    && disposition.root == destination.root
                    && facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(disposition.path)
                        .starts_with(&destination.segments)
            })
    }
}
pub(crate) mod byte_operands {
    /*
        Prepare one byte operand independently of the operation consuming it.

        A comparison, predicate scan and descriptor replacement observe the same
        current place or completed view expression. The caller supplies its actual
        parameter and selector namespaces; preparation never fabricates a call or
        replays a local initializer as current storage. Lifetime and predicate
        obligations remain with the consuming operation.
    */
    use super::{calls, types};
    use crate::published::places::place_at;
    use checked_trees::{
        CheckedOperatorFacts, CheckedRuntimeIndex, CheckedScalarExpressionPlans,
        CheckedStructuralAccess, CheckedSubsliceSite, CheckedUnitStructuralArgumentPlan,
        CheckedUnitStructuralArgumentSourcePlan as Source,
    };
    use typed_trees::{expression::ExpressionHandle, types::TypeReferenceHandle};

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn plan(
        program: &dyn ::validation::CheckingInput,
        operators: &CheckedOperatorFacts,
        pure: &CheckedScalarExpressionPlans,
        computations: &checked_trees::CheckedScalarComputationPlans,
        borrow: &checked_trees::BorrowFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        operand: ExpressionHandle,
        target: TypeReferenceHandle,
        observe_referent: bool,
        dense_parameter: &dyn Fn(usize) -> Option<u32>,
        selector: &dyn Fn(ExpressionHandle) -> Option<CheckedRuntimeIndex>,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        if let Some(literal) = calls::byte_sequence_literal_argument(program, target, operand) {
            return Some(literal);
        }
        if let Some(subject) = place(
            program,
            machine,
            state,
            statement_index,
            operand,
            target,
            observe_referent,
            dense_parameter,
            selector,
        ) {
            return Some(subject);
        }
        ::validation::subslice_expression(program, operand)?;
        let parameters = calls::free_structural_scalar_signature(
            program,
            &mut types::ShapeCollector::new(program),
            state,
            &[],
        )?
        .0;
        calls::view_subslice::admit_resolved(
            program,
            operators,
            pure,
            computations,
            borrow,
            machine,
            state,
            &parameters,
            target,
            operand,
            statement_index,
            CheckedSubsliceSite::Expression {
                expression: operand,
            },
        )
        .map(|resolution| resolution.argument())
    }

    #[allow(clippy::too_many_arguments)]
    fn place(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        operand: ExpressionHandle,
        target: TypeReferenceHandle,
        observe_referent: bool,
        dense_parameter: &dyn Fn(usize) -> Option<u32>,
        selector: &dyn Fn(ExpressionHandle) -> Option<CheckedRuntimeIndex>,
    ) -> Option<CheckedUnitStructuralArgumentPlan> {
        let place = place_at(program, state.symbol, statement_index, operand)?;
        if !validation::place_has_builtin_coordinates(program, machine, Some(state), operand) {
            return None;
        }
        let root = place.root.as_symbol()?;
        let source = if let Some(position) =
            typed_trees::signature::state_parameter_position(program.state_parameters(state), root)
        {
            Source::Parameter {
                parameter_index: dense_parameter(position)?,
            }
        } else {
            Source::StructuralLocal { symbol: root }
        };
        let projector = if observe_referent {
            calls::projected_observed_path
        } else {
            calls::projected_selected_path
        };
        let calls::ProjectedArgumentPath {
            storage_type: projected,
            path,
            ..
        } = projector(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            &place,
            &selector,
        )?;
        let observed = if program.normalized_type_identity(projected)
            == program.normalized_type_identity(target)
        {
            target
        } else {
            match validation::reference_shell(program, target) {
                Some(shell) if shell.access != language_semantics::ReferenceAccess::WriteOnly => {
                    shell.referee
                }
                Some(_) => return None,
                None => target,
            }
        };
        if program.normalized_type_identity(projected) != program.normalized_type_identity(observed)
        {
            return None;
        }
        Some(CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source,
            path,
            type_identity: validation::stored_structural_type_identity(
                program,
                projected,
                &[],
                &[],
            ),
            access: CheckedStructuralAccess::SharedBorrow,
        })
    }

    /*
        Descriptor expressions can produce a standalone carrier even when only an
        inline record field mentions their type. Register the authored expression's
        exact carrier in the same vocabulary as call arguments and scalar operands.
        This records types only; source custody and evaluation remain at the store.
    */
    pub(super) fn store_operand_type_identities(
        program: &dyn ::validation::CheckingInput,
        shapes: &mut types::ShapeCollector<'_>,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operation: &checked_trees::CheckedUnitEffectOperationPlan,
    ) -> Vec<typed_trees::type_identity::TypeIdentityId> {
        let Some(value) = operation.stored_bytes() else {
            return Vec::new();
        };
        let Some(machine) = program.machine(machine) else {
            return Vec::new();
        };
        let Some(state) = program
            .machine_states(machine)
            .iter()
            .find(|candidate| candidate.symbol == state)
        else {
            return Vec::new();
        };
        value
            .nodes
            .iter()
            .filter_map(|(_, node)| {
                let checked_trees::CheckedByteSequenceValue::Descriptor { expression, source } =
                    node
                else {
                    return None;
                };
                if matches!(source.source, Source::ByteSequenceLiteral { .. }) {
                    return None;
                }
                let reference = validation::expression_result_type_reference(
                    program,
                    machine,
                    state,
                    *expression,
                )?;
                let identity = shapes.add_type(reference, &[], &[])?;
                (identity == source.type_identity).then_some(identity)
            })
            .collect()
    }
}
#[path = "execution_terminal_unit_calls.rs"]
pub(crate) mod calls;
mod candidate_closure {
    /*
        Close the fixed candidate roster over exact call dependencies, without
        rebuilding bodies or rechecking unaffected callers after each removal.
    */
    use super::{
        CheckFacts, CheckedComposedUnitControlMachinePlan,
        CheckedPartialAffineUnitCleanupMachinePlan, CheckedUnitEffectMachinePlan,
        CheckedUnitEffectOperationPlan as OperationPlan, SymbolHandle, symbol_key,
    };
    use crate::execution::terminal_unit::plan_gaps::{GapSite, plan_gap};
    use crate::execution::terminal_unit::scalar_targets;
    use checked_trees::{
        CheckedUnitCallResult, CheckedUnitCallRoute, CheckedUnitCleanupBoundary as CleanupBoundary,
        CheckedUnitPlanGap, CheckedUnitPlanGapReason as GapReason,
    };
    use std::collections::BTreeSet;

    /* Why one candidate left the roster, and where its body names the reason. */
    type Cause = (GapReason, GapSite);

    struct CandidateClosure {
        /*
            Symbols are sparse across the complete program; candidate ordinals are
            dense and private to this invocation. Each machine has at most one body:
            its shape selected one plan route.
        */
        entries: Vec<(SymbolHandle, SymbolHandle, usize)>,
        retained: Vec<bool>,
        /*
            The first reason each candidate left the roster, kept beside `retained`
            so the gap names the direct dependency that failed and its call site.
        */
        causes: Vec<Option<Cause>>,
        /*
            (target, caller, index into `sites`): the caller's body depends on the
            target's at that call site.
        */
        dependents: Vec<(usize, usize, usize)>,
        sites: Vec<GapSite>,
    }

    impl CandidateClosure {
        fn new(entries: impl Iterator<Item = (SymbolHandle, SymbolHandle)>) -> Self {
            let mut entries = entries
                .enumerate()
                .map(|(candidate_index, (machine, state))| (machine, state, candidate_index))
                .collect::<Vec<_>>();
            entries.sort_unstable_by_key(|(machine, _, _)| symbol_key(*machine));
            Self {
                retained: vec![true; entries.len()],
                causes: vec![None; entries.len()],
                entries,
                dependents: Vec::new(),
                sites: Vec::new(),
            }
        }

        fn drop_candidate(&mut self, caller_index: usize, reason: GapReason, site: GapSite) {
            if self.retained[caller_index] {
                self.retained[caller_index] = false;
                self.causes[caller_index] = Some((reason, site));
            }
        }

        fn depend(&mut self, target_index: usize, caller_index: usize, site: GapSite) {
            self.dependents
                .push((target_index, caller_index, self.sites.len()));
            self.sites.push(site);
        }

        fn require_entry(
            &mut self,
            caller_index: usize,
            machine: SymbolHandle,
            state: SymbolHandle,
            site: GapSite,
        ) {
            match self
                .entries
                .binary_search_by_key(&symbol_key(machine), |(candidate, _, _)| {
                    symbol_key(*candidate)
                }) {
                Ok(entry_index) if self.entries[entry_index].1 == state => {
                    let target_index = self.entries[entry_index].2;
                    self.depend(target_index, caller_index, site);
                }
                _ => self.drop_candidate(caller_index, GapReason::Callee(machine), site),
            }
        }

        fn close(mut self) -> (Vec<bool>, Vec<Option<Cause>>) {
            let mut unavailable = self
                .retained
                .iter()
                .enumerate()
                .filter_map(|(candidate_index, retained)| (!retained).then_some(candidate_index))
                .collect::<Vec<_>>();
            if unavailable.is_empty() {
                return (self.retained, self.causes);
            }
            self.dependents.sort_unstable();
            self.dependents
                .dedup_by_key(|(target, caller, _)| (*target, *caller));
            let mut queue_position = 0;
            while let Some(&target_index) = unavailable.get(queue_position) {
                queue_position += 1;
                let first = self
                    .dependents
                    .partition_point(|(target, _, _)| *target < target_index);
                let target = self
                    .entries
                    .iter()
                    .find(|(_, _, candidate_index)| *candidate_index == target_index)
                    .map(|(machine, _, _)| *machine)
                    .unwrap_or_else(SymbolHandle::invalid);
                for &(_, caller_index, site) in self.dependents[first..]
                    .iter()
                    .take_while(|(target, _, _)| *target == target_index)
                {
                    if self.retained[caller_index] {
                        self.retained[caller_index] = false;
                        self.causes[caller_index] =
                            Some((GapReason::Callee(target), self.sites[site]));
                        unavailable.push(caller_index);
                    }
                }
            }
            (self.retained, self.causes)
        }
    }

    /*
        Fold a scalar-target availability verdict into dependency edges or a
        missing-target drop.
    */
    fn apply_scalar_target_verdict(
        closure: &mut CandidateClosure,
        candidate_count: usize,
        partial_base: usize,
        caller_index: usize,
        target: SymbolHandle,
        verdict: Option<scalar_targets::AvailableScalarTarget>,
        site: GapSite,
    ) {
        match verdict {
            Some(scalar_targets::AvailableScalarTarget::Ordinary(target_index)) => {
                closure.depend(target_index, caller_index, site);
            }
            Some(scalar_targets::AvailableScalarTarget::Composed(composed_index)) => {
                closure.depend(candidate_count + composed_index, caller_index, site);
            }
            Some(scalar_targets::AvailableScalarTarget::PartialAffine(partial_index)) => {
                closure.depend(partial_base + partial_index, caller_index, site);
            }
            None => closure.drop_candidate(caller_index, GapReason::Callee(target), site),
        }
    }

    /*
        Admit a call operation: boundary coverage, a module entry requirement, or
        the scalar target's availability verdict.
    */
    fn admit_call_target(
        closure: &mut CandidateClosure,
        boundaries: &[(u32, u32)],
        candidate_count: usize,
        partial_base: usize,
        caller_index: usize,
        call: &checked_trees::CheckedUnitCallPlan,
        scalar_verdict: impl FnOnce() -> Option<scalar_targets::AvailableScalarTarget>,
        site: GapSite,
    ) {
        match (call.route(), call.result()) {
            (CheckedUnitCallRoute::Boundary, _) => {
                if boundaries
                    .binary_search(&symbol_key(call.target.machine))
                    .is_err()
                {
                    closure.drop_candidate(
                        caller_index,
                        GapReason::BoundaryTarget(call.target.machine),
                        site,
                    );
                }
            }
            (
                CheckedUnitCallRoute::Module { .. },
                CheckedUnitCallResult::Unit
                | CheckedUnitCallResult::Structural { .. }
                | CheckedUnitCallResult::NoNormal,
            ) => closure.require_entry(caller_index, call.target.machine, call.target.state, site),
            (CheckedUnitCallRoute::Module { .. }, CheckedUnitCallResult::Scalar(_)) => {
                apply_scalar_target_verdict(
                    closure,
                    candidate_count,
                    partial_base,
                    caller_index,
                    call.target.machine,
                    scalar_verdict(),
                    site,
                )
            }
            (
                CheckedUnitCallRoute::Dynamic { .. }
                | CheckedUnitCallRoute::ActivationStorage { .. }
                | CheckedUnitCallRoute::DescriptorTable { .. },
                _,
            ) => {}
        }
    }

    fn admit_operation_cleanups(
        closure: &mut CandidateClosure,
        caller: usize,
        control: &checked_trees::CheckedCleanupControlPlans,
        state: SymbolHandle,
        operation: &OperationPlan,
    ) {
        let site = match operation {
            OperationPlan::ContinuationCleanup { boundary, .. } => cleanup_site(state, boundary),
            OperationPlan::ReleaseReference {
                statement_index, ..
            } => GapSite::Statement {
                state,
                index: *statement_index,
            },
            _ => GapSite::State(state),
        };
        for cleanup in control
            .operation_actions(operation)
            .filter_map(checked_trees::CheckedNormalCleanupAction::nominal)
        {
            closure.require_entry(caller, cleanup.cleanup_machine, cleanup.cleanup_state, site);
        }
    }

    /*
        Prune both catalogs to the bodies whose every call dependency is available,
        returning one gap per dropped machine that names the direct dependency
        that failed at its call site.
        Completion and continuation actions publish their hook dependencies in the
        same body vocabulary as authored calls; no synthetic dispatcher edge exists.
    */
    pub(super) fn retain_available(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        boundary_symbols: &[SymbolHandle],
        cleanup_control: &checked_trees::CheckedCleanupControlPlans,
        candidates: &mut Vec<CheckedUnitEffectMachinePlan>,
        composed_machines: &mut Vec<CheckedComposedUnitControlMachinePlan>,
        partial_affine_machines: &mut Vec<CheckedPartialAffineUnitCleanupMachinePlan>,
        prepared_calls: &super::structural_value_calls::PreparedCalls,
    ) -> Vec<CheckedUnitPlanGap> {
        let partial_base = candidates.len() + composed_machines.len();
        let mut closure = CandidateClosure::new(
            candidates
                .iter()
                .map(|plan| (plan.machine, plan.state))
                .chain(composed_machines.iter().map(|plan| {
                    (
                        plan.machine,
                        plan.states
                            .first()
                            .map_or(SymbolHandle::invalid(), |state| state.state),
                    )
                }))
                .chain(
                    partial_affine_machines
                        .iter()
                        .map(|plan| (plan.machine.machine, plan.machine.state)),
                ),
        );
        let mut boundaries = boundary_symbols
            .iter()
            .copied()
            .map(symbol_key)
            .collect::<Vec<_>>();
        boundaries.sort_unstable();
        boundaries.dedup();
        let ordinary_bodies = candidates.iter().enumerate().chain(
            partial_affine_machines
                .iter()
                .enumerate()
                .map(|(index, plan)| (partial_base + index, &plan.machine)),
        );
        for (caller_index, plan) in ordinary_bodies {
            if !closure.retained[caller_index] {
                continue;
            }
            for operation in plan
                .operations
                .iter()
                .chain(prepared_calls.operations_for(plan.machine, plan.state))
                .flat_map(OperationPlan::with_value_calls)
            {
                match operation {
                    OperationPlan::Call(call) => admit_call_target(
                        &mut closure,
                        &boundaries,
                        candidates.len(),
                        partial_base,
                        caller_index,
                        call,
                        || {
                            scalar_targets::available_target(
                                program,
                                facts,
                                candidates,
                                composed_machines,
                                partial_affine_machines,
                                &scalar_targets::ScalarCallSite::of_plan(plan),
                                operation,
                            )
                        },
                        GapSite::Statement {
                            state: plan.state,
                            index: call.coordinate.statement_index,
                        },
                    ),
                    OperationPlan::RegisterAccess { .. }
                    | OperationPlan::GlobalDescriptorTable { .. }
                    | OperationPlan::EvaluateStructuralValue { .. }
                    | OperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. },
                    )
                    | checked_trees::reference_hold_pattern!()
                    | checked_trees::byte_sequence_establishment_pattern!()
                    | OperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. },
                    )
                    | OperationPlan::Store(_)
                    | OperationPlan::AtomicFence { .. }
                    | OperationPlan::AtomicAccess(_)
                    | checked_trees::structural_field_write_pattern!()
                    | OperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. },
                    )
                    | OperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::Structural { .. },
                    )
                    | OperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. },
                    )
                    | OperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. },
                    ) => {}
                    OperationPlan::Complete { .. } => {
                        for cleanup in cleanup_control
                            .normal_actions(plan.normal_exit_cleanups)
                            .filter_map(checked_trees::CheckedNormalCleanupAction::nominal)
                        {
                            closure.require_entry(
                                caller_index,
                                cleanup.cleanup_machine,
                                cleanup.cleanup_state,
                                GapSite::State(plan.state),
                            );
                        }
                    }
                    OperationPlan::ContinuationCleanup { .. } => {}
                }
                admit_operation_cleanups(
                    &mut closure,
                    caller_index,
                    cleanup_control,
                    plan.state,
                    operation,
                );
                if !closure.retained[caller_index] {
                    break;
                }
            }
            if !closure.retained[caller_index] {
                continue;
            }
        }
        for (composed_index, plan) in composed_machines.iter().enumerate() {
            let caller_index = candidates.len() + composed_index;
            if !closure.retained[caller_index] {
                continue;
            }
            for state in &plan.states {
                for action in cleanup_control.normal_actions(state.normal_exit_cleanups) {
                    if let checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) =
                        action
                    {
                        closure.require_entry(
                            caller_index,
                            cleanup.cleanup_machine,
                            cleanup.cleanup_state,
                            GapSite::State(state.state),
                        );
                    }
                }
                for operation in state
                    .operation_dependencies()
                    .chain(prepared_calls.operations_for(plan.machine, state.state))
                    .flat_map(OperationPlan::with_value_calls)
                {
                    let call_site =
                        |coordinate: checked_trees::CheckedUnitCallCoordinate| GapSite::Statement {
                            state: state.state,
                            index: coordinate.statement_index,
                        };
                    match operation {
                        OperationPlan::Call(call) => admit_call_target(
                            &mut closure,
                            &boundaries,
                            candidates.len(),
                            partial_base,
                            caller_index,
                            call,
                            || {
                                scalar_targets::ScalarCallSite::of_composed_operation(
                                    program, state, operation,
                                )
                                .and_then(|site| {
                                    scalar_targets::available_target(
                                        program,
                                        facts,
                                        candidates,
                                        composed_machines,
                                        partial_affine_machines,
                                        &site,
                                        operation,
                                    )
                                })
                            },
                            call_site(call.coordinate),
                        ),
                        OperationPlan::EvaluateStructuralValue { .. }
                        | OperationPlan::Store(_)
                        | OperationPlan::AtomicFence { .. }
                        | OperationPlan::AtomicAccess(_)
                        | OperationPlan::RegisterAccess { .. }
                        | OperationPlan::GlobalDescriptorTable { .. }
                        | OperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { .. },
                        )
                        | checked_trees::reference_hold_pattern!()
                        | checked_trees::byte_sequence_establishment_pattern!()
                        | OperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Scalar { .. },
                        )
                        | OperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { .. },
                        )
                        | OperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Primitive { .. },
                        )
                        | OperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::TrivialAffine { .. },
                        )
                        | OperationPlan::EstablishLocal(
                            checked_trees::CheckedUnitLocalBindingPlan::Structural { .. },
                        )
                        | checked_trees::structural_field_write_pattern!() => {}
                        OperationPlan::ContinuationCleanup { .. } => {}
                        _ => closure.drop_candidate(
                            caller_index,
                            GapReason::Unsupported(std::borrow::Cow::Borrowed(
                                "composed control vocabulary",
                            )),
                            GapSite::State(state.state),
                        ),
                    }
                    admit_operation_cleanups(
                        &mut closure,
                        caller_index,
                        cleanup_control,
                        state.state,
                        operation,
                    );
                    if !closure.retained[caller_index] {
                        break;
                    }
                }
                if !closure.retained[caller_index] {
                    break;
                }
            }
        }
        let machines = candidates
            .iter()
            .map(|plan| plan.machine)
            .chain(composed_machines.iter().map(|plan| plan.machine))
            .chain(
                partial_affine_machines
                    .iter()
                    .map(|plan| plan.machine.machine),
            )
            .collect::<Vec<_>>();
        let (retained, causes) = closure.close();
        let surviving = machines
            .iter()
            .zip(&retained)
            .filter(|(_, retained)| **retained)
            .map(|(machine, _)| symbol_key(*machine))
            .collect::<BTreeSet<_>>();
        let mut reported = BTreeSet::new();
        let gaps = machines
            .iter()
            .zip(&causes)
            .filter_map(|(machine, cause)| {
                let (reason, site) = cause.clone()?;
                let key = symbol_key(*machine);
                (!surviving.contains(&key) && reported.insert(key))
                    .then(|| plan_gap(program, *machine, site, reason))
            })
            .collect();
        let mut retained = retained.into_iter();
        candidates.retain(|_| retained.next().unwrap_or(false));
        composed_machines.retain(|_| retained.next().unwrap_or(false));
        partial_affine_machines.retain(|_| retained.next().unwrap_or(false));
        gaps
    }

    /*
        The authored statement a continuation cleanup runs at, or its state when
        the cleanup follows a call by coordinate.
    */
    fn cleanup_site(state: SymbolHandle, boundary: &CleanupBoundary) -> GapSite {
        let index = match boundary {
            CleanupBoundary::Call(coordinate) => coordinate.statement_index,
            CleanupBoundary::Replacement {
                statement_index, ..
            }
            | CleanupBoundary::StatementEntry { statement_index } => *statement_index,
        };
        GapSite::Statement { state, index }
    }
}
#[path = "execution_terminal_unit_cleanup.rs"]
pub(crate) mod cleanup;
mod composed_control {
    /*
        Atomic multi-state Unit control plans.
    */
    mod assembly {
        /*
            Atomic assembly of state-graph Unit control plans after provider admission.
        */
        use super::super::{
            CheckFacts, CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan,
        };

        pub(in crate::execution::terminal_unit) fn finish(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: &typed_trees::machine::Machine,
            states: Vec<CheckedComposedUnitControlStatePlan>,
        ) -> Result<CheckedComposedUnitControlMachinePlan, &'static str> {
            let body_qualifications =
                super::super::types::body_structural_qualifications(program, facts, machine.symbol);
            if !body_qualifications.is_empty() {
                return Err("state graph: finish: structural qualification has no value producer");
            }
            let contract = facts
                .contract_plans
                .for_machine(machine.symbol)
                .ok_or("state graph: finish: machine contract is absent")?;
            let machine_reach = facts
                .service_reaches
                .for_machine(machine.symbol)
                .ok_or("state graph: finish: machine reach is absent")?;
            Ok(CheckedComposedUnitControlMachinePlan {
                machine: machine.symbol,
                result: checked_trees::CheckedControlResultPlan::Unit,
                result_reference_sources: Vec::new(),
                natural_ranks: Vec::new(),
                rank_components: Default::default(),
                preserving_rank_edges: Vec::new(),
                body_qualifications,
                contract_report_fingerprint: contract.report_fingerprint,
                contract_commitment: contract.commitment,
                contract_service_reach: facts
                    .service_reaches
                    .plan_for_machine(machine.symbol)
                    .ok_or("state graph: finish: contract reach plan is absent")?,
                service_reach: language_semantics::ServiceReachSummary {
                    direct: machine_reach.inferred_direct,
                    transitive: machine_reach.inferred_transitive,
                },
                states,
            })
        }
    }

    pub(super) use assembly::finish as finish_state_graph;
}
#[path = "execution_terminal_unit_control.rs"]
pub(crate) mod control;
mod initialized_locals {
    /*
        The `let` a structural result initializes, recorded on the result's
        binding plan where planning pairs the result with its home. Lowering then
        names the local, its declared type and its binding's custody from the plan
        instead of re-reading the authored statement.

        The custody classifies the type the binding holds. A declared view
        (`let v: &[u8] = coll[a..b]`) holds its declaration; an element-declared
        view (`let v: u8 = coll[a..b]`) holds the initializer's own view result.
        Exclusive descriptors retain their declared mutable or write-only access.
        Treating a returned write-only descriptor as owned would lose its lending
        contract; treating it as mutable would manufacture observation authority.
    */
    use crate::flow::expressions;
    use checked_trees::{CheckedInitializedLocal, CheckedStructuralAccess};
    use typed_trees::data::DataMember;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    pub(in crate::execution::terminal_unit) fn initialized_local(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        local: &typed_trees::statement::TableLocalData,
    ) -> CheckedInitializedLocal {
        let bound = bound_type(program, machine, state, local);
        let access = if is_shared_borrow(program, bound) {
            CheckedStructuralAccess::SharedBorrow
        } else if let Some(shell) = validation::reference_shell(program, bound)
            && shell.access.is_exclusive()
        {
            shell.access.into()
        } else {
            CheckedStructuralAccess::Owned
        };
        CheckedInitializedLocal {
            symbol: local.symbol,
            declared_type: local.type_reference,
            access,
        }
    }

    fn bound_type(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        local: &typed_trees::statement::TableLocalData,
    ) -> TypeReferenceHandle {
        if validation::reference_shell(program, local.type_reference).is_some()
            || !expressions(program).expression_is_valid(local.initial_value)
        {
            return local.type_reference;
        }
        validation::expression_result_type_reference(program, machine, state, local.initial_value)
            .filter(|result| {
                result.is_valid() && validation::slice_view_shell(program, *result, &[]).is_some()
            })
            .unwrap_or(local.type_reference)
    }

    /*
        A shared borrow observes its referent without moving it: a named record,
        a slice view, or a primitive with linear owned contents. Constraint shells
        qualify the carrier without changing its custody (`&[u8] in Utf8` is a
        shared view). A sum referent and a `&mut` carrier are not shared-borrow
        bindings.
    */
    fn is_shared_borrow(
        program: &dyn validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> bool {
        if validation::slice_view_shell(program, reference, &[])
            .is_some_and(|view| view.access == language_semantics::ReferenceAccess::Shared)
        {
            return true;
        }
        let Some(shell) = validation::reference_shell(program, reference) else {
            return false;
        };
        if shell.access != language_semantics::ReferenceAccess::Shared {
            return false;
        }
        if program.primitive_type_reference(shell.referee).is_some() {
            return validation::has_contents(
                program,
                shell.referee,
                &[],
                validation::ContentsRequirement::LinearOwned,
            );
        }
        let TypeReferenceNode::Named { symbol, .. } =
            program.type_reference_table.type_reference(shell.referee)
        else {
            return false;
        };
        program
            .data_definition_by_symbol(*symbol)
            .is_some_and(|record| {
                !program
                    .data_members(record)
                    .iter()
                    .any(|member| matches!(member, DataMember::Variant(_)))
            })
    }
}
mod local_storage {

    /*
        Named storage and immutable production results have different lifetimes.
        A rebind transfers its RHS into a local home and retires that production
        result; later operands and cleanup must name the home's current value,
        not revive the local's initializer. Consumers use this same ordered
        operation-prefix query. It selects storage identity, never access authority.
    */
    use checked_trees::{
        CheckedUnitEffectOperationPlan as OperationPlan,
        CheckedUnitStructuralArgumentSourcePlan as SourcePlan,
    };
    use symbols::SymbolHandle;
    use typed_trees::statement::StatementNode;

    pub(crate) fn source(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        operations: &[OperationPlan],
        symbol: SymbolHandle,
    ) -> Option<SourcePlan> {
        for operation in operations.iter().rev() {
            if operation.ends_local_occurrence(symbol) {
                return Some(SourcePlan::StructuralLocal { symbol });
            }
            if matches!(operation, OperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Structural { symbol: local, .. }) if *local == symbol)
            {
                return Some(SourcePlan::StructuralLocal { symbol });
            }
            if let Some(result) = operation.statement_structural_result_binding()
                && matches!(program.statement_table.statements(state.statement_nodes).get(result.statement_index as usize),
                    Some(StatementNode::LocalData(local)) if local.symbol == symbol)
            {
                return Some(SourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                });
            }
        }
        None
    }

    /*
        The production still occupying `symbol`'s home after `operations`: the
        binding ordinal of its declaration's establishment or initializer result.
        A rebind has replaced every earlier production, so none occupies it. An
        in-place declaration is still that declaration's own production even
        though operands name its home rather than the result.
    */
    pub(crate) fn occupant(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        operations: &[OperationPlan],
        symbol: SymbolHandle,
    ) -> Option<u32> {
        match source(program, state, operations, symbol)? {
            SourcePlan::StructuralResult { binding_ordinal } => Some(binding_ordinal),
            _ => operations
                .iter()
                .rev()
                .find_map(|operation| match operation {
                    _ if operation.ends_local_occurrence(symbol) => Some(None),
                    OperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::Structural {
                            symbol: local,
                            result,
                            ..
                        },
                    ) if *local == symbol => Some(Some(result.binding_ordinal)),
                    _ => None,
                })?,
        }
    }

    /*
        Resolve one owned operand to its already produced value in this operation
        prefix. Expression roots name immutable producers, while named homes select
        their current occupant. A future or ambiguous producer grants no custody.
    */
    pub(crate) fn operand_source(
        program: &dyn validation::CheckingInput,
        state: &typed_trees::state::State,
        operations: &[OperationPlan],
        results: &[(
            checked_trees::CheckedUnitStructuralResultBindingPlan,
            facts::PlaceRoot,
        )],
        operand: &SourcePlan,
    ) -> Option<SourcePlan> {
        match operand {
            SourcePlan::StructuralLocal { symbol } => source(program, state, operations, *symbol),
            SourcePlan::StructuralExpression { expression, .. } => {
                let mut matching = results.iter().filter(|(binding, root)| {
                    *root == facts::PlaceRoot::Expression(*expression)
                        && operations
                            .iter()
                            .any(|operation| operation.structural_result_binding() == Some(binding))
                });
                let (result, _) = matching.next()?;
                if matching.next().is_some() {
                    return None;
                }
                Some(SourcePlan::StructuralResult {
                    binding_ordinal: result.binding_ordinal,
                })
            }
            source => Some(source.clone()),
        }
    }
}
pub(crate) mod plan_gaps {
    /*
        Join a machine's missing executable plan to the authored source.

        A gap names the construct plan construction could not lower, or the call
        whose callee has no body, at the statement, state or machine where it sits.
        The span and coverage form chosen here are what `omega --check` reports
        when lowering or entry establishment demands that body; an undemanded gap
        (a proof lemma over `Nat`, an unused library body) is never reported.
    */
    use checked_trees::{CheckedUnitPlanGap, CheckedUnitPlanGapReason};
    use diagnostics::CoverageForm;
    use source::{SourceSpan, Span};
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::StatementNode;

    /* Where in a machine's body a gap sits. */
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum GapSite {
        /* The machine as a whole: its declaration or signature. */
        Machine,
        /* One authored state's signature or its cleanup obligations. */
        State(SymbolHandle),
        /* One statement, by its position in `state`'s statement list. */
        Statement { state: SymbolHandle, index: u32 },
    }

    pub(crate) fn plan_gap(
        program: &dyn ::validation::CheckingInput,
        machine: SymbolHandle,
        site: GapSite,
        reason: CheckedUnitPlanGapReason,
    ) -> CheckedUnitPlanGap {
        let declaration = |symbol: SymbolHandle| {
            program
                .symbols
                .symbol_provenance_source_span(symbol)
                .unwrap_or_default()
        };
        let machine_span = declaration(machine);
        let (coverage, source_span) = match site {
            GapSite::Machine => (CoverageForm::MachineHeader, machine_span),
            GapSite::State(state) => (
                CoverageForm::StateSignature,
                authored_or(declaration(state), machine_span),
            ),
            GapSite::Statement { state, index } => {
                let state_span = authored_or(declaration(state), machine_span);
                match program.state_by_symbol(state).and_then(|state| {
                    crate::flow::state_statements(program, state.statement_nodes)
                        .get(index as usize)
                }) {
                    Some(statement) => {
                        let (coverage, span) = statement_site(program, statement);
                        (coverage, authored_or(span, state_span))
                    }
                    None => (CoverageForm::StatementForm, state_span),
                }
            }
        };
        CheckedUnitPlanGap {
            machine,
            state: match site {
                GapSite::Machine => SymbolHandle::default(),
                GapSite::State(state) | GapSite::Statement { state, .. } => state,
            },
            statement_index: match site {
                GapSite::Statement { index, .. } => Some(index),
                _ => None,
            },
            reason,
            source_span,
            coverage,
        }
    }

    /* The coverage form and authored extent of one statement. */
    fn statement_site(
        program: &dyn ::validation::CheckingInput,
        statement: &StatementNode,
    ) -> (CoverageForm, SourceSpan) {
        let expression = |handle: ExpressionHandle| {
            if handle.is_valid() {
                program.expression_table.source_span(handle)
            } else {
                SourceSpan::default()
            }
        };
        match statement {
            StatementNode::Assembly(_) => (CoverageForm::StatementForm, SourceSpan::default()),
            StatementNode::Call(call) => (CoverageForm::CallForm, call.source_span),
            StatementNode::Transition(transition) => {
                (CoverageForm::TransitionForm, transition.source_span)
            }
            StatementNode::Assignment(assignment) => (
                CoverageForm::StatementForm,
                joined(expression(assignment.target), expression(assignment.value)),
            ),
            StatementNode::LocalData(local) => (
                CoverageForm::StatementForm,
                joined(
                    program
                        .symbols
                        .symbol_source_span(local.symbol)
                        .unwrap_or_default(),
                    expression(local.initial_value),
                ),
            ),
            StatementNode::Expression(handle) => {
                (CoverageForm::ExpressionForm, expression(*handle))
            }
            StatementNode::AssemblyFact(fact) => (
                CoverageForm::StatementForm,
                program
                    .proof_fact_source_span(fact.fact)
                    .unwrap_or_default(),
            ),
        }
    }

    fn is_authored(span: SourceSpan) -> bool {
        span != SourceSpan::default()
    }

    fn authored_or(span: SourceSpan, fallback: SourceSpan) -> SourceSpan {
        if is_authored(span) { span } else { fallback }
    }

    /*
        The extent covering both spans when they share a source; otherwise the
        first authored one.
    */
    fn joined(first: SourceSpan, second: SourceSpan) -> SourceSpan {
        match (is_authored(first), is_authored(second)) {
            (true, true) if first.source_id == second.source_id => SourceSpan::new(
                first.source_id,
                Span::new(
                    first.span.start.min(second.span.start),
                    first.span.end.max(second.span.end),
                ),
            ),
            (false, true) => second,
            _ => first,
        }
    }
}
mod receiver_aliases {
    /*
        Erase nonescaping reference carriers, not their captured referent or access.
    */
    use super::{
        CheckFacts, CheckedStructuralAccess, ExpressionNode, SymbolHandle, TypeReferenceHandle,
    };
    use crate::execution::terminal_unit::types::{
        lent_storage_identity, state_flow, structural_access_for_type_reference,
    };
    use crate::flow::{expressions, state_statements};

    use crate::execution::terminal_unit::calls;

    use checked_trees::BorrowAccessKind;
    use typed_trees::expression::ExpressionHandle;

    mod projection {
        /*
            Join source coordinates to checked capture and normalized receiver paths.
        */
        use super::{ExpressionHandle, ExpressionNode};
        use crate::flow::expressions;
        use crate::published::places::place_at;
        pub(super) fn formation_place(
            program: &dyn ::validation::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            expression: ExpressionHandle,
        ) -> Option<(validation::CanonicalPlace, checked_trees::CapturedPlace)> {
            if !validation::place_has_builtin_coordinates(program, machine, Some(state), expression)
            {
                return None;
            }
            let mut current = expression;
            let mut visited = Vec::new();
            loop {
                if !program.expression_table.expression_is_valid(current)
                    || visited.contains(&current)
                {
                    return None;
                }
                visited.push(current);
                match program.expression_table.expression(current) {
                    ExpressionNode::Name(name) => {
                        let symbols =
                            expressions(program).name_path_member_symbols(name.member_symbols);
                        let members = program.expression_table.name_path_members(name.members);
                        if !name.symbol.is_valid()
                            || !name.head_symbol.is_valid()
                            || members.is_empty()
                            || (members.len() == 1
                                && (name.symbol != name.head_symbol
                                    || symbols
                                        .first()
                                        .is_some_and(|symbol| *symbol != name.symbol)))
                            || (members.len() > 1
                                && (symbols.len() != members.len()
                                    || symbols.iter().any(|symbol| !symbol.is_valid())
                                    || symbols.first() != Some(&name.head_symbol)
                                    || symbols.last() != Some(&name.symbol)))
                        {
                            return None;
                        }
                        break;
                    }
                    ExpressionNode::Member(member) => {
                        if validation::exact_self_field(program, machine, current).is_some() {
                            current = member.receiver;
                            continue;
                        }
                        let receiver_type = validation::expression_type_reference_in_state(
                            program,
                            state.symbol,
                            statement_index,
                            member.receiver,
                        )?;
                        let receiver_type =
                            validation::unwrapped_type_reference(program, receiver_type)?;
                        let resolved = validation::resolve_member_symbol_from_type_symbol(
                            program,
                            program.type_reference_table.type_symbol(receiver_type),
                            member.member.as_str(),
                        )?;
                        if member.member_symbol.is_valid() && member.member_symbol != resolved {
                            return None;
                        }
                        current = member.receiver;
                    }
                    ExpressionNode::Indexed(indexed) => {
                        if !program.expression_table.expression_is_valid(indexed.index)
                            || !matches!(
                                validation::index_place_segment(program, indexed.index),
                                facts::PlaceSegment::FixedIndex { .. }
                                    | facts::PlaceSegment::Index { .. }
                            )
                        {
                            return None;
                        }
                        current = indexed.collection;
                    }
                    _ => return None,
                }
            }
            let capture = crate::borrow::accesses::borrow_access_place(
                program,
                state.symbol,
                statement_index,
                expression,
                machine.symbol,
            )?;
            let place = place_at(program, state.symbol, statement_index, expression)?;

            if !place
                .segments
                .iter()
                .chain(&capture.segments)
                .all(|segment| {
                    matches!(segment,
                facts::PlaceSegment::Field { symbol } if symbol.is_valid())
                        || matches!(
                            segment,
                            facts::PlaceSegment::FixedIndex { .. }
                                | facts::PlaceSegment::Index { .. }
                        )
                })
            {
                return None;
            }
            Some((place, capture))
        }
    }

    pub(super) struct ReceiverAlias {
        pub(super) owner: SymbolHandle,
        /* Immediate captured storage, before chaining through an erased parent. */
        captured_place: validation::CanonicalPlace,
        pub(super) root: SymbolHandle,
        pub(super) segments: Vec<facts::PlaceSegment>,
        /*
            The erased loan's exact access and terminal statement. A bare-argument
            move records only a carrier read; this evidence restores the authority
            that argument actually forwards.
        */
        pub(super) kind: BorrowAccessKind,
        pub(super) last_use: usize,
    }

    /*
        Consume the checked occurrence of an immutable exclusive carrier. Its
        source path chooses runtime storage, while its loan supplies the captured
        access and last use. Direct roots and immediate-parent reborrows have the
        same erasure operation; checking owns their different resource obligations.
    */
    fn formation(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        borrow_state: &checked_trees::StateBorrowFact,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        local: &typed_trees::statement::TableLocalData,
        aliases: &[ReceiverAlias],
    ) -> Option<ReceiverAlias> {
        if local.is_mutable
            || !local.symbol.is_valid()
            || aliases.iter().any(|alias| alias.owner == local.symbol)
        {
            return None;
        }
        let access = exclusive_access(program, local.type_reference)?;
        let borrow = expressions(program).borrow_expression(local.initial_value)?;
        if !matches!(
            (&access, borrow.access),
            (
                BorrowAccessKind::Mutable,
                language_semantics::ReferenceAccess::Mutable
            ) | (
                BorrowAccessKind::WriteOnly,
                language_semantics::ReferenceAccess::WriteOnly
            )
        ) {
            return None;
        }
        let (place, _) =
            projection::formation_place(program, machine, state, statement_index, borrow.target)?;
        if place
            .segments
            .iter()
            .any(|segment| matches!(segment, facts::PlaceSegment::Index { .. }))
        {
            return None;
        }
        let source_root = place.root.as_symbol()?;
        let projected_type = calls::projected_borrowed_path(
            program,
            machine.symbol,
            state.symbol,
            statement_index,
            &place,
        )?
        .storage_type;
        if lent_storage_identity(program, projected_type, &[])?
            != lent_storage_identity(program, local.type_reference, &[])?
        {
            return None;
        }
        let mut loans = facts
            .borrow
            .loan_group(borrow_state.loans)?
            .filter(|(_, loan)| {
                loan.owner_symbol == local.symbol
                    && loan.statement_index == statement_index
                    && facts.borrow.loan_owner_path(loan).is_empty()
            });
        let (loan_handle, loan) = loans.next()?;
        if loans.next().is_some() || loan.kind != access {
            return None;
        }
        let has_resource = facts
            .borrow
            .direct_loan_resources
            .iter()
            .any(|(_, resource)| resource.loan == loan_handle)
            || facts
                .borrow
                .reborrow_loan_resources
                .iter()
                .any(|(_, resource)| resource.loan == loan_handle);
        if !has_resource {
            return None;
        }
        let (root, segments) = match aliases.iter().find(|alias| alias.owner == source_root) {
            Some(parent) => {
                let mut segments = parent.segments.clone();
                segments.extend_from_slice(&place.segments);
                (parent.root, segments)
            }
            None => (source_root, place.segments.clone()),
        };
        Some(ReceiverAlias {
            owner: local.symbol,
            captured_place: place,
            root,
            segments,
            kind: access,
            last_use: loan.last_use_statement_index,
        })
    }

    /*
        Collect nonescaping exclusive carriers in declaration order. The operation
        consumes checked resource identity; it imposes no sibling count, parent
        retirement shape, or declaration-prefix requirement. Materialized descriptors
        remain runtime values and stop substitution at their own storage.
    */
    pub(super) fn aliases(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
    ) -> Option<Vec<ReceiverAlias>> {
        state_flow(facts, machine.symbol, state.symbol)?;
        let (_, borrow_state) = facts
            .borrow
            .first_state_borrow(machine.symbol, state.symbol)?;
        if borrow_state.loans.is_empty() {
            return Some(Vec::new());
        }
        let mut aliases = Vec::new();
        for (statement_index, statement) in state_statements(program, state.statement_nodes)
            .iter()
            .enumerate()
        {
            let Some(local) = statement.as_local_data() else {
                continue;
            };
            if let Some(alias) = formation(
                program,
                facts,
                borrow_state,
                machine,
                state,
                statement_index,
                local,
                &aliases,
            ) {
                aliases.push(alias);
            }
        }
        Some(aliases)
    }

    fn exclusive_access(
        program: &dyn ::validation::CheckingInput,
        reference: TypeReferenceHandle,
    ) -> Option<BorrowAccessKind> {
        match structural_access_for_type_reference(program, reference)? {
            CheckedStructuralAccess::MutableBorrow => Some(BorrowAccessKind::Mutable),
            CheckedStructuralAccess::WriteOnlyBorrow => Some(BorrowAccessKind::WriteOnly),
            _ => None,
        }
    }

    /*
        Follow erased carriers only as far as the nearest materialized storage.
        A captured descriptor is the authority for its referent; flattening through
        that descriptor would instead lend its suspended backing a second time.
    */
    pub(super) fn resolve(
        aliases: &[ReceiverAlias],
        place: &validation::CanonicalPlace,
        materialized: impl Fn(facts::PlaceRoot) -> bool,
    ) -> Option<validation::CanonicalPlace> {
        let mut resolved = place.clone();
        for _ in 0..aliases.len() {
            if materialized(resolved.root) {
                break;
            }
            let Some(owner) = resolved.root.as_symbol() else {
                break;
            };
            let Some(alias) = aliases.iter().find(|alias| alias.owner == owner) else {
                break;
            };
            let mut segments = alias.captured_place.segments.clone();
            segments.extend(resolved.segments);
            resolved = validation::CanonicalPlace {
                root: alias.captured_place.root,
                segments,
            };
        }
        (resolved != *place).then_some(resolved)
    }
}
use validation::reference_result_custody as reference_results;
mod scalar_locals {
    /*
        Primitive locals at authored statement and dense binding positions.
    */
    use super::{
        CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole,
        CheckedUnitScalarResultBindingPlan,
    };
    pub(super) fn scalar_expression_local_at(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        statement_index: u32,
        binding_ordinal: u32,
        local: &typed_trees::statement::TableLocalData,
    ) -> Option<(CheckedUnitScalarResultBindingPlan, CheckedScalarExpression)> {
        if local.is_mutable {
            return None;
        }
        let primitive_type = program.primitive_type_reference(local.type_reference)?;
        let (custody, value) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_index,
            CheckedScalarExpressionRole::LocalInitializer { binding_ordinal },
        )?;
        if custody.expression != local.initial_value
            || custody.destination != local.symbol
            || crate::values::scalar_expression_type(value) != Some(primitive_type)
        {
            return None;
        }
        Some((
            CheckedUnitScalarResultBindingPlan {
                statement_index,
                binding_ordinal,
                primitive_type,
            },
            value.clone(),
        ))
    }
}
pub(crate) mod scalar_return_expressions {
    /*
        Classification of scalar and boolean return expressions.
    */
    pub(crate) fn checked_boolean_contains_short_circuit(
        expression: &checked_trees::CheckedBooleanExpression,
    ) -> bool {
        match expression {
            checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
            checked_trees::CheckedBooleanExpression::And { .. }
            | checked_trees::CheckedBooleanExpression::Or { .. } => true,
            checked_trees::CheckedBooleanExpression::Not(operand) => {
                checked_boolean_contains_short_circuit(operand)
            }
            checked_trees::CheckedBooleanExpression::Equal { left, right } => {
                checked_boolean_contains_short_circuit(left)
                    || checked_boolean_contains_short_circuit(right)
            }
            checked_trees::CheckedBooleanExpression::Constant(_)
            | checked_trees::CheckedBooleanExpression::Parameter { .. }
            | checked_trees::CheckedBooleanExpression::ErasedParameter { .. }
            | checked_trees::CheckedBooleanExpression::Local { .. }
            | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
            | checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
            | checked_trees::CheckedBooleanExpression::IntegerMathComparison { .. }
            | checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
            | checked_trees::CheckedBooleanExpression::StructuralIdentity { .. }
            | checked_trees::CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
            | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
            | checked_trees::CheckedBooleanExpression::ByteSequencePredicate { .. }
            | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
        }
    }
}
mod scalar_targets {

    /*
        Ordinary scalar calls retain an existing body plan, never a fabricated one:
        the callee's scalar-result state graph or its ordinary scalar-result body.
        Availability closes dependencies, while the receiving lowerer independently
        replays every retained operation and its authored source custody.
    */
    use super::{
        CarryPolicy, CheckFacts, CheckedUnitEffectMachinePlan,
        CheckedUnitEffectOperationPlan as OperationPlan, MachineSupplyMode, SignatureContractKind,
        SymbolHandle,
    };
    use crate::execution::terminal_unit::types::{ShapeCollector, machine_binders, state_flow};

    use crate::execution::terminal_unit::calls::{
        call_claim_transfers, entry_claims, free_structural_scalar_signature,
    };

    /*
        The body a scalar call depends on, as an edge into the changing candidate
        roster: an ordinary single-state body by its index among the ordinary
        candidates, and a multi-state state graph by its index among the composed
        candidates. A caller survives pruning only while its callee's body does.
    */
    pub(super) enum AvailableScalarTarget {
        Ordinary(usize),
        Composed(usize),
        /*
            An ordinary single-state body published through the partial-affine
            carrier, by its index among those candidates.
        */
        PartialAffine(usize),
    }

    /*
        The call-site half of a scalar-call availability check. Ordinary machine
        plans own their fields directly; composed-control bodies hold the same
        vocabulary on each retained state, so both route through this view.
    */
    pub(super) struct ScalarCallSite<'a> {
        pub machine: SymbolHandle,
        pub state: SymbolHandle,
        pub operations: Vec<&'a OperationPlan>,
        pub structural_parameters: &'a [checked_trees::CheckedUnitStructuralParameterPlan],
    }

    impl ScalarCallSite<'_> {
        pub(super) fn of_plan(plan: &CheckedUnitEffectMachinePlan) -> ScalarCallSite<'_> {
            ScalarCallSite {
                machine: plan.machine,
                state: plan.state,
                operations: plan.operations.iter().collect(),
                structural_parameters: &plan.structural_parameters,
            }
        }

        pub(super) fn of_composed_operation<'a>(
            program: &dyn ::validation::CheckingInput,
            state: &'a checked_trees::CheckedComposedUnitControlStatePlan,
            operation: &OperationPlan,
        ) -> Option<ScalarCallSite<'a>> {
            let machine = program.machine_holding_state(state.state)?.symbol;
            let owns = |operations: &[&OperationPlan]| {
                operations
                    .iter()
                    .flat_map(|operation| operation.with_value_calls())
                    .any(|candidate| std::ptr::eq(candidate, operation))
            };
            let mut operations = state.operations.iter().collect::<Vec<_>>();
            if !owns(&operations) {
                let mut selected = None;
                for arm in state.terminator.return_arms() {
                    for dependencies in arm.operation_scopes() {
                        if owns(&dependencies) && selected.replace(dependencies).is_some() {
                            return None;
                        }
                    }
                }
                if let checked_trees::CheckedComposedUnitControlTerminatorPlan::CompleteExpression {
                completion,
            } = &state.terminator
            {
                for operations in completion.expression.invocation_scopes() {
                    let dependencies = operations.iter().collect::<Vec<_>>();
                    if owns(&dependencies) && selected.replace(dependencies).is_some() {
                        return None;
                    }
                }
            }
                if let checked_trees::CheckedComposedUnitControlTerminatorPlan::Guarded {
                    return_values,
                    ..
                } = &state.terminator
                {
                    for returned in return_values {
                        let dependencies = vec![&returned.operation];
                        if owns(&dependencies) && selected.replace(dependencies).is_some() {
                            return None;
                        }
                    }
                }
                for successor in state.successors() {
                    for transfer in &successor.transfers {
                        let dependencies = match &transfer.source {
                            checked_trees::CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                                operands, establishment, ..
                            } => operands.iter().chain(std::iter::once(establishment.as_ref())).collect::<Vec<_>>(),
                            checked_trees::CheckedStructuralControlTransferSourcePlan::EvaluateNoNormal { operations } => {
                                operations.iter().collect::<Vec<_>>()
                            }
                            _ => continue,
                        };
                        if owns(&dependencies) && selected.replace(dependencies).is_some() {
                            return None;
                        }
                    }
                }
                operations.extend(selected?);
            }
            Some(ScalarCallSite {
                machine,
                state: state.state,
                operations,
                structural_parameters: &state.structural_parameters,
            })
        }
    }

    pub(super) fn available_target(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        candidates: &[CheckedUnitEffectMachinePlan],
        composed_candidates: &[checked_trees::CheckedComposedUnitControlMachinePlan],
        partial_affine_candidates: &[checked_trees::CheckedPartialAffineUnitCleanupMachinePlan],
        caller: &ScalarCallSite<'_>,
        operation: &OperationPlan,
    ) -> Option<AvailableScalarTarget> {
        let planned = operation.call().filter(|call| call.is_module())?;
        let result = planned.scalar_result()?;
        let target_machine = &planned.target.machine;
        let target_state = &planned.target.state;
        let target_contract_report_fingerprint = &planned.target.contract_report_fingerprint;
        let target_contract_commitment = planned.contract_commitment();
        let scalar_arguments = &planned.scalar_arguments;
        let structural_arguments = &planned.structural_arguments;
        let claim_transfers = planned.claim_transfers();
        let coordinate = &planned.coordinate;
        let machine = program.machine_by_symbol(*target_machine)?;
        let composed = composed_candidates.iter().enumerate().find(|(_, plan)| {
            plan.machine == *target_machine
                && matches!(
                    plan.result,
                    checked_trees::CheckedControlResultPlan::Scalar { .. }
                )
        });
        let authored_states = program.machine_states(machine);
        let state = if composed.is_some() {
            authored_states.first()?
        } else {
            let [state] = authored_states else {
                return None;
            };
            state
        };
        let contract = facts.contract_plans.for_machine(*target_machine)?;
        let availability;
        let (structural, scalar, claims, result_type) = if let Some((composed_index, plan)) =
            composed
        {
            let checked_trees::CheckedControlResultPlan::Scalar { primitive_type } = plan.result
            else {
                return None;
            };
            let entry = plan.states.first()?;
            if entry.state != state.symbol
                || plan.contract_report_fingerprint != contract.report_fingerprint
                || plan.contract_commitment != contract.commitment
            {
                return None;
            }
            let mut shapes = ShapeCollector::new(program);
            let (structural, scalar) = super::calls::free_structural_scalar_entry_signature_traced(
                program,
                &mut shapes,
                state,
                &[],
                &super::control::LocalConstructionTrace::default(),
            )?;
            if structural != entry.structural_parameters || scalar != entry.scalar_parameters {
                return None;
            }
            availability = AvailableScalarTarget::Composed(composed_index);
            (
                &entry.structural_parameters,
                &entry.scalar_parameters,
                entry.entry_claims.as_slice(),
                primitive_type,
            )
        } else {
            let mut targets = candidates
                .iter()
                .enumerate()
                .map(|(index, plan)| (AvailableScalarTarget::Ordinary(index), plan))
                .chain(
                    partial_affine_candidates
                        .iter()
                        .enumerate()
                        .map(|(index, plan)| {
                            (AvailableScalarTarget::PartialAffine(index), &plan.machine)
                        }),
                )
                .filter(|(_, plan)| plan.machine == *target_machine);
            let (candidate, plan) = targets.next()?;
            availability = candidate;
            let primitive_type = match (&plan.scalar_result, &plan.scalar_control) {
                (Some(completion), None) => completion.primitive_type,
                (None, Some(completion)) => completion.primitive_type,
                _ => return None,
            };
            if plan.scalar_control.as_ref().is_some_and(|completion| {
                super::control::statement_sequence::scalar_control(program, facts, machine, state)
                    .as_ref()
                    .map(|(expected, _)| expected)
                    != Some(completion)
            }) {
                return None;
            }
            if targets.next().is_some()
                || plan.state != *target_state
                || plan.structural_result.is_some()
                || plan.contract_report_fingerprint != contract.report_fingerprint
                || plan.contract_commitment != contract.commitment
                || program
                    .machine_contracts(machine)
                    .iter()
                    .chain(program.state_contracts(state))
                    .any(|contract| {
                        !matches!(
                            contract.kind,
                            SignatureContractKind::Crashes { .. }
                                | SignatureContractKind::Requires
                                | SignatureContractKind::Ensures
                        ) || contract.binding.is_some()
                    })
                || !validation::scalar_result_contract_is_represented(program, state.return_type)
            {
                return None;
            }
            let mut shapes = ShapeCollector::new(program);
            let binders = machine_binders(program, machine);
            let (structural, scalar) =
                free_structural_scalar_signature(program, &mut shapes, state, &binders)?;
            if structural != plan.structural_parameters
                || scalar != plan.scalar_parameters
                || plan.scalar_result.as_ref().is_some_and(|completion| {
                    plan.operations
                        .iter()
                        .filter(|operation| match operation {
                            OperationPlan::Call(call) => call.scalar_result() == Some(completion),
                            OperationPlan::EstablishLocal(
                                checked_trees::CheckedUnitLocalBindingPlan::Scalar {
                                    result, ..
                                },
                            ) => result == completion,
                            _ => false,
                        })
                        .count()
                        != 1
                })
            {
                return None;
            }
            (
                &plan.structural_parameters,
                &plan.scalar_parameters,
                plan.entry_claims.as_slice(),
                primitive_type,
            )
        };
        if structural_arguments.len() != structural.len()
            || structural_arguments
                .iter()
                .zip(structural)
                .any(|(argument, parameter)| {
                    argument.type_identity != parameter.type_identity
                        || argument.access != parameter.access
                })
            || !claim_transfers_match_entry_catalogue(claim_transfers, claims, structural.len())
        {
            return None;
        }
        if entry_claims(
            program,
            facts,
            machine.symbol,
            state.symbol,
            structural,
            program.state_parameters(state),
        )
        .as_deref()
            != Some(claims)
        {
            return None;
        }
        let flow = state_flow(facts, caller.machine, caller.state)?;
        let mut calls = crate::flow::call_flows(&facts.flow.control, flow.calls)
            .iter()
            .filter(|call| {
                u32::try_from(call.statement_index).ok() == Some(coordinate.statement_index)
                    && u32::try_from(call.call_ordinal).ok() == Some(coordinate.call_ordinal)
            });
        let call = calls.next()?;
        let caller_structural_results = caller
            .operations
            .iter()
            .filter_map(|operation| {
                let result = operation.structural_result_binding()?;
                let owner = super::calls::structural_result_owner(
                    program,
                    facts,
                    caller.state,
                    facts.flow.control.calls.span_or_empty(flow.calls),
                    operation,
                )?;
                Some((result.clone(), owner))
            })
            .collect::<Vec<_>>();
        if calls.next().is_some()
            || call_claim_transfers(
                program,
                facts,
                caller.machine,
                caller.state,
                call,
                caller.structural_parameters,
                &caller_structural_results,
                structural_arguments,
            )
            .as_deref()
                != Ok(claim_transfers)
        {
            return None;
        }
        let planned_positions = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| !parameter.relevance.is_erased())
            .count();
        (machine.supply_mode == MachineSupplyMode::CheckedBody
            && state.symbol == *target_state
            && planned_positions == scalar.len() + structural.len()
            && scalar_arguments.len() == scalar.len()
            && scalar_arguments
                .iter()
                .zip(scalar)
                .all(|(argument, parameter)| {
                    let primitive_type = match argument {
                        checked_trees::CheckedCallScalarArgument::Pure(expression) => {
                            crate::values::scalar_expression_type(expression)
                        }
                        checked_trees::CheckedCallScalarArgument::Computation(root) => {
                            let computations = &facts.values.scalar_computations;
                            computations
                                .nodes
                                .is_valid(*root)
                                .then(|| computations.nodes.get(*root).primitive_type)
                        }
                    };
                    primitive_type == Some(parameter.primitive_type)
                })
            && result_type == result.primitive_type
            && program.primitive_type_reference(state.return_type) == Some(result_type)
            && contract.report_fingerprint == *target_contract_report_fingerprint
            && Some(contract.commitment) == target_contract_commitment)
            .then_some(availability)
    }

    /*
        Entry claims describe every possible payload; a known sum transfers only
        its active payload. Match typed relative paths, not catalogue cardinality or
        claim ordering. Unguarded entries always require a transfer. Source flow's
        exact transfer roster is reconstructed above; consuming custody binds every
        active callee claim against the argument's actual tag. Missing an inactive
        entry is not permission to invent or omit a live claim.
    */
    fn claim_transfers_match_entry_catalogue(
        transfers: &[checked_trees::CheckedUnitClaimTransferPlan],
        claims: &[checked_trees::CheckedUnitEntryClaimPlan],
        structural_parameter_count: usize,
    ) -> bool {
        let matches = |transfer: &checked_trees::CheckedUnitClaimTransferPlan,
                       claim: &checked_trees::CheckedUnitEntryClaimPlan| {
            transfer.argument_index == claim.parameter_index && transfer.path == claim.path
        };
        claims.iter().all(|claim| {
            usize::try_from(claim.parameter_index)
                .is_ok_and(|parameter_index| parameter_index < structural_parameter_count)
                && claim.carry == CarryPolicy::STRICT
                && (claim.path.iter().any(|segment| {
                    matches!(
                        segment,
                        checked_trees::CheckedUnitStructuralPathSegment::Case(_)
                    )
                }) || transfers.iter().any(|transfer| matches(transfer, claim)))
        }) && transfers
            .iter()
            .enumerate()
            .all(|(transfer_position, transfer)| {
                claims
                    .iter()
                    .filter(|claim| matches(transfer, claim))
                    .count()
                    == 1
                    && !transfers[..transfer_position].iter().any(|previous| {
                        previous.argument_index == transfer.argument_index
                            && previous.path == transfer.path
                    })
            })
    }
}
#[path = "execution_terminal_unit_state_graph.rs"]
pub(crate) mod state_graph;
mod storage_transfers {

    /*
        Current custody follows the retained value graph into its receiving leaf.
        Possible transfers fence disposal of the original slot. Definite transfers
        additionally prove that every selected alternative moved that slot.
    */
    use crate::published::places::place_at;
    use arena::Handle;
    use checked_trees::{
        CheckFacts, CheckedCallScalarArgument, CheckedElementViewRegion, CheckedMemberIdentity,
        CheckedScalarComputationHandle, CheckedStorePlan, CheckedStoreRoot,
        CheckedStructuralRecordFieldValue, CheckedStructuralValue, CheckedStructuralValueCall,
        CheckedUnitCallCoordinate, CheckedUnitCallPlan, CheckedUnitLocalBindingPlan, FlowCallFact,
        record_or_structural_case_pattern,
    };
    use checked_trees::{
        CheckedStructuralAccess, CheckedStructuralValueKind, CheckedUnitEffectOperationPlan,
        CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
        CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
    };
    use facts::PlaceRoot;
    use language_semantics::Multiplicity;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::state::State;
    use typed_trees::statement::StatementNode;
    use typed_trees::type_identity::TypeIdentityId;

    type Results = [(CheckedUnitStructuralResultBindingPlan, PlaceRoot)];
    use checked_trees::{BorrowLoanOwnerSegment as OwnerSegment, CheckedStructuralValueHandle};
    use typed_trees::data::DataMember;

    mod completion {

        /*
            Keep referenced data alive through the last return or call argument using it.
            Read the body's prepared completion plan so cleanup follows its selected
            branch and argument order.
        */
        use super::{Destination, Query, Results, Transfers, ValueReceiver, query_for_storage};
        use checked_trees::{
            CheckFacts, CheckedComposedUnitControlTerminatorPlan, CheckedScalarExpressionRole,
            CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
            CheckedUnitScalarControlPlan, CheckedUnitStructuralArgumentSourcePlan,
            CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
            CheckedUnitStructuralReturnPlan, FlowClaimJoinExit,
        };
        use symbols::SymbolHandle;

        mod expressions {
            //! Normal alternatives retain their own invocation and the predicates that
            //! precede them. An aborted alternative cannot justify normal cleanup.

            use super::{Query, Transfers};
            use checked_trees::{
                CheckedUnitCompletionExpression, CheckedUnitCompletionKind,
                CheckedUnitCompletionPlan,
            };

            impl Query<'_> {
                pub(super) fn completion_expression(
                    &self,
                    plan: &CheckedUnitCompletionPlan,
                ) -> Result<Transfers, &'static str> {
                    self.completion_node(&plan.expression)?
                        .ok_or("completion cleanup: expression has no normal transfer path")
                }

                fn completion_node(
                    &self,
                    node: &CheckedUnitCompletionExpression,
                ) -> Result<Option<Transfers>, &'static str> {
                    if !node.normal_continuation {
                        return Ok(None);
                    }
                    if let CheckedUnitCompletionKind::Invocation(operations) = &node.kind {
                        return self.operation_sequence(operations.iter()).map(Some);
                    }
                    let subject = node
                        .subject()
                        .ok_or("completion cleanup: selected subject is missing")?;
                    let mut scope = self.clone();
                    let mut subject_transfers = scope
                        .dispatch_subject(&subject)
                        .ok_or("completion cleanup: selected subject has incomplete transfers")?;
                    match subject {
                        checked_trees::CheckedDispatchSubject::Scalar(value) => {
                            scope.completed_computations.push(value)
                        }
                        checked_trees::CheckedDispatchSubject::Structural { value, .. }
                            if value.is_valid() =>
                        {
                            scope.established_values.push(value)
                        }
                        checked_trees::CheckedDispatchSubject::Structural { .. } => {}
                    }
                    let mut prefixes = Vec::new();
                    for (pattern, _) in node.arms() {
                        let mut transfers = Transfers::default();
                        if let Some(computation) = pattern {
                            transfers = scope.computation(computation).ok_or(
                                "completion cleanup: selected pattern has incomplete transfers",
                            )?;
                            scope.completed_computations.push(computation);
                        }
                        prefixes.push((transfers, scope.completed_computations.len()));
                    }
                    let mut remaining = None;
                    let mut possible = Vec::new();
                    for ((_, child), (mut prefix, completed)) in
                        node.arms().rev().zip(prefixes.into_iter().rev())
                    {
                        scope.completed_computations.truncate(completed);
                        let mut child = scope.completion_node(child)?;
                        // These are named possible destinations, not execution events.
                        // Do not copy them repeatedly while joining definite paths.
                        if let Some(child) = &mut child {
                            possible.append(&mut child.possible);
                        }
                        let joined = match (child, remaining) {
                            (Some(child), Some(fallback)) => {
                                Some(Transfers::alternatives(vec![child, fallback]))
                            }
                            (child, fallback) => child.or(fallback),
                        };
                        remaining = if let Some(joined) = joined {
                            possible.append(&mut prefix.possible);
                            prefix.append(joined);
                            Some(prefix)
                        } else {
                            None
                        };
                    }
                    let remaining = remaining
                        .ok_or("completion cleanup: normal dispatch has no normal alternative")?;
                    possible.append(&mut subject_transfers.possible);
                    subject_transfers.append(remaining);
                    subject_transfers.possible = possible;
                    Ok(Some(subject_transfers))
                }
            }
        }
        mod plans {

            /*
                Follow each selected branch in source order. Cleanup for one branch must
                not include operands belonging to another.
            */
            use super::{CompletionPlan, ComposedCompletion, PreparedCompletion};
            use checked_trees::{
                CheckFacts, CheckedCallScalarArgument,
                CheckedComposedUnitControlTerminatorPlan as Terminator,
                CheckedConditionalReturnArm, CheckedGuardedJumpArmPlan,
                CheckedGuardedJumpFallbackPlan, CheckedScalarBranchDestination,
                CheckedScalarReturnPlan, CheckedStructuralControlSuccessorPlan,
                CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentPlan,
                CheckedUnitStructuralReturnPlan, FlowClaimJoinExit, FlowClaimJoinExitKind,
            };
            use symbols::SymbolHandle;

            #[derive(Clone, Copy)]
            pub(super) enum Guard<'a> {
                Argument(&'a CheckedCallScalarArgument),
                Role(u32),
                Subject(&'a CheckedUnitStructuralArgumentPlan),
            }

            #[derive(Clone, Copy)]
            pub(super) enum CompletionOutcome<'a> {
                Prefix,
                Completion(&'a checked_trees::CheckedUnitCompletionPlan),
                Scalar,
                Structural(&'a CheckedUnitStructuralReturnPlan),
                Operation(&'a CheckedUnitEffectOperationPlan),
                Arm(&'a CheckedConditionalReturnArm),
                Successor(&'a CheckedStructuralControlSuccessorPlan),
                NoNormal,
            }

            impl CompletionOutcome<'_> {
                pub(super) fn has_normal_completion(self) -> bool {
                    match self {
                        Self::NoNormal => false,
                        Self::Completion(plan) => plan.expression.normal_continuation,
                        Self::Arm(arm) => arm.has_normal_continuation(),
                        Self::Successor(successor) => successor.has_normal_caller_completion(),
                        Self::Operation(operation) => operation.has_normal_continuation(),
                        _ => true,
                    }
                }
            }

            pub(super) struct CompletionPath<'a> {
                pub exit: Option<FlowClaimJoinExit>,
                pub guards: Vec<Guard<'a>>,
                pub outcome: CompletionOutcome<'a>,
            }

            fn edge(state: SymbolHandle, ordinal: u32, continuation: bool) -> FlowClaimJoinExit {
                FlowClaimJoinExit {
                    state_symbol: state,
                    statement_index: ordinal as usize,
                    kind: if continuation {
                        FlowClaimJoinExitKind::TransitionContinuation
                    } else {
                        FlowClaimJoinExitKind::TransitionTarget
                    },
                }
            }

            fn successor_path<'a>(
                state: SymbolHandle,
                successor: &'a CheckedStructuralControlSuccessorPlan,
                guards: &[Guard<'a>],
            ) -> CompletionPath<'a> {
                CompletionPath {
                    exit: Some(edge(
                        state,
                        successor.statement_ordinal,
                        successor.is_continuation,
                    )),
                    guards: guards.to_vec(),
                    outcome: CompletionOutcome::Successor(successor),
                }
            }

            fn return_path<'a>(
                state: SymbolHandle,
                arm: &'a CheckedConditionalReturnArm,
                continuation: bool,
                guards: &[Guard<'a>],
            ) -> Result<CompletionPath<'a>, &'static str> {
                let ordinal = arm
                    .statement_ordinal()
                    .ok_or("completion cleanup: a return arm has no source coordinate")?;
                Ok(CompletionPath {
                    exit: Some(edge(state, ordinal, continuation)),
                    guards: guards.to_vec(),
                    outcome: CompletionOutcome::Arm(arm),
                })
            }

            impl<'a> CompletionPlan<'a> {
                pub(in crate::execution::terminal_unit) fn prepare<'context>(
                    self,
                    program: &dyn validation::CheckingInput,
                    facts: &'context CheckFacts,
                    machine: SymbolHandle,
                    state: &typed_trees::state::State,
                ) -> Result<PreparedCompletion<'context>, &'static str>
                where
                    'a: 'context,
                {
                    let plan: CompletionPlan<'context> = self;
                    Ok(PreparedCompletion {
                        plan,
                        paths: plan.paths(program, facts, machine, state)?,
                    })
                }

                pub(super) fn paths(
                    self,
                    program: &dyn validation::CheckingInput,
                    facts: &'a CheckFacts,
                    machine: SymbolHandle,
                    state: &typed_trees::state::State,
                ) -> Result<Vec<CompletionPath<'a>>, &'static str> {
                    match self {
                        Self::Body {
                            scalar_control: Some(control),
                            ..
                        } => scalar_paths(program, facts, machine, state, &control.terminator),
                        Self::Body { returned, .. } => direct_paths(
                            program,
                            facts,
                            machine,
                            state,
                            returned
                                .map_or(CompletionOutcome::Prefix, CompletionOutcome::Structural),
                        ),
                        Self::Composed(completion) => {
                            let mut paths = completion.paths(program, facts, machine, state)?;
                            if let Some(selected) = completion.selected_exit {
                                paths.retain(|path| path.exit == Some(selected));
                                if paths.is_empty() {
                                    return Err(
                                        "completion cleanup: the selected exit is absent from its completion plan",
                                    );
                                }
                            }
                            Ok(paths)
                        }
                    }
                }
            }

            fn direct_paths<'a>(
                program: &dyn validation::CheckingInput,
                facts: &CheckFacts,
                machine: SymbolHandle,
                state: &typed_trees::state::State,
                outcome: CompletionOutcome<'a>,
            ) -> Result<Vec<CompletionPath<'a>>, &'static str> {
                let exits = crate::execution::terminal_unit::cleanup::normal_edges::state_exits(
                    program, facts, machine, state,
                )?;
                if exits.is_empty() {
                    return Err("completion cleanup: the body has no checked normal completion");
                }
                Ok(exits
                    .into_iter()
                    .map(|exit| CompletionPath {
                        exit: Some(exit),
                        guards: Vec::new(),
                        outcome,
                    })
                    .collect())
            }

            fn scalar_paths<'a>(
                program: &dyn validation::CheckingInput,
                facts: &'a CheckFacts,
                machine: SymbolHandle,
                state: &typed_trees::state::State,
                terminator: &checked_trees::CheckedScalarStateTerminator,
            ) -> Result<Vec<CompletionPath<'a>>, &'static str> {
                if let checked_trees::CheckedScalarStateTerminator::Crash { statement_ordinal } =
                    terminator
                {
                    return Ok(vec![CompletionPath {
                        exit: Some(edge(state.symbol, *statement_ordinal, false)),
                        guards: Vec::new(),
                        outcome: CompletionOutcome::NoNormal,
                    }]);
                }
                let paths = terminator.returning_paths(&facts.flow.guarded_exits)?;
                let exits = crate::execution::terminal_unit::cleanup::normal_edges::state_exits(
                    program, facts, machine, state,
                )?;
                paths
            .iter()
            .map(|path| {
                let CheckedScalarBranchDestination::Return {
                    statement_ordinal,
                    is_continuation,
                } = path.destination
                else {
                    return Err(
                        "completion cleanup: a scalar return has no exact returning destination",
                    );
                };
                let mut matching = exits.iter().filter(|exit| {
                    exit.statement_index == statement_ordinal as usize
                        && if is_continuation {
                            exit.kind == FlowClaimJoinExitKind::TransitionContinuation
                        } else {
                            matches!(
                                exit.kind,
                                FlowClaimJoinExitKind::TransitionTarget
                                    | FlowClaimJoinExitKind::FinalExpression
                            )
                        }
                });
                let exit = *matching
                    .next()
                    .ok_or("completion cleanup: the scalar return has no checked source exit")?;
                if matching.next().is_some() {
                    return Err("completion cleanup: the scalar return has ambiguous source exits");
                }
                Ok(CompletionPath {
                    exit: Some(exit),
                    guards: path.guard_ordinals().map(Guard::Role).collect(),
                    outcome: CompletionOutcome::Scalar,
                })
            })
            .collect()
            }

            impl<'a> ComposedCompletion<'a> {
                fn paths(
                    self,
                    program: &dyn validation::CheckingInput,
                    facts: &'a CheckFacts,
                    machine: SymbolHandle,
                    state: &typed_trees::state::State,
                ) -> Result<Vec<CompletionPath<'a>>, &'static str> {
                    let mut output = Vec::new();
                    match self.terminator {
                        Terminator::InvokeNoNormal { .. }
                        | Terminator::EvaluateNoNormal
                        | Terminator::Crash { .. } => {
                            output.push(CompletionPath {
                                exit: self.selected_exit,
                                guards: Vec::new(),
                                outcome: CompletionOutcome::NoNormal,
                            });
                        }
                        Terminator::CompleteExpression { completion } => {
                            output = direct_paths(
                                program,
                                facts,
                                machine,
                                state,
                                CompletionOutcome::Completion(completion),
                            )?;
                        }
                        Terminator::ReturnUnit
                        | Terminator::ReturnScalar {
                            completion: CheckedScalarReturnPlan::Binding(_),
                        } => {
                            output = direct_paths(
                                program,
                                facts,
                                machine,
                                state,
                                CompletionOutcome::Prefix,
                            )?;
                        }
                        Terminator::ReturnStructural { result } => {
                            output = direct_paths(
                                program,
                                facts,
                                machine,
                                state,
                                CompletionOutcome::Structural(result),
                            )?;
                        }
                        Terminator::ReturnScalar {
                            completion: CheckedScalarReturnPlan::Exits(control),
                        } => {
                            output =
                                scalar_paths(program, facts, machine, state, &control.terminator)?;
                        }
                        Terminator::Jump { successor } => {
                            output.push(successor_path(self.state, successor, &[]))
                        }
                        Terminator::Conditional {
                            guard,
                            when_true,
                            when_false,
                        } => {
                            let guards = [Guard::Argument(guard)];
                            output.push(successor_path(self.state, when_true, &guards));
                            output.push(successor_path(self.state, when_false, &guards));
                        }
                        Terminator::ConditionalReturn {
                            guard,
                            jump,
                            return_arm,
                            ..
                        } => {
                            let guards = [Guard::Argument(guard)];
                            output.push(successor_path(self.state, jump, &guards));
                            output.push(return_path(self.state, return_arm, false, &guards)?);
                        }
                        Terminator::ConditionalCompletion {
                            guard,
                            when_true,
                            when_false,
                        } => {
                            let guards = [Guard::Argument(guard)];
                            output.push(return_path(self.state, when_true, false, &guards)?);
                            output.push(return_path(self.state, when_false, false, &guards)?);
                        }
                        Terminator::GuardedJumps { arms, fallback } => {
                            let mut guards = Vec::new();
                            for arm in arms {
                                guards.push(Guard::Argument(arm.guard()));
                                output.push(match arm {
                                    CheckedGuardedJumpArmPlan::Jump(plan) => {
                                        successor_path(self.state, &plan.successor, &guards)
                                    }
                                    CheckedGuardedJumpArmPlan::Return(plan) => {
                                        return_path(self.state, &plan.return_arm, false, &guards)?
                                    }
                                });
                            }
                            output.push(match fallback {
                                CheckedGuardedJumpFallbackPlan::Successor(successor) => {
                                    successor_path(self.state, successor, &guards)
                                }
                                CheckedGuardedJumpFallbackPlan::Return(arm) => {
                                    return_path(self.state, arm, false, &guards)?
                                }
                            });
                        }
                        Terminator::Guarded {
                            arms,
                            fallback,
                            return_values,
                        } => {
                            let arms = facts.flow.guarded_exits.exits.span(*arms).ok_or(
                                "completion cleanup: the ordered guard roster is unavailable",
                            )?;
                            if arms.is_empty() {
                                return Err(
                                    "completion cleanup: the ordered guard roster is empty",
                                );
                            }
                            let mut guards = Vec::new();
                            if return_values.len() != arms.len() + usize::from(fallback.is_some()) {
                                return Err(
                                    "completion cleanup: the ordered return values do not match their selected arms",
                                );
                            }
                            for (index, arm) in arms.iter().enumerate() {
                                guards.push(Guard::Role(arm.guard_statement_ordinal));
                                output.push(self.scalar_destination(
                                    &arm.destination,
                                    &guards,
                                    &return_values[index].operation,
                                )?);
                            }
                            if let Some(fallback) = fallback {
                                output.push(self.scalar_destination(
                                    fallback,
                                    &guards,
                                    &return_values[arms.len()].operation,
                                )?);
                            }
                        }
                        Terminator::ClosedSum { subject, cases, .. } => {
                            if cases.is_empty() {
                                return Err("completion cleanup: the case dispatch has no arms");
                            }
                            let guards = [Guard::Subject(subject)];
                            for case in cases {
                                output.push(match &case.outcome {
                                    checked_trees::CheckedClosedSumCaseOutcomePlan::Successor(
                                        successor,
                                    ) => successor_path(self.state, successor, &guards),
                                    checked_trees::CheckedClosedSumCaseOutcomePlan::Return(arm) => {
                                        return_path(self.state, arm, false, &guards)?
                                    }
                                });
                            }
                        }
                    }
                    Ok(output)
                }

                fn scalar_destination(
                    self,
                    destination: &'a CheckedScalarBranchDestination,
                    guards: &[Guard<'a>],
                    operation: &'a CheckedUnitEffectOperationPlan,
                ) -> Result<CompletionPath<'a>, &'static str> {
                    match destination {
                        CheckedScalarBranchDestination::Jump(successor) => {
                            Ok(successor_path(self.state, successor, guards))
                        }
                        CheckedScalarBranchDestination::Return {
                            statement_ordinal,
                            is_continuation,
                        } => {
                            let outcome = CompletionOutcome::Operation(operation);
                            Ok(CompletionPath {
                                exit: Some(edge(self.state, *statement_ordinal, *is_continuation)),
                                guards: guards.to_vec(),
                                outcome,
                            })
                        }
                        CheckedScalarBranchDestination::Crash { statement_ordinal } => {
                            Ok(CompletionPath {
                                exit: Some(edge(self.state, *statement_ordinal, false)),
                                guards: guards.to_vec(),
                                outcome: CompletionOutcome::NoNormal,
                            })
                        }
                        CheckedScalarBranchDestination::InvokeNoNormal {
                            statement_ordinal,
                            is_continuation,
                        } => Ok(CompletionPath {
                            exit: Some(edge(self.state, *statement_ordinal, *is_continuation)),
                            guards: guards.to_vec(),
                            outcome: CompletionOutcome::NoNormal,
                        }),
                    }
                }
            }
        }
        mod successors {
            /*
                Evaluate arguments in source parameter order before entering another machine.
                A copied reference still keeps the data it points to alive.
            */
            use super::{ComposedCompletion, Query, Transfers};
            use crate::execution::terminal_unit::storage_transfers::{Destination, Transfer};
            use checked_trees::{
                CheckedScalarExpressionRole, CheckedStructuralAccess,
                CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
                CheckedStructuralControlTransferSourcePlan as Source,
                CheckedStructuralScalarArgumentSourcePlan, CheckedUnitStructuralArgumentPlan,
                CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
            };
            use language_semantics::Multiplicity;

            enum Operand<'a> {
                Structural(
                    &'a CheckedStructuralControlTransferPlan,
                    &'a CheckedUnitStructuralParameterPlan,
                ),
                Scalar(&'a checked_trees::CheckedStructuralScalarArgumentPlan),
            }

            impl Query<'_> {
                pub(super) fn successor_transfers(
                    &self,
                    graph: ComposedCompletion<'_>,
                    successor: &CheckedStructuralControlSuccessorPlan,
                ) -> Result<Transfers, &'static str> {
                    let mut signatures = graph
                        .signatures
                        .iter()
                        .filter(|row| row.state == successor.target_state);
                    let signature = signatures
                        .next()
                        .ok_or("completion cleanup: the successor has no prepared signature")?;
                    if signatures.next().is_some() {
                        return Err("completion cleanup: the successor signature is ambiguous");
                    }
                    let mut operands = Vec::with_capacity(
                        successor.transfers.len() + successor.scalar_arguments.len(),
                    );
                    for transfer in &successor.transfers {
                        let parameter = signature
                            .structural
                            .get(transfer.target_parameter_index as usize)
                            .ok_or(
                                "completion cleanup: a structural argument has no target parameter",
                            )?;
                        operands
                            .push((parameter.position, Operand::Structural(transfer, parameter)));
                    }
                    for argument in &successor.scalar_arguments {
                        let parameter = signature
                            .scalar
                            .get(argument.target_scalar_parameter_index as usize)
                            .ok_or(
                                "completion cleanup: a scalar argument has no target parameter",
                            )?;
                        if argument.argument_ordinal != parameter.source_position {
                            return Err(
                                "completion cleanup: a scalar argument has a different authored position",
                            );
                        }
                        operands.push((parameter.source_position, Operand::Scalar(argument)));
                    }
                    operands.sort_by_key(|(position, _)| *position);
                    if operands.windows(2).any(|rows| rows[0].0 == rows[1].0) {
                        return Err("completion cleanup: two successor operands share a position");
                    }
                    let mut output = Transfers::default();
                    for (_, operand) in operands {
                        match operand {
                            Operand::Scalar(argument) => {
                                if argument.source
                                    == CheckedStructuralScalarArgumentSourcePlan::Expression
                                {
                                    let role = CheckedScalarExpressionRole::TransitionArgument {
                                        argument_ordinal: argument.argument_ordinal,
                                    };
                                    output.append(self.scalar_role(
                                        graph.state,
                                        successor.statement_ordinal,
                                        role,
                                    )?);
                                }
                            }
                            Operand::Structural(transfer, parameter) => {
                                output.append(self.successor_argument(transfer, parameter)?);
                                self.retain_successor_parents(transfer, &mut output);
                            }
                        }
                    }
                    Ok(output)
                }

                fn successor_argument(
                    &self,
                    transfer: &CheckedStructuralControlTransferPlan,
                    parameter: &CheckedUnitStructuralParameterPlan,
                ) -> Result<Transfers, &'static str> {
                    let argument = |source| CheckedUnitStructuralArgumentPlan {
                        source,
                        path: Vec::new(),
                        projection_evaluations: Default::default(),
                        type_identity: parameter.type_identity,
                        access: parameter.access,
                    };
                    match &transfer.source {
                        Source::Parameter { index } => self.successor_value(
                            &argument(CheckedUnitStructuralArgumentSourcePlan::Parameter {
                                parameter_index: *index,
                            }),
                            parameter,
                        ),
                        Source::StructuralResult { binding_ordinal } => self.successor_value(
                            &argument(CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal: *binding_ordinal,
                            }),
                            parameter,
                        ),
                        Source::ProjectedValue { subject } | Source::ViewSubslice(subject) => {
                            self.successor_value(subject, parameter)
                        }
                        Source::ProjectedLoan { subject, .. } => self
                            .argument_computations(subject)
                            .ok_or("completion cleanup: a successor loan has incomplete address operands"),
                        Source::CasePayload { subject, .. } => self
                            .argument_computations(subject)
                            .ok_or("completion cleanup: a case payload has incomplete address operands"),
                        Source::ByteSequenceLiteral { .. } => Ok(Transfers::default()),
                        Source::EstablishedValue {
                            operands,
                            establishment,
                            presentation,
                        } => {
                            let mut output = self.operation_sequence(operands.iter())?;
                            output.append(if presentation.access == CheckedStructuralAccess::Owned {
                                self.outgoing_operation(establishment)?
                            } else {
                                self.operation_sequence(std::iter::once(establishment.as_ref()))?
                            });
                            output.append(self.successor_value(presentation, parameter)?);
                            Ok(output)
                        }
                        Source::EvaluateNoNormal { .. } => {
                            Err("completion cleanup: a stopped operand cannot arrive at a successor")
                        }
                    }
                }

                fn successor_value(
                    &self,
                    subject: &CheckedUnitStructuralArgumentPlan,
                    parameter: &CheckedUnitStructuralParameterPlan,
                ) -> Result<Transfers, &'static str> {
                    let mut output = self.argument_computations(subject).ok_or(
                        "completion cleanup: a successor value has incomplete address operands",
                    )?;
                    if parameter.access == CheckedStructuralAccess::Owned {
                        output.append(self.outgoing_argument(
                            subject,
                            parameter.multiplicity == Multiplicity::Unrestricted,
                        )?);
                    }
                    Ok(output)
                }

                fn retain_successor_parents(
                    &self,
                    transfer: &CheckedStructuralControlTransferPlan,
                    output: &mut Transfers,
                ) {
                    if let Some(binding_ordinal) = transfer.retained_loan_backing {
                        self.retain_source(
                            &CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal,
                            },
                            output,
                        );
                    }
                    for (dependent, parent) in &transfer.retained_loan_parents {
                        self.retain_source(dependent, output);
                        self.retain_source(parent, output);
                    }
                    for parent in &transfer.retained_operand_parents {
                        self.retain_source(parent, output);
                    }
                }

                fn retain_source(
                    &self,
                    source: &CheckedUnitStructuralArgumentSourcePlan,
                    output: &mut Transfers,
                ) {
                    let same_home = self.same_source(source)
                        || match (&self.source, source) {
                            (
                                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                                CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                    binding_ordinal,
                                },
                            ) => self.results.iter().any(|(result, root)| {
                                result.binding_ordinal == *binding_ordinal
                                    && *root == facts::PlaceRoot::Symbol(*symbol)
                            }),
                            _ => false,
                        };
                    if same_home {
                        output.possible.push(Transfer {
                            source: Vec::new(),
                            destination: Destination::RetainedBySuccessor,
                        });
                    }
                }
            }
        }
        use plans::{CompletionOutcome, CompletionPath, Guard};

        /*
            Borrow the signatures already prepared for this exact graph. Structural
            parameter positions preserve their order among the scalar arguments.
        */
        #[derive(Clone, Copy)]
        pub(in crate::execution::terminal_unit) struct CompletionSignature<'a> {
            pub state: SymbolHandle,
            pub structural: &'a [CheckedUnitStructuralParameterPlan],
            pub scalar: &'a [CheckedStructuralScalarParameterPlan],
        }

        #[derive(Clone, Copy)]
        pub(in crate::execution::terminal_unit) struct ComposedCompletion<'a> {
            pub state: SymbolHandle,
            pub terminator: &'a CheckedComposedUnitControlTerminatorPlan,
            pub selected_exit: Option<FlowClaimJoinExit>,
            pub signatures: &'a [CompletionSignature<'a>],
        }

        #[derive(Clone, Copy)]
        pub(in crate::execution::terminal_unit) enum CompletionPlan<'a> {
            /*
                Ordinary bodies have already evaluated their result in the operation
                prefix. Any scalar control still selects its actual returning edge.
            */
            Body {
                state: SymbolHandle,
                scalar_control: Option<&'a CheckedUnitScalarControlPlan>,
                returned: Option<&'a CheckedUnitStructuralReturnPlan>,
            },
            Composed(ComposedCompletion<'a>),
        }

        impl CompletionPlan<'_> {
            pub(super) fn state(self) -> SymbolHandle {
                match self {
                    Self::Body { state, .. } => state,
                    Self::Composed(completion) => completion.state,
                }
            }
        }

        /*
            Selected paths are prepared once for this scheduler invocation. Each holder
            then follows the same immutable completion and its exact source exits.
        */
        pub(in crate::execution::terminal_unit) struct PreparedCompletion<'a> {
            plan: CompletionPlan<'a>,
            paths: Vec<CompletionPath<'a>>,
        }

        impl PreparedCompletion<'_> {
            pub(in crate::execution::terminal_unit) fn has_normal_completion(&self) -> bool {
                self.paths
                    .iter()
                    .any(|path| path.outcome.has_normal_completion())
            }
        }

        pub(in crate::execution::terminal_unit) struct CompletionTransfers {
            pub prefix: Transfers,
            pub exits: Vec<(FlowClaimJoinExit, Transfers)>,
        }

        pub(in crate::execution::terminal_unit) fn at_completion(
            program: &dyn validation::CheckingInput,
            facts: &CheckFacts,
            prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
            results: &Results,
            source: CheckedUnitStructuralArgumentSourcePlan,
            formation: u32,
            selection: &[CheckedUnitStructuralPathSegment],
            operations: &[CheckedUnitEffectOperationPlan],
            completion: &PreparedCompletion<'_>,
        ) -> Result<CompletionTransfers, &'static str> {
            let query = query_for_storage(
                program,
                facts,
                prepared_calls,
                results,
                source,
                formation,
                operations,
            );
            let (prefix, still_occupies_storage) = query
                .through_prefix(selection)
                .ok_or("completion cleanup: cannot follow this value through the body")?;
            if !still_occupies_storage {
                return Err("completion cleanup: this storage contains a later value");
            }
            let mut exits = Vec::new();
            for path in &completion.paths {
                if !path.outcome.has_normal_completion() {
                    continue;
                }
                let exit = path
                    .exit
                    .ok_or("completion cleanup: a normal path has no exact source exit")?;
                let mut transfers = Transfers::default();
                for guard in &path.guards {
                    transfers.append(match guard {
                        Guard::Argument(value) => query
                            .scalar(value)
                            .ok_or("completion cleanup: a guard has no complete operand transfers")?,
                        Guard::Role(ordinal) => query.scalar_role(
                            completion.plan.state(),
                            *ordinal,
                            CheckedScalarExpressionRole::Guard,
                        )?,
                        Guard::Subject(subject) => query.argument_computations(subject).ok_or(
                            "completion cleanup: the selected subject has no complete operand transfers",
                        )?,
                    });
                }
                transfers.append(query.completion_outcome(completion.plan, exit, path)?);
                exits.push((exit, transfers));
            }
            Ok(CompletionTransfers { prefix, exits })
        }

        impl Query<'_> {
            fn completion_outcome(
                &self,
                completion: CompletionPlan<'_>,
                exit: FlowClaimJoinExit,
                path: &CompletionPath<'_>,
            ) -> Result<Transfers, &'static str> {
                match path.outcome {
                    CompletionOutcome::Prefix => Ok(Transfers::default()),
                    CompletionOutcome::Completion(plan) => self.completion_expression(plan),
                    CompletionOutcome::Structural(result) => {
                        let argument = checked_trees::CheckedUnitStructuralArgumentPlan {
                            projection_evaluations: Default::default(),
                            source: result.source.clone(),
                            path: Vec::new(),
                            type_identity: result.type_identity,
                            access: checked_trees::CheckedStructuralAccess::Owned,
                        };
                        self.outgoing_argument(
                            &argument,
                            result.multiplicity == language_semantics::Multiplicity::Unrestricted,
                        )
                    }
                    CompletionOutcome::Scalar => self.scalar_role(
                        completion.state(),
                        u32::try_from(exit.statement_index)
                            .map_err(|_| "completion cleanup: return statement is out of range")?,
                        if exit.kind == checked_trees::FlowClaimJoinExitKind::TransitionContinuation
                        {
                            CheckedScalarExpressionRole::ContinuationReturn
                        } else {
                            CheckedScalarExpressionRole::Return
                        },
                    ),
                    CompletionOutcome::Operation(operation) => self.outgoing_operation(operation),
                    CompletionOutcome::Arm(arm) => self.return_arm(completion.state(), exit, arm),
                    CompletionOutcome::Successor(successor) => {
                        let CompletionPlan::Composed(graph) = completion else {
                            return Err(
                                "completion cleanup: a successor has no prepared graph signature",
                            );
                        };
                        self.successor_transfers(graph, successor)
                    }
                    CompletionOutcome::NoNormal => {
                        Err("completion cleanup: an abnormal path has no cleanup successor")
                    }
                }
            }

            fn scalar_role(
                &self,
                state: SymbolHandle,
                statement: u32,
                role: CheckedScalarExpressionRole,
            ) -> Result<Transfers, &'static str> {
                let mut roots = self
                    .facts
                    .values
                    .scalar_computations
                    .roots_for_statement_role(state, statement, role);
                let pure = self
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(state, statement, role);
                match (roots.next(), pure) {
                (Some(root), None) if roots.next().is_none() => self.computation(root.root).ok_or(
                    "completion cleanup: cannot follow the final expression's owned arguments",
                ),
                (None, Some(_)) => Ok(Transfers::default()),
                (None, None) => Err("completion cleanup: final expression has no retained value"),
                _ => Err("completion cleanup: final expression has conflicting retained values"),
            }
            }

            fn outgoing_argument(
                &self,
                argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
                copied: bool,
            ) -> Result<Transfers, &'static str> {
                let mut query = self.clone();
                query.source_is_unrestricted = false;
                let mut output = query
                    .argument_transfers(argument, ValueReceiver::Outgoing, &mut Vec::new())
                    .ok_or(
                        "completion cleanup: the outgoing value has incomplete custody transfers",
                    )?;
                if copied || self.source_is_unrestricted {
                    output.definite.clear();
                    output.after_production = None;
                }
                Ok(output)
            }

            fn outgoing_operation(
                &self,
                operation: &CheckedUnitEffectOperationPlan,
            ) -> Result<Transfers, &'static str> {
                let mut query = self.clone();
                query.source_is_unrestricted = false;
                let mut transfers = query.operation(operation, &mut Vec::new()).ok_or(
                    "completion cleanup: the selected value producer has incomplete transfers",
                )?;
                if self.source_is_unrestricted {
                    transfers.definite.clear();
                    transfers.after_production = None;
                }
                if let Some(result) = operation.structural_result_binding() {
                    for transfer in &mut transfers.possible {
                        if matches!(&transfer.destination, Destination::Holder(holder, _) if holder == result)
                        {
                            transfer.destination = Destination::Outgoing;
                        }
                    }
                    if self.source
                        == (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: result.binding_ordinal,
                        })
                    {
                        transfers.possible.push(super::Transfer {
                            source: Vec::new(),
                            destination: Destination::Outgoing,
                        });
                        if !self.source_is_unrestricted
                            && result.multiplicity != language_semantics::Multiplicity::Unrestricted
                        {
                            transfers.definite.push(Vec::new());
                        }
                    }
                }
                Ok(transfers)
            }

            fn return_arm(
                &self,
                state: SymbolHandle,
                exit: FlowClaimJoinExit,
                arm: &checked_trees::CheckedConditionalReturnArm,
            ) -> Result<Transfers, &'static str> {
                use checked_trees::CheckedConditionalReturnArm as Arm;
                match arm {
                    Arm::Unit { .. } => Ok(Transfers::default()),
                    Arm::Completion(plan) => self.completion_expression(plan),
                    Arm::EvaluateUnit { operations, .. } => {
                        self.operation_sequence(operations.iter())
                    }
                    Arm::Scalar {
                        statement_ordinal, ..
                    } => self.scalar_role(
                        state,
                        *statement_ordinal,
                        if exit.kind == checked_trees::FlowClaimJoinExitKind::TransitionContinuation
                        {
                            CheckedScalarExpressionRole::ContinuationReturn
                        } else {
                            CheckedScalarExpressionRole::Return
                        },
                    ),
                    Arm::Structural(operation) => self.outgoing_operation(operation),
                    Arm::Call {
                        operands,
                        operation,
                        completions,
                    } => {
                        let mut output = self.operation_sequence(operands.iter())?;
                        output.append(self.outgoing_operation(operation)?);
                        output.append(self.operation_sequence(completions.iter())?);
                        Ok(output)
                    }
                    Arm::Crash { .. } | Arm::EvaluateNoNormal { .. } => {
                        Err("completion cleanup: an abnormal return arm has no cleanup successor")
                    }
                }
            }

            fn operation_sequence<'a>(
                &self,
                operations: impl Iterator<Item = &'a CheckedUnitEffectOperationPlan>,
            ) -> Result<Transfers, &'static str> {
                let mut output = Transfers::default();
                for operation in operations {
                    output.append(self.operation(operation, &mut Vec::new()).ok_or(
                        "completion cleanup: an operand sequence has incomplete transfers",
                    )?);
                    if !operation.has_normal_continuation() {
                        return Err("completion cleanup: an operand does not complete normally");
                    }
                }
                Ok(output)
            }
        }
    }
    mod computations {
        /*
            Scalar computations can consume structural owners. Cleanup must follow
            their retained evaluation graph just as it follows standalone calls:
            sequential operands accumulate moves, selected alternatives intersect the
            definite moves, and neither path grants a move from an observation.
        */
        use super::{Query, Transfers, ValueReceiver};
        use checked_trees::{
            CheckedByteSequenceValue as SequenceValue, CheckedByteSequenceValuePlan,
            CheckedCallScalarArgument as ScalarArgument, CheckedScalarComputationHandle,
            CheckedScalarComputationKind as ComputationKind,
            CheckedScalarComputationStructuralArgument as StructuralArgument,
            CheckedScalarStoreValue, CheckedStoreValue,
            CheckedUnitEffectOperationPlan as OperationPlan,
            CheckedUnitLocalBindingPlan as BindingPlan, CheckedUnitStructuralArgumentPlan,
        };

        impl Query<'_> {
            pub(super) fn dispatch_subject(
                &self,
                subject: &checked_trees::CheckedDispatchSubject,
            ) -> Option<Transfers> {
                let mut output = Transfers::default();
                for operand in
                    subject.scalar_operands(&self.facts.values.scalar_computations.operands)
                {
                    output.append(self.computation(operand)?);
                }
                let value = subject.structural_value();
                if value.is_valid() {
                    output.append(self.value(
                        value,
                        &[],
                        false,
                        ValueReceiver::Unbound,
                        &[],
                        &mut Vec::new(),
                    )?);
                }
                Some(output)
            }
            pub(super) fn operation_computations(
                &self,
                operation: &OperationPlan,
            ) -> Option<Transfers> {
                let mut output = Transfers::default();
                if let Some(call) = operation.call() {
                    for operand in call.scalar_operands() {
                        output.append(self.scalar(operand)?);
                    }
                    for argument in &call.structural_arguments {
                        output.append(self.argument_computations(argument)?);
                    }
                }
                match operation {
                    OperationPlan::EstablishLocal(binding) => match binding {
                        BindingPlan::Scalar { value, .. }
                        | BindingPlan::Primitive { value, .. } => {
                            output.append(self.scalar(value)?)
                        }
                        BindingPlan::ScalarArray { elements, .. } => {
                            for element in elements {
                                output.append(self.scalar(element)?);
                            }
                        }
                        BindingPlan::ByteSequence { value, .. } => {
                            output.append(self.bytes(value, value.root)?)
                        }
                        _ => {}
                    },
                    OperationPlan::Store(store) => match &store.value {
                        CheckedStoreValue::Scalar {
                            value: CheckedScalarStoreValue::Computation(root),
                            ..
                        } => {
                            output.append(self.computation(*root)?);
                        }
                        CheckedStoreValue::Place(argument) => {
                            output.append(self.argument_computations(argument)?)
                        }
                        CheckedStoreValue::Bytes(value) => {
                            output.append(self.bytes(value, value.root)?)
                        }
                        _ => {}
                    },
                    OperationPlan::EstablishReference { source, .. }
                    | OperationPlan::EstablishViewSubslice { source, .. }
                    | OperationPlan::MoveStructuralField { source, .. } => {
                        output.append(self.argument_computations(source)?);
                    }
                    OperationPlan::CaptureAssignmentDestination { destination, .. } => {
                        output.append(self.argument_computations(destination)?);
                    }
                    OperationPlan::AtomicAccess(atomic) => {
                        for operand in atomic.event.operands() {
                            output.append(self.scalar(operand)?);
                        }
                    }
                    OperationPlan::AtomicFence { argument, .. } => {
                        output.append(self.argument_computations(argument)?);
                    }
                    _ => {}
                }
                Some(output)
            }

            pub(super) fn scalar(&self, operand: &ScalarArgument) -> Option<Transfers> {
                match operand {
                    ScalarArgument::Pure(_) => Some(Transfers::default()),
                    ScalarArgument::Computation(root) => self.computation(*root),
                }
            }

            pub(super) fn argument_computations(
                &self,
                argument: &CheckedUnitStructuralArgumentPlan,
            ) -> Option<Transfers> {
                let mut output = Transfers::default();
                for operand in
                    argument.computation_operands(&self.facts.values.scalar_computations.operands)
                {
                    output.append(self.computation(operand)?);
                }
                Some(output)
            }

            fn structural_operand_computations(
                &self,
                operand: &StructuralArgument,
            ) -> Option<Transfers> {
                match operand {
                    StructuralArgument::Place(argument) => self.argument_computations(argument),
                    StructuralArgument::Case(case) => {
                        let mut output = Transfers::default();
                        for field in self
                            .facts
                            .values
                            .scalar_computations
                            .case_fields
                            .span(case.fields)?
                        {
                            output.append(self.computation(field.value)?);
                        }
                        Some(output)
                    }
                }
            }

            fn bytes(
                &self,
                value: &CheckedByteSequenceValuePlan,
                root: arena::Handle<SequenceValue>,
            ) -> Option<Transfers> {
                match value.nodes.get(root) {
                    SequenceValue::Descriptor { source, .. } => self.argument_computations(source),
                    SequenceValue::Concatenation { left, right, .. } => {
                        let mut output = self.bytes(value, *left)?;
                        output.append(self.bytes(value, *right)?);
                        Some(output)
                    }
                    _ => Some(Transfers::default()),
                }
            }

            pub(super) fn computation(
                &self,
                root: CheckedScalarComputationHandle,
            ) -> Option<Transfers> {
                if self.completed_computations.contains(&root) {
                    return Some(Transfers::default());
                }
                let plans = &self.facts.values.scalar_computations;
                if !plans.nodes.is_valid(root) {
                    return None;
                }
                let mut output = Transfers::default();
                match &plans.nodes.get(root).kind {
                    ComputationKind::Call {
                        source_call,
                        callee,
                        arguments,
                        structural_arguments,
                        ..
                    } => {
                        if let Some(selector) = callee.selector() {
                            output.append(self.computation(selector)?);
                        }
                        for argument in plans.operands.span(*arguments)? {
                            output.append(self.computation(*argument)?);
                        }
                        for operand in plans.structural_arguments.span(*structural_arguments)? {
                            output.append(self.structural_operand_computations(operand)?);
                            if let StructuralArgument::Place(argument) = operand {
                                let call =
                                    self.facts.flow.control.calls.get(source_call.statement()?);
                                output.append(self.argument_transfers(
                                    argument,
                                    ValueReceiver::Call {
                                        statement_index:
                                            u32::try_from(call.statement_index).ok()?,
                                        structural_result: false,
                                    },
                                    &mut Vec::new(),
                                )?);
                            }
                        }
                    }
                    ComputationKind::WithOperands {
                        operands, operand, ..
                    } => {
                        let mut scope = self.clone();
                        for dependency in plans.evaluation_operands.span(*operands)? {
                            match dependency {
                                checked_trees::CheckedScalarEvaluationOperand::Scalar(value) => {
                                    output.append(scope.computation(*value)?);
                                    scope.completed_computations.push(*value);
                                }
                                checked_trees::CheckedScalarEvaluationOperand::Structural(
                                    value,
                                ) => {
                                    output.append(scope.value(
                                        *value,
                                        &[],
                                        false,
                                        ValueReceiver::Unbound,
                                        &[],
                                        &mut Vec::new(),
                                    )?);
                                    scope.established_values.push(*value);
                                }
                            }
                        }
                        output.append(scope.computation(*operand)?);
                    }
                    ComputationKind::Apply { operands, .. }
                    | ComputationKind::StorageObservation { operands, .. } => {
                        for operand in plans.operands.span(*operands)? {
                            output.append(self.computation(*operand)?);
                        }
                    }
                    ComputationKind::Select {
                        condition,
                        when_true,
                        when_false,
                        ..
                    } => {
                        output.append(self.computation(*condition)?);
                        output.append(Transfers::alternatives(vec![
                            self.computation(*when_true)?,
                            self.computation(*when_false)?,
                        ]));
                    }
                    ComputationKind::Dispatch { subject, arms, .. } => {
                        output.append(self.dispatch_subject(subject)?);
                        let mut prefix = Vec::new();
                        let mut alternatives = Vec::new();
                        for arm in plans.dispatch_arms.span(*arms)? {
                            if let Some(pattern) = arm.pattern.computation() {
                                prefix.push(pattern);
                            }
                            let mut branch = Transfers::default();
                            for pattern in &prefix {
                                branch.append(self.computation(*pattern)?);
                            }
                            branch.append(self.computation(arm.value)?);
                            alternatives.push(branch);
                        }
                        output.append(Transfers::alternatives(alternatives));
                    }
                    ComputationKind::SelectedComparison { left, right, .. } => {
                        output.append(self.computation(*left)?);
                        output.append(self.computation(*right)?);
                    }
                    checked_trees::cast_or_qualification_triple_pattern!(operand, ..) => {
                        output.append(self.computation(*operand)?)
                    }
                    ComputationKind::CaseMembership { subject, .. } => {
                        output.append(self.structural_operand_computations(subject)?)
                    }
                    ComputationKind::StorageAddress { subject, .. }
                    | ComputationKind::StructuralField { subject, .. }
                    | ComputationKind::ByteSequenceLiteralComparison { subject, .. }
                    | ComputationKind::ByteSequencePredicate { subject, .. } => {
                        output.append(self.argument_computations(subject)?)
                    }
                    ComputationKind::RetainedConstant { .. } | ComputationKind::Value(_) => {}
                }
                Some(output)
            }
        }
    }
    pub(in crate::execution::terminal_unit) use completion::{
        CompletionPlan, CompletionSignature, ComposedCompletion, PreparedCompletion, at_completion,
    };

    type Path = Vec<CheckedUnitStructuralPathSegment>;

    pub(in crate::execution::terminal_unit) struct Transfer {
        pub(in crate::execution::terminal_unit) source: Path,
        pub(in crate::execution::terminal_unit) destination: Destination,
    }

    pub(in crate::execution::terminal_unit) enum Destination {
        Holder(CheckedUnitStructuralResultBindingPlan, Vec<OwnerSegment>),
        /*
            An independently registered anonymous home, constructed before a
            descriptor lends it. Owned children move into this backing, never into
            the reference carrier that observes it.
        */
        Backing(CheckedStructuralValueHandle, Vec<OwnerSegment>),
        /*
            The call consumes this source. A structural result can carry the
            consumed descriptors onward: its returned occurrences retain exact
            supplying-loan identities independently of result nesting. A call
            without one ends the carrier inside the callee; only a stored
            reference effect can still publish a receiving occurrence.
        */
        Call {
            statement_index: u32,
            structural_result: bool,
        },
        Storage {
            /*
                One authored assignment can expand into several distinct stores;
                retain the producer's selection within its captured destination.
            */
            statement_index: u32,
            captured_selection: Path,
        },
        /*
            This selected completion captures the value for its return or successor.
            It ends the local holder, but never authorizes releasing its backing.
        */
        Outgoing,
        /*
            A successor still needs this local source through its captured parents.
            Retention is not a transfer or permission to release the source.
        */
        RetainedBySuccessor,
        Unknown,
    }

    /*
        A completed value can enter a named holder, an anonymous backing, or a
        consuming call. Traversal must not invent a local binding for the latter two.
    */
    #[derive(Clone, Copy)]
    enum ValueReceiver<'a> {
        Holder(&'a CheckedUnitStructuralResultBindingPlan),
        Backing(CheckedStructuralValueHandle),
        Call {
            statement_index: u32,
            structural_result: bool,
        },
        Outgoing,
        Unbound,
    }

    impl ValueReceiver<'_> {
        fn destination(self, path: &[OwnerSegment]) -> Destination {
            match self {
                Self::Holder(result) => Destination::Holder(result.clone(), path.to_vec()),
                Self::Backing(value) => Destination::Backing(value, path.to_vec()),
                Self::Call {
                    statement_index,
                    structural_result,
                } => Destination::Call {
                    statement_index,
                    structural_result,
                },
                Self::Outgoing => Destination::Outgoing,
                Self::Unbound => Destination::Unknown,
            }
        }
    }

    #[derive(Default)]
    pub(in crate::execution::terminal_unit) struct Transfers {
        pub(in crate::execution::terminal_unit) possible: Vec<Transfer>,
        pub(in crate::execution::terminal_unit) definite: Vec<Path>,
        /*
            Moves after this exact producing occurrence, on every normal path
            where it completes. None means this summary contains no such producer.
            Unconditional `definite` remains independent for pre-existing storage.
        */
        after_production: Option<Vec<Path>>,
    }

    impl Transfers {
        fn without_receiver(mut self) -> Self {
            for transfer in &mut self.possible {
                transfer.destination = Destination::Unknown;
            }
            self
        }

        fn append(&mut self, other: Self) {
            let preceding = self.after_production.take().map(|mut paths| {
                paths.extend(other.definite.iter().cloned());
                paths
            });
            self.after_production = match (preceding, other.after_production) {
                (Some(preceding), Some(following)) => Some(common_paths(&[&preceding, &following])),
                (preceding, None) => preceding,
                (None, following) => following,
            };
            self.possible.extend(other.possible);
            self.definite.extend(other.definite);
        }

        fn alternatives(alternatives: Vec<Self>) -> Self {
            let definite = common_paths(
                &alternatives
                    .iter()
                    .map(|arm| arm.definite.as_slice())
                    .collect::<Vec<_>>(),
            );
            let produced = alternatives
                .iter()
                .filter_map(|arm| arm.after_production.as_deref())
                .collect::<Vec<_>>();
            let after_production = (!produced.is_empty()).then(|| common_paths(&produced));
            Self {
                definite,
                after_production,
                possible: alternatives
                    .into_iter()
                    .flat_map(|arm| arm.possible)
                    .collect(),
            }
        }

        /*
            Paths transferred on every normal path producing this exact occurrence.
            A source without a producer in this summary keeps the unconditional
            all-alternative proof; a branch-born source cannot donate sibling facts.
        */
        pub(in crate::execution::terminal_unit) fn occurrence_transfers(&self) -> &[Path] {
            self.after_production.as_deref().unwrap_or(&self.definite)
        }
    }

    fn common_paths(alternatives: &[&[Path]]) -> Vec<Path> {
        alternatives
            .iter()
            .flat_map(|paths| paths.iter())
            .filter(|path| {
                alternatives
                    .iter()
                    .all(|paths| paths.iter().any(|covered| path.starts_with(covered)))
            })
            .cloned()
            .collect()
    }

    #[derive(Clone)]
    struct Query<'a> {
        program: &'a dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        results: &'a Results,
        source: CheckedUnitStructuralArgumentSourcePlan,
        source_is_unrestricted: bool,
        formation: u32,
        operations: &'a [CheckedUnitEffectOperationPlan],
        prepared_calls: &'a super::structural_value_calls::PreparedCalls,
        established_values: Vec<CheckedStructuralValueHandle>,
        completed_computations: Vec<CheckedScalarComputationHandle>,
    }

    pub(in crate::execution::terminal_unit) fn transfers(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        prepared_calls: &super::structural_value_calls::PreparedCalls,
        results: &Results,
        binding: u32,
        operations: &[CheckedUnitEffectOperationPlan],
    ) -> Option<Transfers> {
        let formation = results
            .iter()
            .find(|(result, _)| result.binding_ordinal == binding)
            .map_or(0, |(result, _)| result.statement_index);
        from_storage(
            program,
            facts,
            prepared_calls,
            results,
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: binding,
            },
            formation,
            &[],
            operations,
        )
    }

    pub(in crate::execution::terminal_unit) fn from_storage(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        prepared_calls: &super::structural_value_calls::PreparedCalls,
        results: &Results,
        source: CheckedUnitStructuralArgumentSourcePlan,
        formation: u32,
        selection: &[CheckedUnitStructuralPathSegment],
        operations: &[CheckedUnitEffectOperationPlan],
    ) -> Option<Transfers> {
        query_for_storage(
            program,
            facts,
            prepared_calls,
            results,
            source,
            formation,
            operations,
        )
        .through_prefix(selection)
        .map(|(transfers, _)| transfers)
    }

    fn query_for_storage<'a>(
        program: &'a dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        prepared_calls: &'a super::structural_value_calls::PreparedCalls,
        results: &'a Results,
        source: CheckedUnitStructuralArgumentSourcePlan,
        formation: u32,
        operations: &'a [CheckedUnitEffectOperationPlan],
    ) -> Query<'a> {
        let mut bindings = results.iter().filter(|(result, root)| match &source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                result.binding_ordinal == *binding_ordinal
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                *root == PlaceRoot::Symbol(*symbol)
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression, ..
            } => *root == PlaceRoot::Expression(*expression),
            _ => false,
        });
        let source_is_unrestricted = bindings.next().is_some_and(|(result, _)| {
            result.multiplicity == Multiplicity::Unrestricted
                && bindings.all(|(other, _)| other.multiplicity == Multiplicity::Unrestricted)
        });
        Query {
            program,
            facts,
            results,
            source,
            source_is_unrestricted,
            formation,
            operations,
            prepared_calls,
            established_values: Vec::new(),
            completed_computations: Vec::new(),
        }
    }

    impl Query<'_> {
        /*
            An assignment uses the old value before installing the new one. Track
            that boundary so the final expression cannot use a move recorded for
            an earlier value that occupied the same storage.
        */
        fn through_prefix(
            &self,
            selection: &[CheckedUnitStructuralPathSegment],
        ) -> Option<(Transfers, bool)> {
            let mut output = Transfers::default();
            for operation in self.operations {
                if operation
                    .source_statement_index()
                    .is_some_and(|statement| statement < self.formation)
                {
                    continue;
                }
                output.append(self.operation(operation, &mut Vec::new())?);
                if let CheckedUnitEffectOperationPlan::Store(store) = operation
                    && store.statement_index > self.formation
                    && store.installed_value().is_some()
                    && self.owner().is_some_and(|symbol| {
                        store.destination.root == (CheckedStoreRoot::Local { symbol })
                    })
                    && self.local_store_owns_destination(store)?
                    && selection.starts_with(&store.destination.path)
                {
                    return Some((output, false));
                }
            }
            Some((output, true))
        }
    }

    /*
        Retained computations include selected exits not yet appended to the body.
        A possible owned use fences unconditional retirement, but proves no actual
        transfer: the selected control path still owns its move or cleanup receipt.
        This is a conservative fence, not a current-occupant transfer count. Named
        replacement cleanup separately owns an old occurrence's death; this query
        cannot authorize retirement or infer that a later root actually executes.
    */
    pub(in crate::execution::terminal_unit) fn has_computation_transfer(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: SymbolHandle,
        results: &Results,
        result: &CheckedUnitStructuralResultBindingPlan,
        operations: &[CheckedUnitEffectOperationPlan],
    ) -> Option<bool> {
        let query = Query {
            program,
            facts,
            results,
            source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            },
            source_is_unrestricted: result.multiplicity == Multiplicity::Unrestricted,
            formation: result.statement_index,
            operations,
            prepared_calls: &super::structural_value_calls::PreparedCalls::EMPTY,
            established_values: Vec::new(),
            completed_computations: Vec::new(),
        };
        for (_, root) in facts.values.scalar_computations.roots.iter() {
            if root.state == state
                && root.statement_ordinal >= result.statement_index
                && !query.computation(root.root)?.possible.is_empty()
            {
                return Some(true);
            }
        }
        Some(false)
    }

    /*
        A parameter's vacancies at an ordinary completion: the maximal paths moved
        on every alternative, and the paths moved on only some alternatives
        (`maybe`). One completion cannot dispose storage present on only some
        alternatives, so its caller decides whether a maybe-moved path is admitted.
    */
    pub(super) struct ParameterVacancies {
        pub(super) definite: Vec<Path>,
        pub(super) maybe: Vec<Path>,
    }

    /*
        Current vacancies differ from historical transfers: a checked installation
        restores its destination after evaluating its RHS. Both queries consume the
        same operation/value traversal; no permission count stands for a move.
    */
    pub(super) fn parameter_vacancies(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        prepared_calls: &super::structural_value_calls::PreparedCalls,
        parameter_index: u32,
        parameter_position: u32,
        parameter_symbol: SymbolHandle,
        parameter_multiplicity: Multiplicity,
        operations: &[CheckedUnitEffectOperationPlan],
    ) -> Option<ParameterVacancies> {
        let query = Query {
            program,
            facts,
            results: &[],
            source: CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index },
            source_is_unrestricted: parameter_multiplicity == Multiplicity::Unrestricted,
            formation: 0,
            operations,
            prepared_calls,
            established_values: Vec::new(),
            completed_computations: Vec::new(),
        };
        let mut definite = Vec::<Path>::new();
        let mut possible = Vec::<Path>::new();
        for operation in operations {
            let transfers = query.operation(operation, &mut Vec::new())?;
            definite.extend(transfers.definite);
            possible.extend(
                transfers
                    .possible
                    .into_iter()
                    .map(|transfer| transfer.source),
            );
            if let CheckedUnitEffectOperationPlan::Store(store) = operation
                && store.installed_value().is_some()
                && match store.destination.root {
                    CheckedStoreRoot::Parameter { position } => position == parameter_position,
                    CheckedStoreRoot::Local { symbol } => symbol == parameter_symbol,
                }
            {
                let written = &store.destination.path;
                if possible
                    .iter()
                    .any(|path| written.len() > path.len() && written.starts_with(path))
                {
                    return None;
                }
                definite.retain(|path| !path.starts_with(written));
                possible.retain(|path| !path.starts_with(written));
            }
        }
        let mut maybe = possible
            .into_iter()
            .filter(|path| !definite.iter().any(|covered| path.starts_with(covered)))
            .collect::<Vec<_>>();
        maybe.sort();
        maybe.dedup();
        definite.sort();
        definite.dedup();
        let maximal = definite
            .iter()
            .filter(|path| {
                !definite
                    .iter()
                    .any(|ancestor| ancestor.len() < path.len() && path.starts_with(ancestor))
            })
            .cloned()
            .collect();
        Some(ParameterVacancies {
            definite: maximal,
            maybe,
        })
    }

    impl Query<'_> {
        fn owner(&self) -> Option<SymbolHandle> {
            match self.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => Some(symbol),
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    self.results.iter().find_map(|(result, root)| {
                        (result.binding_ordinal == binding_ordinal)
                            .then_some(root.as_symbol())
                            .flatten()
                    })
                }
                _ => None,
            }
        }
        fn operation(
            &self,
            operation: &CheckedUnitEffectOperationPlan,
            active: &mut Vec<CheckedStructuralValueHandle>,
        ) -> Option<Transfers> {
            let mut output = Transfers::default();
            output.append(self.operation_computations(operation)?);
            if let Some(call) = operation.call() {
                output.append(self.call(call, active)?);
            }
            if let CheckedUnitEffectOperationPlan::MoveStructuralField {
                source,
                result,
                expression,
            } = operation
                && self.belongs(source)
                && extraction_owns_source(self.program, source, result, *expression)?
            {
                output.append(self.leaf(
                    source.path.clone(),
                    Destination::Holder(result.clone(), Vec::new()),
                ));
            }
            if let Some(value) = operation.installed_value()
                && self.belongs(value)
            {
                let destination = match operation {
                    CheckedUnitEffectOperationPlan::EstablishLocal(
                        CheckedUnitLocalBindingPlan::Structural { result, .. },
                    ) => Destination::Holder(result.clone(), Vec::new()),
                    CheckedUnitEffectOperationPlan::Store(store) => Destination::Storage {
                        statement_index: store.statement_index,
                        captured_selection: store.captured_selection.clone(),
                    },
                    _ => Destination::Unknown,
                };
                output.append(self.leaf(value.path.clone(), destination))
            }
            if let CheckedUnitEffectOperationPlan::EstablishLocal(
                CheckedUnitLocalBindingPlan::StructuralValue { result, value, .. },
            ) = operation
            {
                output.append(self.value(
                    *value,
                    &[],
                    true,
                    ValueReceiver::Holder(result),
                    &[],
                    active,
                )?);
            }
            if operation.structural_result_binding().is_some_and(|result| {
                self.source
                    == CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                        binding_ordinal: result.binding_ordinal,
                    }
            }) {
                output.after_production = Some(Vec::new());
            }
            Some(output)
        }
        fn local_store_owns_destination(&self, store: &CheckedStorePlan) -> Option<bool> {
            let CheckedStoreRoot::Local { symbol } = store.destination.root else {
                return None;
            };
            if store
                .destination
                .path
                .contains(&CheckedUnitStructuralPathSegment::Referent)
            {
                return Some(false);
            }
            let state = local_state(self.program, symbol)?;
            let StatementNode::Assignment(assignment) =
                crate::flow::state_statements(self.program, state.statement_nodes)
                    .get(store.statement_index as usize)?
            else {
                return None;
            };
            local_place_is_owned(
                self.program,
                symbol,
                store.statement_index,
                assignment.target,
                store.installed_value()?.type_identity,
            )
        }

        fn belongs(&self, argument: &CheckedUnitStructuralArgumentPlan) -> bool {
            argument.access == CheckedStructuralAccess::Owned && self.same_source(&argument.source)
        }

        fn same_source(&self, source: &CheckedUnitStructuralArgumentSourcePlan) -> bool {
            *source == self.source
                || match (&self.source, source) {
                    (
                        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal,
                        },
                        CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                    ) => self.results.iter().any(|(result, root)| {
                        result.binding_ordinal == *binding_ordinal
                            && *root == PlaceRoot::Symbol(*symbol)
                    }),
                    _ => false,
                }
        }

        fn leaf(&self, source: Path, destination: Destination) -> Transfers {
            if self.source_is_unrestricted {
                return Transfers::default();
            }
            Transfers {
                definite: vec![source.clone()],
                after_production: None,
                possible: vec![Transfer {
                    source,
                    destination,
                }],
            }
        }

        fn call(
            &self,
            call: &CheckedUnitCallPlan,
            active: &mut Vec<CheckedStructuralValueHandle>,
        ) -> Option<Transfers> {
            let mut output = Transfers::default();
            for argument in &call.structural_arguments {
                output.append(self.argument_transfers(
                    argument,
                    ValueReceiver::Call {
                        statement_index: call.coordinate.statement_index,
                        structural_result: call.structural_result().is_some(),
                    },
                    active,
                )?);
            }
            Some(output)
        }

        /*
            Evaluating an anonymous operand constructs its value before the call
            consumes or lends it. Moves into that construction retire their original
            homes exactly as moves into a named local do.
        */
        fn argument_transfers(
            &self,
            argument: &CheckedUnitStructuralArgumentPlan,
            receiver: ValueReceiver<'_>,
            active: &mut Vec<CheckedStructuralValueHandle>,
        ) -> Option<Transfers> {
            if self.belongs(argument) {
                return Some(self.leaf(argument.path.clone(), receiver.destination(&[])));
            }
            let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression, ..
            } = argument.source
            else {
                return Some(Transfers::default());
            };
            let values = &self.facts.values.structural_values;
            let mut roots = values
                .roots
                .iter()
                .map(|(_, root)| root)
                .filter(|root| root.expression == expression);
            let Some(root) = roots.next() else {
                return Some(Transfers::default());
            };
            if roots.next().is_some() {
                return None;
            }
            self.value(
                root.root,
                &argument.path,
                argument.access == CheckedStructuralAccess::Owned,
                receiver,
                &[],
                active,
            )
        }

        fn value(
            &self,
            value: CheckedStructuralValueHandle,
            selection: &[CheckedUnitStructuralPathSegment],
            consumes_selection: bool,
            receiver: ValueReceiver<'_>,
            destination: &[OwnerSegment],
            active: &mut Vec<CheckedStructuralValueHandle>,
        ) -> Option<Transfers> {
            if self.established_values.contains(&value) {
                return Some(Transfers::default());
            }
            let values = &self.facts.values.structural_values;
            if !values.nodes.is_valid(value) || active.contains(&value) {
                return None;
            }
            active.push(value);
            let output = self.value_kind(
                values.nodes.get(value),
                selection,
                consumes_selection,
                if consumes_selection {
                    receiver
                } else {
                    ValueReceiver::Unbound
                },
                destination,
                active,
            );
            active.pop();
            output
        }

        /*
            Join one retained value occurrence to its actual producer operation.
            Aggregate schedules own nested calls; ordinary calls own their result
            directly. Both retain the same checked source occurrence, while only
            the former supplies operand preparation at this DAG position.
        */
        fn value_call(
            &self,
            value: &CheckedStructuralValue,
            source_call: Handle<FlowCallFact>,
            active: &[CheckedStructuralValueHandle],
        ) -> Option<(
            &CheckedUnitEffectOperationPlan,
            Option<&CheckedStructuralValueCall>,
        )> {
            let control = &self.facts.flow.control;
            if !control.calls.is_valid(source_call) {
                return None;
            }
            let source = control.calls.get(source_call);
            let state = control.state(value.machine, value.state)?;
            if source.authored_expression != value.expression
                || source.statement_index != value.statement_index as usize
                || !control
                    .calls
                    .span(state.calls)?
                    .iter()
                    .any(|candidate| std::ptr::eq(candidate, source))
            {
                return None;
            }
            let coordinate = CheckedUnitCallCoordinate {
                statement_index: u32::try_from(source.statement_index).ok()?,
                call_ordinal: u32::try_from(source.call_ordinal).ok()?,
            };
            let mut operations = self
                .operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .filter(|operation| {
                    operation.call().is_some_and(|call| {
                        call.coordinate == coordinate && call.target.state == source.target_symbol
                    })
                });
            let ordinary = operations.next();
            if operations.next().is_some() {
                return None;
            }
            let deferred = if ordinary.is_none() {
                let calls = active.iter().find_map(|root| {
                    self.prepared_calls.schedule(*root).or_else(|| {
                        self.facts
                            .values
                            .structural_values
                            .call_schedule(*root)
                            .map(|(_, calls)| calls)
                    })
                })?;
                let mut calls = super::structural_value_calls::dependencies(calls).filter(|call| {
                    self.facts.values.structural_values.nodes.is_valid(call.value)
                        && matches!(self.facts.values.structural_values.nodes.get(call.value).kind,
                            CheckedStructuralValueKind::Call { source_call: captured } if captured == source_call)
                });
                let call = calls.next()?;
                if calls.next().is_some() {
                    return None;
                }
                Some(call)
            } else {
                None
            };
            let operation = ordinary.or_else(|| deferred.and_then(|call| call.operation()))?;
            if operation.call()?.coordinate != coordinate
                || operation.call()?.target.state != source.target_symbol
                || operation.call()?.structural_result()?.statement_index != value.statement_index
            {
                return None;
            }
            let mut schedules = self
                .operations
                .iter()
                .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::EstablishLocal(
                        CheckedUnitLocalBindingPlan::StructuralValue { calls, .. },
                    ) => Some(calls.as_slice()),
                    _ => None,
                })
                .flatten()
                .filter(|scheduled| {
                    scheduled
                        .operation()
                        .is_some_and(|candidate| std::ptr::eq(candidate, operation))
                });
            let scheduled = schedules.next().or(deferred);
            if schedules.next().is_some()
                || scheduled.is_some_and(|scheduled| {
                    !self
                        .facts
                        .values
                        .structural_values
                        .nodes
                        .is_valid(scheduled.value)
                        || !matches!(
                            self.facts.values.structural_values.nodes.get(scheduled.value).kind,
                            CheckedStructuralValueKind::Call { source_call: captured }
                                if captured == source_call
                        )
                })
            {
                return None;
            }
            Some((operation, scheduled))
        }

        fn value_kind(
            &self,
            value: &CheckedStructuralValue,
            selection: &[CheckedUnitStructuralPathSegment],
            consumes_selection: bool,
            receiver: ValueReceiver<'_>,
            destination: &[OwnerSegment],
            active: &mut Vec<CheckedStructuralValueHandle>,
        ) -> Option<Transfers> {
            let values = &self.facts.values.structural_values;
            let mut output = Transfers::default();
            match &value.kind {
                CheckedStructuralValueKind::Place(argument)
                    if consumes_selection && self.belongs(argument) =>
                {
                    let mut path = argument.path.clone();
                    path.extend_from_slice(selection);
                    return Some(self.leaf(path, receiver.destination(destination)));
                }
                CheckedStructuralValueKind::Call { source_call } => {
                    let (operation, scheduled) = self.value_call(value, *source_call, active)?;
                    if let Some(scheduled) = scheduled {
                        for operand in scheduled.operands().iter() {
                            output.append(self.operation(operand, active)?);
                        }
                        output.append(self.operation(operation, active)?);
                    }
                    let result = operation.call()?.structural_result()?;
                    if consumes_selection
                        && self.source
                            == (CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                                binding_ordinal: result.binding_ordinal,
                            })
                    {
                        output.append(
                            self.leaf(selection.to_vec(), receiver.destination(destination)),
                        );
                    }
                    return Some(output);
                }
                CheckedStructuralValueKind::Projection {
                    source,
                    path,
                    projection_evaluations,
                    copies_selected_value,
                    ..
                } => {
                    let mut selected = path.clone();
                    selected.extend_from_slice(selection);
                    output.append(self.value(
                        *source,
                        &selected,
                        consumes_selection && !copies_selected_value,
                        receiver,
                        destination,
                        active,
                    )?);
                    for operand in
                        CheckedUnitStructuralArgumentPlan::projection_computation_operands(
                            path,
                            *projection_evaluations,
                            &self.facts.values.scalar_computations.operands,
                        )
                    {
                        output.append(self.computation(operand)?);
                    }
                    return Some(output);
                }
                record_or_structural_case_pattern!(data_symbol, fields, ..) => {
                    let data = self.program.data_definition_by_symbol(*data_symbol)?;
                    let mut declared = self
                        .program
                        .data_members(data)
                        .iter()
                        .filter_map(|member| match member {
                            DataMember::Field(field) => Some((field, None)),
                            DataMember::Variant(_) => None,
                        })
                        .collect::<Vec<_>>();
                    if let CheckedStructuralValueKind::StructuralCase { case, .. } = &value.kind {
                        let variant = self.program.data_members(data).iter().find_map(
                            |member| match member {
                                DataMember::Variant(variant) if variant.symbol == *case => {
                                    Some(variant)
                                }
                                _ => None,
                            },
                        )?;
                        declared.extend(
                            self.program
                                .data_payload_fields(variant)
                                .iter()
                                .map(|field| (field, Some(variant))),
                        );
                    }
                    for field in values.record_fields.span(*fields)? {
                        let (declaration, case) = declared
                            .iter()
                            .find(|(candidate, _)| candidate.symbol == field.field)?;
                        let mut relative = Vec::new();
                        let mut owner_path = Vec::new();
                        if let Some(case) = case {
                            relative.push(CheckedUnitStructuralPathSegment::Case(
                                CheckedMemberIdentity::of_member(case.identity, case.name.clone()),
                            ));
                            owner_path.push(OwnerSegment::Case(case.symbol));
                        }
                        relative.push(CheckedUnitStructuralPathSegment::Field(
                            CheckedMemberIdentity::of_member(
                                declaration.identity,
                                declaration.name.clone(),
                            ),
                        ));
                        owner_path.push(OwnerSegment::Field(field.field));
                        let child = match field.value {
                            CheckedStructuralRecordFieldValue::Structural(child) => child,
                            CheckedStructuralRecordFieldValue::Scalar(operand) => {
                                output.append(self.computation(operand)?);
                                continue;
                            }
                        };
                        if !consumes_selection {
                            output.append(self.value(child, &[], true, receiver, &[], active)?);
                        } else if selection.starts_with(&relative) {
                            output.append(self.value(
                                child,
                                &selection[relative.len()..],
                                true,
                                receiver,
                                destination,
                                active,
                            )?);
                        } else if relative.starts_with(selection) {
                            let mut path = destination.to_vec();
                            path.extend_from_slice(&owner_path[selection.len()..]);
                            output.append(self.value(child, &[], true, receiver, &path, active)?);
                        } else {
                            output.append(
                                self.value(child, &[], true, receiver, &[], active)?
                                    .without_receiver(),
                            );
                        }
                    }
                }
                CheckedStructuralValueKind::FixedArray { elements } => {
                    if !consumes_selection {
                        for child in elements {
                            output.append(self.value(*child, &[], true, receiver, &[], active)?);
                        }
                        return Some(output);
                    }
                    if let Some((CheckedUnitStructuralPathSegment::FixedIndex(selected), suffix)) =
                        selection.split_first()
                    {
                        let selected = usize::try_from(*selected).ok()?;
                        elements.get(selected)?;
                        for (index, child) in elements.iter().enumerate() {
                            let transferred = if index == selected {
                                self.value(*child, suffix, true, receiver, destination, active)?
                            } else {
                                self.value(*child, &[], true, receiver, &[], active)?
                                    .without_receiver()
                            };
                            output.append(transferred);
                        }
                        return Some(output);
                    }
                    if !selection.is_empty() {
                        return None;
                    }
                    for (index, child) in elements.iter().enumerate() {
                        let mut path = destination.to_vec();
                        path.push(OwnerSegment::FixedIndex(index));
                        output.append(self.value(*child, &[], true, receiver, &path, active)?);
                    }
                }
                CheckedStructuralValueKind::Dispatch { subject, arms } => {
                    output.append(self.dispatch_subject(subject)?);
                    let alternatives = values
                        .dispatch_arms
                        .span(*arms)?
                        .iter()
                        .map(|arm| {
                            self.value(
                                arm.value,
                                selection,
                                consumes_selection,
                                receiver,
                                destination,
                                active,
                            )
                        })
                        .collect::<Option<Vec<_>>>()?;
                    output.append(Transfers::alternatives(alternatives));
                }
                CheckedStructuralValueKind::ScalarArray { elements } => {
                    for element in elements {
                        output.append(self.computation(*element)?);
                    }
                }
                CheckedStructuralValueKind::ZeroedScalarArray { element, .. } => {
                    output.append(self.computation(*element)?)
                }
                CheckedStructuralValueKind::Case(case) => {
                    for field in self
                        .facts
                        .values
                        .scalar_computations
                        .case_fields
                        .span(case.fields)?
                    {
                        output.append(self.computation(field.value)?);
                    }
                }
                CheckedStructuralValueKind::Reference {
                    source,
                    source_value,
                    ..
                } => {
                    if source_value.is_valid() {
                        let backing = values.nodes.get(*source_value);
                        if values
                            .root_for_expression(
                                backing.state,
                                backing.statement_index,
                                backing.expression,
                            )?
                            .root
                            != *source_value
                        {
                            return None;
                        }
                        output.append(self.value(
                            *source_value,
                            &[],
                            true,
                            ValueReceiver::Backing(*source_value),
                            &[],
                            active,
                        )?);
                    }
                    output.append(self.argument_computations(source)?);
                }
                CheckedStructuralValueKind::Place(argument)
                | CheckedStructuralValueKind::CopiedStructuralPlace {
                    source: argument, ..
                } => {
                    output.append(self.argument_computations(argument)?);
                }
                CheckedStructuralValueKind::ViewSubslice {
                    source, start, end, ..
                } => {
                    output.append(self.value(
                        *source,
                        &[],
                        false,
                        ValueReceiver::Unbound,
                        &[],
                        active,
                    )?);
                    for endpoint in start.iter().chain(end.iter()) {
                        if let CheckedCallScalarArgument::Computation(root) = endpoint {
                            output.append(self.computation(*root)?);
                        }
                    }
                }
                CheckedStructuralValueKind::BorrowedSliceView { source, region }
                | CheckedStructuralValueKind::MutableSliceView { source, region } => {
                    output.append(self.argument_computations(source)?);
                    if let CheckedElementViewRegion::RepresentationBytes { start, .. } = region {
                        output.append(self.computation(*start)?);
                    }
                }
                _ => {}
            }
            Some(output)
        }
    }

    /*
        Window operands name the extracted value with Owned access, even when
        the storage belongs to a referent. Only an inline extraction transfers
        custody out of this carrier. Reuse the planner's typed-place judgment:
        its implicit root reference crossing is not present in the checked path.
    */
    pub(in crate::execution::terminal_unit) fn extraction_owns_source(
        program: &dyn ::validation::CheckingInput,
        source: &CheckedUnitStructuralArgumentPlan,
        result: &CheckedUnitStructuralResultBindingPlan,
        expression: ExpressionHandle,
    ) -> Option<bool> {
        if source
            .path
            .contains(&CheckedUnitStructuralPathSegment::Referent)
        {
            return Some(false);
        }
        match source.source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol } => {
                local_place_is_owned(
                    program,
                    symbol,
                    result.statement_index,
                    expression,
                    result.type_identity,
                )
            }
            CheckedUnitStructuralArgumentSourcePlan::Parameter { .. } => Some(true),
            _ => None,
        }
    }

    fn local_state(
        program: &dyn ::validation::CheckingInput,
        symbol: SymbolHandle,
    ) -> Option<&State> {
        let positions = program.local_data_positions_for_symbol(symbol);
        let [position] = positions.as_slice() else {
            return None;
        };
        let machine = program.machines().get(position.0 as usize)?;
        program.machine_states(machine).get(position.1 as usize)
    }

    fn local_place_is_owned(
        program: &dyn ::validation::CheckingInput,
        symbol: SymbolHandle,
        statement: u32,
        expression: ExpressionHandle,
        selected_type: TypeIdentityId,
    ) -> Option<bool> {
        let state = local_state(program, symbol)?;
        let place = place_at(program, state.symbol, statement as usize, expression)?;
        Some(
            super::borrowed_windows::owned_assignment_source(
                program,
                state,
                &[],
                statement as usize,
                &place,
                Some(selected_type),
            ) == Some(CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }),
        )
    }
}
#[path = "execution_terminal_unit_structural_scalar_store.rs"]
mod structural_scalar_store;
mod structural_value_calls {
    /*
        Structural expression consumers share ordinary call operation schedules.
        A catalog describes each value's calls; only the value evaluator executes
        them, at their exact DAG positions. Preceding statement results supply the
        same source namespace as an ordinary construction or call argument.
        Result bindings retain their named home; anonymous producers retain their
        authored occurrence. Internal displacement slots still reserve ordinals and
        stay in the preamble without pretending to bind an authored source. An
        unrelated preceding producer must not erase a later value's call schedule.

        Prepare schedules against candidate prefixes before custody or dependency
        admission needs them. Prefix queries and completed-state preparation share
        one catalogue and one root builder. Rejected bodies discard their schedules;
        final retention publishes surviving schedules without rebuilding calls.
    */
    use super::calls::structural_result_owner;
    use super::control::statement_sequence::consume_retained_results;
    use super::control::{LocalConstructionTrace, structural_operands};
    use super::types::ShapeCollector;
    use super::types::state_flow;
    use super::{CheckFacts, ScalarCalleeClosure};
    use checked_trees::{
        CheckedStructuralValueRoot, CheckedUnitEffectOperationPlan as OperationPlan,
    };

    /*
        Calls prepared against one candidate body's real operation prefix. This
        scratch belongs to the construction attempt; only its winning body moves
        the rows into the retained value arenas. Queries borrow the same executable
        schedules that lowering will later consume, never reconstructing a call.
    */
    #[derive(Default)]
    pub(crate) struct PreparedCalls {
        rows: Vec<PreparedRoot>,
    }

    struct PreparedRoot {
        owner: symbols::SymbolHandle,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        value: checked_trees::CheckedStructuralValueHandle,
        preceding_bindings: u32,
        calls: Vec<checked_trees::CheckedStructuralValueCall>,
    }

    impl PreparedRoot {
        fn operation_dependencies(&self) -> impl Iterator<Item = &OperationPlan> {
            self.calls.iter().flat_map(|call| {
                call.operands().iter().chain(call.operation()).chain(
                    call.completions()
                        .into_iter()
                        .flat_map(checked_trees::CheckedOperandSequence::iter),
                )
            })
        }
    }

    impl PreparedCalls {
        pub(super) fn append(&mut self, owner: symbols::SymbolHandle, mut completed: Self) {
            for row in &mut completed.rows {
                row.owner = owner;
            }
            self.rows.append(&mut completed.rows);
        }

        pub(super) fn retain_bodies(
            &mut self,
            mut survives: impl FnMut(symbols::SymbolHandle, symbols::SymbolHandle) -> bool,
        ) {
            self.rows.retain(|row| survives(row.owner, row.state));
        }

        pub(super) fn state_schedules(
            &self,
            machine: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
        ) -> impl Iterator<Item = &[checked_trees::CheckedStructuralValueCall]> {
            self.rows
                .iter()
                .filter(move |row| row.machine == machine && row.state == state)
                .map(|row| row.calls.as_slice())
        }

        pub(super) fn operations_for(
            &self,
            owner: symbols::SymbolHandle,
            state: symbols::SymbolHandle,
        ) -> impl Iterator<Item = &OperationPlan> {
            self.rows
                .iter()
                .filter(move |row| row.owner == owner && row.state == state)
                .flat_map(PreparedRoot::operation_dependencies)
        }

        pub(super) fn operation_dependencies(&self) -> impl Iterator<Item = &OperationPlan> {
            self.rows
                .iter()
                .flat_map(PreparedRoot::operation_dependencies)
        }

        pub(super) const EMPTY: Self = Self { rows: Vec::new() };

        pub(super) fn schedule(
            &self,
            value: checked_trees::CheckedStructuralValueHandle,
        ) -> Option<&[checked_trees::CheckedStructuralValueCall]> {
            let mut rows = self.rows.iter().filter(|row| row.value == value);
            let row = rows.next()?;
            if rows.next().is_some() {
                return None;
            }
            Some(&row.calls)
        }

        /*
            Complete only roots whose authored statement has executed. A future
            selected root remains unavailable to the conservative future-use fence.
        */
        #[allow(clippy::too_many_arguments)]
        pub(super) fn prepare_before(
            &mut self,
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            shapes: &mut ShapeCollector<'_>,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
            trivial_locals: &[(
                checked_trees::CheckedTrivialAffineStructuralLocalPlan,
                symbols::SymbolHandle,
            )],
            entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
            operations: &[OperationPlan],
            boundary: usize,
        ) {
            let order = program
                .statement_table
                .admitted_execution_order(state.statement_nodes);
            for root in facts.values.structural_values.roots_for_state(state.symbol) {
                if root.machine != machine.symbol
                    || root.state != state.symbol
                    || !order.precedes(root.statement_ordinal as usize, boundary)
                    || self.rows.iter().any(|row| row.value == root.root)
                {
                    continue;
                }
                if let Some((preceding_bindings, calls, _)) = prepare_root(
                    program,
                    facts,
                    shapes,
                    machine,
                    state,
                    parameters,
                    trivial_locals,
                    entry_claims,
                    operations,
                    root,
                ) {
                    self.rows.push(PreparedRoot {
                        owner: machine.symbol,
                        machine: root.machine,
                        state: root.state,
                        value: root.root,
                        preceding_bindings,
                        calls,
                    });
                }
            }
        }
    }

    /*
        Keep nested operand preparations in their owning root's namespace. A
        separately catalogued inner root can start with different private ordinals.
        Existing operation traversal visits each embedded preparation and normal
        completion; catalog enumeration still does not select an execution path.
    */
    pub(super) fn dependencies(
        calls: &[checked_trees::CheckedStructuralValueCall],
    ) -> impl Iterator<Item = &checked_trees::CheckedStructuralValueCall> {
        calls.iter().chain(calls.iter().flat_map(|call| {
            call.operands()
                .iter()
                .chain(
                    call.completions()
                        .into_iter()
                        .flat_map(checked_trees::CheckedOperandSequence::iter),
                )
                .flat_map(OperationPlan::with_value_calls)
                .filter_map(|operation| match operation {
                    OperationPlan::EstablishLocal(
                        checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                            calls, ..
                        },
                    )
                    | OperationPlan::EvaluateStructuralValue { calls, .. } => {
                        Some(calls.as_slice())
                    }
                    _ => None,
                })
                .flatten()
        }))
    }

    pub(crate) fn retain(facts: &mut CheckFacts, prepared: PreparedCalls) -> Result<(), String> {
        for row in prepared.rows {
            if let Some((preceding, calls)) =
                facts.values.structural_values.call_schedule(row.value)
            {
                if preceding != row.preceding_bindings || calls != row.calls {
                    return Err(
                        "surviving bodies disagree on a structural value call schedule".into(),
                    );
                }
                continue;
            }
            facts.values.structural_values.retain_calls_for_root(
                row.value,
                row.preceding_bindings,
                row.calls,
            );
        }
        Ok(())
    }

    /*
        Catalog calls execute in scalar/value continuations, while the preceding
        producer's disposal debt belongs to its enclosing checked operation body.
        Rejoin that shared namespace inside the candidate using ordinary transfer rules.
    */
    pub(super) fn consume_caller_results<'a>(
        program: &dyn ::validation::CheckingInput,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        operations: &mut [OperationPlan],
        schedules: impl Iterator<Item = &'a [checked_trees::CheckedStructuralValueCall]>,
    ) -> Option<()> {
        let authored_state = program.state_by_symbol(state)?;
        let order = program
            .statement_table
            .admitted_execution_order(authored_state.statement_nodes);
        let mut consumed = Vec::new();
        for calls in schedules {
            for producer in calls {
                for operation in producer
                    .operands()
                    .iter()
                    .chain(producer.operation())
                    .flat_map(OperationPlan::with_value_calls)
                {
                    let Some(call) = operation.call() else {
                        continue;
                    };
                    if operations
                        .iter()
                        .flat_map(OperationPlan::with_value_calls)
                        .any(|existing| {
                            existing
                                .call()
                                .is_some_and(|existing| existing.coordinate == call.coordinate)
                        })
                    {
                        continue;
                    }
                    let mut earlier = operations
                        .iter()
                        .filter(|operation| {
                            operation.structural_result_binding().is_some_and(|result| {
                                order.precedes(
                                    result.statement_index as usize,
                                    call.coordinate.statement_index as usize,
                                )
                            })
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    let arguments = call
                        .structural_arguments
                        .iter()
                        .filter(|argument| {
                            argument
                                .source_structural_result_binding_ordinal()
                                .is_some_and(|ordinal| {
                                    earlier.iter().any(|operation| {
                                        operation
                                            .structural_result_binding()
                                            .is_some_and(|result| result.binding_ordinal == ordinal)
                                    })
                                })
                        })
                        .cloned()
                        .collect::<Vec<_>>();
                    if arguments.is_empty() {
                        continue;
                    }
                    if let Some((_, target, previous)) = consumed
                        .iter()
                        .find(|(coordinate, _, _)| *coordinate == call.coordinate)
                    {
                        if *target != call.target.state || *previous != arguments {
                            return None;
                        }
                        continue;
                    }
                    let retires_caller = arguments.iter().any(|argument| {
                        argument.access == checked_trees::CheckedStructuralAccess::Owned
                            && argument.path.is_empty()
                            && earlier.iter().any(|operation| {
                                operation.discardable_structural_result().is_some_and(
                                    |(result, discard)| {
                                        Some(result.binding_ordinal)
                                            == argument.source_structural_result_binding_ordinal()
                                            && discard
                                            && !result.local.is_named()
                                            && result.multiplicity
                                                == language_semantics::Multiplicity::Affine
                                    },
                                )
                            })
                    });
                    if retires_caller {
                        validation::call_is_unconditional(
                            program,
                            machine,
                            state,
                            call.coordinate.statement_index as usize,
                            call.coordinate.call_ordinal as usize,
                        )?;
                    }
                    consume_retained_results(&mut earlier, operation)?;
                    for producer in earlier {
                        let binding = producer.structural_result_binding()?;
                        let mut targets = operations.iter_mut().filter(|operation| {
                            operation.structural_result_binding().is_some_and(|result| {
                                result.binding_ordinal == binding.binding_ordinal
                            })
                        });
                        let target = targets.next()?;
                        if targets.next().is_some() {
                            return None;
                        }
                        *target = producer;
                    }
                    consumed.push((call.coordinate, call.target.state, arguments));
                }
            }
        }
        Some(())
    }

    pub(super) struct StateCallInputs<'state> {
        pub parameters: &'state [checked_trees::CheckedUnitStructuralParameterPlan],
        pub entry_claims: &'state [checked_trees::CheckedUnitEntryClaimPlan],
        pub operations: &'state [OperationPlan],
        pub trivial_locals: &'state [checked_trees::CheckedTrivialAffineStructuralLocalPlan],
    }

    /*
        Complete the same catalogue after the full candidate prefix is known.
        Earlier query boundaries keep their schedules and private ordinals; only
        missing roots are prepared here, before custody and dependency admission.
    */
    pub(super) fn prepare_state(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        prepared: &mut PreparedCalls,
        machine: symbols::SymbolHandle,
        state: symbols::SymbolHandle,
        inputs: &StateCallInputs<'_>,
    ) -> Option<()> {
        let (machine, state) =
            validation::find_state(program, state).filter(|(owner, _)| owner.symbol == machine)?;
        let trivial_locals = inputs
            .trivial_locals
            .iter()
            .filter_map(|local| {
                match crate::flow::state_statements(program, state.statement_nodes)
                    .get(local.declaration_ordinal as usize)?
                {
                    typed_trees::statement::StatementNode::LocalData(source) => {
                        Some((local.clone(), source.symbol))
                    }
                    _ => None,
                }
            })
            .collect::<Vec<_>>();
        for root in facts.values.structural_values.roots_for_state(state.symbol) {
            if root.machine != machine.symbol
                || root.state != state.symbol
                || prepared.rows.iter().any(|row| row.value == root.root)
            {
                continue;
            }
            if let Some((preceding_bindings, calls, _)) = prepare_root(
                program,
                facts,
                shapes,
                machine,
                state,
                inputs.parameters,
                &trivial_locals,
                inputs.entry_claims,
                inputs.operations,
                root,
            ) {
                prepared.rows.push(PreparedRoot {
                    owner: machine.symbol,
                    machine: root.machine,
                    state: root.state,
                    value: root.root,
                    preceding_bindings,
                    calls,
                });
            }
        }
        Some(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare_root(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        shapes: &mut ShapeCollector<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
        trivial_locals: &[(
            checked_trees::CheckedTrivialAffineStructuralLocalPlan,
            symbols::SymbolHandle,
        )],
        entry_claims: &[checked_trees::CheckedUnitEntryClaimPlan],
        operations: &[OperationPlan],
        root: &CheckedStructuralValueRoot,
    ) -> Option<(
        u32,
        Vec<checked_trees::CheckedStructuralValueCall>,
        checked_trees::CheckedValueCompletion,
    )> {
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let mut preceding = Vec::new();
        let mut results = Vec::new();
        let mut count = 0;
        let source_flow = state_flow(facts, machine.symbol, state.symbol)?;
        let source_calls = facts.flow.control.calls.span(source_flow.calls)?;
        for operation in operations {
            let Some(result) = operation.structural_result_binding() else {
                let statement = operation.source_statement_index();
                if statement.is_some_and(|statement| {
                    order.precedes(statement as usize, root.statement_ordinal as usize)
                }) {
                    preceding.push(operation.clone());
                }
                continue;
            };
            if !order.precedes(
                result.statement_index as usize,
                root.statement_ordinal as usize,
            ) {
                continue;
            }
            count = count.max(result.binding_ordinal as usize + 1);
            if let Some(owner) =
                structural_result_owner(program, facts, state.symbol, source_calls, operation)
            {
                results.push((result.clone(), owner));
            }
            preceding.push(operation.clone());
        }
        let preceding_bindings = u32::try_from(count).ok()?;
        let (calls, completion) = structural_operands::value_calls(
            program,
            facts,
            ScalarCalleeClosure,
            shapes,
            machine,
            state,
            parameters,
            trivial_locals,
            entry_claims,
            &results,
            &preceding,
            &mut count,
            root.root,
            &LocalConstructionTrace::default(),
            &mut crate::execution::terminal_unit::borrowed_windows::OpenWindows::default(),
        )?;
        Some((preceding_bindings, calls, completion))
    }
}
pub(crate) use structural_value_calls::retain as retain_structural_value_calls;
mod calling_signatures {
    /*
        Collect declaration ABI storage in the ordinary checked shape namespace.

        A refused dormant ABI remains a refused row, not a failed source check.
        Every projection retains callback binders universally and substitutes only
        the other local operands. Selected Handler bodies cannot choose storage.
    */
    use super::types::{self, ShapeCollector};
    use checked_trees::{CheckedCallingSignaturePlan, CheckedCallingSignatureStorage};
    use typed_trees::service_declarations::signatures::PreparedBoundarySignatures;

    pub(super) fn collect(
        program: &dyn validation::CheckingInput,
        signatures: &PreparedBoundarySignatures,
        shapes: &mut ShapeCollector<'_>,
    ) -> Vec<CheckedCallingSignaturePlan> {
        let mut plans = Vec::with_capacity(signatures.abi_signatures().len());
        for declaration in signatures.abi_signatures() {
            shapes.take_unselected_opaque();
            let mut storage = (|| {
                let telescope = declaration.telescope.as_ref().map_err(Clone::clone)?;
                let mut completed = None;
                for projection in &declaration.projections {
                    let projection = projection.as_ref().map_err(Clone::clone)?;
                    let mut parameters = Vec::with_capacity(projection.parameters.len());
                    for (ordinal, reference) in projection.parameters.iter().copied().enumerate() {
                        let parameter = shapes
                            .add_calling_parameter(reference, &projection.substitutions)
                            .ok_or_else(|| {
                                format!(
                                    "declaration ABI parameter {ordinal} has no retained storage"
                                )
                            })?;
                        parameters.push(parameter);
                    }
                    let effective_result = types::substituted_formal_type(
                        program,
                        projection.result,
                        &projection.substitutions,
                    );
                    let result = if !effective_result.is_valid()
                        || types::is_unit(program, effective_result)
                    {
                        None
                    } else {
                        Some(
                            shapes
                                .add_stored_type(projection.result, &[], &projection.substitutions)
                                .ok_or("declaration ABI result has no retained storage")?,
                        )
                    };
                    if parameters.len() != telescope.parameters.len() {
                        return Err(
                            "declaration ABI storage differs from its exact native telescope"
                                .into(),
                        );
                    }
                    let next = CheckedCallingSignatureStorage {
                        parameters,
                        result,
                        telescope: telescope.clone(),
                    };
                    if completed.as_ref().is_some_and(|previous| previous != &next) {
                        return Err(
                            "storage projections disagree under one exact declaration ABI application"
                                .into(),
                        );
                    }
                    completed = Some(next);
                }
                completed.ok_or_else(|| "declaration ABI has no produced storage projection".into())
            })();
            if shapes.take_unselected_opaque().is_some() {
                storage = storage.map_err(|reason: String| {
                    format!("{reason}: opaque representation is not selected")
                });
            }
            plans.push(CheckedCallingSignaturePlan {
                id: declaration.id,
                source_template: declaration.template,
                requirement_identity: declaration.requirement_identity.clone(),
                declaring_application: declaration.declaring_application.clone(),
                operands: declaration.operands.clone(),
                formals: declaration.formals.clone(),
                default_eligible: declaration.default_eligible,
                physical_owner: declaration.physical_owner.clone(),
                storage,
            });
        }
        plans
    }
}
#[path = "execution_terminal_unit_types.rs"]
pub(crate) mod types;

pub(crate) use calls::ComputationCallArguments;
pub(crate) use types::{is_reference, strips_erased_parameter};

/*
    Present when the planning pass ends in the candidate closure, which
    resolves each scalar call to its callee's composed or ordinary
    scalar-result Unit body. A pass without it (provider settlement) has no closure
    to defer to and admits only callees it can settle itself.
*/
#[derive(Clone, Copy)]
pub(crate) struct ScalarCalleeClosure;

pub(crate) fn build_checked_unit_effect_plans(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    calling_declarations: &typed_trees::service_declarations::signatures::PreparedBoundarySignatures,
    boundary_signature_reaches: &[(symbols::SymbolHandle, language_semantics::ServiceReachRowId)],
    scalar_callees: ScalarCalleeClosure,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> (
    EffectPlans,
    CheckedPartialAffineUnitCleanupPlans,
    structural_value_calls::PreparedCalls,
) {
    let mut shapes = ShapeCollector::new(program);
    let calling_signatures =
        calling_signatures::collect(program, calling_declarations, &mut shapes);
    let mut prepared_calls = structural_value_calls::PreparedCalls::default();
    let retained_records = state_graph::RetainedBorrowRecords::new(facts);
    let mut boundary_gaps = Vec::new();
    let (boundary_machines, dynamic_slots, boundary_symbols) = {
        let mut boundary_machines = program
            .machines()
            .iter()
            .filter(|machine| !machine.supplies_checked_body())
            .filter_map(|machine| {
                shapes.take_unselected_opaque();
                match build_boundary_machine(program, facts, &mut shapes, machine) {
                    Ok(plan) => Some(plan),
                    Err(reason) => {
                        let reason = shapes.take_unselected_opaque().map_or(
                            GapReason::Unsupported(std::borrow::Cow::Borrowed(reason)),
                            GapReason::UnselectedOpaqueRepresentation,
                        );
                        boundary_gaps.push(plan_gaps::plan_gap(
                            program,
                            machine.symbol,
                            plan_gaps::GapSite::Machine,
                            reason,
                        ));
                        None
                    }
                }
            })
            .collect::<Vec<_>>();
        boundary_machines.extend(build_static_boundary_requirements(
            program,
            facts,
            boundary_signature_reaches,
            &mut boundary_gaps,
            &mut shapes,
        ));
        let dynamic_slots = control::build_dynamic_slot_plans(program, &mut shapes);
        let boundary_symbols = boundary_machines
            .iter()
            .map(|plan| plan.machine)
            .collect::<Vec<_>>();
        (boundary_machines, dynamic_slots, boundary_symbols)
    };
    let mut candidates = Vec::new();
    let mut composed_machines = Vec::new();
    let mut partial_affine_machines = Vec::new();
    let mut construction_gaps = BTreeMap::new();
    {
        let bodies = program
            .machines()
            .iter()
            .filter(|machine| machine.supplies_checked_body())
            .collect::<Vec<_>>();
        let plan = |shapes: &mut ShapeCollector<'_>, machine: &typed_trees::machine::Machine| {
            plan_checked_body(
                program,
                facts,
                &retained_records,
                scalar_callees,
                call_frames,
                shapes,
                machine,
            )
        };
        shapes.prepare_forks();
        let fork_from = |shapes: &ShapeCollector<'_>, base: usize, roster: &[usize]| {
            let snapshot = shapes.fork();
            ::validation::checking_thread_pool().install(|| {
                use rayon::prelude::*;
                roster
                    .par_iter()
                    .map(|&index| {
                        let mut fork = snapshot.fork();
                        let marks = fork.row_marks();
                        let planned = plan(&mut fork, bodies[index]);
                        (
                            index,
                            Some((base, planned, fork.edits_since(Some(&snapshot), marks))),
                        )
                    })
                    .collect::<Vec<_>>()
            })
        };
        let mut attempts = Vec::new();
        attempts.resize_with(bodies.len(), || None);
        for (index, attempt) in fork_from(&shapes, 0, &(0..bodies.len()).collect::<Vec<_>>()) {
            attempts[index] = attempt;
        }
        let mut log = ChangeLog::default();
        let mut position = 0;
        while let Some(&machine) = bodies.get(position) {
            let planned = match attempts[position].take() {
                Some((base, planned, edits)) if log.admits(&edits, base) => {
                    log.record(&edits, position);
                    shapes.apply(edits);
                    planned
                }
                stale => {
                    attempts[position] = stale;
                    let roster = (position..bodies.len())
                        .filter(|&index| {
                            index == position
                                || attempts[index]
                                    .as_ref()
                                    .is_none_or(|(base, _, edits)| !log.admits(edits, *base))
                        })
                        .collect::<Vec<_>>();
                    if roster.len() > 1 {
                        for (index, attempt) in fork_from(&shapes, position, &roster) {
                            attempts[index] = attempt;
                        }
                        continue;
                    }
                    attempts[position] = None;
                    shapes.forget_consulted();
                    let marks = shapes.row_marks();
                    let planned = plan(&mut shapes, machine);
                    log.record(&shapes.edits_since(None, marks), position);
                    planned
                }
            };
            position += 1;
            match planned {
                Ok((body, attempt_calls)) => {
                    match body {
                        PlannedBody::Sequence(plan) => candidates.push(plan),
                        PlannedBody::PartialAffine(plan) => partial_affine_machines.push(plan),
                        PlannedBody::Graph(plan) => composed_machines.push(plan),
                    }
                    prepared_calls.append(machine.symbol, attempt_calls);
                }
                Err(gap) => {
                    construction_gaps.insert(symbol_key(machine.symbol), gap);
                }
            }
        }
    };

    let planned = planned_machines(&candidates, &composed_machines, &partial_affine_machines);
    let mut gaps = construction_gaps
        .into_iter()
        .filter(|(key, _)| !planned.contains(key))
        .map(|(_, gap)| gap)
        .collect::<Vec<_>>();
    gaps.extend(boundary_gaps);
    let targets = candidates
        .iter()
        .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
        .map(CheckedUnitEffectMachinePlan::callable_entry)
        .chain(
            composed_machines
                .iter()
                .filter_map(CheckedComposedUnitControlMachinePlan::callable_entry),
        )
        .collect::<Vec<_>>();
    let mut select_sequence_cleanup = |plan: &CheckedUnitEffectMachinePlan| {
        let temporary = cleanup::temporary_actions::operations(
            program,
            facts,
            &targets,
            &mut shapes,
            plan.machine,
            plan.state,
            &plan.structural_parameters,
            &plan.operations,
        )
        .ok()?;
        let normal =
            cleanup::normal_edges::sequence_cleanups(program, facts, &targets, &mut shapes, plan)?;
        if let Some(stopping) = plan
            .operations
            .iter()
            .position(|operation| !operation.has_normal_continuation())
        {
            if stopping + 1 != plan.operations.len()
                || !shapes.cleanup_control.normal_exits(normal).is_empty()
            {
                return None;
            }
        } else if plan.scalar_control.is_none() {
            let [row] = shapes.cleanup_control.normal_exits(normal) else {
                return None;
            };
            if row.exit.kind != checked_trees::FlowClaimJoinExitKind::FinalExpression {
                return None;
            }
        }
        Some((temporary, normal))
    };
    let mut temporary_selections = candidates
        .iter()
        .map(&mut select_sequence_cleanup)
        .collect::<Vec<_>>()
        .into_iter();
    let mut partial_affine_selections = partial_affine_machines
        .iter()
        .map(|plan| select_sequence_cleanup(&plan.machine))
        .collect::<Vec<_>>()
        .into_iter();
    let mut composed_selections = composed_machines
        .iter()
        .map(|plan| {
            plan.states
                .iter()
                .map(|state| {
                    let temporary = cleanup::temporary_actions::operations(
                        program,
                        facts,
                        &targets,
                        &mut shapes,
                        plan.machine,
                        state.state,
                        &state.structural_parameters,
                        &state.operations,
                    )
                    .map_err(|reason| (state.state, reason))?;
                    let normal = cleanup::normal_edges::state_cleanups(
                        program,
                        facts,
                        &targets,
                        &mut shapes,
                        state,
                    )
                    .map_err(|reason| (state.state, reason))?;
                    Ok((temporary, normal))
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Vec<_>>()
        .into_iter();
    drop(targets);
    let mut cleanup_gaps = BTreeMap::new();
    composed_machines.retain_mut(|plan| match composed_selections.next() {
        Some(Ok(selected)) => {
            for (state, (selected, normal)) in plan.states.iter_mut().zip(selected) {
                if let Err(reason) =
                    cleanup::temporary_actions::install(&mut state.operations, selected)
                {
                    cleanup_gaps
                        .entry(symbol_key(plan.machine))
                        .or_insert_with(|| {
                            plan_gap(
                                program,
                                plan.machine,
                                GapSite::State(state.state),
                                GapReason::Unsupported(std::borrow::Cow::Borrowed(reason)),
                            )
                        });
                    return false;
                }
                state.normal_exit_cleanups = normal;
            }
            true
        }
        Some(Err((state, construct))) => {
            cleanup_gaps
                .entry(symbol_key(plan.machine))
                .or_insert_with(|| {
                    plan_gap(
                        program,
                        plan.machine,
                        GapSite::State(state),
                        GapReason::Unsupported(std::borrow::Cow::Borrowed(construct)),
                    )
                });
            false
        }
        None => false,
    });
    let install_sequence_cleanup = |plan: &mut CheckedUnitEffectMachinePlan, (selected, normal)| {
        if cleanup::temporary_actions::install(&mut plan.operations, selected).is_err() {
            return false;
        }
        plan.normal_exit_cleanups = normal;
        if plan
            .operations
            .last()
            .is_some_and(|operation| !operation.has_normal_continuation())
        {
            return true;
        }
        let Some(CheckedUnitEffectOperationPlan::Complete {
            trivial_affine_discards,
            ..
        }) = plan.operations.last_mut()
        else {
            return false;
        };
        trivial_affine_discards.clear();
        true
    };
    partial_affine_machines.retain_mut(|plan| match partial_affine_selections.next() {
        Some(Some(selection)) => install_sequence_cleanup(&mut plan.machine, selection),
        _ => {
            cleanup_gaps
                .entry(symbol_key(plan.machine.machine))
                .or_insert_with(|| {
                    plan_gap(
                        program,
                        plan.machine.machine,
                        GapSite::State(plan.machine.state),
                        GapReason::Unsupported(std::borrow::Cow::Borrowed(
                            "sequence cleanup actions",
                        )),
                    )
                });
            false
        }
    });
    candidates.retain_mut(|plan| match temporary_selections.next() {
        Some(Some(selection)) => install_sequence_cleanup(plan, selection),
        _ => {
            cleanup_gaps
                .entry(symbol_key(plan.machine))
                .or_insert_with(|| {
                    plan_gap(
                        program,
                        plan.machine,
                        GapSite::State(plan.state),
                        GapReason::Unsupported(std::borrow::Cow::Borrowed(
                            "sequence cleanup actions",
                        )),
                    )
                });
            false
        }
    });
    let planned = planned_machines(&candidates, &composed_machines, &partial_affine_machines);
    gaps.extend(
        cleanup_gaps
            .into_iter()
            .filter(|(key, _)| !planned.contains(key))
            .map(|(_, gap)| gap),
    );

    let closure_gaps = candidate_closure::retain_available(
        program,
        facts,
        &boundary_symbols,
        &shapes.cleanup_control,
        &mut candidates,
        &mut composed_machines,
        &mut partial_affine_machines,
        &prepared_calls,
    );
    gaps.extend(closure_gaps);
    prepared_calls.retain_bodies(|owner, state_symbol| {
        candidates
            .iter()
            .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
            .any(|plan| plan.machine == owner && plan.state == state_symbol)
            || composed_machines.iter().any(|plan| {
                plan.machine == owner && plan.states.iter().any(|state| state.state == state_symbol)
            })
    });
    gaps.sort_by_key(|gap| symbol_key(gap.machine));
    let computations = &facts.values.scalar_computations;
    let mut structural_roots_by_expr: symbols::SymbolKeyMap<
        typed_trees::expression::ExpressionHandle,
        Vec<checked_trees::CheckedStructuralValueHandle>,
    > = symbols::SymbolKeyMap::default();
    for (_, root) in facts.values.structural_values.roots.iter() {
        structural_roots_by_expr
            .entry(root.expression)
            .or_default()
            .push(root.root);
    }
    let scalar_value_roots = computations
        .nodes
        .iter()
        .filter_map(|(_, node)| match &node.kind {
            checked_trees::CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } => computations
                .structural_arguments
                .span(*structural_arguments),
            _ => None,
        })
        .flatten()
        .filter_map(|argument| match argument.as_place()?.source {
            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                expression, ..
            } => Some(expression),
            _ => None,
        })
        .flat_map(|expression| {
            structural_roots_by_expr
                .get(&expression)
                .map_or(&[][..], Vec::as_slice)
                .iter()
                .copied()
        });
    let value_roots = candidates
        .iter()
        .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
        .flat_map(|plan| plan.operations.iter())
        .chain(
            composed_machines
                .iter()
                .flat_map(|plan| plan.states.iter())
                .flat_map(|state| state.operation_dependencies()),
        )
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        .filter_map(|operation| {
            operation
                .structural_value_evaluation()
                .map(|(value, _)| value)
        })
        .chain(scalar_value_roots)
        .collect::<Vec<_>>();
    let value_type_identities = retain_value_types(program, facts, &mut shapes, value_roots);
    let mut retained_type_identities = boundary_machines
        .iter()
        .flat_map(|plan| {
            plan.structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity)
                .chain(plan.result.structural_identity())
        })
        .chain(calling_signatures.iter().filter_map(|row| row.storage.as_ref().ok()).flat_map(|storage| storage.type_roots()))
        .chain(dynamic_slots.iter().flat_map(|plan| {
            plan.structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity)
                .chain(plan.result.structural_identity())
        }))
        .chain(candidates.iter().chain(partial_affine_machines.iter().map(|plan| &plan.machine)).flat_map(|plan| {
            plan.structural_parameters
                .iter()
                .map(|parameter| parameter.type_identity)
                .chain(
                    plan.parameter_storage
                        .iter()
                        .map(|storage| storage.type_identity),
                )
                .chain(
                    shapes.cleanup_control.normal_actions(plan.normal_exit_cleanups).map(|action| action.type_identity()),
                )
                .chain(
                    plan.trivial_affine_locals
                        .iter()
                        .map(|local| local.type_identity),
                )
                .chain(
                    plan.trivial_affine_locals
                        .iter()
                        .filter_map(|local| local.construction.as_ref())
                        .map(|element| element.root_type_identity),
                )
                .chain(
                    plan.operations
                        .iter()
                        .filter_map(|operation| match operation {
                            CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Primitive {
                                type_identity,
                                ..
                            }) => Some(*type_identity),
                            _ => None,
                        }),
                )
        }))
        .chain(composed_machines.iter().flat_map(|plan| {
            plan.result
                .structural_identity()
                .into_iter()
                .chain(
                    plan.states
                        .iter()
                        .flat_map(|state| &state.structural_parameters)
                        .map(|parameter| parameter.type_identity),
                )
                .chain(
                    plan.states
                        .iter()
                        .flat_map(|state| &state.parameter_storage)
                        .map(|storage| storage.type_identity),
                )
                .chain(plan.states.iter().flat_map(|state| shapes.cleanup_control.normal_actions(state.normal_exit_cleanups)).map(|action| match action {
                        checked_trees::CheckedNormalCleanupAction::Discard(discard) => discard.type_identity,
                        checked_trees::CheckedNormalCleanupAction::InvokeNominal(cleanup) => cleanup.type_identity,
                        checked_trees::CheckedNormalCleanupAction::RetireHome { type_identity, .. } => *type_identity,
                    }))
                .chain(plan.states.iter().flat_map(|state| {
                    state
                        .operation_dependencies()
                        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                        .flat_map(|operation| {
                            let mut seeded: Vec<typed_trees::type_identity::TypeIdentityId> = operation
                                .structural_operands()
                                .iter()
                                .flat_map(|argument| {
                                    std::iter::once(argument.type_identity)
                                        .chain(argument.borrowed_root_type_identity())
                                })
                                .collect();
                            match operation {
                                CheckedUnitEffectOperationPlan::EstablishReference { result, source, .. } => {
                                    seeded.push(result.type_identity);
                                    seeded.push(source.type_identity);
                                }
                                CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Primitive {
                                    type_identity,
                                    ..
                                }) => {
                                    seeded.push(*type_identity);
                                }
                                CheckedUnitEffectOperationPlan::Call(call) => {
                                    seeded.extend(
                                        call.structural_result()
                                            .map(|result| result.type_identity),
                                    );
                                }
                                CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::ByteSequence {
                                    result,
                                    ..
                                })
                                | CheckedUnitEffectOperationPlan::EstablishViewSubslice {
                                    result,
                                    ..
                                }
                                | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Structural {
                                    result,
                                    ..
                                })
                                | CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::ScalarArray {
                                    result,
                                    ..
                                }) => {
                                    seeded.push(result.type_identity);
                                }
                                CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                                    result,
                                    ..
                                }) => {
                                    seeded.push(result.type_identity);
                                }
                                CheckedUnitEffectOperationPlan::Store(
                                    checked_trees::CheckedStorePlan {
                                        destination:
                                            checked_trees::CheckedStoreDestination {
                                                leaf:
                                                    checked_trees::CheckedStoreLeaf::StructuralStore {
                                                        type_identity,
                                                        ..
                                                    },
                                                ..
                                            },
                                        value: checked_trees::CheckedStoreValue::Place(value),
                                        ..
                                    },
                                ) => {
                                    seeded.push(*type_identity);
                                    seeded.push(value.type_identity);
                                }
                                CheckedUnitEffectOperationPlan::MoveStructuralField {
                                    result,
                                    source,
                                    ..
                                } => {
                                    seeded.push(result.type_identity);
                                    seeded.push(source.type_identity);
                                }
                                _ => {}
                            }
                            seeded
                        })
                }))
        }))
        .collect::<BTreeSet<_>>();
    retained_type_identities.extend(value_type_identities);
    for operation in candidates
        .iter()
        .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
        .flat_map(|plan| &plan.operations)
        .chain(composed_machines.iter().flat_map(|plan| {
            plan.states
                .iter()
                .flat_map(|state| state.operation_dependencies())
        }))
        .chain(prepared_calls.operation_dependencies())
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        for argument in operation.structural_operands() {
            retained_type_identities.insert(argument.type_identity);
            retained_type_identities.extend(argument.borrowed_root_type_identity());
            retained_type_identities.extend(argument.subslice_root_type_identity());
            if let Some(storage_type_identity) = argument.subslice_storage_type_identity() {
                retained_type_identities.insert(storage_type_identity);
            }
        }
        match operation {
            CheckedUnitEffectOperationPlan::AtomicAccess(access) => {
                retained_type_identities.insert(access.receiver_root_type_identity);
                if let Some(result) = &access.structural_result {
                    retained_type_identities.insert(result.type_identity);
                }
            }
            CheckedUnitEffectOperationPlan::CaptureAssignmentDestination {
                destination, ..
            } => {
                retained_type_identities.insert(destination.type_identity);
            }
            CheckedUnitEffectOperationPlan::Call(call) => {
                if let (
                    CheckedUnitCallRoute::Boundary,
                    CheckedUnitCallResult::Structural {
                        binding: result, ..
                    },
                ) = (call.route(), call.result())
                {
                    retained_type_identities.insert(result.type_identity);
                }
            }
            CheckedUnitEffectOperationPlan::EstablishReference { result, source, .. } => {
                retained_type_identities.insert(result.type_identity);
                retained_type_identities.insert(source.type_identity);
            }
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ByteSequence { result, .. },
            )
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::Structural { result, .. },
            )
            | CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::ScalarArray { result, .. },
            ) => {
                retained_type_identities.insert(result.type_identity);
            }
            CheckedUnitEffectOperationPlan::EstablishViewSubslice { result, source } => {
                retained_type_identities.insert(result.type_identity);
                retained_type_identities.extend(source.subslice_root_type_identity());
                if let Some(storage_type_identity) = source.subslice_storage_type_identity() {
                    retained_type_identities.insert(storage_type_identity);
                }
            }
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { result, .. },
            ) => {
                retained_type_identities.insert(result.type_identity);
            }
            CheckedUnitEffectOperationPlan::Store(checked_trees::CheckedStorePlan {
                destination:
                    checked_trees::CheckedStoreDestination {
                        leaf: checked_trees::CheckedStoreLeaf::StructuralStore { type_identity, .. },
                        ..
                    },
                value: checked_trees::CheckedStoreValue::Place(value),
                ..
            }) => {
                retained_type_identities.insert(*type_identity);
                retained_type_identities.insert(value.type_identity);
            }
            CheckedUnitEffectOperationPlan::MoveStructuralField { result, source, .. } => {
                retained_type_identities.insert(result.type_identity);
                retained_type_identities.insert(source.type_identity);
            }
            _ => {}
        }
    }
    let loaned_storage_types = candidates
        .iter()
        .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
        .map(|plan| plan.machine)
        .chain(composed_machines.iter().map(|plan| plan.machine))
        .collect::<Vec<_>>();
    let mut retained_loan_identities = Vec::new();
    for machine in loaned_storage_types {
        let Some(source_machine) = program.machine(machine) else {
            continue;
        };
        for state in program.machine_states(source_machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(local) = statement.as_local_data() else {
                    continue;
                };
                let TypeReferenceNode::Reference { referee, .. } = program
                    .type_reference_table
                    .type_reference(local.type_reference)
                else {
                    continue;
                };
                if let Some(identity) = shapes.add_type(*referee, &[], &[]) {
                    retained_loan_identities.push(identity);
                }
            }
        }
    }
    for identity in &retained_loan_identities {
        retained_type_identities.insert(*identity);
    }
    let operand_identities = facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(_, node)| {
            let operand_type = match node.kind {
                checked_trees::CheckedScalarComputationKind::ByteSequenceLiteralComparison {
                    operand_type,
                    ..
                }
                | checked_trees::CheckedScalarComputationKind::ByteSequencePredicate {
                    operand_type,
                    ..
                } => operand_type,
                _ => return None,
            };
            let backing = shapes.add_type(operand_type, &[], &[])?;
            let carrier = shapes.add_stored_type(operand_type, &[], &[])?;
            Some([backing, carrier])
        })
        .flatten()
        .collect::<Vec<_>>();
    retained_type_identities.extend(operand_identities.iter().copied());
    let layout_observations =
        types::retain_layout_observation_types(program, &mut shapes, &mut retained_type_identities);
    for (_, node) in facts.values.scalar_computations.nodes.iter() {
        if let checked_trees::CheckedScalarComputationKind::StructuralField {
            source_type,
            subject,
            field,
            field_identity,
            atomic_ordering,
            ..
        } = &node.kind
            && let Ok(references) = validation::observed_storage_type_references(
                program,
                *source_type,
                &[],
                &subject.path,
                *field,
                field_identity,
                atomic_ordering.is_some(),
            )
        {
            for (reference, substitutions) in references.entries() {
                if let Some(identity) = shapes.add_type(reference, &[], substitutions) {
                    retained_type_identities.insert(identity);
                }
                if let Some(identity) = shapes.add_stored_type(reference, &[], substitutions) {
                    retained_type_identities.insert(identity);
                }
            }
        }
    }
    for (_, node) in facts.values.scalar_computations.nodes.iter() {
        if let checked_trees::CheckedScalarComputationKind::StorageObservation {
            collections, ..
        } = node.kind
        {
            for reference in facts
                .values
                .scalar_computations
                .observation_collections
                .span(collections)
                .into_iter()
                .flatten()
            {
                if let Some(identity) = shapes.add_type(*reference, &[], &[]) {
                    retained_type_identities.insert(identity);
                }
                if let Some(identity) = shapes.add_stored_type(*reference, &[], &[]) {
                    retained_type_identities.insert(identity);
                }
            }
        }
    }

    let mut formation_binders = BTreeMap::new();
    for (_, node) in facts.values.structural_values.nodes.iter() {
        if !node.type_reference.is_valid() {
            continue;
        }
        let Some(decisions) = facts
            .qualifications
            .value_formations
            .span(node.qualification_decisions)
        else {
            continue;
        };
        if decisions.is_empty() {
            continue;
        }
        let binders = formation_binders
            .entry(symbol_key(node.machine))
            .or_insert_with(|| {
                program
                    .machine_by_symbol(node.machine)
                    .map(|machine| types::machine_binders(program, machine))
                    .unwrap_or_default()
            });
        for decision in decisions {
            if let Some(identity) =
                shapes.add_domain(decision.semantic_domain, node.type_reference, binders)
            {
                retained_type_identities.insert(identity);
            }
        }
    }
    let retained_operations = || {
        candidates
            .iter()
            .chain(partial_affine_machines.iter().map(|plan| &plan.machine))
            .flat_map(|plan| {
                plan.operations
                    .iter()
                    .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                    .map(move |operation| (plan.machine, plan.state, operation))
            })
            .chain(composed_machines.iter().flat_map(|plan| {
                plan.states.iter().flat_map(move |state| {
                    state
                        .operation_dependencies()
                        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                        .map(move |operation| (plan.machine, state.state, operation))
                })
            }))
    };
    types::retain_call_qualification_domains(
        program,
        facts,
        &mut shapes,
        retained_operations(),
        &mut retained_type_identities,
    );
    let stored_operand_identities = retained_operations()
        .flat_map(|(machine, state, operation)| {
            let mut identities = byte_operands::store_operand_type_identities(
                program,
                &mut shapes,
                machine,
                state,
                operation,
            );
            if let CheckedUnitEffectOperationPlan::EstablishReference {
                source_contract, ..
            } = operation
            {
                shapes.retain_reference_contract_types(source_contract, &mut identities);
            }
            identities
        })
        .collect::<Vec<_>>();
    retained_type_identities.extend(stored_operand_identities.iter().copied());
    for transfer in composed_machines
        .iter()
        .flat_map(|machine| &machine.states)
        .flat_map(|state| state.successors())
        .flat_map(|edge| &edge.transfers)
    {
        if let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
            carrier_type_identity,
            source_contract,
            ..
        } = &transfer.source
        {
            if carrier_type_identity.is_valid() {
                retained_type_identities.insert(*carrier_type_identity);
            }
            shapes.retain_reference_contract_types(source_contract, &mut retained_type_identities);
        }
    }
    retained_type_identities.extend(partial_affine_machines.iter().flat_map(|plan| {
        plan.residual_affine_discards
            .iter()
            .map(|discard| discard.type_identity)
            .chain(plan.machine.operations.iter().filter_map(|operation| {
                let result = operation.call()?.structural_result()?;
                Some(result.type_identity)
            }))
    }));
    shapes.retain_transitive(&retained_type_identities);
    let opaque_representations = shapes
        .opaque_representations
        .into_iter()
        .filter(|row| shapes.types.contains_key(&row.opaque))
        .collect();
    types::retain_domain_predicates(program, &facts.operators, &mut shapes.domains);

    let partial_affine_plans = CheckedPartialAffineUnitCleanupPlans {
        machines: partial_affine_machines,
    };
    (
        EffectPlans {
            cleanup_control: shapes.cleanup_control.into_inner(),
            layout_observations,
            structural_types: shapes.types.into_values().collect(),
            structural_domains: {
                shapes.domains.sort_by_key(|domain| domain.domain.0);
                shapes.domains
            },
            boundary_machines,
            calling_signatures,
            opaque_representations,
            dynamic_slots,
            machines: candidates,
            composed_machines,
            gaps,
        },
        partial_affine_plans,
        prepared_calls,
    )
}

pub(super) fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

fn planned_machines(
    candidates: &[CheckedUnitEffectMachinePlan],
    composed_machines: &[CheckedComposedUnitControlMachinePlan],
    partial_affine_machines: &[CheckedPartialAffineUnitCleanupMachinePlan],
) -> BTreeSet<(u32, u32)> {
    candidates
        .iter()
        .map(|plan| symbol_key(plan.machine))
        .chain(
            composed_machines
                .iter()
                .map(|plan| symbol_key(plan.machine)),
        )
        .chain(
            partial_affine_machines
                .iter()
                .map(|plan| symbol_key(plan.machine.machine)),
        )
        .collect()
}

/* One checked body's plan, under the catalog its route publishes it in. */
enum PlannedBody {
    Sequence(CheckedUnitEffectMachinePlan),
    PartialAffine(CheckedPartialAffineUnitCleanupMachinePlan),
    Graph(CheckedComposedUnitControlMachinePlan),
}

/*
    Plan one checked body against `shapes`, or name the gap where its builder
    stopped. Everything it changes besides `shapes` is in the result, so the
    same body planned against an equal collector plans the same way.
*/
fn plan_checked_body(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    retained_records: &state_graph::RetainedBorrowRecords<'_>,
    scalar_callees: ScalarCalleeClosure,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Result<(PlannedBody, structural_value_calls::PreparedCalls), checked_trees::CheckedUnitPlanGap>
{
    let trace = LocalConstructionTrace::default();
    shapes.take_unselected_opaque();
    let mut attempt_calls = structural_value_calls::PreparedCalls::default();
    let route = body_route(program, machine);
    let planned = match route {
        BodyRoute::Sequence => (|| {
            let (mut plan, residual_affine_discards, has_projected_parameter_moves) =
                build_checked_machine_residual_parts(
                    program,
                    facts,
                    scalar_callees,
                    shapes,
                    &mut attempt_calls,
                    machine,
                    call_frames,
                    &trace,
                )?;
            structural_value_calls::prepare_state(
                program,
                facts,
                shapes,
                &mut attempt_calls,
                plan.machine,
                plan.state,
                &structural_value_calls::StateCallInputs {
                    parameters: &plan.structural_parameters,
                    entry_claims: &plan.entry_claims,
                    operations: &plan.operations,
                    trivial_locals: &plan.trivial_affine_locals,
                },
            )?;
            trace.phase("single-state body: value call source custody");
            structural_value_calls::consume_caller_results(
                program,
                plan.machine,
                plan.state,
                &mut plan.operations,
                attempt_calls.state_schedules(
                    cleanup::owning_cleanup_machine(program, plan.state),
                    plan.state,
                ),
            )?;
            Some(if has_projected_parameter_moves {
                PlannedBody::PartialAffine(CheckedPartialAffineUnitCleanupMachinePlan {
                    machine: plan,
                    residual_affine_discards,
                })
            } else {
                PlannedBody::Sequence(plan)
            })
        })(),
        BodyRoute::Graph => state_graph::build_traced(
            program,
            facts,
            retained_records,
            scalar_callees,
            shapes,
            &mut attempt_calls,
            machine,
            call_frames,
            &trace,
        )
        .map(PlannedBody::Graph),
    };
    if let Some(body) = planned {
        return Ok((body, attempt_calls));
    }
    let mut gap = trace.gap(program, machine.symbol);
    if let Some(opaque) = shapes.take_unselected_opaque() {
        gap.reason = GapReason::UnselectedOpaqueRepresentation(opaque);
    }
    Err(gap)
}

/*
    The one plan route of a checked body, chosen by its authored shape before
    any builder runs. A builder that cannot plan its route's body reports the
    gap; the other route is never tried, so no body has two descriptions.
*/
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyRoute {
    /*
        One state whose result the graph cannot describe, or one without
        transitions, state contracts or nominal-cleanup owners whose result is
        Unit or a scalar: a statement sequence that completes or returns at its
        end.
    */
    Sequence,
    /*
        Everything else: the general graph of states, transitions, normal-exit
        cleanup and successor transfers.
    */
    Graph,
}

fn body_route(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> BodyRoute {
    if machine.supply_mode != MachineSupplyMode::CheckedBody {
        return BodyRoute::Sequence;
    }
    let [state] = program.machine_states(machine) else {
        return BodyRoute::Graph;
    };
    if types::declared_result_plan(program, &mut ShapeCollector::new(program), machine).is_none() {
        return BodyRoute::Sequence;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let owes_nominal_drop = |type_reference| {
        !types::is_reference(program, type_reference)
            && crate::checks::type_multiplicity(program, type_reference) != Multiplicity::Linear
            && types::type_graph_requires_nominal_drop(
                program,
                type_reference,
                &[],
                &mut std::collections::BTreeSet::new(),
            )
    };
    if statements.iter().any(|statement| match statement {
        StatementNode::Transition(_) => true,
        StatementNode::LocalData(local) => owes_nominal_drop(local.type_reference),
        _ => false,
    }) || program
        .state_parameters(state)
        .iter()
        .any(|parameter| owes_nominal_drop(parameter.type_reference))
        || !program.state_contracts(state).is_empty()
    {
        return BodyRoute::Graph;
    }
    if !state.return_type.is_valid()
        || types::is_unit(program, state.return_type)
        || program
            .primitive_type_reference(state.return_type)
            .is_some()
    {
        BodyRoute::Sequence
    } else {
        BodyRoute::Graph
    }
}

/*
    Retain direct nominal caller prerequisites for admitted declarations.

    The executable body and its source-selected normal-exit transactions have
    already been published. This metadata catalogue supplies signature
    requirements and entry identity; it cannot replace or reconstruct that body.
*/
pub(crate) fn build_checked_nominal_affine_unit_cleanup_plans(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    unit_effects: &EffectPlans,
    diagnostics: &mut Vec<Diagnostic>,
) -> (
    CheckedNominalAffineUnitCleanupPlans,
    Vec<CheckedUnitStructuralTypePlan>,
) {
    let mut shapes = ShapeCollector::new(program);
    let targets = unit_effects
        .machines
        .iter()
        .map(CheckedUnitEffectMachinePlan::callable_entry)
        .chain(
            unit_effects
                .composed_machines
                .iter()
                .filter_map(CheckedComposedUnitControlMachinePlan::callable_entry),
        )
        .collect::<Vec<_>>();
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            let plan = build_nominal_affine_unit_cleanup_machine(
                program,
                facts,
                &targets,
                &mut shapes,
                machine,
                diagnostics,
            )?;
            Some(plan)
        })
        .collect::<Vec<_>>();
    (
        CheckedNominalAffineUnitCleanupPlans { machines },
        shapes.types.into_values().collect(),
    )
}

/* Shape and signature admission shared by the plan builders above. */

/*
    A computed structural operand owns its storage vocabulary independently of
    source parameters. Retain both its canonical backing and its exact stored
    carrier: inline aggregate fields can materialize a descriptor even when the
    field's geometry names no standalone type. Access and evaluation remain with
    the operation plan.
*/
pub(super) fn scalar_operand_shapes(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<Vec<CheckedUnitStructuralTypePlan>> {
    let mut shapes = ShapeCollector::new(program);
    shapes.add_type(reference, &[], &[])?;
    shapes.add_stored_type(reference, &[], &[])?;
    Some(shapes.types.into_values().collect())
}

/*
    Scalar and Unit value construction share the exact formation dependency.
    The source identity names the borrowed leaf; a computed containing value
    retains its own storage separately through `scalar_operand_shapes`.
*/
pub(super) fn reference_backing_shapes(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    source_identity: typed_trees::type_identity::TypeIdentityId,
) -> Option<Vec<CheckedUnitStructuralTypePlan>> {
    let mut shapes = ShapeCollector::new(program);
    shapes.add_reference_backing(reference, source_identity, &[])?;
    Some(shapes.types.into_values().collect())
}

/*
    Construction and structural results share the same closed sum admission.
    The namespace includes real value types even when no parameter names them.
*/
pub(super) fn scalar_case_value_shapes(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<Vec<CheckedUnitStructuralTypePlan>> {
    let mut shapes = ShapeCollector::new(program);
    if let checked_trees::CheckedControlResultPlan::Scalar { .. } =
        types::declared_result_type(program, &mut shapes, reference)?
    {
        return None;
    }
    Some(shapes.types.into_values().collect())
}

/*
    Register the declarations of retained value producers and their operands.
    Projection leaves and their evaluated owners survive catalog pruning
    independently; retaining a shape grants no construction or cleanup custody.
*/
fn retain_value_types(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut types::ShapeCollector<'_>,
    mut pending: Vec<checked_trees::CheckedStructuralValueHandle>,
) -> Vec<typed_trees::type_identity::TypeIdentityId> {
    let plans = &facts.values.structural_values;
    let mut identities = Vec::new();
    let mut visited = Vec::new();
    while let Some(node) = pending.pop() {
        if visited.contains(&node) {
            continue;
        }
        visited.push(node);
        let node = plans.nodes.get(node);
        let binders = program
            .machine_by_symbol(node.machine)
            .map(|machine| types::machine_binders(program, machine))
            .unwrap_or_default();
        if node.type_reference.is_valid()
            && let Some(identity) = shapes.add_stored_type(node.type_reference, &binders, &[])
        {
            identities.push(identity);
        }
        match &node.kind {
            checked_trees::CheckedStructuralValueKind::Place(source) => {
                identities.push(source.type_identity);
            }
            checked_trees::CheckedStructuralValueKind::CopiedStructuralPlace { source, .. }
            | checked_trees::CheckedStructuralValueKind::BorrowedSliceView { source, .. }
            | checked_trees::CheckedStructuralValueKind::MutableSliceView { source, .. } => {
                identities.push(source.type_identity);
            }
            checked_trees::CheckedStructuralValueKind::Reference {
                source,
                source_value,
                source_contract,
                ..
            } => {
                identities.push(source.type_identity);
                shapes.retain_reference_contract_types(source_contract, &mut identities);
                if source_value.is_valid() {
                    pending.push(*source_value);
                }
            }
            checked_trees::CheckedStructuralValueKind::Projection { source, .. }
            | checked_trees::CheckedStructuralValueKind::ViewSubslice { source, .. }
            | checked_trees::CheckedStructuralValueKind::QuotientAdaptation { source, .. } => {
                pending.push(*source);
            }
            checked_trees::CheckedStructuralValueKind::QuotientCall { arguments, .. } => {
                pending.extend(arguments.iter().copied());
            }
            checked_trees::CheckedStructuralValueKind::Record { fields, .. }
            | checked_trees::CheckedStructuralValueKind::StructuralCase { fields, .. } => {
                for field in plans.record_fields.span_or_empty(*fields) {
                    if let checked_trees::CheckedStructuralRecordFieldValue::Structural(nested) =
                        field.value
                    {
                        pending.push(nested);
                    }
                }
            }
            checked_trees::CheckedStructuralValueKind::FixedArray { elements } => {
                pending.extend(elements.iter().copied());
            }
            checked_trees::CheckedStructuralValueKind::Dispatch { arms, .. } => {
                pending.extend(
                    plans
                        .dispatch_arms
                        .span_or_empty(*arms)
                        .iter()
                        .map(|arm| arm.value),
                );
            }
            _ => {}
        }
    }
    identities
}
