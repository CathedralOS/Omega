//! Immutable typed-tree checking: validate contracts and ownership, assemble
//! execution plans, then publish checked trees. Preparation has already closed
//! the executable bodies and call identities; this stage never repairs them.
//!
//! `lower_typed_trees` is the one entrance. `TypedPreparation` carries the
//! producer's admission checkpoint and source judgments; `CheckingRequest`
//! carries package declaration authority. Checking cannot change the prepared
//! input's checkpoint. Every production request enforces crash admission.
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
//! still runs on every request: preparation admits wire demands, constant
//! completeness and asm before this entrance; checking admits declaration
//! authority even after reuse. A global exact-input memo once served such repeats
//! implicitly; it was deleted, and reuse is now an explicit caller input.

pub(crate) mod call_acknowledgements;
pub(crate) mod program_validation;
mod target_families;

use crate::checking::program_validation::prepare_typed_program;
use crate::checks;
use crate::facts::build_check_facts;
use checked_trees::CheckedTrees;
use representation_selections::OpaqueRepresentationSelection;
use typed_trees_to_typed_trees::TypedPreparation;

/// Invocation-owned package declaration authority. The preparation producer
/// carries its admission checkpoint in TypedPreparation; checking cannot choose
/// a different checkpoint over that producer's result.
#[derive(Clone, Copy, Default)]
pub struct CheckingRequest<'request> {
    declaration_authority: Option<&'request dyn validation::DeclarationSelectionAuthority>,
}

impl<'request> CheckingRequest<'request> {
    pub const fn new() -> Self {
        Self {
            declaration_authority: None,
        }
    }

    /// Admit checked selections against this invocation's reconciled package authority.
    pub const fn with_declaration_authority(
        mut self,
        authority: Option<&'request dyn validation::DeclarationSelectionAuthority>,
    ) -> Self {
        self.declaration_authority = authority;
        self
    }
}

/// Check the prepared typed input under this invocation's declaration authority. Toolchain-owned
/// selections may remain late-bound until build-time evaluation; ordinary
/// authored selections remain strict in both modes.
pub fn lower_typed_trees(
    program: typed_trees::FrozenTypedTrees,
    preparation: TypedPreparation,
    request: &CheckingRequest<'_>,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    lower_typed_trees_reusing(program, preparation, None, request, None, false).map(|outcome| {
        std::sync::Arc::try_unwrap(outcome.checked).unwrap_or_else(|shared| (*shared).clone())
    })
}

/// One language judgment kept with the exact input it judged. Only this module
/// constructs one, from a check it ran, so a judgment taken from it is a
/// judgment this function computed for an equal input.
#[derive(Debug, Clone)]
pub struct RetainedCheck {
    /// The typed program before preparation and the admission inputs it was
    /// prepared under. Preparation is a pure function of that pair, so an
    /// equal request can take this judgment without preparing or checking.
    preparation_input: std::sync::Arc<typed_trees::TypedTrees>,
    preparation_admission: typed_trees_to_typed_trees::PreparationAdmission,
    program: std::sync::Arc<typed_trees::FrozenTypedTrees>,
    admits_pending_inputs: bool,
    opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
    checked: std::sync::Arc<CheckedTrees>,
}

impl RetainedCheck {
    /// Identity of an already-issued language judgment, not semantic equality
    /// or permission to skip admission. Invocation owners use it only to avoid
    /// retaining the same immutable result twice.
    pub fn same_judgment(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.checked, &other.checked)
    }

    /// Whether this judgment was computed for `program` under `admission`.
    /// Preparation is a pure function of the typed program and its admission
    /// inputs, so an equal pair reproduces this judgment's preparation and
    /// check exactly — [`reuse_retained_check`] may be taken without either.
    pub fn judges_preparation_input(
        &self,
        program: &typed_trees::TypedTrees,
        admission: &typed_trees_to_typed_trees::PreparationAdmission,
    ) -> bool {
        self.preparation_admission == *admission && *self.preparation_input == *program
    }
}

/// What [`lower_typed_trees_reusing`] returns: the checked program, this
/// judgment when retention was asked for, and whether an equal prior supplied
/// it instead of a check.
pub struct CheckingOutcome {
    pub checked: std::sync::Arc<CheckedTrees>,
    pub retained: Option<RetainedCheck>,
    pub took_prior: bool,
}

/// Take `prior`'s judgment for a preparation input it already judged. The
/// caller has compared the input and admission and found them equal
/// ([`RetainedCheck::judges_preparation_input`]); preparation and checking are
/// both pure over that pair, so nothing recomputes. Invocation-owned
/// declaration authority still runs.
pub fn reuse_retained_check(
    prior: &RetainedCheck,
    request: &CheckingRequest<'_>,
    retain: bool,
) -> Result<CheckingOutcome, Vec<diagnostics::Diagnostic>> {
    let checked = prior.checked.clone();
    let retained = retain.then(|| prior.clone());
    admit_declaration_authority(&checked, request)?;
    Ok(CheckingOutcome {
        checked,
        retained,
        took_prior: true,
    })
}

