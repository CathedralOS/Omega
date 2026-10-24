//! Normal-call transfer of reference slots. A type supplies the slot inventory;
//! the complete body supplies its possible exit contents. Identity rows mean
//! incoming contents survive that route, not that the carrier is its backing.
//! Unknown new contents, local backing escapes and recursive summaries fail
//! closed. Rows retain each slot's declared access alongside provenance;
//! checked stores still own permission and predicate authority.
//!
//! Slice inventories retain incoming defaults plus exact slot replacements.
//! Normal-exit summaries refine those defaults across all reachable exits before
//! joining their origins. A branch which never writes therefore retains its
//! incoming alternative; an untouched sibling keeps its original descriptor.
//! Type-default selectors are not executable indices. A runtime-selected store
//! publishes one possible destination region, retaining old contents, rather
//! than exporting its expression identity or duplicating the transfer per slot.

mod captured_inputs;
mod case_generations;
pub(super) use case_generations::CaseStorageEffect;
mod expression_effects;
mod invocation_contract;
mod opaque_outputs;
pub use invocation_contract::ReferenceSlotContract;
pub(super) use invocation_contract::invocation_contract;
pub(super) mod retention;
pub(super) use expression_effects::call_contexts;
pub(super) use expression_effects::{
    StatementSlotTransfers, effects_at_expression, statement_effects,
};

use super::caller_aliases::{LocalWriteOrigin, LocalWriteSource};
use super::place_paths::{FramePathPrecision, FramePlaceOrigin, FrameSourcePlace};
use super::state_write_walk::{CollectedStatementPrefix, stored_reference_transfer};
use super::stored_origins::{self, StoredLocalOrigins, StoredWriteOrigin};
use super::{FrameInference, Machine, SymbolHandle, TableCall, TopLevelSymbols};
use facts::PlaceSegment;
use typed_trees::expression::ExpressionHandle;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

/// One exact destination leaf after an ordinary call. All sources must remain
/// live while this leaf is used. Incoming means the call may leave its prior
/// contents intact; an empty new-source set is not permission to drop them.
#[derive(Clone)]
pub struct StoredReferenceEffect {
    /// Static alternatives carry no local loan but remain value alternatives.
    pub has_static_values: bool,
    pub destination: LocalWriteOrigin,
    pub sources: Vec<StoredReferenceSource>,
    pub retains_incoming: bool,
    /// The complete private relation has only this slot's incoming value.
    /// Established before projecting away static sources: an empty public
    /// source roster alone cannot distinguish identity from lost geometry.
    pub incoming_identity: bool,
    /// A runtime-selected destination may leave other physical slots intact.
    /// Unlike a conditional write to one exact slot, it cannot replace the
    /// destination's complete current descriptor occurrence.
    pub may_select: bool,
    pub access: language_semantics::ReferenceAccess,
}

/// Construction graph of exact normal-call descriptor contexts. Each node
/// depends only on preceding nodes, so alternatives merge once without
/// enumerating combinations. Indices identify query-local snapshots, never
/// storage or permission. The consumer supplies the authentic entry roster.
#[derive(Clone)]
pub enum StoredReferenceCallContext {
    Entry,
    Invocation {
        entry: usize,
        /// Zero is an ordinary statement call, never a transition target.
        expression: ExpressionHandle,
        /// Unknown transfer stays unavailable, including through later joins.
        effects: Option<Vec<StoredReferenceEffect>>,
    },
    Join {
        alternatives: Vec<usize>,
    },
}

/// The actual descriptor home and its backing are distinct coordinates.
/// Both are required: backing alone cannot authorize an exclusive transfer.
#[derive(Clone)]
pub struct StoredReferenceSource {
    /// Keeps this source alive without identifying the output's referent.
    pub lifetime_access: Option<language_semantics::ReferenceAccess>,
    pub carrier_symbol: SymbolHandle,
    pub carrier_segments: Vec<PlaceSegment>,
    pub backing: LocalWriteSource,
}

#[derive(Clone)]
pub(super) struct SlotEffect {
    root: SymbolHandle,
    segments: Vec<PlaceSegment>,
    /// None retains unknown contents; Some([]) is not an unknown write.
    sources: Option<Vec<SlotSource>>,
    incoming: bool,
    may_select: bool,
    kind: stored_origins::StoredOriginKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SlotSource {
    pub(super) origin: FramePlaceOrigin,
    pub(super) carrier: FrameSourcePlace,
    pub(super) lifetime_access: Option<language_semantics::ReferenceAccess>,
}

#[derive(Clone)]
struct SlotInventory {
    origin: StoredWriteOrigin,
    access: language_semantics::ReferenceAccess,
    /// Each crossed pointer still denotes its incoming descriptor occurrence.
    /// A final spelling cannot recover a destination captured before rebinding.
    ancestors: Vec<Vec<PlaceSegment>>,
}

/// One value and its evaluated operands. For Calls, context is the call-entry
/// inventory after all actuals and before its own invocation effects, so the
/// checked result relation can complete relative dereferences at that boundary.
/// Non-call contexts are their ordinary expression-completion inventories.
/// Child records retain their own earlier descriptor observations; the
/// enclosing evaluator independently retains the post-call ambient inventory.
/// These are query construction records, not runtime values or loan authority.
#[derive(Clone)]
struct SlotArgumentCapture {
    expression: ExpressionHandle,
    context: CollectedStatementPrefix,
    inference: FrameInference,
    operands: Vec<SlotArgumentCapture>,
}

impl SlotArgumentCapture {
    fn at(
        expression: ExpressionHandle,
        context: &CollectedStatementPrefix,
        inference: &FrameInference,
        operands: Vec<SlotArgumentCapture>,
    ) -> Self {
        Self {
            expression,
            context: context.clone(),
            inference: inference.clone(),
            operands,
        }
    }

    /// One evaluated occurrence is required. Shared syntax alone cannot select
    /// between observations on different alternatives or at different epochs.
    fn observation(&self, expression: ExpressionHandle) -> Option<&Self> {
        let mut pending = vec![self];
        let mut found = None;
        while let Some(captured) = pending.pop() {
            if captured.expression == expression {
                if found.is_some() {
                    return None;
                }
                found = Some(captured);
            }
            pending.extend(captured.operands.iter());
        }
        found
    }
}

/// A failed invocation remains an obligation even if a later call overwrites
/// every affected slot. Declaration dependence is recorded separately.
#[derive(Clone)]
pub struct StoredReferenceCallFailure {
    pub target: SymbolHandle,
    pub requires_relation: bool,
}

#[derive(Clone, Default)]
pub(super) struct SlotRelationTransfer {
    pub effects: Vec<SlotEffect>,
    /// Old constructor/arrival case epochs retired by completed invocation
    /// writes. These install possible cases, never selected-tag evidence.
    pub case_effects: Vec<CaseStorageEffect>,
    pub template_dependent: bool,
    pub incomplete_calls: Vec<StoredReferenceCallFailure>,
}

impl SlotRelationTransfer {
    pub fn complete(effects: Vec<SlotEffect>) -> Self {
        Self {
            effects,
            ..Self::default()
        }
    }

