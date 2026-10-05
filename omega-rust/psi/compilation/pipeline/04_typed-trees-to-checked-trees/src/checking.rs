//! Typed-tree checking: specialize calls, validate contracts and ownership,
//! assemble execution plans, then publish checked trees.
//!
//! `lower_typed_trees` is the one entrance; a `CheckingRequest` names the
//! package checkpoint (preliminary or settled) and carries source admission
//! evidence. Only the test-only crash-inspection mode omits crash admission.
//!
//! Every request checks its program afresh. An exact-input memo once served
//! repeated checks of an equal program; only package review's passes ever
//! hit it, so it was deleted rather than kept as a second path to a
//! judgment.

pub(crate) mod call_acknowledgements;
mod default_domain_zero;
mod deferred_constants;
mod opaque_property_receipts;
pub(crate) mod program_validation;

use crate::checking::program_validation::prepare_typed_program;
use crate::checks;
use crate::facts::build_check_facts;
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
    admits_pending_inputs: bool,
    asm_authority: Option<validation::AsmAuthorityAdmission>,
    opaque_property_receipts: &'a [validation::OpaqueDataPropertyReceipt],
    declaration_authority: Option<&'a dyn validation::DeclarationSelectionAuthority>,
    wire_compatibility_demands: &'a [language_semantics::wire::WireCompatibilityDemand],
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
    program: typed_trees::TypedTrees,
    request: &CheckingRequest<'_>,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    // Declaration descriptions may retain pending implementations, but the
    // executable roster must be complete before any proof or memo lookup.
    for machine in program.machines() {
        if machine.body_status == typed_trees::machine::MachineBodyStatus::Deferred {
            return Err(vec![diagnostics::Diagnostic::error(format!(
                "machine `{}` requires implementation completion before checking",
                machine.name.as_str(),
            ))]);
        }
    }
    // Wire validity is part of this stage's contract, not an optional report
    // pass in a product compiler. Demands are invocation-owned, so recheck them
    // before any exact-input memo can return a prior language judgment.
    crate::wire_compatibility::validate_wire_protocol(
        &program,
        request.wire_compatibility_demands,
    )?;
    // Input completeness belongs to this stage.
    if !request.admits_pending_inputs {
        deferred_constants::require_evaluated_constants(&program)?;
    }
    if let Some(authority) = request.asm_authority {
        validation::validate_asm_discharge(&program, authority)?;
    }
    // Rejoin authored source instances before specialization can introduce
    // generated declarations with the same source provenance.
    let opaque_property_receipts =
        opaque_property_receipts::for_source_instances(&program, request.opaque_property_receipts);
    let request = request.with_opaque_property_receipts(&opaque_property_receipts);
    let checked = check_program(program, &request)?;
    // Package authority is invocation-owned, not part of the language
    // judgment: admit the returned selections under the caller's authority.
    if let Some(authority) = request.declaration_authority {
        let mut sibling_occurrences = crate::target_sibling_selection_occurrences(&checked);
        sibling_occurrences.extend(crate::build_program::descriptor_selection_occurrences(
            &checked,
        ));
        validation::validate_authored_declaration_selections(
            &checked,
            authority,
            &sibling_occurrences,
        )?;
    }
    Ok(checked)
}

