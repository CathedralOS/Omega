mod arithmetic_domains;
mod call_cycles;
mod callable_overloads;
mod calls;
mod cleanup;
mod content_conservation;
mod content_projections;
mod contract_entailment;
mod data;
mod declaration_visibility;
mod default_domains;
mod denotational_calls;
mod destructure;
mod domain_weakening;
mod domains;
mod effects;
mod expression_types;
mod fact_call_projections;
mod float_projection_bindings;
mod float_projection_invocations;
mod invocations;
mod literals;
mod locals;
mod machine_data;
mod machine_parameters;
mod machine_specialization_identity;
mod operators;
mod placed_views;
mod places;
mod plan_laid;
mod proof_facts;
mod proof_only_faces;
mod properties;
mod proposition_entailment;
mod qualification_evidence;
mod quotients;
mod recasts;
mod relevance;
mod result_overloads;
mod state_signatures;
mod struct_literals;
mod symbols;
mod traits;
mod transitions;
mod type_references;
mod wire;
mod write_only_borrows;

pub use crate::calls::{CallFrameResolver, frame_paths_overlap};
use crate::calls::{
    validate_call_node, validate_proof_machine_recursion, validate_self_recursive_call_positions,
    validate_value_position_calls,
};
pub use crate::cleanup::validate_reserved_cleanup_selections;
pub use crate::content_conservation::{
    ContentConservationSourcePlan, build_content_conservation_plans,
};
pub use crate::content_projections::build_content_projection_plans;
use crate::contract_entailment::validate_machine_contract_entailment;
pub use crate::declaration_visibility::validate_declaration_visibility;

pub use crate::data::data_requires_establishment;
use crate::data::validate_data_field_types;
use crate::domains::validate_domain_definitions;
use crate::expression_types::{ExpressionTypeOwner, validate_expression_type_handle};
use crate::locals::{WritableRoots, validate_local_data_names};
use crate::machine_data::validate_owned_data;
use crate::places::validate_assignment_target_handle;
use crate::proof_facts::validate_proposition_definitions;
use crate::state_signatures::{
    StateSignatureOwner, validate_callable_state_signatures, validate_machine_contracts,
};
use crate::symbols::MachineSymbols;
pub use crate::symbols::TopLevelSymbols;
use crate::traits::{
    validate_conformances, validate_external_leaf_native_shapes,
    validate_generic_conformance_bounds, validate_machine_trait_conformances,
    validate_trait_conformance_bounds, validate_trait_requirements,
};
use crate::transitions::validate_transition_target_node;
use crate::type_references::{
    TypeReferenceOwner, validate_type_reference_handle_with_type_parameters,
};
pub use default_domains::{OpenInvariantCrashSite, build_open_invariant_crash_sites};
pub use effects::{validate_asm_discharge, validate_behavior_plan};
pub use expression_types::argument_matches_type_reference_handle as checked_argument_matches_type_reference;
/// The declared type of a simple place argument (bare name / `self.field`,
/// through the `&mut` marker), WITH its Constrained shells -- exposed for the
/// typed-trees machine-monomorphization pass's param-position inference.
pub use literals::land_float_literal_destinations;
pub use machine_parameters::{
    ValidatedNominalMachineUse, ValidatedNominalMachineUseSite, validate_static_machine_selections,
    validate_static_machine_selections_with_facts,
};
pub use machine_specialization_identity::recompute_checked_machine_specialization_commitment;
pub use operators::{
    ValidatedBoundaryOperatorApplication, ValidatedBoundaryOperatorApplicationArgument,
    ValidatedBoundaryOperatorApplicationUseSite, validate_named_operator_type_application,
};
pub use placed_views::{
    CheckedAtomicResidentAccess, CheckedAtomicResidentAccessRejection,
    bind_checked_atomic_resident_access,
};
pub use places::declared_place_type_raw;
pub use places::unwrapped_type_reference;
pub use properties::{
    DeclaredPropertyRequirement, declared_property_requirements, effective_data_carry_policy,
    effective_type_carry_policy, type_satisfies_declared_property,
};
pub use proposition_entailment::select_subjectless_evidence_conformance;
use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::statement::{StatementNode, TransitionTargetNode};
pub use quotients::{
    NonExecutableQuotientCorrespondenceBatch, ValidatedQuotientFormation,
    extract_non_executable_quotient_correspondences, validate_quotient_formations,
};
pub use recasts::{
    ValidatedLiteralIndexedRecastFootprint, validate_literal_indexed_recast_footprint,
};
pub use result_overloads::resolve_named_result_overloads;
pub use traits::{
    DynamicConformanceSelection, collect_dynamic_conformance_selections,
    resolve_dynamic_call_targets,
};
pub use type_references::normalize_open_index_expressions;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExactIntegerCastFact {
    pub expression: ExpressionHandle,
    pub source_type: psi_typed_trees::types::PrimitiveType,
    pub target_type: psi_typed_trees::types::PrimitiveType,
    pub minimum: psi_numerics::bignum::BigInt,
    pub maximum: psi_numerics::bignum::BigInt,
}

