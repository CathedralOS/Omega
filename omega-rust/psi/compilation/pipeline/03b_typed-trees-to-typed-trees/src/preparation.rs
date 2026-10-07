//! Typed-to-typed preparation: settle executable bodies and call identities
//! before checking borrows its immutable input. This owns the specialization
//! fixed point and its required rebinding order; no checker consumer repairs
//! an open typed program. Retained tables are preparation evidence, not a
//! second program representation or permission to skip checking.

mod default_domain_zero;
mod deferred_constants;
mod opaque_property_receipts;
mod selections;

use typed_trees::TypedTrees;

/// Invocation-owned admission inputs for typed elaboration. Package declaration
/// authority is checked by the following stage, not accepted here.
#[derive(Clone, Copy)]
pub struct PreparationRequest<'request> {
    admits_pending_inputs: bool,
    asm_authority: Option<validation::AsmAuthorityAdmission>,
    opaque_property_receipts: &'request [validation::OpaqueDataPropertyReceipt],
    wire_compatibility_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
}

impl<'request> PreparationRequest<'request> {
    /// Final source admission requires all constant operands to be settled.
    pub const fn settled() -> Self {
        Self {
            admits_pending_inputs: false,
            asm_authority: None,
            opaque_property_receipts: &[],
            wire_compatibility_demands: &[],
        }
    }

    /// Private compile-time probes can retain pending source obligations.
    pub const fn preliminary() -> Self {
        Self {
            admits_pending_inputs: true,
            ..Self::settled()
        }
    }

    /// Admit privileged instructions against this invocation's evaluated Build.
    pub const fn with_asm_authority(
        mut self,
        authority: validation::AsmAuthorityAdmission,
    ) -> Self {
        self.asm_authority = Some(authority);
        self
    }

    /// Rejoin independently rederived receipts before specialization grows arenas.
    pub const fn with_opaque_property_receipts(
        mut self,
        receipts: &'request [validation::OpaqueDataPropertyReceipt],
    ) -> Self {
        self.opaque_property_receipts = receipts;
        self
    }

    /// Check authored wire demands independently on every invocation.
    pub const fn with_wire_compatibility_demands(
        mut self,
        demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    ) -> Self {
        self.wire_compatibility_demands = demands;
        self
    }
}

/// Judgments retained while closing the typed program. Its ordinary TypedTrees
/// form is passed separately; this record is not another program representation.
pub struct TypedPreparation {
    admits_pending_inputs: bool,
    static_machine_selections: validation::ValidatedStaticMachineSelections,
    mathematics: proof::mathematical_declarations::CheckedMathematics,
    opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
}

impl TypedPreparation {
    /// Transfer the producer's immutable judgments and admission checkpoint.
    /// Only preparation constructs this sidecar; callers cannot replace its
    /// receipts or change the checkpoint independently of source admission.
    /// This is an internal pipeline handoff, not foreign evidence admission.
    pub fn into_checking_judgments(
        self,
    ) -> (
        bool,
        validation::ValidatedStaticMachineSelections,
        proof::mathematical_declarations::CheckedMathematics,
        Vec<validation::OpaqueDataPropertyReceipt>,
    ) {
        (
            self.admits_pending_inputs,
            self.static_machine_selections,
            self.mathematics,
            self.opaque_property_receipts,
        )
    }
}

