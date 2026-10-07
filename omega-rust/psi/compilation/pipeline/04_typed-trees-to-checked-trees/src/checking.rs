//! Immutable typed-tree checking: validate contracts and ownership, assemble
//! execution plans, then publish checked trees. Preparation has already closed
//! the executable bodies and call identities; this stage never repairs them.
//!
//! `lower_typed_trees` is the one entrance; a `CheckingRequest` names the
//! package checkpoint (preliminary or settled) and carries source admission
//! evidence. Every production request enforces crash admission.
//!
//! A request checks its prepared program afresh unless its caller hands back a
//! [`RetainedCheck`] from earlier in the same invocation. Package review is
//! that caller: its bound pass recompiles occurrences whose discovered
//! bindings changed, and provider settlement usually leaves the settled typed
//! program identical, so the bound pass would repeat a judgment the discovery
//! pass already made (about 200 s of std's checking per update in a debug
//! build). The judgment is a pure function of the typed program, the
//! checkpoint and the opaque-property receipts, so an equal input takes the
//! retained judgment, which this function computed. Everything invocation-owned
//! (wire demands, constant completeness, asm and declaration authority) still
//! runs on every request. A global exact-input memo once served such repeats
//! implicitly; it was deleted, and reuse is now an explicit caller input.

pub(crate) mod call_acknowledgements;
pub(crate) mod program_validation;

use crate::checking::program_validation::prepare_typed_program;
use crate::checks;
use crate::facts::build_check_facts;
use crate::preparation::TypedPreparation;
use checked_trees::CheckedTrees;

/// One typed->checked lowering request. `settled()` is the final package
/// checkpoint. Candidate specializations derive from the source itself.
/// Standalone programs and package builds
/// share it: ordinary package selections stay strict while unresolved
/// compiler-owned toolchain selections remain TCB input. A package build supplies
/// its declaration authority, which this stage checks before returning the
/// result. `preliminary()` is the pre-settlement checkpoint: unresolved
/// selections are retained only for compiler-owned toolchain source, pending
/// opaque-copy evidence and deferred const array lengths/range endpoints are
/// tolerated; supplied package authority still rejects unresolved ordinary-package
/// selections before the caller can grant build authority.
///
/// Psi derives applications from authored uses and uses ordinary
/// authoritative specialization; the request carries no application strings,
/// capability assertions, or provider-selection policy.
#[derive(Clone, Copy)]
pub struct CheckingRequest<'a> {
    /// The pre-settlement checkpoint: deferred constants and pending opaque
    /// `[copy]` claims are tolerated.
    pub(crate) admits_pending_inputs: bool,
    pub(crate) asm_authority: Option<validation::AsmAuthorityAdmission>,
    pub(crate) opaque_property_receipts: &'a [validation::OpaqueDataPropertyReceipt],
    pub(crate) declaration_authority: Option<&'a dyn validation::DeclarationSelectionAuthority>,
    pub(crate) wire_compatibility_demands:
        &'a [language_semantics::wire::WireCompatibilityDemand],
}

impl<'a> CheckingRequest<'a> {
    const fn at_checkpoint(admits_pending_inputs: bool) -> Self {
        Self {
            admits_pending_inputs,
            asm_authority: None,
            opaque_property_receipts: &[],
            declaration_authority: None,
            wire_compatibility_demands: &[],
        }
    }

    /// The final package checkpoint without invocation-owned admission evidence.
    pub const fn settled() -> Self {
        Self::at_checkpoint(false)
    }

    /// The pre-settlement package checkpoint.
    pub const fn preliminary() -> Self {
        Self::at_checkpoint(true)
    }

    /// Request privileged-instruction admission under the evaluated build.
    /// Language-only probes leave this absent: checking an unused runtime
    /// body during constant evaluation is not permission to install it.
    /// Final product checking supplies authority and validates it on every
    /// invocation, independently of the memoized language judgment.
    pub const fn with_asm_authority(
        mut self,
        authority: validation::AsmAuthorityAdmission,
    ) -> Self {
        self.asm_authority = Some(authority);
        self
    }

    /// Independently rederived representation receipts. Input preparation
    /// rejoins the same authored declaration's dependency-scope instances;
    /// property validation still checks every exact resulting row.
    pub const fn with_opaque_property_receipts(
        mut self,
        receipts: &'a [validation::OpaqueDataPropertyReceipt],
    ) -> Self {
        self.opaque_property_receipts = receipts;
        self
    }

    /// Authored wire requirements for this invocation. Generated codecs are
    /// checked even without demands; changing demands cannot reuse a previous
    /// invocation's acceptance of the same typed source.
    pub const fn with_wire_compatibility_demands(
        mut self,
        demands: &'a [language_semantics::wire::WireCompatibilityDemand],
    ) -> Self {
        self.wire_compatibility_demands = demands;
        self
    }