pub use float_projection_invocations::{
    ValidatedFloatMeaningEqualityProposition, ValidatedFloatMeaningProjectionInvocation,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramValidationFacts {
    pub boundary_operator_applications: Vec<ValidatedBoundaryOperatorApplication>,
    pub exact_integer_casts: Vec<ExactIntegerCastFact>,
    pub float_meaning_projection_invocations: Vec<ValidatedFloatMeaningProjectionInvocation>,
    pub float_meaning_equality_propositions: Vec<ValidatedFloatMeaningEqualityProposition>,
    pub fact_call_projections: Vec<fact_call_projections::ValidatedFactCallProjection>,
}

/// Exact source-independent coordinate of a contract-entailment goal that the
/// current proof engines declined to judge. Ordinary compilation may continue
/// because later semantic checks can still constrain the program; package
/// admission must fail closed until an exact later-discharge ledger exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContractEntailmentStandDown {
    pub machine_symbol: psi_symbols::SymbolHandle,
    pub contract_index: usize,
    pub fact_index: usize,
    pub reason: ContractEntailmentStandDownReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContractEntailmentStandDownReason {
    /// The ensures fact is a membership/proposition form not consumed by this
    /// entailment path.
    UnsupportedEnsuresFact,
    /// A bodied contract lies outside the recognized inductive body shape.
    UnrecognizedInductiveBody,
    /// The goal or a hypothesis needed to judge it is outside the engine's
    /// current language.
    OutsideEntailmentLanguage,
}

impl ContractEntailmentStandDownReason {
    pub const fn label(self) -> &'static str {
        match self {
            Self::UnsupportedEnsuresFact => "unsupported ensures fact",
            Self::UnrecognizedInductiveBody => "unrecognized inductive body",
            Self::OutsideEntailmentLanguage => "outside entailment language",
        }
    }
}

/// Audit every pristine typed machine, including generic templates, for proof
/// claims that ordinary validation deliberately leaves unjudged. Diagnostics
/// are intentionally discarded here: the normal validation path owns compile
/// failure, while this function supplies only successful-compilation admission
/// accounting.
pub fn collect_contract_entailment_stand_downs(
    program: &TypedTrees,
) -> Vec<ContractEntailmentStandDown> {
    let mut stand_downs = Vec::new();
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        contract_entailment::validate_machine_contract_entailment_with_stand_downs(
            program,
            machine,
            &mut diagnostics,
            &mut stand_downs,
        );
    }
    stand_downs.sort_unstable_by_key(|stand_down| {
        (
            stand_down.machine_symbol.arena_index(),
            stand_down.machine_symbol.generation(),
            stand_down.contract_index,
            stand_down.fact_index,
            stand_down.reason,
        )
    });
    stand_downs.dedup();
    stand_downs
}

pub fn validate_program(program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
    validate_program_internal(program, false).map(|_| ())
}

/// Recheck one already-resolved checked-body operator realization at a
/// compiler-internal evidence boundary. Callers must still establish the exact
/// selected operator and establish a retained checked baseline separately; this
/// reruns the contract-coverage judgment as the final semantic gate.
pub fn validate_checked_operator_realization_contract(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    operator: &psi_typed_trees::operator::OperatorDefinition,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    contract_entailment::check_operator_contract_conformance(
        program,
        machine,
        operator,
        &mut diagnostics,
    );
    finish_diagnostics(diagnostics)
}

/// Capture the exact typed contract structure consumed by checked operator
/// conformance. The bytes are compiler-private custody for equality within one
/// checked compilation; they are deliberately not a persisted format or hash.
pub fn checked_operator_contract_snapshot(
    program: &TypedTrees,
    contracts: &[psi_typed_trees::signature::SignatureContract],
) -> Vec<u8> {
    contract_entailment::checked_operator_contract_snapshot(program, contracts)
}

/// Validate after machine-generic contracts were checked on the pristine
/// typed graph, before monomorphization consumed the first template in place.
/// All other validation still runs on the concrete graph; only those already
/// proven universal contract entailments are not judged a second time against
/// a body whose static-machine environment has been substituted.
pub fn validate_program_after_generic_contract_entailment(
    program: &TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    validate_program_internal(program, true).map(|_| ())
}

pub fn validate_program_after_generic_contract_entailment_with_facts(
    program: &TypedTrees,
) -> Result<ProgramValidationFacts, Vec<Diagnostic>> {
    validate_program_internal(program, true)
}

/// Prove machine-generic contracts before specialization. Static selections
/// must already have been checked against their declared callable contracts.
pub fn validate_generic_machine_contract_entailment(
    program: &TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        if program
            .machine_type_parameters(machine)
            .iter()
            .any(|parameter| {
                matches!(
                    parameter.kind,
                    psi_typed_trees::data::TypeParameterKind::Machine { .. }
                )
            })
        {
            validate_machine_contract_entailment(program, machine, &mut diagnostics);
        }
    }
    finish_diagnostics(diagnostics)
}