/// Complete typed bodies before their immutable checking boundary. Input
/// admission keeps diagnostic order; source receipts join before arena growth.
pub fn prepare_typed_trees(
    mut program: TypedTrees,
    request: &PreparationRequest<'_>,
) -> Result<(typed_trees::FrozenTypedTrees, TypedPreparation), Vec<diagnostics::Diagnostic>> {
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
    validation::validate_wire_protocol(&program, request.wire_compatibility_demands)?;
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
    // Mathematical `let`/`boundary let` declarations elaborate into checked
    // records and a kernel signature the kernel re-decides. Check them first,
    // so a declaration that fails either elaboration fails with its own
    // diagnostic, and carry the result to the proof facts.
    let mathematics = proof::mathematical_declarations::check_mathematical_declarations(&program)?;
    // Stage-1 machine monomorphization MUST precede validation: a generic
    // machine whose value calls agree on one instantiation is substituted to a
    // concrete machine here. Validation permits unused template bodies but the
    // generic-value-call fence still rejects any emitted concrete caller whose
    // callee remains generic (an incomplete specialization).
    // Projected statement receivers retain their lexical root before their
    // field endpoint is known. Resolve both call forms before specialization
    // and effect inference, not only when finalizing authored selections.
    crate::receivers::resolve_projected_receiver_calls(&mut program)?;
    // MP2b must judge the authored requirement -> selected implementation edge
    // before MP4 consumes the call-site selections and clears the template's
    // parameter list.
    // PDI3 operation/algebra authority is part of the authored generic
    // contract, so bind it before specialization captures the universal
    // template fingerprint. Normalize again afterward to cover cloned
    // expression handles and concrete substitutions.
    selections::bind_pre_specialization_authored_selections(&mut program)
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
        static_machine_selections = specialize_generated_static_machine_calls(&mut program)?;
        normalize_open_index_identities(&mut program)?;
    }
    if generated_bodies {
        validation::conformance_applications::validate_conformance_applications(&program)?;
    }
    // F2b: unsuffixed float literals at declared f32/f64 destinations land
    // their format on the text carrier HERE, while the tree is still mutable
    // and before both engines fork off it -- every downstream read (native
    // and interpreter) then rounds once from the spelling.
    validation::land_float_literal_destinations(&mut program);
    // Calls through a `Binding<R>` service receiver cannot be resolved during
    // the earlier symbol pass because local declared types are not available
    // there. Bind their declaring-trait requirement now so the ordinary
    // result-overload pass below starts in the correct trait family rather
    // than from an ambient same-named machine.
    validation::resolve_dynamic_call_targets(&mut program)?;
    // Concrete substitutions may make previously open field types selectable.
    crate::receivers::resolve_projected_receiver_calls(&mut program)?;
    // Named-machine result overloads are provisionally bound to the first
    // same-named symbol during early resolution. Rebind them now, after domain
    // normalization and destination typing, before validation/backend facts
    // consume the call identity.
    validation::resolve_named_result_overloads(&mut program)?;
    // Float casts select their destination-owned conversion by the cast's
    // result qualification. Derived calls retain the authored Exact target;
    // occurrence checking proves unchanged denotation from live value facts.
    // Bind after named rebinding above, which would otherwise reselect the
    // unqualified overload for a call whose context expects no result domain.
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
    // Every selection and selector call is now final: instantiate each
    // reached requirement under its selector application and retarget the
    // calls to those slots before validation judges the call edges.
    crate::monomorphization::instantiate_dynamic_slots(&mut program)?;
    Ok((
        program.into(),
        TypedPreparation {
            admits_pending_inputs: request.admits_pending_inputs,
            static_machine_selections,
            mathematics,
            opaque_property_receipts,
        },
    ))
}

/// Bind exact PDI3 operation/algebra authority and refresh every enclosing
/// indexed-domain semantic ID. Orchestration calls this before typed
/// snapshots and trust receipts; checked lowering calls it before capturing
/// generic template fingerprints and again after specialization.
fn normalize_open_index_identities(
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
    crate::conformance::explicit_requirement_calls::route_explicit_requirement_calls(program)?;
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
    validation::conformance_applications::validate_conformance_applications(program)?;
    crate::conformance::explicit_requirement_calls::route_explicit_requirement_calls(program)?;
    let mut selections = ::validation::validate_static_machine_selections_with_facts(program)?;
    ::validation::validate_generic_machine_contract_entailment(program)?;
    crate::monomorphization::monomorphize_generic_machine_value_calls_with_selections(
        program,
        &mut selections,
        enforce_complete_concrete_selections,
    )?;
    Ok(selections)
}