    pub fn is_complete(&self) -> bool {
        !self.template_dependent && self.incomplete_calls.is_empty()
    }

    /// Unknown reach cannot preserve any possibly aliased incoming origin.
    /// Preserve typed opaque regions as opaque, never invent reference access.
    pub fn unknown(
        context: &CollectedStatementPrefix,
        target: SymbolHandle,
        template_dependent: bool,
        requires_relation: bool,
    ) -> Self {
        let mut effects = Vec::new();
        for record in &context.stored {
            for leaf in &record.references {
                if effects.iter().any(|prior: &SlotEffect| {
                    prior.root == record.local_symbol
                        && prior.segments == leaf.local_segments
                        && prior.kind == leaf.kind
                        && prior.may_select == leaf.may_select
                }) {
                    continue;
                }
                effects.push(SlotEffect {
                    root: record.local_symbol,
                    segments: leaf.local_segments.clone(),
                    sources: None,
                    incoming: false,
                    may_select: leaf.may_select,
                    kind: leaf.kind,
                });
            }
        }
        Self {
            effects,
            case_effects: Vec::new(),
            template_dependent,
            incomplete_calls: if template_dependent {
                Vec::new()
            } else {
                vec![StoredReferenceCallFailure {
                    target,
                    requires_relation,
                }]
            },
        }
    }
}

struct DestinationInventory {
    slots: Vec<SlotInventory>,
    template_dependent: bool,
}

pub(super) fn public_effects(effects: Vec<SlotEffect>) -> Option<Vec<StoredReferenceEffect>> {
    effects
        .into_iter()
        .map(|effect| {
            let stored_origins::StoredOriginKind::Reference(access) = effect.kind else {
                return None;
            };
            let sources = effect.sources?;
            let has_static_values = sources
                .iter()
                .any(|source| source.origin.source.is_static() && source.lifetime_access.is_none());
            let incoming_identity = effect.incoming && sources.is_empty();
            Some(StoredReferenceEffect {
                has_static_values,
                destination: LocalWriteOrigin {
                    local_symbol: effect.root,
                    local_segments: effect.segments,
                    source: None,
                },
                sources: sources
                    .into_iter()
                    .filter(|source| !source.origin.source.is_static())
                    .map(|source| StoredReferenceSource {
                        lifetime_access: source.lifetime_access,
                        carrier_symbol: source.carrier.root,
                        carrier_segments: source.carrier.segments,
                        backing: LocalWriteSource::from(source.origin),
                    })
                    .collect(),
                retains_incoming: effect.incoming,
                incoming_identity,
                may_select: effect.may_select,
                access,
            })
        })
        .collect()
}

fn call_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    caller_state: &State,
    call: &TableCall,
    captures: &[SlotArgumentCapture],
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<SlotRelationTransfer> {
    invocation_effects(
        program,
        machine,
        caller_state,
        call.target_symbol,
        ExpressionHandle::invalid(),
        super::caller_aliases::CallerWriteSite::Call(call),
        program.statement_table.expression_handles(call.arguments),
        captures,
        call.machine_arguments.is_empty(),
        context,
        symbols,
        inference,
    )
}

fn invocation_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    caller_state: &State,
    target: SymbolHandle,
    call_expression: ExpressionHandle,
    call_site: super::caller_aliases::CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    captures: &[SlotArgumentCapture],
    machine_arguments_empty: bool,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<SlotRelationTransfer> {
    let mut transfer = invocation_reference_effects(
        program,
        machine,
        caller_state,
        target,
        call_expression,
        call_site,
        arguments,
        captures,
        machine_arguments_empty,
        context,
        symbols,
        inference,
    )?;
    if transfer.is_complete() {
        transfer.case_effects = case_generations::completed_effects(
            program,
            machine,
            caller_state,
            target,
            captures,
            context,
            symbols,
            inference,
        );
    }
    Some(transfer)
}

fn invocation_reference_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    caller_state: &State,
    target: SymbolHandle,
    call_expression: ExpressionHandle,
    call_site: super::caller_aliases::CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    captures: &[SlotArgumentCapture],
    machine_arguments_empty: bool,
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<SlotRelationTransfer> {
    let trace_refusal = |phase: &str| {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!(
                "CHECKSITE stored-reference-invocation-refusal machine={:?} state={:?} target={:?} expression={:?} phase={} argument_count={} capture_count={} divergent_count={} machine_arguments_empty={}",
                machine.symbol,
                caller_state.symbol,
                target,
                call_expression,
                phase,
                arguments.len(),
                captures.len(),
                context.divergent.len(),
                machine_arguments_empty,
            );
        }
    };
    if arguments.len() != captures.len()
        || arguments
            .iter()
            .zip(captures)
            .any(|(expression, capture)| *expression != capture.expression)
    {
        trace_refusal("argument-capture-correspondence");
        return None;
    }
    // The occurrence's applied logical contract owns caller-visible access.
    // A private body's open telescope or availability cannot turn a known
    // empty writable slot frontier into an unknown descriptor mutation.
    let parameters = match call_site {
        super::caller_aliases::CallerWriteSite::Call(call)
            if !call_expression.is_valid() && call.target_symbol == target =>
        {
            crate::CallSite::Statement(call).parameters(program, machine.symbol)?
        }
        super::caller_aliases::CallerWriteSite::Expression(expression)
            if expression == call_expression =>
        {
            let call = program.expression_table.call_node(expression)?;
            if call.target_symbol != target {
                return None;
            }
            crate::CallSite::Expression { expression, call }.parameters(program, machine.symbol)?
        }
        _ => return None,
    };
    if parameters.len() != captures.len() {
        trace_refusal("parameter-capture-correspondence");
        return None;
    }
    let inventory = parameter_destination_inventory(program, parameters)?;
    // Captured machine arguments are not state operands. The state-only
    // inventory cannot classify those captures; their existing relation owner
    // must still succeed below before this invocation can be complete.
    if machine_arguments_empty && !inventory.template_dependent && inventory.slots.is_empty() {
        // This proves only that no argument can replace caller reference
        // slots. Returned descriptor identity, loan formation, qualifications,
        // scalar/content writes and normal-call permission have other owners.
        return Some(SlotRelationTransfer::complete(Vec::new()));
    }
    let Some((callee, state)) = super::machine_state_by_symbol(program, target) else {
        if let Some((definition, _)) = crate::selected_requirement_signature(program, target)
            && definition.is_boundary
        {
            return opaque_outputs::effects(
                program,
                machine,
                parameters,
                captures,
                context,
                symbols,
                call_site,
                target,
                call_expression,
            );
        }
        // A missing body cannot summarize a nonempty or symbolic frontier.
        return None;
    };
    let body_parameters = program.state_parameters(state);
    if body_parameters.len() != arguments.len()
        || !machine_arguments_empty
        || !program.machine_type_parameters(callee).is_empty()
    {
        trace_refusal("checked-callee-application");
        return None;
    }
    if matches!(
        callee.supply_mode,
        language_semantics::MachineSupplyMode::Boundary
            | language_semantics::MachineSupplyMode::AdmissionClaim
            | language_semantics::MachineSupplyMode::ExternalRealization { .. }
    ) {
        return opaque_outputs::effects(
            program,
            machine,
            parameters,
            captures,
            context,
            symbols,
            call_site,
            target,
            call_expression,
        );
    }
    let parameters = body_parameters;
    if !callee.body_is_present || !callee.supply_mode.is_checked_body() {
        trace_refusal("checked-body-unavailable");
        return None;
    }
    let Some(destinations_empty) =
        destination_inventory(program, state).map(|destinations| destinations.is_empty())
    else {
        trace_refusal("destination-inventory");
        return None;
    };
    if destinations_empty {
        return Some(SlotRelationTransfer::complete(Vec::new()));
    }
    let Some(mut relative) = state_effects(program, callee, state, symbols, inference) else {
        trace_refusal("relative-state-effects");
        return None;
    };
    rebase_output_occurrences(
        program,
        machine,
        caller_state,
        call_site,
        target,
        call_expression,
        parameters,
        &mut relative,
    )?;
    instantiate(
        program, machine, parameters, captures, &relative, context, symbols,
    )
    .map(|instantiated| SlotRelationTransfer::complete(instantiated.effects))
}

