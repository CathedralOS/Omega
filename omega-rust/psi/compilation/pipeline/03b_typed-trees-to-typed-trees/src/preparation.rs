//! Typed-to-typed preparation: settle executable bodies and call identities
//! before checking borrows its immutable input. This owns the specialization
//! fixed point and its required rebinding order; no checker consumer repairs
//! an open typed program. Retained tables are preparation evidence, not a
//! second program representation or permission to skip checking.

mod data_applications;
mod default_domain_zero;
mod deferred_constants;
mod opaque_property_receipts;
mod runtime_index_fields;
mod selections;

pub(crate) use data_applications::elaborate_projected_receivers;

use representation_selections::OpaqueRepresentationSelection;
use typed_trees::TypedTrees;

/// Invocation-owned admission inputs for typed elaboration. Package declaration
/// authority is checked by the following stage, not accepted here.
#[derive(Clone, Copy)]
pub struct PreparationRequest<'request> {
    admits_pending_inputs: bool,
    asm_authority: Option<validation::AsmAuthorityAdmission>,
    opaque_property_receipts: &'request [validation::OpaqueDataPropertyReceipt],
    opaque_representation_selections: &'request [OpaqueRepresentationSelection],
    wire_compatibility_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
}

impl<'request> PreparationRequest<'request> {
    /// Final source admission requires all constant operands to be settled.
    pub const fn settled() -> Self {
        Self {
            admits_pending_inputs: false,
            asm_authority: None,
            opaque_property_receipts: &[],
            opaque_representation_selections: &[],
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

    /// Rejoin the authoritative build's exact opaque applications before
    /// shape admission needs a carrier for a boundary-opaque value.
    pub const fn with_opaque_representation_selections(
        mut self,
        selections: &'request [OpaqueRepresentationSelection],
    ) -> Self {
        self.opaque_representation_selections = selections;
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

/// One request's complete admission inputs in owned form. A caller retaining
/// a language judgment keeps this beside it: preparation is a pure function
/// of the typed program and these inputs, so an equal pair reproduces the
/// exact preparation and the judgment may be taken without preparing again.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparationAdmission {
    admits_pending_inputs: bool,
    asm_authority: Option<validation::AsmAuthorityAdmission>,
    opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
    opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
    wire_compatibility_demands: Vec<language_semantics::wire::WireCompatibilityDemand>,
}

impl PreparationRequest<'_> {
    /// Snapshot every admission input this request carries. The snapshot is
    /// the comparison a retained judgment needs; the borrowed slices stay
    /// construction-time data.
    pub fn admission(&self) -> PreparationAdmission {
        PreparationAdmission {
            admits_pending_inputs: self.admits_pending_inputs,
            asm_authority: self.asm_authority,
            opaque_property_receipts: self.opaque_property_receipts.to_vec(),
            opaque_representation_selections: self.opaque_representation_selections.to_vec(),
            wire_compatibility_demands: self.wire_compatibility_demands.to_vec(),
        }
    }
}

/// Judgments retained while closing the typed program. Its ordinary TypedTrees
/// form is passed separately; this record is not another program representation.
pub struct TypedPreparation {
    admits_pending_inputs: bool,
    mathematics: proof::mathematical_declarations::CheckedMathematics,
    opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
    opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
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
        proof::mathematical_declarations::CheckedMathematics,
        Vec<validation::OpaqueDataPropertyReceipt>,
        Vec<OpaqueRepresentationSelection>,
    ) {
        (
            self.admits_pending_inputs,
            self.mathematics,
            self.opaque_property_receipts,
            self.opaque_representation_selections,
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
    let opaque_representation_selections = request.opaque_representation_selections.to_vec();
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
    // A Binding<R> selects R's callable, not a method on its data carrier.
    // Settle authored service calls before specialization reads their targets.
    // Cloning binds only its newly produced instance after substitution; no
    // final whole-program receiver repair or data-method search is needed.
    validation::resolve_dynamic_call_targets(&mut program)?;
    selections::retain_static_argument_contexts(&mut program);
    // Executable specialization retains each consumed source application;
    // immutable checking judges its requirement -> implementation edge once.
    // Indexed-domain identities participate in static tuple selection. Bind
    // their operation/algebra authority once before deriving instances. Copies
    // retain it and substitute exact type/evidence binders at their producer;
    // no later instance may choose authority for the universal template.
    // Immutable checking, not this derivation, commits template contracts.
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
    crate::operators::bind_selected_operator_calls(
        &mut program,
        &typed_trees::references::DeclarationSelection::all(),
    )?;
    let provider_candidates =
        crate::monomorphization::OperatorCandidateTemplates::candidate_requests(&program);
    let provider_templates = crate::monomorphization::OperatorCandidateTemplates::prepare(
        &program,
        &provider_candidates,
    );
    loop {
        // Close ordinary calls before discovering boundary demands. A bound
        // token or generated candidate can expose another ordinary application,
        // so all rounds return through this same specialization entrypoint.
        // Specialization retains authored machines and only appends instances.
        let first_new_machine = program.machines().len();
        let mut changed_roots = specialize_machine_calls(&mut program, true)?;
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
        // Bind only complete producer outputs: newly cloned bodies and callers
        // whose calls or inferred results changed. The ordinary fixed point
        // settles targets and dependent temporaries before binding reads them;
        // keep that boundary without sweeping every unchanged body.
        for machine in &program.machines()[first_new_machine..] {
            changed_roots.include(machine.symbol);
        }
        materialized +=
            crate::operators::bind_selected_operator_calls(&mut program, &changed_roots)?;
        if materialized == 0 {
            break;
        }
    }
    settle_value_destinations(&mut program)?;
    default_domain_zero::refine_default_domain_zero(&mut program);
    // Constructed result types finish the source roster that field elaboration reads;
    // substituted member edges do not become new application demands.
    crate::constructed_results::retain_constructed_result_types(&mut program);
    data_applications::elaborate(&mut program)?;
    runtime_index_fields::materialize(&mut program)?;
    // A projected receiver reads its elaborated field telescope. Binding it
    // before that producer would borrow the declaration's open field instead
    // of the caller's exact application. This is the ordinary dependency order,
    // not a separate path for nested Binding carriers.
    validation::resolve_dynamic_call_targets(&mut program)?;
    let signature_type_start = program.type_reference_table.type_reference_count();
    crate::monomorphization::instantiate_dynamic_slots(&mut program)?;
    crate::monomorphization::instantiate_requirement_applications(&mut program)?;
    // The public signatures above own newly instantiated carrier occurrences.
    // Complete their field views just as for source types, without revisiting
    // the unchanged source roster or electing a private realization's fields.
    data_applications::elaborate_new_types(&mut program, signature_type_start)?;
    Ok((
        program.into(),
        TypedPreparation {
            admits_pending_inputs: request.admits_pending_inputs,
            mathematics,
            opaque_property_receipts,
            opaque_representation_selections,
        },
    ))
}

/// Calls establish destinations before anonymous values can round. Full source
/// preparation and private constant execution share this ordering; coordinators
/// must not stamp the canonical program to compensate for an unprepared clone.
fn settle_value_destinations(program: &mut TypedTrees) -> Result<(), Vec<diagnostics::Diagnostic>> {
    // Named-machine result overloads are provisionally bound to the first
    // same-named symbol during early resolution. Rebind them now, after domain
    // normalization and destination typing, before validation/backend facts
    // consume the call identity.
    validation::resolve_named_result_overloads(program)?;
    // Float casts select their destination-owned conversion by the cast's
    // result qualification. Derived calls retain the authored Exact target;
    // occurrence checking proves unchanged denotation from live value facts.
    // Bind after named rebinding above, which would otherwise reselect the
    // unqualified overload for a call whose context expects no result domain.
    validation::bind_float_conversion_casts(program)?;
    // Overload rebinding changes existing call targets, and cast elaboration
    // produces exact ordinary calls. Neither introduces token syntax to bind.
    // Land anonymous float values once, against final destinations. Provisional
    // named targets are not authority to round an argument: overload rebinding
    // and conversion elaboration above settle the exact formal types first.
    // Readonly type queries leave anonymous values without a carrier until
    // their destination is known; no provisional default-f64 landing is needed.
    // Both execution engines consume this same rounded tree. Source-suffixed
    // literals keep their declared format and its agreement checks.
    validation::land_float_literal_destinations(program);
    Ok(())
}

/// Bind exact PDI3 operation/algebra authority and refresh every enclosing
/// indexed-domain semantic ID before static tuple derivation. Later copies carry
/// their selected authority and finalize domain identities at their producer.
/// This is the only whole-program finalization: later specialization rounds
/// never rescan unchanged declaration graphs. Immutable checking owns template
/// commitments.
fn normalize_open_index_identities(
    program: &mut typed_trees::TypedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    ::validation::normalize_open_index_expressions(program)?;
    crate::monomorphization::finalize_domain_instance_identities(
        program,
        &typed_trees::references::DeclarationSelection::all(),
    )
    .map_err(|diagnostic| vec![diagnostic])
}

/// Resume canonical constant dependencies by closing complete static tuples.
/// This does not form numeric destinations: pending evaluation is not authority
/// to round the caller's canonical program. Private execution instead enters
/// [`prepare_constant_execution`]; final source admission uses the full stage.
///
/// This speculative route tolerates an incomplete concrete selection: build
/// preparation runs while const endpoint folds are still outstanding, so an
/// underivable tuple is interim evidence, not a rejected program. The
/// authoritative checking pass enforces the complete-tuple gate itself.
pub fn specialize_static_machine_calls(
    program: &mut typed_trees::TypedTrees,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    selections::retain_static_argument_contexts(program);
    specialize_machine_calls(program, false)?;
    // This private preparation can execute before ordinary source checking.
    // Admit its completed static edges once at that execution boundary, never
    // inside the executable derivation loop.
    validation::static_arguments::validate_authored_static_arguments(program)?;
    validation::static_bindings::validate_static_machine_bindings(program)?;
    validation::conformance_applications::validate_conformance_applications(program)?;
    validation::validate_static_machine_selections(program)?;
    validation::validate_generic_machine_contract_entailment(program)
}

/// Form one private constant-execution graph, including its exact call and
/// numeric destinations. Pending tuples remain pending as in static derivation;
/// this is not full source checking or permission to execute them. The semantic
/// evaluation owner still admits each demanded invocation before execution.
pub fn prepare_constant_execution(
    mut program: TypedTrees,
) -> Result<TypedTrees, Vec<diagnostics::Diagnostic>> {
    validation::resolve_dynamic_call_targets(&mut program)?;
    specialize_static_machine_calls(&mut program)?;
    settle_value_destinations(&mut program)?;
    crate::constructed_results::retain_constructed_result_types(&mut program);
    Ok(program)
}

fn specialize_machine_calls(
    program: &mut typed_trees::TypedTrees,
    enforce_complete_concrete_selections: bool,
) -> Result<typed_trees::references::DeclarationSelection, Vec<diagnostics::Diagnostic>> {
    crate::conformance::conformance_application_lifetimes::resolve_elided_conformance_lifetimes(
        program,
    )?;
    crate::conformance::explicit_requirement_calls::route_explicit_requirement_calls(program)?;
    crate::monomorphization::monomorphize_generic_machine_value_calls(
        program,
        enforce_complete_concrete_selections,
    )
}