fn validate_program_internal(
    program: &TypedTrees,
    generic_contract_entailment_prevalidated: bool,
) -> Result<ProgramValidationFacts, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut boundary_operator_applications = Vec::new();
    let mut exact_integer_casts = Vec::new();
    callable_overloads::validate_named_callable_overload_declarations(program, &mut diagnostics);
    float_projection_bindings::validate_float_projection_operator_bindings(
        program,
        &mut diagnostics,
    );
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    let fact_plan = psi_facts::build_definition_fact_plan(program);

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
    validate_conformances(program, &mut diagnostics);
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
    let proof_only = psi_typed_trees::proof_only::classify(program);
    quotients::validate_quotients(program, &proof_only, &mut diagnostics);
    let fact_call_projections =
        fact_call_projections::validate_fact_call_projections(program, &mut diagnostics);
    proof_only_faces::validate_proof_only_consumption(program, &proof_only, &mut diagnostics);
    // Chapter 3 / MR4: runtime call cycles require the constant-stack tail
    // admission; erased proof-only SCCs instead require strict structural descent.
    call_cycles::validate_machine_call_cycles(program, &symbols, &mut diagnostics);
    properties::validate_data_properties(program, &symbols, &mut diagnostics);
    // Bare-payload-case `==` (decision 11) is checked on the RESOLVED trees,
    // before membership lowering synthesizes its internal tag compares; see
    // psi-symbol-resolved-trees-to-typed-trees/src/equality.rs.
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
        let generic_contract_was_prevalidated = generic_contract_entailment_prevalidated
            && (program
                .machine_type_parameters(machine)
                .iter()
                .any(|parameter| {
                    matches!(
                        parameter.kind,
                        psi_typed_trees::data::TypeParameterKind::Machine { .. }
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
        validate_machine_trait_conformances(program, machine, &mut diagnostics);

        // PRV4 step 1: a `via <Binding>` clause is the EXTERNAL LEAF's
        // realization -- it must never parse and then silently drop. Exactly
        // one via clause, on a bodyless non-boundary machine, populates
        // ExternalRealization; every other carrier refuses here.
        {
            let via_count = program
                .machine_trait_conformances(machine)
                .iter()
                .filter(|conformance| conformance.external_binding.is_some())
                .count();
            let is_external = matches!(
                machine.supply_mode,
                psi_language_semantics::MachineSupplyMode::ExternalRealization { .. }
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
                // surface is type-check-only; value calls stay fenced).
                && !program
                    .machine_specializations
                    .iter()
                    .any(|specialization| specialization.template == machine.symbol)
                // Bodyless boundary declarations have no Omega body by
                // design. ACCEPTED declarations mean their ensures through
                // the trust carrier; claim-free BOUNDARY declarations merely
                // introduce a symbol and assert nothing.
                && !matches!(
                    machine.supply_mode,
                    psi_language_semantics::MachineSupplyMode::AdmissionClaim
                        | psi_language_semantics::MachineSupplyMode::TopLevelRequirement
                        | psi_language_semantics::MachineSupplyMode::Boundary
                )
                // PRV4: an EXTERNAL LEAF's body IS its binding -- the
                // realization produces the value at the seam.
                && !matches!(
                    machine.supply_mode,
                    psi_language_semantics::MachineSupplyMode::ExternalRealization { .. }
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
            let writable_roots = WritableRoots {
                program,
                machine_symbols: &machine_symbols,
                statements: program.statement_table.statements(state.statement_nodes),
                parameters: program.state_parameters(state),
            };

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
                arithmetic_domains::incoming_guard_env(program, machine, state)
            };
            for (statement_handle, statement) in program
                .statement_table
                .iter_statements(state.statement_nodes)
            {
                // R5 value-call frame: conservatively apply the aggregate
                // may-write set of every call nested in this statement before
                // checking any of its value uses. This intentionally gives up
                // evaluation-order precision within one expression, but never
                // carries a pre-call fact across a mutating value call. A
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
                validate_state_statement_node(
                    program,
                    machine,
                    &state.name,
                    &machine_symbols,
                    &symbols,
                    &writable_roots,
                    statement_handle,
                    statement,
                    &mut value_env,
                    &mut exact_integer_casts,
                    &mut boundary_operator_applications,
                    &mut diagnostics,
                );
            }
        }
    }

    finish_diagnostics(diagnostics)?;
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
    Ok(ProgramValidationFacts {
        boundary_operator_applications,
        exact_integer_casts,
        float_meaning_projection_invocations,
        float_meaning_equality_propositions,
        fact_call_projections,
    })
}

fn is_exact_executable_drop_body(
    program: &TypedTrees,
    cleanup: &psi_typed_trees::machine::Machine,
) -> bool {
    let [cleanup_state] = program.machine_states(cleanup) else {
        return false;
    };
    let statements = program
        .statement_table
        .statements(cleanup_state.statement_nodes);
    if statements.is_empty() {
        return false;
    }

    let mut helper_symbols = Vec::with_capacity(statements.len());
    for statement in statements {
        let StatementNode::Call(call) = statement else {
            return false;
        };
        if !call.machine_arguments.is_empty()
            || !program
                .statement_table
                .expression_handles(call.arguments)
                .is_empty()
            || call.discards_result
        {
            return false;
        }

        let helpers = program
            .machines()
            .iter()
            .filter_map(|machine| {
                let [state] = program.machine_states(machine) else {
                    return None;
                };
                (state.symbol == call.target_symbol).then_some((machine, state))
            })
            .collect::<Vec<_>>();
        let [(helper, helper_state)] = helpers.as_slice() else {
            return false;
        };
        let Some(helper_attachment) = helper.attached_data.as_ref() else {
            return false;
        };
        let helper_data = program
            .data_definitions()
            .iter()
            .filter(|data| &data.name == helper_attachment)
            .collect::<Vec<_>>();
        let [helper_data] = helper_data.as_slice() else {
            return false;
        };

        if helper.symbol == cleanup.symbol
            || helper_symbols.contains(&helper.symbol)
            || helper.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody
            || !helper.lifetime_parameters.is_empty()
            || !program.machine_type_parameters(helper).is_empty()
            || !program.machine_owned_data(helper).is_empty()
            || !program.machine_trait_conformances(helper).is_empty()
            || !helper.conformance_bounds.is_empty()
            || !program.machine_invokes(helper).is_empty()
            || helper.suspends
            || helper.blocks
            || !program.machine_contracts(helper).is_empty()
            || !program.state_parameters(helper_state).is_empty()
            || !program.state_contracts(helper_state).is_empty()
            || !matches!(
                program
                    .type_reference_table
                    .type_reference(helper_state.return_type),
                psi_typed_trees::types::TypeReferenceNode::Unit
            )
            || !program
                .statement_table
                .statements(helper_state.statement_nodes)
                .is_empty()
            || !helper_data.lifetime_parameters.is_empty()
            || !program.data_type_parameters(helper_data).is_empty()
            || !program.data_members(helper_data).is_empty()
        {
            return false;
        }
        helper_symbols.push(helper.symbol);
    }
    true
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

#[allow(clippy::too_many_arguments)]
fn validate_state_statement_node(
    program: &TypedTrees,
    machine: &psi_typed_trees::machine::Machine,
    state_name: &str,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    writable_roots: &WritableRoots<'_, '_>,
    statement_handle: psi_typed_trees::statement::StatementHandle,
    statement: &StatementNode,
    value_env: &mut arithmetic_domains::ValueEnv,
    exact_integer_casts: &mut Vec<ExactIntegerCastFact>,
    boundary_operator_applications: &mut Vec<ValidatedBoundaryOperatorApplication>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let Some(state) = machine_symbols.state(state_name) {
        placed_views::validate_statement(program, machine, state, statement, diagnostics);
    }
    match statement {
        StatementNode::AssemblyFact(fact) => {
            let state = machine_symbols.state(state_name);
            if !proof_facts::is_boolean_asm_fact_expression(
                program,
                machine,
                state,
                fact.expression,
            ) {
                let kind = match fact.kind {
                    psi_typed_trees::statement::AssemblyFactKind::Requires => "requires",
                    psi_typed_trees::statement::AssemblyFactKind::Ensures => "ensures",
                };
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` asm `{kind}` fact `{}` is not boolean-shaped",
                    machine.name,
                    program.expression_table.display_name(fact.expression),
                )));
            }
        }
        StatementNode::Assignment(assignment) => {
            let state = machine_symbols.state(state_name);
            if !state.is_some_and(|state| {
                placed_views::assignment_is_placed_atomic_operation(
                    program, machine, state, assignment,
                )
            }) {
                validate_assignment_target_handle(
                    program,
                    assignment.target,
                    writable_roots,
                    diagnostics,
                    machine,
                    state,
                    state_name,
                );
            }
            calls::validate_asm_value_destination(program, machine, state, assignment, diagnostics);
            let assignment_target_type =
                places::declared_place_type(program, machine, state, assignment.target)
                    // An indexed target (`self.xs[i] = ..`) has no member path, so
                    // `declared_place_type` returns None -- fall back to the array/slice
                    // ELEMENT type so the store checks below see the real slot type.
                    .or_else(|| {
                        places::declared_indexed_projection_type(
                            program,
                            machine,
                            machine_symbols.state(state_name),
                            assignment.target,
                        )
                    });
            // Weakening is about the target's STATIC qualification, so retain
            // the Constrained shell that the older representation/class checks
            // intentionally unwrap above.
            let assignment_target_type_raw =
                places::declared_place_type_raw(program, machine, state, assignment.target)
                    .or_else(|| {
                        places::declared_indexed_projection_type_raw(
                            program,
                            machine,
                            machine_symbols.state(state_name),
                            assignment.target,
                        )
                    });
            let assignment_target_primitive =
                assignment_target_type.and_then(|handle| program.primitive_type_reference(handle));
            // An array-literal RHS into a `[T; N]` target: check each element's
            // class + narrowing against T. The scalar guards below skip a non-
            // primitive (array) target, so this is the element-level complement.
            if let Some(current_state) = machine_symbols.state(state_name)
                && let Some(target_type) = assignment_target_type
            {
                struct_literals::validate_array_literal_elements(
                    program,
                    machine,
                    current_state,
                    assignment.value,
                    target_type,
                    diagnostics,
                );
            }
            let owner = format!("machine `{}` state `{state_name}` assignment", machine.name);
            // Cross-class scalar guard: `self.i32 = true` (a bool into a numeric
            // field) is a soundness hole -- the backend silently stores `1` --
            // whether the bool arrives as a literal or through a `self.bool_field`
            // place. Reject the unambiguous cross-class cases before value-range
            // analysis (which assumes a class-compatible RHS).
            if let Some(target_primitive) = assignment_target_primitive {
                expression_types::report_cross_class_store(
                    program,
                    Some(machine),
                    machine_symbols.state(state_name),
                    assignment.value,
                    target_primitive,
                    &owner,
                    "place",
                    diagnostics,
                );
            }
            // Nominal guard: `self.foo = self.bar` (a `Bar` value into a `Foo` place)
            // is silently accepted -- the wrong-data-type complement of the scalar
            // guard above.
            if let Some(target_type) = assignment_target_type {
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    assignment.value,
                    target_type,
                    &owner,
                    "place",
                    diagnostics,
                );
                // Shape guard: `self.scalar = self.xs` (array into a scalar place) --
                // caught by the backend today (a crude `NeedsMachineOwnedWrite`); this
                // gives a clear frontend message and covers the mirror.
                expression_types::report_array_scalar_shape_mismatch(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    assignment.value,
                    target_type,
                    &owner,
                    "place",
                    diagnostics,
                );
                // Scalar-vs-data shape guard: `self.struct_field = 5` / `self.scalar
                // = self.struct` (a scalar into a struct slot or the mirror), between
                // the scalar-class and nominal gates.
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    assignment.value,
                    target_type,
                    &owner,
                    "place",
                    diagnostics,
                );
            }
            if let Some(target_type) = assignment_target_type_raw {
                domain_weakening::validate_implicit_domain_weakening(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    assignment.value,
                    target_type,
                    &owner,
                    diagnostics,
                );
            }
            calls::report_nested_call_in_bound_value_call(
                program,
                machine,
                state_name,
                assignment.value,
                diagnostics,
            );
            calls::report_local_receiver_value_call(
                program,
                machine,
                state_name,
                assignment.value,
                diagnostics,
            );
            let before = diagnostics.len();
            let assignment_target_domain = assignment_target_type
                .map(|handle| program.arithmetic_domain_for_type_reference(handle))
                .unwrap_or(psi_numerics::arithmetic::ArithmeticDomain::Exact);
            let (interval, source_primitive) = arithmetic_domains::validate_value_range(
                program,
                machine,
                machine_symbols.state(state_name),
                assignment.value,
                value_env,
                assignment_target_primitive,
                assignment_target_domain,
                &owner,
                diagnostics,
            );
            // Only a CLEANLY-analyzed RHS reaches the narrowing check -- an RHS that
            // already erred (its own overflow, a type error) is not re-flagged.
            if diagnostics.len() == before {
                arithmetic_domains::check_narrowing_assignment(
                    assignment_target_primitive,
                    interval,
                    source_primitive,
                    &owner,
                    diagnostics,
                );
                // Containment for non-literal stores into ranged places (see
                // the local arm's twin note; literal stores refuse through
                // the proof plan).
                if let Some(handle) = assignment_target_type {
                    arithmetic_domains::check_range_containment(
                        program,
                        handle,
                        interval,
                        &owner,
                        diagnostics,
                    );
                }
            }
            arithmetic_domains::record_assignment(
                value_env,
                arithmetic_domains::place_path(program, assignment.target),
                interval,
                places::declared_place_type_raw(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    assignment.target,
                )
                .and_then(|handle| arithmetic_domains::enforced_declared_range(program, handle)),
            );
        }
        StatementNode::Call(call) => {
            if let Some(state) = machine_symbols.state(state_name) {
                crate::operators::validate_named_statement_operator_application(
                    program,
                    machine,
                    state,
                    statement_handle,
                    call,
                    boundary_operator_applications,
                    diagnostics,
                );
            }
            validate_call_node(
                program,
                call,
                machine,
                state_name,
                machine_symbols,
                symbols,
                writable_roots,
                value_env,
                diagnostics,
            );
            // R5 frame seed: a resolved acyclic INTERNAL call preserves facts
            // outside its conservatively instantiated may-write set. Unknown,
            // unsummarized, and overlapping implementations remain
            // conservative. Authored `stores` clauses are retired; exactness
            // grows through inferred implementation summaries.
            let written = crate::calls::known_call_written_paths(
                program,
                call,
                machine,
                machine_symbols,
                symbols,
            )
            .or_else(|| {
                crate::calls::known_boundary_call_written_paths(
                    program,
                    machine_symbols,
                    symbols,
                    call,
                )
            })
            .or_else(|| crate::calls::conservative_call_written_paths(program, call));
            if let Some(written) = written {
                value_env.invalidate_written_paths(&written);
            } else {
                value_env.clear();
            }
            // R4 witness mint: a BOUNDARY callee's `ensures` re-seeds the
            // `&mut` out-arguments' places (the boundary model's citable
            // fact) -- `fw.get_size(&mut self.n)` with `ensures size <= 8`
            // leaves `self.n` in [type_low, 8].
            if let Some(signature) =
                crate::calls::boundary_trait_signature(program, machine_symbols, symbols, call)
            {
                arithmetic_domains::seed_out_param_ensures(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    call,
                    signature,
                    value_env,
                );
            }
        }
        StatementNode::Expression(expression) => {
            let Some(state) = machine_symbols.state(state_name) else {
                return;
            };

            if !state.return_type.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` has a terminal expression but no return type",
                    machine.name
                )));
                return;
            }

            let shape_before = diagnostics.len();
            validate_expression_type_handle(
                program,
                *expression,
                state.return_type,
                diagnostics,
                ExpressionTypeOwner::StateTerminalExpression {
                    machine: machine.name.as_str(),
                    state: state_name,
                },
            );
            let owner = format!(
                "machine `{}` state `{state_name}` terminal expression",
                machine.name
            );
            let return_primitive = program.primitive_type_reference(state.return_type);
            // The shape gate above rejects a cross-class LITERAL terminal return
            // (`-> i32 { true }`) but blanket-accepts place/name values, so a bool
            // FIELD returned from an `-> i32` machine slips. Add the class check for
            // those -- only when the shape gate did not already error, so a literal
            // is not double-reported.
            if diagnostics.len() == shape_before {
                let slot_context = format!("machine `{}` state `{state_name}`", machine.name);
                if let Some(target_primitive) = return_primitive {
                    expression_types::report_cross_class_store(
                        program,
                        Some(machine),
                        Some(state),
                        *expression,
                        target_primitive,
                        &slot_context,
                        "return value",
                        diagnostics,
                    );
                }
                // Nominal guard: `-> Foo { self.bar }` returns a `Bar` as a `Foo`.
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    "return value",
                    diagnostics,
                );
                // Shape guard: `-> i32 { self.xs }` returns an array as a scalar.
                expression_types::report_array_scalar_shape_mismatch(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    "return value",
                    diagnostics,
                );
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    "return value",
                    diagnostics,
                );
                domain_weakening::validate_implicit_domain_weakening(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    state.return_type,
                    &slot_context,
                    diagnostics,
                );
            }
            let before = diagnostics.len();
            let (return_interval, source_primitive) = arithmetic_domains::validate_value_range(
                program,
                machine,
                Some(state),
                *expression,
                value_env,
                return_primitive,
                program.arithmetic_domain_for_type_reference(state.return_type),
                &owner,
                diagnostics,
            );
            // A cleanly-analyzed return value that cannot fit the declared return
            // type is a silent narrowing (`-> i8 { 300 }`), same as a store.
            if diagnostics.len() == before {
                arithmetic_domains::check_narrowing_assignment(
                    return_primitive,
                    return_interval,
                    source_primitive,
                    &owner,
                    diagnostics,
                );
            }
            // S4: enforce a declared return `[a..=b]` so call-site narrowing
            // that trusts it stays sound (the interval is already computed above).
            arithmetic_domains::enforce_declared_return_range(
                program,
                state.return_type,
                return_interval,
                &owner,
                diagnostics,
            );
            arithmetic_domains::collect_exact_integer_cast_facts(
                program,
                machine,
                Some(state),
                *expression,
                value_env,
                exact_integer_casts,
            );
        }
        StatementNode::LocalData(local_data) => {
            let mut type_parameters = program.machine_type_parameters(machine).to_vec();
            let mut lifetime_parameters = machine.lifetime_parameters.clone();
            if let Some(attached_data) = &machine.attached_data
                && let Some(definition) = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name == *attached_data)
            {
                for parameter in program.data_type_parameters(definition) {
                    if !type_parameters
                        .iter()
                        .any(|existing| existing.symbol == parameter.symbol)
                    {
                        type_parameters.push(parameter.clone());
                    }
                }
                for parameter in &definition.lifetime_parameters {
                    if !lifetime_parameters
                        .iter()
                        .any(|existing| existing == parameter)
                    {
                        lifetime_parameters.push(parameter.clone());
                    }
                }
            }
            validate_type_reference_handle_with_type_parameters(
                program,
                local_data.type_reference,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateLocalData {
                    machine: machine.name.as_str(),
                    state: state_name,
                    local: local_data.name.as_str(),
                    generic_depth: 0,
                },
                &type_parameters,
                &lifetime_parameters,
            );
            let local_target_primitive =
                program.primitive_type_reference(local_data.type_reference);
            // An array-literal initializer (`let a: [T; N] = [300, ..]`) is checked
            // element-wise against T.
            if let Some(current_state) = machine_symbols.state(state_name) {
                struct_literals::validate_array_literal_elements(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    diagnostics,
                );
            }
            let owner = format!(
                "machine `{}` state `{state_name}` local `{}`",
                machine.name,
                local_data.name.as_str()
            );
            // Cross-class guard: `let x: i32 = true` stores a bool into a numeric
            // local -- a silent miscompile, same as the assignment / arg / field
            // positions. Only an INITIALIZED `let` has a value to class-check (a
            // bare `let x: bool;` filled later by an `&mut` out-param has an invalid
            // initializer). (Narrowing on the initializer is checked below.)
            if local_data.initial_value.is_valid()
                && let Some(target_primitive) = local_target_primitive
            {
                expression_types::report_cross_class_store(
                    program,
                    Some(machine),
                    machine_symbols.state(state_name),
                    local_data.initial_value,
                    target_primitive,
                    &owner,
                    "local",
                    diagnostics,
                );
            }
            // Nominal guard: `let f: Foo = self.bar` binds a `Bar` value to a `Foo`
            // local -- wrong data type. Only an initialized `let` has a value.
            if local_data.initial_value.is_valid() {
                expression_types::report_data_type_conflict(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    "local",
                    diagnostics,
                );
                // Shape guard: `let y: i32 = self.xs` (array -> scalar) or
                // `let xs: [i32; 3] = 5` (scalar -> array) otherwise bind a
                // wrong-shaped value silently (the array case read a ZII 0).
                expression_types::report_array_scalar_shape_mismatch(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    "local",
                    diagnostics,
                );
                expression_types::report_scalar_data_shape_mismatch(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    "local",
                    diagnostics,
                );
                domain_weakening::validate_implicit_domain_weakening(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    diagnostics,
                );
            }
            calls::report_nested_call_in_bound_value_call(
                program,
                machine,
                state_name,
                local_data.initial_value,
                diagnostics,
            );
            calls::report_local_receiver_value_call(
                program,
                machine,
                state_name,
                local_data.initial_value,
                diagnostics,
            );
            let before = diagnostics.len();
            let (interval, source_primitive) = arithmetic_domains::validate_value_range(
                program,
                machine,
                machine_symbols.state(state_name),
                local_data.initial_value,
                value_env,
                local_target_primitive,
                program.arithmetic_domain_for_type_reference(local_data.type_reference),
                &owner,
                diagnostics,
            );
            // A cleanly-analyzed initializer whose value cannot fit the declared
            // local type is a silent narrowing (`let x: i8 = 300`).
            if local_data.initial_value.is_valid() && diagnostics.len() == before {
                arithmetic_domains::check_narrowing_assignment(
                    local_target_primitive,
                    interval,
                    source_primitive,
                    &owner,
                    diagnostics,
                );
                // Containment against a declared Exact `[a..=b]`: literal
                // initializers refuse through the proof plan, but a
                // NON-LITERAL initializer's interval was never checked --
                // `let idx: u32 [0..=11] = <expr provably up to 12>` stored
                // unproven and the index prover then TRUSTED the range (a
                // confirmed native OOB read, found landing the R3 product
                // rule). The enforced range only exists under Exact shells,
                // so non-Exact declarations are untouched (the
                // `range-constraints-require-exact-domain` gate
                // already rejects range+domain combinations).
                // A reference binding stores the reference, not a fresh value
                // into the referee. Its referee facts are checked by ordinary
                // borrow compatibility and, for a stated recast, by the
                // bidirectional representation judgment. Treating the borrow
                // expression as a numeric store into the referee incorrectly
                // rejects every range-refined reference initializer.
                if !matches!(
                    program
                        .type_reference_table
                        .type_reference(local_data.type_reference),
                    psi_typed_trees::types::TypeReferenceNode::Reference { .. }
                ) {
                    arithmetic_domains::check_range_containment(
                        program,
                        local_data.type_reference,
                        interval,
                        &owner,
                        diagnostics,
                    );
                }
            }
            if local_data.initial_value.is_valid() {
                arithmetic_domains::collect_exact_integer_cast_facts(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    local_data.initial_value,
                    value_env,
                    exact_integer_casts,
                );
                arithmetic_domains::record_assignment(
                    value_env,
                    Some(local_data.name.as_str().to_owned()),
                    interval,
                    local_data
                        .type_reference
                        .is_valid()
                        .then(|| {
                            arithmetic_domains::enforced_declared_range(
                                program,
                                local_data.type_reference,
                            )
                        })
                        .flatten(),
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let psi_typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                arithmetic_domains::collect_exact_integer_cast_facts(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    guard,
                    value_env,
                    exact_integer_casts,
                );
            }
            validate_transition_target_node(
                program,
                machine,
                machine_symbols.state(state_name),
                value_env,
                transition.target,
                machine_symbols,
                symbols,
                writable_roots,
                diagnostics,
            );

            if transition.continuation.is_valid() {
                validate_transition_target_node(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    value_env,
                    transition.continuation,
                    machine_symbols,
                    symbols,
                    writable_roots,
                    diagnostics,
                );
            }

            // Decision 17 completeness: an arm fires only when its guard holds, so
            // BOTH a value return (`n > 0 { true -> (n - 1) }`) and a call argument
            // (`count_down(n - 1)`) may assume the guard's bound. Narrow the env by
            // the arm's guard once, up front, for both. `guard_narrowed_env`
            // intersects each bounded place with its type range (so a one-sided
            // `n > 0` keeps the type's other end) and negates for the `false` arm,
            // so an unguarded (or wrong-arm) decrement is still correctly rejected.
            let narrowed = arithmetic_domains::guard_narrowed_env(
                program,
                machine,
                machine_symbols.state(state_name),
                &transition.guard,
                value_env,
            );

            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                match program.statement_table.transition_target(target) {
                    TransitionTargetNode::Named { arguments, .. } => {
                        for argument in program.statement_table.expression_handles(*arguments) {
                            arithmetic_domains::collect_exact_integer_cast_facts(
                                program,
                                machine,
                                machine_symbols.state(state_name),
                                *argument,
                                &narrowed,
                                exact_integer_casts,
                            );
                        }
                    }
                    TransitionTargetNode::Value(expression) => {
                        arithmetic_domains::collect_exact_integer_cast_facts(
                            program,
                            machine,
                            machine_symbols.state(state_name),
                            *expression,
                            &narrowed,
                            exact_integer_casts,
                        );
                    }
                    TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
                }
            }

            // Fall-through complement: an exit-if-true transition (valid
            // target, no `_` arm) leaves the guard REFUTED for every later
            // statement in this state -- the MR2 terminal-tail shape's
            // `n - 1` after `transition n == 0 { true -> exit }`.
            if transition.target.is_valid() && !transition.continuation.is_valid() {
                *value_env = arithmetic_domains::fall_through_narrowed_env(
                    program,
                    machine,
                    machine_symbols.state(state_name),
                    &transition.guard,
                    value_env,
                );
            }

            // A transition VALUE target (`_ -> (expr)`) is a return value. When the
            // state's return type declares a `[a..=b]`, enforce the value is provably
            // within it (so call-site narrowing that trusts the range is sound); the
            // exact-overflow + narrowing obligations apply too. Gated inside
            // `validate_return_value_range`, so plain returns are unaffected.
            if let Some(state) = machine_symbols.state(state_name)
                && state.return_type.is_valid()
            {
                for target in [transition.target, transition.continuation] {
                    if !target.is_valid() {
                        continue;
                    }
                    if let TransitionTargetNode::Value(return_expression) =
                        program.statement_table.transition_target(target)
                    {
                        // Cross-class guard: `_ -> (true)` returning a bool from an
                        // i32-returning machine is a silent miscompile. The terminal
                        // `{ true }` return form is caught by the general shape gate;
                        // the transition-VALUE form was not. Class complement of the
                        // range/overflow/narrowing check below.
                        if let Some(return_primitive) =
                            program.primitive_type_reference(state.return_type)
                        {
                            expression_types::report_cross_class_store(
                                program,
                                Some(machine),
                                Some(state),
                                *return_expression,
                                return_primitive,
                                &format!("machine `{}` state `{state_name}`", machine.name),
                                "return value",
                                diagnostics,
                            );
                        }
                        // Nominal guard: `-> Foo { transition { _ -> (self.bar) } }`
                        // returns a `Bar` as a `Foo`.
                        expression_types::report_data_type_conflict(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            "return value",
                            diagnostics,
                        );
                        // Shape guard: an array returned as a scalar (or vice versa).
                        expression_types::report_array_scalar_shape_mismatch(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            "return value",
                            diagnostics,
                        );
                        expression_types::report_scalar_data_shape_mismatch(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            "return value",
                            diagnostics,
                        );
                        domain_weakening::validate_implicit_domain_weakening(
                            program,
                            machine,
                            Some(state),
                            *return_expression,
                            state.return_type,
                            &format!("machine `{}` state `{state_name}`", machine.name),
                            diagnostics,
                        );
                        arithmetic_domains::validate_return_value_range(
                            program,
                            machine,
                            state,
                            *return_expression,
                            &narrowed,
                            &format!(
                                "machine `{}` state `{state_name}` return value",
                                machine.name
                            ),
                            diagnostics,
                        );
                    }
                }
            }

            for target in [transition.target, transition.continuation] {
                if !target.is_valid() {
                    continue;
                }
                if let TransitionTargetNode::Named { arguments, .. } =
                    program.statement_table.transition_target(target)
                {
                    for argument in program.statement_table.expression_handles(*arguments) {
                        arithmetic_domains::validate_arithmetic_domains(
                            program,
                            machine,
                            machine_symbols.state(state_name),
                            *argument,
                            &narrowed,
                            None,
                            psi_numerics::arithmetic::ArithmeticDomain::Exact,
                            &format!(
                                "machine `{}` state `{state_name}` transition argument",
                                machine.name
                            ),
                            diagnostics,
                        );
                    }
                }
            }
        }
    }
}