fn destination_inventory(
    program: &dyn crate::CheckingInput,
    state: &State,
) -> Option<Vec<SlotInventory>> {
    let inventory = parameter_destination_inventory(program, program.state_parameters(state))?;
    (!inventory.template_dependent).then_some(inventory.slots)
}

/// Form the output namespace from an authenticated invocation occurrence.
/// This carries no spatial relation or permission by itself.
fn output_identity(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    site: super::caller_aliases::CallerWriteSite<'_>,
    target: SymbolHandle,
    expression: ExpressionHandle,
    parameter: &StateParameter,
) -> Option<facts::InvocationOutputIdentity> {
    let (state, _, statement_index) =
        super::caller_aliases::caller_statement_owner(program, machine, site)?;
    let mut sites =
        crate::collect_call_sites_in_statement(program, machine, state, statement_index)
            .into_iter()
            .enumerate()
            .filter(|(_, occurrence)| match (site, occurrence) {
                (
                    super::caller_aliases::CallerWriteSite::Call(expected),
                    crate::CallSite::Statement(actual),
                ) => std::ptr::eq(expected, *actual),
                (
                    super::caller_aliases::CallerWriteSite::Expression(expected),
                    crate::CallSite::Expression { expression, .. },
                ) => expected == *expression,
                _ => false,
            });
    let (call_ordinal, occurrence) = sites.next().filter(|_| sites.next().is_none())?;
    if occurrence.target_symbol(program) != target
        || match occurrence {
            crate::CallSite::Statement(_) => expression.is_valid(),
            crate::CallSite::Expression {
                expression: actual, ..
            } => actual != expression,
            crate::CallSite::TransitionNamed { .. } => true,
        }
    {
        return None;
    }
    Some(facts::InvocationOutputIdentity {
        state_symbol: state.symbol,
        statement_index,
        call_ordinal,
        parameter_symbol: parameter.symbol,
    })
}

#[allow(clippy::too_many_arguments)]
fn rebase_output_occurrences(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    site: super::caller_aliases::CallerWriteSite<'_>,
    target: SymbolHandle,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    effects: &mut [SlotEffect],
) -> Option<()> {
    for effect in effects {
        for source in effect.sources.as_mut()? {
            if source.origin.source.output_identity().is_none() {
                continue;
            }
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == effect.root)?;
            let identity = output_identity(program, machine, site, target, expression, parameter)?;
            if identity.state_symbol != state.symbol {
                return None;
            }
            // A checked body supplies an opaque output on this normal route.
            // Caller instantiation owns the new descriptor occurrence; it may
            // not reuse an internal callee expression as a caller value epoch.
            let place = FrameSourcePlace::invocation_output(identity, effect.segments.clone());
            source.origin = FramePlaceOrigin {
                path: "invocation output".to_owned(),
                precision: FramePathPrecision::Exact,
                source: place.clone(),
            };
            source.carrier = place;
        }
    }
    Some(())
}

/// Opaque calls with possible reference outputs require a complete call-site
/// relation even when failed attribution produced no loans. This is an
/// obligation query, not provider admission and not an empty-summary fallback.
pub(super) fn provider_output_relation_required(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
    target: symbols::SymbolHandle,
    call_expression: ExpressionHandle,
) -> bool {
    let parameters = if let Some((definition, signature)) =
        crate::selected_requirement_signature(program, target)
        && definition.is_boundary
    {
        let signature = if call_expression.is_valid() {
            let Some(call) = program.expression_table.call_node(call_expression) else {
                return true;
            };
            if call.target_symbol != target {
                return true;
            }
            let Some(signature) = program.requirement_call_signature(machine, call) else {
                return true;
            };
            signature
        } else {
            signature
        };
        program.state_signature_parameters(signature)
    } else if let Some(state) = crate::call_target_state(program, target)
        && let Some(machine) = program.machine_holding_state(state.symbol)
        && matches!(
            machine.supply_mode,
            language_semantics::MachineSupplyMode::Boundary
                | language_semantics::MachineSupplyMode::AdmissionClaim
                | language_semantics::MachineSupplyMode::ExternalRealization { .. }
        )
    {
        program.state_parameters(state)
    } else {
        return false;
    };
    // A symbolic-only frontier supplies no concrete destination relation yet.
    // Its transfer remains incomplete and invalidates possible origins; it
    // never becomes an empty preserving effect. Known slots still require a
    // relation beside symbolic leaves, and an uncomputable inventory fails
    // closed. Exact applied parameters above prevent concrete calls from
    // borrowing this deferral from an open declaration.
    parameter_destination_inventory(program, parameters)
        .is_none_or(|inventory| !inventory.slots.is_empty())
}

