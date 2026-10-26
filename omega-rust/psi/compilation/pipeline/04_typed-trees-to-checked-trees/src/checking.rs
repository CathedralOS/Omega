//! Immutable typed-tree checking: validate contracts and ownership, assemble
//! execution plans, then publish checked trees. Preparation has already closed
//! the executable bodies and call identities; this stage never repairs them.
//!
//! `lower_typed_trees` is the one entrance. `TypedPreparation` carries the
//! producer's admission checkpoint and source judgments; `CheckingRequest`
//! carries package declaration authority. Checking cannot change the prepared
//! input's checkpoint. Every production request enforces crash admission.
//!
//! Whole-program reuse has one gate: the source facade compares the original
//! program and complete preparation admission against a [`RetainedCheck`] from
//! this invocation. A miss prepares and checks afresh. Equal prepared trees
//! and Copy receipts do not establish equal checking inputs: selected opaque
//! carriers travel separately and determine structural plans. A second partial
//! match here would silently keep the preceding Build's carrier.
//!
//! Retention keeps that original input beside the shared checked result, not
//! another prepared-program copy. Matching dependency-body entailments remain
//! independently reusable during fresh checking; pending checks supply none.
//! Reuse conveys no authority: preparation admits wire demands, completeness
//! and asm, and the source facade rejoins declaration and placement authority
//! for each request. No process-global memo supplies an implicit judgment.

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
    lower_typed_trees_reusing(program, preparation, None, request, &[], false).map(|outcome| {
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
    admits_pending_inputs: bool,
    checked: std::sync::Arc<CheckedTrees>,
    /// Entailment judgments for non-instance machines in this strict check.
    /// A later dependent in the same invocation may reuse one of them when
    /// the body's correspondence still matches. Pending checks record none.
    dependency_bodies: Vec<validation::DependencyBodyJudgment>,
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

    /// Dependency-body entailment judgments a strict consumer may reuse.
    /// A pending admission does not supply a judgment to anyone.
    pub fn strict_dependency_bodies(&self) -> &[validation::DependencyBodyJudgment] {
        if self.admits_pending_inputs {
            &[]
        } else {
            &self.dependency_bodies
        }
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
    phase_profile::scope("05.e.declaration_authority", || {
        admit_declaration_authority(&checked, request)
    })?;
    Ok(CheckingOutcome {
        checked,
        retained,
        took_prior: true,
    })
}

/// Check this prepared program, reusing only matching dependency-body entailments.
/// Whole-program reuse belongs to the source facade's complete preparation-input
/// match. Retain that original input beside the shared checked result when the
/// caller will ask again; no second prepared-program snapshot or partial
/// admission key is needed.
pub fn lower_typed_trees_reusing(
    program: typed_trees::FrozenTypedTrees,
    preparation: TypedPreparation,
    retained_input: Option<(
        std::sync::Arc<typed_trees::TypedTrees>,
        typed_trees_to_typed_trees::PreparationAdmission,
    )>,
    request: &CheckingRequest<'_>,
    prior_dependency_bodies: &[validation::DependencyBodyJudgment],
    retain: bool,
) -> Result<CheckingOutcome, Vec<diagnostics::Diagnostic>> {
    let (
        admits_pending_inputs,
        mathematics,
        opaque_property_receipts,
        opaque_representation_selections,
    ) = preparation.into_checking_judgments();
    let (checked, dependency_bodies) = check_program(
        program,
        admits_pending_inputs,
        mathematics,
        opaque_property_receipts,
        opaque_representation_selections,
        prior_dependency_bodies,
        retain && !admits_pending_inputs,
    )?;
    let checked = std::sync::Arc::new(checked);
    let retained = retained_input.map(|(preparation_input, preparation_admission)| RetainedCheck {
        preparation_input,
        preparation_admission,
        admits_pending_inputs,
        checked: checked.clone(),
        dependency_bodies,
    });
    phase_profile::scope("05.e.declaration_authority", || {
        admit_declaration_authority(&checked, request)
    })?;
    Ok(CheckingOutcome {
        checked,
        retained,
        took_prior: false,
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
    prior_dependency_bodies: &[validation::DependencyBodyJudgment],
    record_dependency_bodies: bool,
) -> Result<(CheckedTrees, Vec<validation::DependencyBodyJudgment>), Vec<diagnostics::Diagnostic>> {
    // Borrow the prepared program with invocation-owned query state. Read-only inputs
    // carry this context through the checking flow; no consumer discovers it
    // by thread or address. Metadata publication waits until checking ends.
    let typed_program = program;
    let program = validation::CheckingContext::new(&typed_program)
        .with_opaque_representation_selections(&opaque_representation_selections)
        .with_prepared_mathematics(mathematics.prepared_view());
    validation::validate_selected_attached_method_bounds(&program)?;
    let declared_content = checks::prepare_retained_content_custody(
        &program,
        crate::facts::build_content_facts(&program),
    )?;
    let program = program
        .with_retained_borrow_custodies(declared_content.retained_borrow_custodies.clone().into());
    let mut validated = phase_profile::scope("05.d.2.prepare_typed_program", || {
        prepare_typed_program(
            &program,
            &opaque_property_receipts,
            admits_pending_inputs,
            prior_dependency_bodies,
            record_dependency_bodies,
        )
    })?;
    let dependency_body_judgments = std::mem::take(&mut validated.dependency_body_judgments);
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
    // Retain the arithmetic walk's occurrence intervals for value publication
    // and the later range/contract checks. The analysis already hulled every
    // clean evaluation of a shared expression; no initializer is replayed.
    let expression_intervals = std::mem::take(&mut validated.validation_facts.expression_intervals);
    let proof_recipe_obligations = std::mem::take(&mut validated.constant_recipe_obligations);
    let integer_range_obligations =
        std::mem::take(&mut validated.validation_facts.integer_range_obligations);
    let (mut facts, prepared_effects) = phase_profile::scope("05.d.3.build_check_facts", || {
        build_check_facts(
            &program,
            &validated.proof_plan,
            declared_content,
            &validated.integer_assignments,
            validated.operational,
            validated.service_reaches,
            validated.validation_facts,
            &expression_intervals,
            &mutation_summaries,
        )
    })?;
    let program = program.with_prepared_effect_plans(&prepared_effects);
    facts.integer_range_obligations = integer_range_obligations;
    // Sealed quotient requests were deferred by validation until termination
    // was proved: judge them now against the checked termination facts. The
    // batch publishes exact admission facts consumed by runtime planning.
    let mid_guard = phase_profile::guard("05.d.4_6.quotients_borrows_identities");
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
    drop(mid_guard);

    phase_profile::scope("05.d.7.check_checked_facts", || {
        checks::check_checked_facts_recording(
            &program,
            &mut facts,
            &mutation_summaries,
            &incoming_guard_index,
            &proven_machine_contracts,
            &expression_intervals,
        )
    })?;
    let late_guard = phase_profile::guard("05.d.8_10.finalize_execution_identities");
    crate::quotient_eligibility::check(&program, &facts)?;
    checks::validate_selected_cleanup_guarantees(&program, &facts)?;
    validation::validate_runtime_proof_citations(&program, &|machine| {
        facts
            .termination
            .for_machine(machine)
            .map(|plan| plan.checked_summary.clone())
    })?;
    facts
        .constant_recipe_obligations
        .extend(proof_recipe_obligations);
    crate::facts::refresh_realized_contract_envelopes(&mut facts);

    let mut facts = phase_profile::scope("05.d.8_10.a.finalize_execution", || {
        crate::execution::finalize_execution::finalize_execution(&program, facts)
    })?;
    facts.flow.semantic_dependencies =
        phase_profile::scope("05.d.8_10.b.semantic_dependencies", || {
            crate::flow::derive_checked_semantic_dependencies(&program, &facts)
        });

    facts.specialization_identities = specialization_identities;
    // One retained identity row per admission-claim template, specialized or
    // not: consumers read the classification checking computed instead of
    // rerunning operational and service-reach inference for canonical bytes.
    facts.accepted_template_identities = phase_profile::scope(
        "05.d.8_10.c.accepted_template_identities",
        || {
            checked_trees::CheckedAcceptedTemplateIdentities::with_rows(
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
            )
        },
    );
    drop(late_guard);
    let final_guard = phase_profile::guard("05.d.11_13.selections_custody_types_boundaries");
    facts.finalized_selections = phase_profile::scope("05.d.11_13.a.finalize_selections", || {
        crate::authored_selections::finalize_checked_authored_selections(&program, &mut facts)
            .map_err(|diagnostic| vec![diagnostic])
    })?;
    // Visibility and cleanup consume checked choices over the same immutable
    // source. No cloned typed program or consumer fallback supplies this ledger.
    let finalized = program.with_authored_declaration_selections(&facts.finalized_selections);
    phase_profile::scope(
        "05.d.11_13.b.visibility_and_cleanup_selections",
        || -> Result<(), Vec<diagnostics::Diagnostic>> {
            validation::validate_reserved_cleanup_selections(&finalized)?;
            validation::validate_declaration_visibility(&finalized)?;
            Ok(())
        },
    )?;

    // Classify borrow custody last: `program`'s type, machine and state
    // tables stopped growing in preparation, after monomorphization, so a
    // row recorded before preparation would miss
    // the references and specialized states those added. Nothing below
    // takes `program` mutably — `CheckedTrees::with_roots` only pairs it
    // with the facts — so neither table can grow past this point.
    facts.reference_custody = phase_profile::scope("05.d.11_13.c.reference_custody", || {
        validation::reference_result_custody::classify_program_reference_custody(&finalized)
    });
    // Content classification rides the same placement, and for the same
    // reason. Both memos it leans on -- the data-definition index and the
    // attached-`::drop` index -- are owned by `program` and keyed by table
    // revision, so they still answer from cache here.
    facts.content_classes = phase_profile::scope("05.d.11_13.d.content_classes", || {
        validation::classify_program_content_classes(&finalized)
    });
    // Retain place/result decisions under final occurrence handles. Recording
    // before specialization or authored-selection finalization would miss the
    // generated bodies; lowering must not repair an incomplete table itself.
    facts.resolved_types = phase_profile::scope("05.d.11_13.e.resolved_types", || {
        crate::facts::resolved_types::resolve_program_types(&finalized)
    });
    facts.state_locals = phase_profile::scope("05.d.11_13.f.state_locals", || {
        crate::facts::state_locals::record_state_locals(&finalized)
    });
    facts.provider_closed_requirements =
        phase_profile::scope("05.d.11_13.g.provider_closed_requirements", || {
            crate::facts::provider_closed_requirements::record_provider_closed_requirements(
                &finalized,
            )
        });
    drop(finalized);

    // Mathematical terms remained borrowed by every source judgment. Transfer
    // their checked metadata only after the final context has been dropped.
    facts.proof.mathematical_declarations = mathematics.declarations;
    facts.proof.source_evidence_carriers = mathematics
        .applications
        .source_carriers
        .iter()
        .map(|(_, carrier)| checked_trees::CheckedSourceEvidenceCarrier {
            type_identity: carrier.type_identity,
            ty: carrier.ty,
            domains: carrier.domains.clone(),
        })
        .collect();
    for (_, theorem) in mathematics.applications.indexed_theorems.iter() {
        let lost = || {
            vec![diagnostics::Diagnostic::error(
                "indexed theorem lost its exact authored contract association",
            )]
        };
        let machine = typed_program.machine(theorem.machine).ok_or_else(lost)?;
        let exact_clause = typed_program
            .machine_contracts(machine)
            .iter()
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Ensures
            })
            .flat_map(|contract| {
                typed_program
                    .proof_facts
                    .span_or_empty(contract.facts)
                    .iter()
            })
            .filter_map(|fact| match fact {
                typed_trees::domain::ProofFact::Expression(expression) => Some(*expression),
                _ => None,
            })
            .nth(theorem.ensure_ordinal as usize);
        if exact_clause != Some(theorem.expression) {
            return Err(lost());
        }
        let entry = typed_program
            .machine_states(machine)
            .first()
            .ok_or_else(lost)?;
        let (position, formal) = typed_program
            .state_parameters(entry)
            .iter()
            .enumerate()
            .find(|(_, formal)| formal.symbol == theorem.evidence_formal)
            .ok_or_else(lost)?;
        let type_identity = typed_program.normalized_type_identity(theorem.source_type);
        if !formal.relevance.is_erased()
            || typed_program.normalized_type_identity(formal.type_reference) != type_identity
        {
            return Err(lost());
        }
        facts
            .proof
            .indexed_theorems
            .push(checked_trees::CheckedIndexedTheorem {
                machine: theorem.machine,
                expression: theorem.expression,
                ensure_ordinal: theorem.ensure_ordinal,
                evidence_source_position: u32::try_from(position).map_err(|_| lost())?,
                evidence_formal: theorem.evidence_formal,
                type_identity,
                term: theorem.theorem,
                ty: theorem.ty,
                assumption_closure: theorem.assumption_closure.clone(),
            });
    }
    facts.proof.mathematical_safety_assumptions = mathematics.applications.safety_assumptions;
    facts.proof.mathematical_signature = mathematics.signature;
    let mut checked = CheckedTrees::with_roots(typed_program, facts);
    phase_profile::scope(
        "05.d.11_13.h.boundary_and_portable_checks",
        || -> Result<(), Vec<diagnostics::Diagnostic>> {
            validation::validate_boundary_calls(&checked)?;
            target_families::validate_portable_references(&checked)?;
            crate::operators::retain_boundary_body_candidates(&mut checked)
                .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?;
            Ok(())
        },
    )?;
    drop(final_guard);
    Ok((checked, dependency_body_judgments))
}