    /// Reconciled package authority for this invocation. Psi checks authored
    /// selections before returning the program; loading a declaration does not
    /// authorize its use. Standalone source has no package admission request.
    pub const fn with_declaration_authority(
        mut self,
        authority: Option<&'a dyn validation::DeclarationSelectionAuthority>,
    ) -> Self {
        self.declaration_authority = authority;
        self
    }
}

/// Check typed trees under one explicit request: the checkpoint mode plus
/// the source admission evidence supplied by its caller. Toolchain-owned
/// selections may remain late-bound until build-time evaluation; ordinary
/// authored selections remain strict in both modes.
pub fn lower_typed_trees(
    program: typed_trees::FrozenTypedTrees,
    preparation: TypedPreparation,
    request: &CheckingRequest<'_>,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees_reusing(program, preparation, request, None, false)
        .map(|outcome| outcome.checked)
}

/// One language judgment kept with the exact input it judged. Only this module
/// constructs one, from a check it ran, so a judgment taken from it is a
/// judgment this function computed for an equal input.
#[derive(Debug, Clone)]
pub struct RetainedCheck {
    program: std::sync::Arc<typed_trees::FrozenTypedTrees>,
    admits_pending_inputs: bool,
    opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
    checked: std::sync::Arc<CheckedTrees>,
}

/// What [`lower_typed_trees_reusing`] returns: the checked program, this
/// judgment when retention was asked for, and whether an equal prior supplied
/// it instead of a check.
pub struct CheckingOutcome {
    pub checked: CheckedTrees,
    pub retained: Option<RetainedCheck>,
    pub took_prior: bool,
}

/// [`lower_typed_trees`], taking `prior`'s judgment when its program, checkpoint
/// and receipts equal this request's, and returning this request's judgment
/// for a later request when `retain`. Retention clones the input program and
/// the result; only a caller that will ask again should request it.
pub fn lower_typed_trees_reusing(
    program: typed_trees::FrozenTypedTrees,
    preparation: TypedPreparation,
    request: &CheckingRequest<'_>,
    prior: Option<&RetainedCheck>,
    retain: bool,
) -> Result<CheckingOutcome, Vec<diagnostics::Diagnostic>> {
    let opaque_property_receipts = &preparation.opaque_property_receipts;
    let equal_prior = prior.filter(|prior| {
        prior.admits_pending_inputs == request.admits_pending_inputs
            && prior.opaque_property_receipts == *opaque_property_receipts
            && *prior.program == program
    });
    let took_prior = equal_prior.is_some();
    let (checked, retained) = match equal_prior {
        Some(prior) => ((*prior.checked).clone(), retain.then(|| prior.clone())),
        None => {
            let input = retain.then(|| {
                (
                    std::sync::Arc::new(program.clone()),
                    preparation.opaque_property_receipts.clone(),
                )
            });
            let checked = check_program(program, preparation, request)?;
            let retained = input.map(|(program, opaque_property_receipts)| RetainedCheck {
                program,
                admits_pending_inputs: request.admits_pending_inputs,
                opaque_property_receipts,
                checked: std::sync::Arc::new(checked.clone()),
            });
            (checked, retained)
        }
    };
    // Package authority is invocation-owned, not part of the language
    // judgment: admit the returned selections under the caller's authority.
    if let Some(authority) = request.declaration_authority {
        let mut sibling_occurrences = crate::target_sibling_selection_occurrences(&checked.typed);
        sibling_occurrences.extend(crate::build_program::descriptor_selection_occurrences(
            &checked.typed,
        ));
        validation::validate_authored_declaration_selections(
            &checked,
            authority,
            &sibling_occurrences,
        )?;
    }
    Ok(CheckingOutcome {
        checked,
        retained,
        took_prior,
    })
}