/// The check itself, over a program `lower_typed_trees` has found complete.
fn check_program(
    mut program: typed_trees::TypedTrees,
    request: &CheckingRequest<'_>,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let CheckingRequest {
        admits_pending_inputs,
        opaque_property_receipts,
        declaration_authority: _,
        asm_authority: _,
        wire_compatibility_demands: _,
    } = *request;
    // Mathematical `let`/`boundary let` declarations elaborate into checked
    // records and a kernel signature the kernel re-decides. Check them first,
    // so a declaration that fails either elaboration fails with its own
    // diagnostic, and carry the result to the proof facts.
    let mathematics = crate::proof::check_mathematical_declarations(&program)?;
    // Stage-1 machine monomorphization MUST precede validation: a generic
    // machine whose value calls agree on one instantiation is substituted to a
    // concrete machine here. Validation permits unused template bodies but the
    // generic-value-call fence still rejects any emitted concrete caller whose
    // callee remains generic (an incomplete specialization).
    // Projected statement receivers retain their lexical root before their
    // field endpoint is known. Resolve both call forms before specialization
    // and effect inference, not only when finalizing authored selections.
    crate::lookup::resolve_projected_receiver_calls(&mut program)?;
    // MP2b must judge the authored requirement -> selected implementation edge
    // before MP4 consumes the call-site selections and clears the template's
    // parameter list.
    // PDI3 operation/algebra authority is part of the authored generic
    // contract, so bind it before specialization captures the universal
    // template fingerprint. Normalize again afterward to cover cloned
    // expression handles and concrete substitutions.
    crate::authored_selections::bind_pre_specialization_authored_selections(&mut program)
        .map_err(|diagnostic| vec![diagnostic])?;
    normalize_open_index_identities(&mut program)?;
    // Local dynamic selections and every authored boundary conformance own
    // their complete finite roster. Runtime provider selection is not a
    // checking input; every candidate is checked before Omega can choose it.
    crate::monomorphization::generate_dynamic_family_specializations(&mut program)?;
    // Keep the authored generic provider templates immutable while ordinary
    // machine specialization closes caller binders. All operator candidates
    // derive from authored conformance edges without Build selection. Demands
    // may appear only in newly concrete bodies, and a candidate may itself
    // expose another ordinary or boundary application. Alternate elaborators to a
    // fixed point; neither open applications nor template mutation may be
    // mistaken for final D29 coverage. A generated candidate body
    // can itself contain a dynamic selection, so family generation repeats
    // inside the same fixed point.
    // Token syntax must expose its ordinary call before specialization sees
    // the program. Otherwise a concrete caller keeps a generic requirement
    // merely because it wrote `items[index]` rather than the named call.
    crate::operators::bind_selected_operator_calls(&mut program)?;
    let provider_candidates =
        crate::monomorphization::OperatorCandidateTemplates::candidate_requests(&program);
    let provider_templates = crate::monomorphization::OperatorCandidateTemplates::prepare(
        &program,
        &provider_candidates,
    );
    let mut static_machine_selections =
        specialize_static_machine_calls_with_selections(&mut program, true)?;
    normalize_open_index_identities(&mut program)?;
    let mut generated_bodies = false;
    loop {
        let mut materialized = match &provider_templates {
            Some(templates) => crate::monomorphization::specialize_operator_candidates(
                templates,
                &mut program,
                &provider_candidates,
            )?,
            None => 0,
        };
        materialized +=
            crate::monomorphization::generate_dynamic_family_specializations(&mut program)?;
        // Generated bodies can introduce new token uses. Feed them back into
        // the same specialization loop, with unchanged call/contract checks.
        materialized += crate::operators::bind_selected_operator_calls(&mut program)?;
        if materialized == 0 {
            break;
        }
        generated_bodies = true;
        // Generated bodies only add concrete calls; the authored generic
        // templates stay unchanged, so their contract entailment and the
        // conformance applications are judged once, after convergence.
        static_machine_selections =
            specialize_generated_static_machine_calls(&mut program)?;
        normalize_open_index_identities(&mut program)?;
    }
    if generated_bodies {
        crate::conformance::conformance_applications::validate_conformance_applications(
            &program,
        )?;
    }
    // F2b: unsuffixed float literals at declared f32/f64 destinations land
    // their format on the text carrier HERE, while the tree is still mutable
    // and before both engines fork off it -- every downstream read (native
    // and interpreter) then rounds once from the spelling.
    validation::land_float_literal_destinations(&mut program);
    // Calls through a typed `dyn Trait` receiver cannot be resolved during the
    // earlier symbol pass because local declared types are not available there.
    // Bind their declaring-trait requirement now so the ordinary result-
    // overload pass below starts in the correct trait family rather than from
    // an ambient same-named machine.
    validation::resolve_dynamic_call_targets(&mut program)?;
    // Concrete substitutions may make previously open field types selectable.
    crate::lookup::resolve_projected_receiver_calls(&mut program)?;
    // Named-machine result overloads are provisionally bound to the first
    // same-named symbol during early resolution. Rebind them now, after domain
    // normalization and destination typing, before validation/backend facts
    // consume the call identity.
    validation::resolve_named_result_overloads(&mut program)?;
    // A float-to-integer policy cast selects its destination-owned conversion
    // overload by the cast's own result qualification. Bind it after the
    // named rebinding above, which would otherwise reselect the unqualified
    // overload for a call whose context expects no result domain.
    validation::bind_float_conversion_casts(&mut program)?;
    // A spelled use that selects a token-bearing machine is supplied by that
    // declaration's own body: bind it to an ordinary call now, after every
    // specialization and call-identity rebinding above, so validation and
    // every executing consumer see the same call edge a named call would make.
    crate::operators::bind_selected_operator_calls(&mut program)?;
    // Early landing supports operand-driven selection, but provisional named
    // targets need not expose their final formal types yet. Result-overload
    // rebinding and conversion elaboration above can also introduce call
    // destinations. Finish landing against those exact selected signatures
    // before scalar plans or either evaluator consumes the frozen program.
    // Already-landed literals retain their format; this is not a default-f64
    // fallback or permission to reinterpret a typed argument.
    validation::land_float_literal_destinations(&mut program);
    default_domain_zero::refine_default_domain_zero(&mut program);
    // The program is now frozen for checking: specialization and every
    // call-identity rebinding above mutate it, so the plan memo scope can only
    // open here — the first of the consumers below computes the program-pure
    // operational and service-reach plans and the rest serve from them.
    let _plan_scope = ::validation::enter_program_plan_scope();
    let _field_domain_scope = crate::facts::field_domain::enter_field_domain_scope();
    let _root_currency_scope = checks::enter_root_currency_scope();
    // Consumers below share one set of write-frame memos and one proof-only
    // classification. The only mutation in the scope, contract-identity
    // binding, rewrites specialization fingerprints neither reads; the scope
    // closes before authored-selection finalization rewrites call targets.
    let frozen_program_scope = validation::enter_frozen_program_scope(&program);
    crate::monomorphization::validate_selected_attached_method_bounds(&program)?;
    let mut validated =
        prepare_typed_program(&program, opaque_property_receipts, admits_pending_inputs)?;
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
    let (mut facts, _prepared_effect_scope) = build_check_facts(
        &program,
        &validated.proof_plan,
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
    // MP5: specialization selection happens before checked contract plans
    // exist. Bind the selected machines' normalized contract identities now
    // and make them part of the instance fingerprint used by caches and
    // artifacts.
    crate::monomorphization::bind_specialization_contract_identities(
        &mut program,
        &facts.contract_plans,
    )?;

    checks::check_checked_facts_recording_with_mutation_summaries(
        &program,
        &mut facts,
        &mutation_summaries,
        &incoming_guard_index,
        &proven_machine_contracts,
    )?;
    checks::validate_selected_cleanup_guarantees(&program, &facts)?;
    crate::facts::refresh_realized_contract_envelopes(&mut facts);

    let mut facts = crate::execution::finalize_execution::finalize_execution(&program, facts)?;
    facts.flow.semantic_dependencies =
        crate::flow::derive_checked_semantic_dependencies(&program, &facts);

    crate::authored_selections::bind_checked_intrinsic_call_facts(&program, &mut facts)
        .map_err(|diagnostic| vec![diagnostic])?;
    drop(frozen_program_scope);
    crate::authored_selections::finalize_checked_authored_selections(&mut program, &facts)
        .map_err(|diagnostic| vec![diagnostic])?;
    validation::validate_reserved_cleanup_selections(&program)?;
    validation::validate_declaration_visibility(&program)?;

    // Classify borrow custody last: `program`'s type, machine and state
    // tables stop growing only here, after monomorphization and
    // authored-selection finalization, so a row recorded earlier would miss
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

    let checked = CheckedTrees::with_roots(program, facts);
    validation::validate_boundary_calls(&checked)?;
    Ok(checked)
}