fn parameter_destination_inventory(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
) -> Option<DestinationInventory> {
    let mut leaves = Vec::new();
    let mut template_dependent = false;
    for parameter in parameters {
        let shell = crate::reference_shell(program, parameter.type_reference);
        if shell.is_some_and(|shell| !shell.access.is_exclusive()) {
            continue;
        }
        let reference = shell.map_or(parameter.type_reference, |shell| shell.referee);
        let mut pending = vec![(
            reference,
            Vec::new(),
            shell.is_some(),
            shell.is_none_or(|shell| shell.access.is_readable()),
            Vec::new(),
            Vec::<Vec<PlaceSegment>>::new(),
        )];
        while let Some((reference, prefix, borrowed, readable, mut visited, ancestors)) =
            pending.pop()
        {
            // A recursive pointee needs demand-bounded captured destinations;
            // do not turn an incomplete reachable inventory into an empty one.
            if visited.contains(&reference) {
                return None;
            }
            visited.push(reference);
            let inventory = stored_origins::declared_origins_for_query(
                program,
                parameter.symbol,
                parameter.name.as_str(),
                reference,
                true,
            )?;
            for mut leaf in inventory.references {
                let leaf_type = stored_origins::projected_storage_type(
                    program,
                    reference,
                    &leaf.local_segments,
                )?;
                let Some(shell) = crate::reference_shell(program, leaf_type) else {
                    let leaf_type = program
                        .type_reference_table
                        .unconstrained_type_reference(leaf_type);
                    if matches!(program.type_reference_table.type_reference(leaf_type),
                        typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                        if program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter)
                    {
                        template_dependent = true;
                        continue;
                    }
                    let (definition, _) = super::type_instantiation::DeclarationScopes::default()
                        .definition(program, leaf_type, 0)?;
                    if definition.supply_mode != language_semantics::DataSupplyMode::BoundaryOpaque
                    {
                        return None;
                    }
                    continue;
                };
                let mut segments = prefix.clone();
                segments.extend_from_slice(&leaf.local_segments);
                leaf.local_segments = segments.clone();
                leaf.local_path = super::place_paths::coordinate_storage_path(
                    program,
                    parameter.name.as_str(),
                    &segments,
                )
                .0;
                if borrowed {
                    leaves.push(SlotInventory {
                        origin: leaf,
                        access: shell.access,
                        ancestors: ancestors.clone(),
                    });
                }
                // A pointer under write-only storage cannot be loaded to
                // reach another destination. An owned/readable carrier can
                // lend the storage behind its exclusive pointer fields.
                if readable && shell.access.is_exclusive() {
                    let mut child_ancestors = ancestors.clone();
                    child_ancestors.push(segments.clone());
                    pending.push((
                        shell.referee,
                        segments,
                        true,
                        shell.access.is_readable(),
                        visited.clone(),
                        child_ancestors,
                    ));
                }
            }
        }
    }
    Some(DestinationInventory {
        slots: leaves,
        template_dependent,
    })
}

/// Both source-call effects and portable boundary publication require the
/// complete writable frontier. A value containing an exclusive pointer can
/// expose output storage even without an outer reference parameter.
pub(crate) fn provider_completion_covers_writable_slots(
    program: &dyn crate::CheckingInput,
    parameters: &[StateParameter],
    outputs: &[crate::borrow_contracts::ProviderOutput],
) -> Option<crate::borrow_contracts::BorrowRelationReadiness> {
    let inventory = parameter_destination_inventory(program, parameters)?;
    if !inventory.slots.iter().all(|slot| {
        outputs.iter().any(|output| {
            let crate::borrow_contracts::ProviderOutputDestination::Parameter(position) =
                output.destination;
            parameters
                .get(position)
                .is_some_and(|parameter| parameter.symbol == slot.origin.local_symbol)
                && opaque_outputs::owner_segments(&output.owner_path).as_ref()
                    == Some(&slot.origin.local_segments)
                && output.access == slot.access
        })
    }) {
        return None;
    }
    Some(if inventory.template_dependent {
        crate::borrow_contracts::BorrowRelationReadiness::TemplateDependent
    } else {
        crate::borrow_contracts::BorrowRelationReadiness::Complete
    })
}

/// Seed the incoming pointee slots before the ordinary transfer walk. Missing
/// rows after that walk are unknown, never evidence that a slot was untouched.
/// Unsupported reachable inventories add no knowledge; any demanded summary
/// still fails in `destination_inventory`/`collect_exit`.
pub(super) fn seed_incoming_slots(
    program: &dyn crate::CheckingInput,
    state: &State,
    stored: &mut [StoredLocalOrigins],
) {
    let Some(inventory) = destination_inventory(program, state) else {
        return;
    };
    for declaration in inventory {
        if declaration.ancestors.is_empty() {
            continue;
        }
        let mut slot = declaration.origin;
        let Some(record) = stored
            .iter_mut()
            .find(|record| record.local_symbol == slot.local_symbol)
        else {
            continue;
        };
        if record
            .references
            .iter()
            .any(|leaf| leaf.local_segments == slot.local_segments)
        {
            continue;
        }
        slot.origin = Some(FramePlaceOrigin {
            path: slot.local_path.clone(),
            precision: FramePathPrecision::Exact,
            source: FrameSourcePlace {
                root: slot.local_symbol,
                segments: slot.local_segments.clone(),
                builtin_coordinates: true,
                ..Default::default()
            },
        });
        record.references.push(slot);
    }
}

pub(super) fn state_effects(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    if inference.active_states.contains(&entry.symbol) {
        return None;
    }
    inference.active_states.push(entry.symbol);
    let result = solve_state_effects(program, machine, entry, symbols, inference);
    inference.active_states.pop();
    result
}

enum SlotExit {
    Return(Vec<StoredLocalOrigins>),
    Edge {
        target: usize,
        arguments: Vec<SlotArgumentCapture>,
        prefix: CollectedStatementPrefix,
    },
}

struct SlotEquation<'program> {
    state: &'program State,
    inventory: Vec<SlotInventory>,
    exits: Vec<SlotExit>,
}