/// [`lower_typed_trees`], taking `prior`'s judgment when its program, checkpoint
/// and receipts equal this preparation's, and returning this request's judgment
/// for a later request when `retain`. `retained_input` is the pre-preparation
/// program beside its admission snapshot, present only when `retain`: it lets
/// a later caller prove equal inputs before preparing at all. Retention clones
/// the input program and the result; only a caller that will ask again should
/// request it.
pub fn lower_typed_trees_reusing(
    program: typed_trees::FrozenTypedTrees,
    preparation: TypedPreparation,
    retained_input: Option<(
        std::sync::Arc<typed_trees::TypedTrees>,
        typed_trees_to_typed_trees::PreparationAdmission,
    )>,
    request: &CheckingRequest<'_>,
    prior: Option<&RetainedCheck>,
    retain: bool,
) -> Result<CheckingOutcome, Vec<diagnostics::Diagnostic>> {
    let (
        admits_pending_inputs,
        mathematics,
        opaque_property_receipts,
        opaque_representation_selections,
    ) = preparation.into_checking_judgments();
    let equal_prior = prior.filter(|prior| {
        prior.admits_pending_inputs == admits_pending_inputs
            && prior.opaque_property_receipts == opaque_property_receipts
            && *prior.program == program
    });
    let took_prior = equal_prior.is_some();
    let (checked, retained) = match equal_prior {
        Some(prior) => (prior.checked.clone(), retain.then(|| prior.clone())),
        None => {
            let input = retained_input.map(|(preparation_input, preparation_admission)| {
                (
                    preparation_input,
                    preparation_admission,
                    std::sync::Arc::new(program.clone()),
                    opaque_property_receipts.clone(),
                )
            });
            let checked = std::sync::Arc::new(check_program(
                program,
                admits_pending_inputs,
                mathematics,
                opaque_property_receipts,
                opaque_representation_selections,
            )?);
            let retained = input.map(
                |(preparation_input, preparation_admission, program, opaque_property_receipts)| {
                    RetainedCheck {
                        preparation_input,
                        preparation_admission,
                        program,
                        admits_pending_inputs,
                        opaque_property_receipts,
                        checked: checked.clone(),
                    }
                },
            );
            (checked, retained)
        }
    };
    admit_declaration_authority(&checked, request)?;
    Ok(CheckingOutcome {
        checked,
        retained,
        took_prior,
    })
}

/// Package authority is invocation-owned, not part of the language judgment:
/// admit the returned selections under the caller's authority on every path.
fn admit_declaration_authority(
    checked: &CheckedTrees,
    request: &CheckingRequest<'_>,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    if let Some(authority) = request.declaration_authority {
        let descriptor_occurrences =
            validation::execution_projection::descriptor_selection_occurrences(&checked.typed);
        validation::validate_authored_declaration_selections(
            checked,
            authority,
            &descriptor_occurrences,
        )?;
    }
    Ok(())
}