/// Bind exact PDI3 operation/algebra authority and refresh every enclosing
/// indexed-domain semantic ID. Orchestration calls this before typed
/// snapshots and trust receipts; checked lowering calls it before capturing
/// generic template fingerprints and again after specialization.
pub fn normalize_open_index_identities(
    program: &mut typed_trees::TypedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    ::validation::normalize_open_index_expressions(program)?;
    crate::monomorphization::refresh_closed_domain_instance_identities(program)
        .map_err(|diagnostic| vec![diagnostic])
}

/// Validate and consume compile-time machine-symbol selections, rewriting
/// every complete generic call tuple to direct concrete calls. The ordinary
/// checked-tree path invokes this before validation; orchestration also uses
/// it on a private clone before interpreting build.omg so build-time execution
/// sees the same specialized program as runtime lowering.
///
/// This speculative route tolerates an incomplete concrete selection: build
/// preparation runs while const endpoint folds are still outstanding, so an
/// underivable tuple is interim evidence, not a rejected program. The
/// authoritative checking pass enforces the complete-tuple gate itself.
pub fn specialize_static_machine_calls(
    program: &mut typed_trees::TypedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    specialize_static_machine_calls_with_selections(program, false).map(|_| ())
}

/// One iteration of the specialization fixed point over newly generated
/// bodies: derive the static selections and close them, without re-judging
/// the authored templates the first pass already judged.
fn specialize_generated_static_machine_calls(
    program: &mut typed_trees::TypedTrees,
) -> Result<::validation::ValidatedStaticMachineSelections, Vec<diagnostics::Diagnostic>> {
    crate::conformance::conformance_application_lifetimes::resolve_elided_conformance_lifetimes(
        program,
    )?;
    let mut selections = ::validation::validate_static_machine_selections_with_facts(program)?;
    crate::monomorphization::monomorphize_generic_machine_value_calls_with_selections(
        program,
        &mut selections,
        true,
    )?;
    Ok(selections)
}

pub(crate) fn specialize_static_machine_calls_with_selections(
    program: &mut typed_trees::TypedTrees,
    enforce_complete_concrete_selections: bool,
) -> Result<::validation::ValidatedStaticMachineSelections, Vec<diagnostics::Diagnostic>> {
    crate::conformance::conformance_application_lifetimes::resolve_elided_conformance_lifetimes(
        program,
    )?;
    crate::conformance::conformance_applications::validate_conformance_applications(program)?;
    let mut selections = ::validation::validate_static_machine_selections_with_facts(program)?;
    ::validation::validate_generic_machine_contract_entailment(program)?;
    crate::monomorphization::monomorphize_generic_machine_value_calls_with_selections(
        program,
        &mut selections,
        enforce_complete_concrete_selections,
    )?;
    Ok(selections)
}