/// Least fixed point over normal exits. An unvisited return is not an identity
/// route: only a real normal exit can seed incoming contents. Root substitution
/// finiteness bounds the source set before iteration; there is no depth cutoff.
fn solve_state_effects<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    entry: &'program State,
    symbols: &TopLevelSymbols<'_>,
    inference: &mut FrameInference,
) -> Option<Vec<SlotEffect>> {
    let mut states = vec![entry];
    let mut equations = Vec::new();
    let mut substitutions = super::cycle_substitutions::Substitutions::default();
    let mut index = 0;
    while index < states.len() {
        let state = states[index];
        let inventory = destination_inventory(program, state)?;
        let (last, prefixes) =
            stored_reference_transfer(program, machine, state, symbols, inference)?;
        let mut exits = Vec::new();
        let mut has_transition = false;
        for (ordinal, statement) in program
            .statement_table
            .indexed_statements(state.statement_nodes)
        {
            let Some(transition) = statement.as_transition() else {
                continue;
            };
            has_transition = true;
            if transition.exit != TransitionExit::Ordinary {
                continue;
            }
            let prefix = prefixes.get(ordinal)?.as_ref()?;
            let completed = expression_effects::completed_targets(
                program, machine, state, statement, prefix, symbols, inference,
            )?;
            for edge in transition.targets() {
                let completed = completed
                    .iter()
                    .find(|completed| completed.target == edge)?;
                match program.statement_table.transition_target(edge) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        if super::terminal_calls::target(program, machine, edge).is_valid() {
                            return None;
                        }
                        let target = super::transition_topology::named_transition_target_state(
                            program, machine, state, edge,
                        )?;
                        let target_index = if let Some(index) = states
                            .iter()
                            .position(|candidate| candidate.symbol == target.symbol)
                        {
                            index
                        } else {
                            states.push(target);
                            states.len() - 1
                        };
                        let arguments = program.statement_table.expression_handles(*arguments);
                        let parameters = program.state_parameters(target);
                        if parameters.len() != arguments.len() {
                            return None;
                        }
                        if completed.arguments.len() != arguments.len()
                            || completed
                                .arguments
                                .iter()
                                .zip(arguments)
                                .any(|(captured, expression)| captured.expression != *expression)
                        {
                            return None;
                        }
                        exits.push(SlotExit::Edge {
                            target: target_index,
                            arguments: completed.arguments.clone(),
                            prefix: completed.context.clone(),
                        });
                    }
                    TransitionTargetNode::Terminal | TransitionTargetNode::Value(_) => {
                        exits.push(SlotExit::Return(completed.context.stored.clone()))
                    }
                }
            }
        }
        if !has_transition {
            exits.push(SlotExit::Return(last.stored));
        }
        equations.push(SlotEquation {
            state,
            inventory,
            exits,
        });
        index += 1;
    }
    let mut known = vec![None::<Vec<SlotEffect>>; equations.len()];
    loop {
        let mut changed = false;
        for (index, equation) in equations.iter().enumerate() {
            let mut candidate = Vec::new();
            let mut returned = Vec::new();
            for exit in &equation.exits {
                let stored = match exit {
                    SlotExit::Return(stored) => stored.clone(),
                    SlotExit::Edge {
                        target,
                        arguments,
                        prefix,
                    } => {
                        let Some(relative) = &known[*target] else {
                            continue;
                        };
                        let instantiated = instantiate(
                            program,
                            machine,
                            program.state_parameters(equations[*target].state),
                            arguments,
                            relative,
                            prefix,
                            symbols,
                        )?;
                        retain_demanded_substitutions(
                            equation.state,
                            equations[*target].state,
                            &instantiated.substitutions,
                            &mut substitutions,
                        )?;
                        let mut stored = prefix.stored.clone();
                        apply(program, &mut stored, &instantiated.effects);
                        stored
                    }
                };
                returned.push(stored);
            }
            if returned.is_empty() {
                continue;
            }
            // A slice's type inventory is a default for every element. Exact
            // writes refine that inventory, and every exit must answer every
            // refined slot. Otherwise an unchanged branch would disappear
            // when another branch replaces one element.
            let mut inventory = equation.inventory.clone();
            for declaration in &equation.inventory {
                for stored in &returned {
                    let record = stored
                        .iter()
                        .find(|record| record.local_symbol == declaration.origin.local_symbol)?;
                    for leaf in &record.references {
                        if leaf.local_segments.len() != declaration.origin.local_segments.len()
                            || !(stored_origins::path_covers(
                                &declaration.origin.local_segments,
                                &leaf.local_segments,
                            ) || (leaf.may_select
                                && stored_origins::path_covers(
                                    &leaf.local_segments,
                                    &declaration.origin.local_segments,
                                )))
                            || inventory.iter().any(|existing| {
                                existing.origin.local_symbol == leaf.local_symbol
                                    && existing.origin.local_segments == leaf.local_segments
                                    && existing.origin.may_select == leaf.may_select
                            })
                        {
                            continue;
                        }
                        let mut refined = declaration.clone();
                        refined.origin.local_segments = leaf.local_segments.clone();
                        refined.origin.may_select = leaf.may_select;
                        for ancestor in &mut refined.ancestors {
                            for (segment, selected) in ancestor.iter_mut().zip(&leaf.local_segments)
                            {
                                if matches!(segment, PlaceSegment::Index { expression } if !expression.is_valid())
                                {
                                    *segment = *selected;
                                }
                            }
                        }
                        inventory.push(refined);
                    }
                }
            }
            for stored in &returned {
                collect_exit(program, equation.state, &inventory, stored, &mut candidate)?;
            }
            if known[index].is_none() {
                known[index] = Some(Vec::new());
                changed = true;
            }
            let accumulated = known[index].as_mut()?;
            for effect in candidate {
                if let Some(prior) = accumulated.iter_mut().find(|prior| {
                    prior.root == effect.root
                        && prior.segments == effect.segments
                        && prior.may_select == effect.may_select
                }) {
                    if prior.kind != effect.kind {
                        return None;
                    }
                    if effect.incoming && !prior.incoming {
                        prior.incoming = true;
                        changed = true;
                    }
                    let prior_sources = prior.sources.as_mut()?;
                    for source in effect.sources? {
                        if !prior_sources.contains(&source) {
                            prior_sources.push(source);
                            changed = true;
                        }
                    }
                } else {
                    accumulated.push(effect);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    known.into_iter().next()?
}

/// Only actual relation roots participate in substitution. A checked empty
/// relation demands no argument origin; an unknown relation never reaches here.
/// Ordered argument effects have already established each selected edge's slot
/// inventory. The same growing-cycle law still bounds every demanded root.
fn retain_demanded_substitutions(
    source: &State,
    target: &State,
    demanded: &[(SymbolHandle, FrameSourcePlace)],
    substitutions: &mut super::cycle_substitutions::Substitutions,
) -> Option<()> {
    for (parameter, actual) in demanded {
        if !actual.builtin_coordinates {
            return None;
        }
        if actual.is_static() {
            continue;
        }
        if !actual.root.is_valid() {
            return None;
        }
        let from = substitutions.parameter(target.symbol, *parameter);
        let to = substitutions.parameter(source.symbol, actual.root);
        substitutions.insert(
            from,
            to,
            !actual.segments.is_empty() || !actual.recasts.is_empty(),
        );
    }
    substitutions.is_finite().then_some(())
}

fn collect_exit(
    program: &dyn crate::CheckingInput,
    state: &State,
    inventory: &[SlotInventory],
    stored: &[StoredLocalOrigins],
    effects: &mut Vec<SlotEffect>,
) -> Option<()> {
    for declaration in inventory {
        let slot = &declaration.origin;
        // Runtime selector identity cannot escape its expression namespace.
        // A type-derived wildcard is different: it records incoming contents
        // for untouched elements, beside independently checked exact overlays.
        if slot
            .local_segments
            .iter()
            .any(|part| matches!(part, PlaceSegment::Index { expression } if expression.is_valid()))
        {
            return None;
        }
        let record = stored
            .iter()
            .find(|record| record.local_symbol == slot.local_symbol)?;
        for ancestor in &declaration.ancestors {
            let matching = stored_origins::slot_origins(record, ancestor)?;
            let origin = matching.first()?;
            if matching.len() != 1
                || origin.source.root != slot.local_symbol
                || &origin.source.segments != ancestor
                || !origin.source.builtin_coordinates
            {
                return None;
            }
        }
        let sources = stored_origins::selected_slot_sources(
            record,
            &slot.local_segments,
            Some(slot.may_select),
        );
        // A possible store on another exit need not occur on this one. Its
        // presence never kills an incoming alternative on either route.
        let sources = match sources {
            Some(sources) => sources,
            None if slot.may_select
                && !record
                    .references
                    .iter()
                    .any(|leaf| leaf.may_select && leaf.local_segments == slot.local_segments) =>
            {
                continue;
            }
            None => return None,
        };
        for supplied in sources {
            let source = &supplied.origin;
            if !source.source.builtin_coordinates {
                return None;
            }
            if !source.source.is_static() && source.source.output_identity().is_none() {
                if !source.source.root.is_valid() {
                    return None;
                }
                let parameter = program.state_parameter_by_symbol(state, source.source.root)?;
                // Owned input storage dies in the callee. Only a reference
                // boundary actually crossed by this source survives invocation.
                if !super::type_reference_is_reference(program, parameter.type_reference) {
                    let frontier = stored_origins::declared_origins_for_query(
                        program,
                        parameter.symbol,
                        parameter.name.as_str(),
                        parameter.type_reference,
                        true,
                    )?;
                    if !frontier.references.iter().any(|leaf| {
                        stored_origins::source_reaches_leaf(
                            &source.source.segments,
                            &leaf.local_segments,
                        )
                    }) {
                        return None;
                    }
                }
            }
            let incoming = !supplied.lifetime_access.is_some()
                && source.source.root == slot.local_symbol
                && source.source.segments == slot.local_segments;
            if !incoming
                && !slot.may_select
                && slot
                    .local_segments
                    .iter()
                    .any(|part| matches!(part, PlaceSegment::Index { .. }))
            {
                return None;
            }
            let effect = if let Some(index) = effects.iter().position(|effect| {
                effect.root == slot.local_symbol
                    && effect.segments == slot.local_segments
                    && effect.may_select == slot.may_select
            }) {
                &mut effects[index]
            } else {
                effects.push(SlotEffect {
                    root: slot.local_symbol,
                    segments: slot.local_segments.clone(),
                    sources: Some(Vec::new()),
                    incoming: slot.may_select,
                    may_select: slot.may_select,
                    kind: stored_origins::StoredOriginKind::Reference(declaration.access),
                });
                effects.last_mut()?
            };
            if effect.kind != stored_origins::StoredOriginKind::Reference(declaration.access) {
                return None;
            }
            effect.incoming |= incoming;
            let sources = effect.sources.as_mut()?;
            if !incoming && !sources.contains(&supplied) {
                sources.push(supplied);
            }
        }
    }
    Some(())
}

struct InstantiatedSlots {
    effects: Vec<SlotEffect>,
    /// Exact input prefixes discovered while instantiating demanded effects.
    /// Keeping them beside the result avoids querying an argument a second
    /// time or counting a callee-authored suffix as argument growth.
    substitutions: Vec<(SymbolHandle, FrameSourcePlace)>,
}

fn instantiate(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameters: &[StateParameter],
    arguments: &[SlotArgumentCapture],
    relative: &[SlotEffect],
    context: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<InstantiatedSlots> {
    if parameters.len() != arguments.len() || !context.divergent.is_empty() {
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            eprintln!(
                "CHECKSITE stored-reference-input-context-refusal machine={:?} phase={} parameter_count={} capture_count={} divergent_count={}",
                machine.symbol,
                if parameters.len() != arguments.len() {
                    "parameter-count"
                } else {
                    "divergent-entry"
                },
                parameters.len(),
                arguments.len(),
                context.divergent.len(),
            );
        }
        return None;
    }
    // Every demanded actual uses its own completed input inventory. The
    // evaluator has already retained all ordered argument effects in context;
    // applying those effects again here would retarget an earlier descriptor.
    let argument = |index: usize| {
        let captured = arguments.get(index)?;
        captured.context.divergent.is_empty().then_some(captured)
    };
    let substitute = |root, segments: &[PlaceSegment]| {
        let index = typed_trees::signature::state_parameter_position(parameters, root)?;
        let captured = argument(index)?;
        destination_source(
            program,
            machine,
            &parameters[index],
            captured,
            segments,
            context,
            symbols,
        )
    };
    let mut output = Vec::new();
    let mut substitutions = Vec::new();
    for effect in relative {
        // An identity row proves preservation; it has no store to instantiate.
        // In particular, a wildcard default is not an executable selector.
        if effect.incoming && effect.sources.as_ref()?.is_empty() {
            continue;
        }
        let Some((destination, input)) = substitute(effect.root, &effect.segments) else {
            trace_instantiation_refusal(
                machine,
                parameters,
                arguments,
                effect,
                None,
                "destination",
            );
            return None;
        };
        if !substitutions.contains(&(effect.root, input.clone())) {
            substitutions.push((effect.root, input));
        }
        let mut sources = Vec::new();
        for supplied in effect.sources.as_ref()? {
            let Some(observed_sources) = captured_inputs::instantiate_sources(
                program, machine, parameters, arguments, supplied, context, symbols,
            ) else {
                trace_instantiation_refusal(
                    machine,
                    parameters,
                    arguments,
                    effect,
                    Some(supplied),
                    "source",
                );
                return None;
            };
            for observed in observed_sources {
                for substitution in observed.substitutions {
                    if !substitutions.contains(&substitution) {
                        substitutions.push(substitution);
                    }
                }
                if !sources.contains(&observed.source) {
                    sources.push(observed.source);
                }
            }
        }
        output.push(SlotEffect {
            root: destination.root,
            segments: destination.segments,
            sources: Some(sources),
            incoming: effect.incoming,
            may_select: effect.may_select,
            kind: effect.kind,
        });
    }
    Some(InstantiatedSlots {
        effects: output,
        substitutions,
    })
}

/// A refused summary remains refused. Keep diagnostics at the substitution
/// owner so a missing destination cannot be mistaken for a source association
/// failure, without exporting complete body inventories or unbounded paths.
fn trace_instantiation_refusal(
    machine: &Machine,
    parameters: &[StateParameter],
    arguments: &[SlotArgumentCapture],
    effect: &SlotEffect,
    source: Option<&SlotSource>,
    reason: &str,
) {
    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_none() {
        return;
    }
    eprintln!(
        "CHECKSITE stored-reference-instantiation-refusal machine={:?} reason={} destination={:?}:path_count={}:prefix={:?} incoming={} may_select={} source_count={:?} input_count={} inputs_prefix={:?}",
        machine.symbol,
        reason,
        effect.root,
        effect.segments.len(),
        &effect.segments[..effect.segments.len().min(32)],
        effect.incoming,
        effect.may_select,
        effect.sources.as_ref().map(Vec::len),
        arguments.len(),
        parameters
            .iter()
            .zip(arguments)
            .take(8)
            .map(|(parameter, argument)| (parameter.symbol, argument.expression))
            .collect::<Vec<_>>(),
    );
    if let Some(source) = source {
        eprintln!(
            "CHECKSITE stored-reference-instantiation-source carrier={:?}:path_count={}:prefix={:?} backing={:?}:path_count={}:prefix={:?} precision={:?} lifetime_only={:?} recast_count={} representation={}",
            source.carrier.root,
            source.carrier.segments.len(),
            &source.carrier.segments[..source.carrier.segments.len().min(32)],
            source.origin.source.root,
            source.origin.source.segments.len(),
            &source.origin.source.segments[..source.origin.source.segments.len().min(32)],
            source.origin.precision,
            source.lifetime_access,
            source.origin.source.recasts.len(),
            source.origin.source.representation.is_some(),
        );
    }
}

/// Resolve each pointer boundary before appending the destination slot.
/// Only the selected descriptor is interpreted; the final slot's old contents
/// never become its own storage address.
#[allow(clippy::too_many_arguments)]
fn destination_source(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    captured: &SlotArgumentCapture,
    mut segments: &[PlaceSegment],
    entry: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<(FrameSourcePlace, FrameSourcePlace)> {
    if segments.iter().any(|segment| {
        !matches!(
            segment,
            PlaceSegment::Field { .. }
                | PlaceSegment::Case { .. }
                | PlaceSegment::FixedIndex { .. }
        ) && !matches!(segment, PlaceSegment::Index { expression } if !expression.is_valid())
    }) {
        return None;
    }
    let mut reference = parameter.type_reference;
    let mut base = None;
    let mut input = None;
    if let Some(shell) = crate::reference_shell(program, reference) {
        if !shell.access.is_exclusive() {
            return None;
        }
        let relative = FrameSourcePlace {
            root: parameter.symbol,
            builtin_coordinates: true,
            ..Default::default()
        };
        let mut sources = aggregate_argument_sources(
            program,
            machine,
            parameter,
            captured,
            &SlotSource {
                origin: FramePlaceOrigin {
                    path: parameter.name.to_string(),
                    precision: FramePathPrecision::Exact,
                    source: relative.clone(),
                },
                carrier: relative,
                lifetime_access: None,
            },
            entry,
            symbols,
        )?
        .into_iter();
        let source = sources.next()?;
        if source.lifetime_access.is_some() || sources.next().is_some() {
            return None;
        }
        input = Some(source.origin.source.clone());
        base = Some(source.origin);
        reference = shell.referee;
    }
    loop {
        let inventory = stored_origins::declared_origins_for_query(
            program,
            parameter.symbol,
            parameter.name.as_str(),
            reference,
            true,
        )?;
        let mut boundaries = inventory.references.iter().filter(|leaf| {
            leaf.local_segments.len() < segments.len() && segments.starts_with(&leaf.local_segments)
        });
        let Some(boundary) = boundaries.next() else {
            let leaf = stored_origins::projected_storage_type(program, reference, segments)?;
            crate::reference_shell(program, leaf)?;
            let base = base?;
            if !base.source.root.is_valid()
                || base.source.is_static()
                || !base.source.builtin_coordinates
            {
                return None;
            }
            return Some((base.source.append_segments(segments), input?));
        };
        if boundaries.next().is_some() {
            return None;
        }
        let shell = crate::reference_shell(
            program,
            stored_origins::projected_storage_type(program, reference, &boundary.local_segments)?,
        )?;
        if !shell.access.is_exclusive() {
            return None;
        }
        let candidates = if let Some(captured) = &base {
            let mut projected = captured.clone();
            projected.source = captured.source.append_segments(&boundary.local_segments);
            projected.path = path(program, &projected.source);
            stored_origins::captured_reference_projection_origins(
                program,
                captured,
                &projected,
                &entry.stored,
            )
            .into_iter()
            .collect::<Option<Vec<_>>>()?
        } else {
            let relative = FrameSourcePlace {
                root: parameter.symbol,
                segments: boundary.local_segments.clone(),
                builtin_coordinates: true,
                ..Default::default()
            };
            aggregate_argument_sources(
                program,
                machine,
                parameter,
                captured,
                &SlotSource {
                    lifetime_access: None,
                    origin: FramePlaceOrigin {
                        path: path(program, &relative),
                        precision: FramePathPrecision::Exact,
                        source: relative.clone(),
                    },
                    carrier: relative,
                },
                entry,
                symbols,
            )?
            .into_iter()
            .map(|source| source.origin)
            .collect()
        };
        let mut candidates = candidates.into_iter();
        let selected = candidates.next()?;
        if input.is_none() {
            input = Some(selected.source.clone());
        }
        base = Some(selected);
        if candidates.next().is_some() {
            return None;
        }
        reference = shell.referee;
        segments = &segments[boundary.local_segments.len()..];
    }
}

/// Returned source substitution uses each operand's completed observation.
/// Spatial backing and descriptor carrier stay associated through computed
/// results; no later ambient inventory can reinterpret a prior descriptor.
fn aggregate_argument_sources(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    parameter: &StateParameter,
    captured: &SlotArgumentCapture,
    supplied: &SlotSource,
    entry: &CollectedStatementPrefix,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<SlotSource>> {
    captured_inputs::sources(
        program, machine, parameter, captured, supplied, entry, symbols,
    )
}

fn path(program: &dyn crate::CheckingInput, source: &FrameSourcePlace) -> String {
    super::place_paths::coordinate_storage_path(
        program,
        program.symbols.name(source.root),
        &source.segments,
    )
    .0
}

pub(super) fn apply_transfer(
    program: &dyn crate::CheckingInput,
    stored: &mut Vec<StoredLocalOrigins>,
    transfer: &SlotRelationTransfer,
) {
    // Retire prior case/move epochs before publishing current pointer slots.
    // Only a complete normal route can justify a closed case-frontier update.
    if transfer.is_complete() {
        for effect in &transfer.case_effects {
            effect.install(stored);
        }
    }
    apply(program, stored, &transfer.effects);
}

pub(super) fn apply(
    program: &dyn crate::CheckingInput,
    stored: &mut Vec<StoredLocalOrigins>,
    effects: &[SlotEffect],
) {
    // Every source above was captured from the same pre-call inventory.
    for effect in effects {
        let index = stored
            .iter()
            .position(|record| record.local_symbol == effect.root)
            .unwrap_or_else(|| {
                stored.push(StoredLocalOrigins {
                    local_symbol: effect.root,
                    references: Vec::new(),
                    cases: Vec::new(),
                    moves: Vec::new(),
                    symbolic: false,
                });
                stored.len() - 1
            });
        let record = &mut stored[index];
        let invalidates_descendants = effect.may_select
            && effect
                .sources
                .as_ref()
                .is_none_or(|sources| !sources.is_empty());
        if effect.sources.is_none() || invalidates_descendants {
            // Exact overlays cannot hide an unknown covering region, including
            // an equal-depth wildcard. Keep each known debt, beside a separate
            // incomplete row at every already-materialized covered coordinate.
            let mut unresolved = Vec::new();
            for leaf in &record.references {
                if stored_origins::path_covers(&effect.segments, &leaf.local_segments)
                    && (effect.sources.is_none()
                        || leaf.local_segments.len() > effect.segments.len())
                {
                    let mut unknown = leaf.clone();
                    unknown.origin = None;
                    unknown.lifetime_dependencies.clear();
                    if !record
                        .references
                        .iter()
                        .chain(unresolved.iter())
                        .any(|prior| {
                            prior.local_segments == unknown.local_segments
                                && prior.kind == unknown.kind
                                && prior.may_select == unknown.may_select
                                && prior.origin.is_none()
                                && prior.lifetime_dependencies.is_empty()
                        })
                    {
                        unresolved.push(unknown);
                    }
                }
            }
            record.references.extend(unresolved);
        }
        // Materialize the old concrete slot before installing an overlay on
        // a conditional write. Keeping only its wildcard default would let
        // the new exact row shadow the no-write branch's surviving origin.
        let incoming = if effect.incoming && !effect.may_select {
            stored_origins::selected_slot_sources(record, &effect.segments, Some(false))
        } else {
            Some(Vec::new())
        };
        let incoming_unknown = incoming.is_none() || effect.sources.is_none();
        // Installing a pointer changes the storage reached beneath it. Old
        // pointee-slot snapshots cannot outlive that replacement, even when
        // another normal route preserves the pointer's incoming contents.
        record.references.retain(|leaf| {
            // Unknown contents authorize poisoning, not replacing a typed row.
            // A later complete definite overwrite may retire these snapshots.
            effect.sources.is_none()
                || effect.may_select
                || (!((!effect.incoming
                    || effect
                        .sources
                        .as_ref()
                        .is_none_or(|sources| !sources.is_empty()))
                    && leaf.local_segments.len() > effect.segments.len()
                    && leaf.local_segments.starts_with(&effect.segments))
                    && (effect.incoming || leaf.local_segments != effect.segments))
        });
        let sources: Vec<_> = incoming
            .into_iter()
            .flatten()
            .chain(effect.sources.iter().flatten().cloned())
            .collect();
        let lifetime_dependencies: Vec<_> = sources
            .iter()
            .filter(|source| source.lifetime_access.is_some())
            .cloned()
            .collect();
        let origins = sources
            .iter()
            .filter(|source| !source.lifetime_access.is_some())
            .map(|source| (Some(source.origin.clone()), lifetime_dependencies.clone()))
            .chain(
                (!lifetime_dependencies.is_empty())
                    .then_some((None, lifetime_dependencies.clone())),
            )
            // A partial debt roster is not a complete opaque dependency answer.
            .chain(incoming_unknown.then_some((None, Vec::new())));
        for (origin, lifetime_dependencies) in origins {
            if record.references.iter().any(|leaf| {
                leaf.local_segments == effect.segments
                    && leaf.kind == effect.kind
                    && leaf.origin == origin
                    && leaf.may_select == effect.may_select
                    && leaf.lifetime_dependencies == lifetime_dependencies
            }) {
                continue;
            }
            record.references.push(StoredWriteOrigin {
                lifetime_dependencies: lifetime_dependencies.clone(),
                kind: effect.kind,
                local_symbol: effect.root,
                local_path: path(
                    program,
                    &FrameSourcePlace {
                        root: effect.root,
                        segments: effect.segments.clone(),
                        builtin_coordinates: true,
                        ..Default::default()
                    },
                ),
                local_segments: effect.segments.clone(),
                may_select: effect.may_select,
                origin,
            });
        }
        if effect.kind == stored_origins::StoredOriginKind::OpaqueRegion {
            record
                .cases
                .retain(|path| !stored_origins::path_covers(&effect.segments, path));
            record.moves.retain(|moved| {
                !stored_origins::path_covers(&effect.segments, &moved.local_segments)
            });
        }
        record.symbolic = false;
    }
}
