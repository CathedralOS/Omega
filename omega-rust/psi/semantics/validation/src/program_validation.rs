//! Validate typed programs and retain the analyses consumed by checking.

use crate::declarations::state_signatures::validate_callable_state_signatures;
use crate::declarations::state_signatures::validate_machine_contracts;
use crate::declarations::symbols::MachineSymbols;
use crate::declarations::traits::validate_conformances;
use crate::declarations::traits::validate_external_leaf_native_shapes;
use crate::declarations::traits::validate_external_via_expression;
use crate::declarations::traits::validate_generic_conformance_bounds;
use crate::declarations::traits::validate_machine_trait_conformances;
use crate::declarations::traits::validate_trait_conformance_bounds;
use crate::declarations::traits::validate_trait_requirements;
use crate::machine_calls::calls::validate_proof_machine_recursion;
use crate::machine_calls::calls::validate_self_recursive_call_positions;
use crate::machine_calls::calls::validate_value_position_calls;
use crate::machine_calls::machine_data::validate_owned_data;
use crate::proof_contracts::contract_entailment::validate_machine_contract_entailment;
use crate::proof_contracts::domains::validate_domain_definitions;
use crate::proof_contracts::proof_facts::validate_proposition_definitions;
use crate::value_custody::data::validate_data_field_types;
use crate::value_custody::locals::WritableRoots;
use crate::value_custody::locals::validate_local_data_names;
use crate::{
    OpaqueDataPropertyReceipt, TopLevelSymbols, ValidatedBoundaryOperatorApplication,
    ValidatedFloatMeaningEqualityProposition, ValidatedFloatMeaningProjectionInvocation,
    ValidatedIntegerEmbeddingCall, ValidatedProofRecursiveComponent, build_definition_fact_plan,
    collect_dynamic_conformance_selections, declarations::declaration_visibility,
    declarations::operators, declarations::transitions, infer_operational_may,
    infer_service_reaches, machine_calls::call_cycles, machine_calls::callable_overloads,
    machine_calls::calls, machine_calls::fact_call_projections, machine_calls::invocations,
    machine_calls::machine_parameters, proof_contracts::arithmetic_domains,
    proof_contracts::default_domains, proof_contracts::float_projection_bindings,
    proof_contracts::float_projection_invocations, proof_contracts::proof_embeddings,
    proof_contracts::proof_only_faces, proof_contracts::properties,
    proof_contracts::proposition_entailment, proof_contracts::qualification_evidence,
    proof_contracts::quotients, proof_contracts::relevance, value_custody::cleanup,
    value_custody::constants, value_custody::content_conservation,
    value_custody::content_projections, value_custody::destructure, value_custody::literals,
    value_custody::placed_views, value_custody::plan_laid, value_custody::recasts,
    value_custody::struct_literals, value_custody::wire, value_custody::write_only_borrows,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementNode;

mod contract_queries;
mod statements;
pub use contract_queries::{
    ContractEntailmentStandDown, ContractEntailmentStandDownReason,
    checked_operator_contract_snapshot, collect_contract_entailment_stand_downs,
    proven_machine_contract_expressions, validate_checked_operator_realization_contract,
    validate_generic_machine_contract_entailment,
};
use statements::{is_exact_executable_drop_body, validate_state_statement_node};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactIntegerCastFact {
    pub expression: ExpressionHandle,
    pub source_type: typed_trees::types::PrimitiveType,
    pub target_type: typed_trees::types::PrimitiveType,
    pub minimum: numerics::bignum::BigInt,
    pub maximum: numerics::bignum::BigInt,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramValidationFacts {
    pub boundary_operator_applications: Vec<ValidatedBoundaryOperatorApplication>,
    pub exact_integer_casts: Vec<ExactIntegerCastFact>,
    pub float_meaning_projection_invocations: Vec<ValidatedFloatMeaningProjectionInvocation>,
    pub float_meaning_equality_propositions: Vec<ValidatedFloatMeaningEqualityProposition>,
    pub fact_call_projections: Vec<fact_call_projections::ValidatedFactCallProjection>,
    pub integer_embedding_calls: Vec<ValidatedIntegerEmbeddingCall>,
    /// Canonical proof-only call SCCs accepted by the structural-subterm
    /// validator. Every exact internal call site retains its own witness.
    pub proof_recursive_components: Vec<ValidatedProofRecursiveComponent>,
}

/// Whether opaque property claims must be backed now or remain pending during
/// preliminary build checking. Pending evidence is not final admission.
#[derive(Debug, Clone, Copy)]
pub enum OpaquePropertyValidation<'a> {
    Required(&'a [OpaqueDataPropertyReceipt]),
    PendingBuildSelection,
}

/// Results computed against one immutable typed program. Later stages consume
/// these same analyses; this carrier is not Terminal verification authority.
#[derive(Debug)]
pub struct ProgramValidation {
    pub facts: ProgramValidationFacts,
    pub operational: flow_effects::OperationalPlan,
    pub service_reaches: flow_effects::ServiceReachInferencePlan,
}

/// Validate a standalone program, including its generic contracts.
pub fn validate_program(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    validate(
        program,
        GenericContracts::Check,
        OpaquePropertyValidation::Required(&[]),
    )
    .map(|_| ())
}

/// Validate the concrete graph after generic contracts were checked before
/// specialization. Only those already-checked universal entailments are skipped;
/// all other checks run on this exact graph, including opaque property admission.
pub fn validate_specialized_program(
    program: &TypedTrees,
    opaque_properties: OpaquePropertyValidation<'_>,
) -> Result<ProgramValidation, Vec<Diagnostic>> {
    validate(program, GenericContracts::Prevalidated, opaque_properties)
}

#[derive(Clone, Copy)]
enum GenericContracts {
    Check,
    Prevalidated,
}

fn validate(
    program: &TypedTrees,
    generic_contracts: GenericContracts,
    opaque_properties: OpaquePropertyValidation<'_>,
) -> Result<ProgramValidation, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    literals::validate_anonymous_remainders(program, &mut diagnostics);
    literals::validate_anonymous_divisions(program, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mut boundary_operator_applications = Vec::new();
    let mut exact_integer_casts = Vec::new();
    callable_overloads::validate_named_callable_overload_declarations(program, &mut diagnostics);
    float_projection_bindings::validate_float_projection_operator_bindings(
        program,
        &mut diagnostics,
    );
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    let fact_plan = build_definition_fact_plan(program);

    declaration_visibility::collect_declaration_visibility_diagnostics(program, &mut diagnostics);
    cleanup::collect_reserved_cleanup_selection_diagnostics(program, &mut diagnostics);
    validate_proposition_definitions(program, &mut diagnostics);
    proposition_entailment::validate_proposition_entailment(program, &mut diagnostics);

    literals::validate_literal_widths(program, &mut diagnostics);
    literals::validate_suffix_landings(program, &mut diagnostics);
    literals::validate_suffix_magnitudes(program, &mut diagnostics);
    validate_domain_definitions(program, &symbols, &fact_plan, &mut diagnostics);
    validate_callable_state_signatures(program, &symbols, &mut diagnostics);
    write_only_borrows::validate_checked_write_only_slice(program, &mut diagnostics);
    arithmetic_domains::validate_abstract_total_specification_arithmetic(program, &mut diagnostics);
    cleanup::validate_cleanup_machine_declarations(program, &mut diagnostics);
    validate_trait_requirements(program, &symbols, &mut diagnostics);
    for trait_definition in program.traits() {
        validate_trait_conformance_bounds(program, trait_definition, &mut diagnostics);
    }
    content_projections::validate_content_projection_conformances(program, &mut diagnostics);
    content_conservation::validate_content_conservation_contracts(program, &mut diagnostics);
    qualification_evidence::validate_qualification_authorization(program, &mut diagnostics);
    let conformance_operational = infer_operational_may(program);
    let conformance_service_reaches = infer_service_reaches(program, &conformance_operational);
    validate_conformances(program, &conformance_service_reaches, &mut diagnostics);
    if let Err(mut dynamic_diagnostics) = collect_dynamic_conformance_selections(program) {
        diagnostics.append(&mut dynamic_diagnostics);
    }
    validate_data_field_types(program, &symbols, &mut diagnostics);
    plan_laid::validate_plans(program, &mut diagnostics);
    placed_views::validate_plans(program, &mut diagnostics);
    relevance::validate_relevance(program, &mut diagnostics);
    // Math roster N1: recursive data is legal and PROOF-ONLY (computed, never
    // spelled); every runtime consumption face refuses with the
    // classification named.
    let proof_only = typed_trees::proof_only::classify(program);
    proof_embeddings::validate_proof_embeddings(program, &proof_only, &mut diagnostics);
    let integer_embedding_calls =
        proof_embeddings::validate_integer_embedding_calls(program, &mut diagnostics);
    quotients::validate_quotients(program, &proof_only, &mut diagnostics);
    let fact_call_projections =
        fact_call_projections::validate_fact_call_projections(program, &mut diagnostics);
    proof_only_faces::validate_proof_only_consumption(program, &proof_only, &mut diagnostics);
    // Chapter 3 / MR4: runtime call cycles require the constant-stack tail
    // admission; erased proof-only SCCs instead require strict structural descent.
    let proof_recursive_components =
        call_cycles::validate_machine_call_cycles(program, &symbols, &mut diagnostics);
    let (opaque_property_receipts, allow_pending_opaque_copy) = match opaque_properties {
        OpaquePropertyValidation::Required(receipts) => (receipts, false),
        OpaquePropertyValidation::PendingBuildSelection => (&[][..], true),
    };
    properties::validate_data_properties(
        program,
        &symbols,
        opaque_property_receipts,
        allow_pending_opaque_copy,
        &mut diagnostics,
    );
    constants::validate_constants(program, &mut diagnostics);
    // Bare-payload-case `==` (decision 11) is checked on the RESOLVED trees,
    // before membership lowering synthesizes its internal tag compares; see
    // symbol-resolved-trees-to-typed-trees/src/equality.rs.
    struct_literals::validate_struct_literal_fields(program, &mut diagnostics);
    // Record patterns in LET (owner spec 2026-07-18): the exhaustiveness
    // law on the parse-minted `__destructure#*` marker.
    destructure::validate_destructure_exhaustiveness(program, &mut diagnostics);
    // R2 rung 3 slice 1: the default-domain write obligation (strict
    // store-time semantics; obligations before hypotheses).
    default_domains::validate_default_domain_writes(program, &mut diagnostics);
    recasts::validate_recasts(program, &mut diagnostics);
    wire::validate_wire_schemas(program, &symbols, &mut diagnostics);
    operators::validate_operator_declarations(program, &symbols, &mut diagnostics);
    machine_parameters::validate_static_machine_arguments(program, &mut diagnostics);
    invocations::validate_invocation_contracts(program, &mut diagnostics);

    let call_frames = calls::CallFrameResolver::new(program);
    for machine in program.machines() {
        let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);

        // Besides the established empty body, the executable cleanup slice
        // admits a finite nonempty source-ordered list of ordinary zero-argument
        // calls to mutually distinct exact-empty attached helpers.
        if machine.name.as_str().ends_with("::drop")
            && program.machine_states(machine).iter().any(|state| {
                !program
                    .statement_table
                    .statements(state.statement_nodes)
                    .is_empty()
            })
            && !is_exact_executable_drop_body(program, machine)
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` has a non-empty `drop` body outside the executable cleanup slice. Keep the body empty, or use a finite nonempty source-ordered list of ordinary zero-argument calls to mutually distinct empty attached helpers.",
                machine.name,
            )));
        }

        validate_owned_data(program, machine, &symbols, &mut diagnostics);
        validate_generic_conformance_bounds(program, machine, &mut diagnostics);
        validate_machine_contracts(program, machine, &mut diagnostics);
        arithmetic_domains::validate_machine_total_specification_arithmetic(
            program,
            machine,
            &mut diagnostics,
        );
        let generic_contract_was_prevalidated =
            matches!(generic_contracts, GenericContracts::Prevalidated)
                && (program
                    .machine_type_parameters(machine)
                    .iter()
                    .any(|parameter| {
                        matches!(
                            parameter.kind,
                            typed_trees::data::TypeParameterKind::Machine { .. }
                        )
                    })
                    || program
                        .machine_specializations
                        .iter()
                        .any(|specialization| {
                            !specialization.machine_arguments.is_empty()
                                && (specialization.template == machine.symbol
                                    || specialization.instance == machine.symbol)
                        }));
        if !generic_contract_was_prevalidated {
            validate_machine_contract_entailment(program, machine, &mut diagnostics);
        }
        validate_machine_trait_conformances(
            program,
            &conformance_service_reaches,
            machine,
            &symbols,
            &mut diagnostics,
        );

        // PRV4 step 1: a `via <Binding>` clause is the EXTERNAL LEAF's
        // realization -- it must never parse and then silently drop. Exactly
        // one via clause, on a bodyless non-boundary machine, populates
        // ExternalRealization; every other carrier refuses here.
        {
            let conformances = program.machine_trait_conformances(machine);
            let via_count = conformances
                .iter()
                .filter(|conformance| {
                    conformance.external_binding.is_some() || conformance.via_expression.is_valid()
                })
                .count();
            for conformance in conformances {
                if conformance.external_binding.is_some() && conformance.via_expression.is_valid() {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` retains both a bootstrap binding and an ordinary `via` expression on one conformance",
                        machine.name,
                    )));
                }
                validate_external_via_expression(program, machine, conformance, &mut diagnostics);
            }
            let is_external = matches!(
                machine.supply_mode,
                language_semantics::MachineSupplyMode::ExternalRealization { .. }
            );
            if via_count > 1 {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` carries {via_count} `via` bindings; an external \
                     leaf has exactly one realization",
                    machine.name,
                )));
            } else if via_count == 1 && !is_external {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` carries a `via` binding but is not an external \
                     leaf: `satisfies Requirement via <Binding>;` belongs on a \
                     BODYLESS non-boundary machine (a composite lowering is an \
                     ordinary checked body; an accepted axiom carries no \
                     realization)",
                    machine.name,
                )));
            }
            if via_count == 1 && is_external {
                validate_external_leaf_native_shapes(program, machine, &mut diagnostics);
            }
        }

        // Arrival analysis covers the entire immutable machine. State-local
        // statement validation borrows that batch and mutates only its clone.
        let mut incoming_environments = None;
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            // A state that DECLARES a return type but has an EMPTY body can
            // never produce the value -- callers would silently bind 0 (ZII),
            // and native/interp diverge on what that zero reads as. GENERIC
            // machines are exempt: the core/std container surface (`machine
            // Vec::as_slice<T>(&self) -> &[T] { }`) is deliberately
            // type-check-only, and value calls to generics are already fenced
            // (fence_generic_value_callee).
            if state.return_type.is_valid()
                && program.machine_type_parameters(machine).is_empty()
                // A SPECIALIZED generic keeps its declaration symbol while
                // MP4 substitution consumes its type parameters in place --
                // it inherits the generic exemption (the core container
                // surface is type-check-only; value calls stay fenced). The
                // exemption follows the specialization either way: the first
                // instance may reuse the template symbol, later instances are
                // fresh clones.
                && !program.machine_specializations.iter().any(|specialization| {
                    specialization.template == machine.symbol
                        || specialization.instance == machine.symbol
                })
                // Bodyless boundary declarations have no Omega body by
                // design. ACCEPTED declarations mean their ensures through
                // the trust carrier; claim-free BOUNDARY declarations merely
                // introduce a symbol and assert nothing.
                && !matches!(
                    machine.supply_mode,
                    language_semantics::MachineSupplyMode::AdmissionClaim
                        | language_semantics::MachineSupplyMode::TopLevelRequirement
                        | language_semantics::MachineSupplyMode::Boundary
                )
                // PRV4: an EXTERNAL LEAF's body IS its binding -- the
                // realization produces the value at the seam.
                && !matches!(
                    machine.supply_mode,
                    language_semantics::MachineSupplyMode::ExternalRealization { .. }
                )
                && program
                    .statement_table
                    .statements(state.statement_nodes)
                    .is_empty()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{}` declares a return type but its body is \
                     empty -- it can never produce the value (callers would silently \
                     bind 0). Return a value or drop the `-> T`.",
                    machine.name,
                    state.name.as_str(),
                )));
            }
            validate_local_data_names(
                program.statement_table.statements(state.statement_nodes),
                &machine_symbols,
                program.state_parameters(state),
                machine.name.as_str(),
                state.name.as_str(),
                &mut diagnostics,
            );
            // S4: a per-state-body value environment tracks each place's proven
            // interval along the straight-line prefix, so the exact-overflow proof
            // can use actual values (`self.v = 10; self.v += 5`) instead of the
            // full type range. The ENTRY state (first) is pre-seeded with the
            // machine's `requires` bounds on its parameters (`requires amount <=
            // 100`), so bounded param arithmetic stays exact. Statements are
            // validated in order so the env is current at each use.
            let mut value_env = if state_index == 0 {
                arithmetic_domains::requires_value_env(program, machine, state)
            } else {
                // A non-entry state may assume the facts established by every
                // incoming guarded transition. Multiple predecessors join at
                // their common facts and widest admitted interval; call,
                // continuation, and self-loop entries remain conservative
                // fences.
                incoming_environments
                    .get_or_insert_with(|| {
                        arithmetic_domains::incoming_guard_environments(program, machine)
                    })
                    .iter()
                    .find_map(|(symbol, environment)| {
                        (*symbol == state.symbol).then(|| environment.clone())
                    })
                    .unwrap_or_default()
            };
            for (statement_index, (statement_handle, statement)) in program
                .statement_table
                .iter_statements(state.statement_nodes)
                .enumerate()
            {
                let writable_roots = WritableRoots {
                    program,
                    machine,
                    machine_symbols: &machine_symbols,
                    statements: &program.statement_table.statements(state.statement_nodes)
                        [..statement_index],
                    parameters: program.state_parameters(state),
                };
                let transition_values = transitions::TransitionValueEnvironments::collect(
                    program,
                    machine,
                    state,
                    statement,
                    &value_env,
                    call_frames.as_ref(),
                );
                // R5 value-call frame: conservatively apply the aggregate
                // may-write set of every call nested in this statement before
                // checking general value uses. Transition arguments retain
                // their individual evaluation-point environments above. Other
                // uses give up evaluation-order precision within one expression
                // rather than carry pre-call facts across a mutating call. A
                // call-free expression reports an empty frame; any unresolved
                // call fails closed and clears the environment.
                let value_written = call_frames.as_ref().and_then(|frames| {
                    frames.statement_value_may_write_paths_with_symbols(
                        machine,
                        &machine_symbols,
                        statement,
                    )
                });
                if let Some(written) = value_written {
                    value_env.invalidate_written_paths(&written);
                } else {
                    value_env.clear();
                }
                // VALUE-position calls inside this statement's expression trees
                // (LocalData initializers, transition arguments, guard subjects,
                // etc.) are not reached by `validate_state_statement_node`; run
                // their bound + argument checks here, before the statement records
                // its own writes. The flow-sensitive environment has already
                // crossed every nested value-call frame above, so no argument can
                // rely on a fact an earlier or opaque call may invalidate.
                if !content_projections::is_content_projection_machine(program, machine) {
                    validate_value_position_calls(
                        program,
                        machine,
                        state,
                        statement,
                        &machine_symbols,
                        &symbols,
                        &writable_roots,
                        &value_env,
                        &transition_values,
                        &mut boundary_operator_applications,
                        &mut diagnostics,
                    );
                }
                // PROOF MACHINES (free machines over proof-only data) are
                // exempt from the tail-only rule: they emit no runtime code,
                // so there is no frame to survive a non-tail call --
                // structural recursion (`Succ { prev: double(prev) }`) is
                // the induction the measure licenses. What still applies is
                // the measure itself: every self-call must structurally
                // descend (N2d gateway).
                if proof_only.is_proof_machine(program, machine) {
                    validate_proof_machine_recursion(
                        program,
                        machine,
                        state,
                        statement,
                        &mut diagnostics,
                    );
                } else {
                    validate_self_recursive_call_positions(
                        program,
                        machine,
                        state,
                        statement,
                        &mut diagnostics,
                    );
                }
                let direct_written = match statement {
                    StatementNode::Call(call) => call_frames
                        .as_ref()
                        .and_then(|frames| frames.may_write_paths(machine, call)),
                    StatementNode::Assignment(_) => call_frames.as_ref().and_then(|frames| {
                        frames
                            .assignment_write_frame(machine, statement)
                            .into_complete_paths()
                    }),
                    _ => None,
                };
                validate_state_statement_node(
                    program,
                    machine,
                    &state.name,
                    Some(state),
                    &machine_symbols,
                    &symbols,
                    &writable_roots,
                    statement_handle,
                    statement,
                    &mut value_env,
                    &transition_values,
                    direct_written,
                    &mut exact_integer_casts,
                    &mut boundary_operator_applications,
                    &mut diagnostics,
                );
            }
        }
    }

    finish_diagnostics(diagnostics)?;
    // Warning collection follows all formation and flow checks: a warning must
    // not make a clean assignment skip its range or value-environment update.
    finish_diagnostics(literals::anonymous_integer_landing_warnings(program))?;
    exact_integer_casts
        .sort_by_key(|fact| (fact.expression.arena_index(), fact.expression.generation()));
    exact_integer_casts.dedup_by(|right, left| {
        if left.expression != right.expression {
            return false;
        }
        left.minimum = left.minimum.clone().min(right.minimum.clone());
        left.maximum = left.maximum.clone().max(right.maximum.clone());
        true
    });
    let (float_meaning_projection_invocations, float_meaning_equality_propositions) =
        float_projection_invocations::collect_float_meaning_projection_invocations(program)?;
    Ok(ProgramValidation {
        operational: conformance_operational,
        service_reaches: conformance_service_reaches,
        facts: ProgramValidationFacts {
            boundary_operator_applications,
            exact_integer_casts,
            float_meaning_projection_invocations,
            float_meaning_equality_propositions,
            fact_call_projections,
            integer_embedding_calls,
            proof_recursive_components,
        },
    })
}

/// Errors fail the build; a WARNING-only batch surfaces on stderr and
/// passes (the Decision-12 relaxation: uniform compilation, deadness
/// outside proofs warns). stderr is the v1 warning channel -- report
/// integration is recorded in TASKS.md.
pub(crate) fn finish_diagnostics(diagnostics: Vec<Diagnostic>) -> Result<(), Vec<Diagnostic>> {
    if diagnostics.iter().any(Diagnostic::is_error) {
        return Err(diagnostics);
    }
    for warning in &diagnostics {
        eprintln!("{warning}");
    }
    Ok(())
}