/// The check itself, over a program `lower_typed_trees` has found complete.
fn check_program(
    program: typed_trees::FrozenTypedTrees,
    admits_pending_inputs: bool,
    mathematics: proof::mathematical_declarations::CheckedMathematics,
    opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
    opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    // Borrow the prepared program with invocation-owned query state. Read-only inputs
    // carry this context through the checking flow; no consumer discovers it
    // by thread or address. Metadata publication waits until checking ends.
    let typed_program = program;
    let program = validation::CheckingContext::new(&typed_program)
        .with_opaque_representation_selections(&opaque_representation_selections)
        .with_prepared_mathematics(mathematics.prepared_view());
    validation::validate_selected_attached_method_bounds(&program)?;
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
    // Validation judges each checked body's contract entailment once; the
    // stage-04 prover reads those proved postconditions at its own
    // decision point — nobody rejudges them.
    let proven_machine_contracts =
        std::mem::take(&mut validated.validation_facts.proven_machine_contracts);
    // The range checker bounds indexes with the intervals the same
    // arithmetic walk established; nothing after checking reads them.
    let expression_intervals = std::mem::take(&mut validated.validation_facts.expression_intervals);
    let proof_recipe_obligations = std::mem::take(&mut validated.constant_recipe_obligations);
    let integer_range_obligations =
        std::mem::take(&mut validated.validation_facts.integer_range_obligations);
    let (mut facts, prepared_effects) = build_check_facts(
        &program,
        &validated.proof_plan,
        &validated.integer_assignments,
        validated.operational,
        validated.service_reaches,
        validated.validation_facts,
        &mutation_summaries,
    )?;
    let program = program.with_prepared_effect_plans(&prepared_effects);
    facts.integer_range_obligations = integer_range_obligations;
    // Sealed quotient requests were deferred by validation until termination
    // was proved: judge them now against the checked termination facts. The
    // batch publishes exact admission facts consumed by runtime planning.
    let quotient_operations = validation::admit_checked_quotient_requests(&program, &|machine| {
        facts
            .termination
            .for_machine(machine)
            .map(|plan| plan.checked_summary.clone())
    })?;
    facts
        .quotient_operations
        .extend(quotient_operations.into_iter().map(|operation| {
            checked_trees::CheckedQuotientOperation {
                request: operation.request,
                public_machine: operation.public_machine,
                public_state: operation.public_state,
                representative_state: operation.representative_state,
                correspondence: operation.correspondence,
            }
        }));
    checks::initialize_checked_direct_borrow_resources(&program, &mut facts, &mutation_summaries)?;
    // Compute at the original failure boundary, preserving diagnostic order.
    // Nothing below consumes the specialization commitments. Publish them
    // with the other checked facts, never by rewriting the typed roster.
    let specialization_identities =
        validation::contract_identities::prepare_specialization_contract_identities(
            &program,
            &facts.contract_plans,
            &prepared_effects,
        )?;

    checks::check_checked_facts_recording(
        &program,
        &mut facts,
        &mutation_summaries,
        &incoming_guard_index,
        &proven_machine_contracts,
        &expression_intervals,
    )?;
    crate::quotient_eligibility::check(&program, &facts)?;
    checks::validate_selected_cleanup_guarantees(&program, &facts)?;
    facts
        .constant_recipe_obligations
        .extend(proof_recipe_obligations);
    crate::facts::refresh_realized_contract_envelopes(&mut facts);

    let mut facts = crate::execution::finalize_execution::finalize_execution(&program, facts)?;
    facts.flow.semantic_dependencies =
        crate::flow::derive_checked_semantic_dependencies(&program, &facts);

    crate::authored_selections::bind_checked_intrinsic_call_facts(&program, &mut facts)
        .map_err(|diagnostic| vec![diagnostic])?;
    facts.specialization_identities = specialization_identities;
    // One retained identity row per admission-claim template, specialized or
    // not: consumers read the classification checking computed instead of
    // rerunning operational and service-reach inference for canonical bytes.
    facts.accepted_template_identities = checked_trees::CheckedAcceptedTemplateIdentities::with_rows(
        program
            .machines()
            .iter()
            .filter(|machine| {
                machine.supply_mode == language_semantics::MachineSupplyMode::AdmissionClaim
            })
            .map(|machine| checked_trees::CheckedAcceptedTemplateClassification {
                machine: machine.symbol,
                identity: validation::contract_identities::generic_machine_template_report_fingerprint(
                    &program,
                    machine.symbol,
                )
                .zip(validation::contract_identities::generic_machine_template_commitment(
                    &program,
                    machine.symbol,
                ))
                .map(|(report_fingerprint, commitment)| {
                    checked_trees::CheckedAcceptedTemplateIdentity {
                        report_fingerprint,
                        commitment,
                    }
                }),
            })
            .collect(),
    );
    drop(program);
    let program = typed_program;
    facts.finalized_selections =
        crate::authored_selections::finalize_checked_authored_selections(&program, &mut facts)
            .map_err(|diagnostic| vec![diagnostic])?;
    // Visibility and cleanup consume checked choices over the same immutable
    // source. No cloned typed program or consumer fallback supplies this ledger.
    let finalized = validation::CheckingContext::new(&program)
        .with_authored_declaration_selections(&facts.finalized_selections)
        .with_prepared_mathematics(mathematics.prepared_view())
        .with_prepared_effect_plans(&prepared_effects)
        .with_opaque_representation_selections(&opaque_representation_selections);
    validation::validate_reserved_cleanup_selections(&finalized)?;
    validation::validate_declaration_visibility(&finalized)?;

    // Classify borrow custody last: `program`'s type, machine and state
    // tables stopped growing in preparation, after monomorphization, so a
    // row recorded before preparation would miss
    // the references and specialized states those added. Nothing below
    // takes `program` mutably — `CheckedTrees::with_roots` only pairs it
    // with the facts — so neither table can grow past this point.
    facts.reference_custody =
        validation::reference_result_custody::classify_program_reference_custody(&finalized);
    // Content classification rides the same placement, and for the same
    // reason. Both memos it leans on -- the data-definition index and the
    // attached-`::drop` index -- are owned by `program` and keyed by table
    // revision, so they still answer from cache here.
    facts.content_classes = validation::classify_program_content_classes(&finalized);
    // Retain place/result decisions under final occurrence handles. Recording
    // before specialization or authored-selection finalization would miss the
    // generated bodies; lowering must not repair an incomplete table itself.
    facts.resolved_types = crate::facts::resolved_types::resolve_program_types(&finalized);
    facts.state_locals = crate::facts::state_locals::record_state_locals(&finalized);
    facts.provider_closed_requirements =
        crate::facts::provider_closed_requirements::record_provider_closed_requirements(&finalized);
    drop(finalized);

    // Mathematical terms remained borrowed by every source judgment. Transfer
    // their checked metadata only after the final context has been dropped.
    facts.proof.mathematical_declarations = mathematics.declarations;
    facts.proof.mathematical_signature = mathematics.signature;
    let mut checked = CheckedTrees::with_roots(program, facts);
    validation::validate_boundary_calls(&checked)?;
    target_families::validate_portable_references(&checked)?;
    crate::operators::retain_boundary_body_candidates(&mut checked)
        .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?;
    Ok(checked)
}