/// The check itself, over a program `lower_typed_trees` has found complete.
fn check_program(
    program: typed_trees::FrozenTypedTrees,
    preparation: TypedPreparation,
    request: &CheckingRequest<'_>,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let CheckingRequest {
        admits_pending_inputs,
        opaque_property_receipts: _,
        declaration_authority: _,
        asm_authority: _,
        wire_compatibility_demands: _,
    } = *request;
    let TypedPreparation {
        static_machine_selections,
        mathematics,
        opaque_property_receipts,
    } = preparation;
    // Preparation's specialization and call rebinding have finished. Open
    // the immutable plan scope here: the first consumer computes program-pure
    // operational and service-reach plans and the rest serve from them.
    let _plan_scope = ::validation::enter_program_plan_scope();
    let _field_domain_scope = crate::facts::field_domain::enter_field_domain_scope();
    let _root_currency_scope = checks::enter_root_currency_scope();
    // Borrow the prepared program with invocation-owned query state. Read-only inputs
    // carry this context through the checking flow; no consumer discovers it
    // by thread or address. Metadata publication waits until checking ends.
    let typed_program = program;
    let program = validation::CheckingContext::new(&typed_program);
    crate::monomorphization::validate_selected_attached_method_bounds(&program)?;
    let mut validated =
        prepare_typed_program(&program, &opaque_property_receipts, admits_pending_inputs)?;
    // Caller-visible mutation summaries depend only on the immutable typed
    // program and the borrow facts assembled once here. Fact construction, the
    // direct-borrow resource closures, and the independent check replay all
    // query the same (program, borrow) pair, so one lazily-filled table serves
    // the whole check pass instead of each consumer rebuilding it.
    let mutation_summaries = crate::flow::StateMutationSummaryCache::default();
    // Incoming guard discovery is immutable program data keyed only on
    // resolver presence; certificate construction and the check replay share
    // one lazily-filled table instead of rebuilding it per entry.
    let incoming_guard_index = checks::IncomingGuardIndexCache::default();
    // Validation already judged each checked body's contract entailment on
    // this program; the exit checks read those proved postconditions
    // instead of rejudging every machine.
    let proven_machine_contracts =
        std::mem::take(&mut validated.validation_facts.proven_machine_contracts);
    // The range checker bounds indexes with the intervals the same
    // arithmetic walk established; nothing after checking reads them.
    let expression_intervals = std::mem::take(&mut validated.validation_facts.expression_intervals);
    let (mut facts, _prepared_effect_scope) = build_check_facts(
        &program,
        &validated.proof_plan,
        &validated.integer_assignments,
        validated.operational,
        validated.service_reaches,
        validated.validation_facts,
        static_machine_selections,
        mathematics,
        &mutation_summaries,
    )?;
    // Sealed quotient requests were deferred by validation until termination
    // was proved: judge them now against the checked termination facts. The
    // batch is all-or-nothing and grants no executable operation.
    validation::admit_checked_quotient_requests(&program, &|machine| {
        facts
            .termination
            .for_machine(machine)
            .map(|plan| plan.checked_summary.clone())
    })?;
    checks::initialize_checked_direct_borrow_resources(&program, &mut facts, &mutation_summaries)?;
    // Compute at the original failure boundary, preserving diagnostic order.
    // Nothing below consumes the specialization commitments. Publish them
    // with the other checked facts, never by rewriting the typed roster.
    let specialization_identities =
        validation::contract_identities::prepare_specialization_contract_identities(
            &program,
            &facts.contract_plans,
        )?;

    checks::check_checked_facts_recording_with_mutation_summaries(
        &program,
        &mut facts,
        &mutation_summaries,
        &incoming_guard_index,
        &proven_machine_contracts,
        &expression_intervals,
    )?;
    checks::validate_selected_cleanup_guarantees(&program, &facts)?;
    crate::facts::refresh_realized_contract_envelopes(&mut facts);

    let mut facts = crate::execution::finalize_execution::finalize_execution(&program, facts)?;
    facts.flow.semantic_dependencies =
        crate::flow::derive_checked_semantic_dependencies(&program, &facts);

    crate::authored_selections::bind_checked_intrinsic_call_facts(&program, &mut facts)
        .map_err(|diagnostic| vec![diagnostic])?;
    facts.specialization_identities = specialization_identities;
    drop(program);
    let program = typed_program;
    facts.finalized_selections =
        crate::authored_selections::finalize_checked_authored_selections(&program, &mut facts)
            .map_err(|diagnostic| vec![diagnostic])?;
    // Visibility and cleanup consume checked choices over the same immutable
    // source. No cloned typed program or consumer fallback supplies this ledger.
    let finalized = validation::CheckingContext::new(&program)
        .with_authored_declaration_selections(&facts.finalized_selections);
    validation::validate_reserved_cleanup_selections(&finalized)?;
    validation::validate_declaration_visibility(&finalized)?;
    drop(finalized);

    // Classify borrow custody last: `program`'s type, machine and state
    // tables stopped growing in preparation, after monomorphization, so a
    // row recorded before preparation would miss
    // the references and specialized states those added. Nothing below
    // takes `program` mutably — `CheckedTrees::with_roots` only pairs it
    // with the facts — so neither table can grow past this point.
    facts.reference_custody =
        validation::reference_result_custody::classify_program_reference_custody(&program);
    facts.state_reference_custody =
        validation::reference_result_custody::classify_program_state_reference_custody(&program);
    // Content classification rides the same placement, and for the same
    // reason. Both memos it leans on -- the data-definition index and the
    // attached-`::drop` index -- are owned by `program` and keyed by table
    // revision, not by the frozen scope dropped above, so they still answer
    // from cache here.
    facts.content_classes = validation::classify_program_content_classes(&program);
    // Retain place/result decisions under final occurrence handles. Recording
    // before specialization or authored-selection finalization would miss the
    // generated bodies; lowering must not repair an incomplete table itself.
    facts.resolved_types = crate::facts::resolved_types::resolve_program_types(&program);

    let checked = CheckedTrees::with_roots(program, facts);
    validation::validate_boundary_calls(&checked)?;
    Ok(checked)
}
