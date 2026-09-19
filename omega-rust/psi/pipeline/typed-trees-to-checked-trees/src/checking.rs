//! Typed-tree checking: specialize calls, validate contracts and ownership,
//! assemble execution plans, then publish checked trees.
//!
//! Package checkpoints select explicit checking modes; only the test-only
//! crash-inspection mode omits crash admission.

pub(crate) mod call_acknowledgements;
pub(crate) mod program_validation;

use crate::checking::program_validation::validate_typed_program;
use crate::checks;
use crate::facts::build_check_facts;
use checked_trees::CheckedTrees;

/// Check a standalone program. Toolchain-owned selections may remain late-bound
/// until build-time evaluation; ordinary authored selections remain strict.
pub fn lower_typed_trees(
    program: typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    check_program(program, CheckingMode::SettledPackage, &[], &[], &[])
}

fn check_program(
    program: typed_trees::TypedTrees,
    mode: CheckingMode,
    selected_generic_operator_providers: &[crate::SelectedGenericOperatorProviderSpecialization],
    selected_boundary_families: &[crate::SelectedBoundaryFamilySpecialization],
    opaque_property_receipts: &[validation::OpaqueDataPropertyReceipt],
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    // Mathematical `let`/`boundary let` declarations elaborate into
    // `CheckedMathematicalDeclaration` records; run that elaboration so a
    // declaration the checked surface cannot record fails with its own
    // diagnostic. Kernel-term elaboration and downstream consumption remain
    // separate PROOF-CONTRACT-MIGRATION legs, so an elaborated declaration
    // still refuses here rather than silently dropping from checked trees.
    if let Some(definition) = program.mathematical_definitions().first() {
        crate::proof::build_checked_mathematical_declarations(&program)?;
        let mut diagnostic = diagnostics::Diagnostic::error(
            "mathematical `let`/`boundary let` declarations elaborate to the checked \
                 surface, but kernel-term elaboration and downstream consumption are \
                 not implemented yet (PROOF-CONTRACT-MIGRATION)",
        );
        if let Some(span) = program.symbols.symbol_source_span(definition.symbol) {
            diagnostic = diagnostic.with_source_span(span);
        }
        return Err(vec![diagnostic]);
    }
    // A deferred range endpoint is pre-check-continuation custody: the
    // semantic evaluation owner marks it when its fold must wait for selected
    // execution and clears the mark as it lands the integer. Only the
    // preliminary package checkpoint runs inside that window; every other
    // checking mode must receive the bound already folded, so a surviving
    // mark is a lost continuation and refuses rather than reading the
    // still-authored call as an unbounded or dependent range.
    if !mode.allows_pending_const_range_endpoints()
        && !program.pending_const_range_endpoints.is_empty()
    {
        return Err(vec![diagnostics::Diagnostic::error(
            "a deferred range endpoint reached checked lowering still unevaluated",
        )]);
    }
    // Stage-1 machine monomorphization MUST precede validation: a generic
    // machine whose value calls agree on one instantiation is substituted to a
    // concrete machine here. Validation permits unused template bodies but the
    // generic-value-call fence still rejects any emitted concrete caller whose
    // callee remains generic (an incomplete specialization).
    let mut program = program;
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
    // A local `dyn` selection generates the complete finite family of its
    // selected conformance rows, and a selected boundary adapter row demands
    // the same complete roster of its chosen provider: every roster tuple's
    // provider body becomes checked evidence without requiring a static call
    // site.
    crate::monomorphization::generate_dynamic_family_specializations(
        &mut program,
        selected_boundary_families,
    )?;
    // Keep the authored generic provider templates immutable while ordinary
    // machine specialization closes caller binders. Selected providers may be
    // demanded only by applications copied into those newly concrete bodies,
    // and a newly selected provider may itself expose another ordinary or
    // selected generic application. Alternate the existing elaborators to a
    // fixed point; neither open applications nor template mutation may be
    // mistaken for final D29 coverage. A generated or provider-selected body
    // can itself contain a dynamic selection, so family generation repeats
    // inside the same fixed point.
    let selected_provider_templates = crate::monomorphization::SelectedProviderTemplates::prepare(
        &program,
        selected_generic_operator_providers,
    );
    let mut static_machine_selections =
        specialize_static_machine_calls_with_selections(&mut program, true)?;
    normalize_open_index_identities(&mut program)?;
    loop {
        let mut materialized = match &selected_provider_templates {
            Some(templates) => {
                crate::monomorphization::specialize_selected_generic_operator_providers(
                    templates,
                    &mut program,
                    selected_generic_operator_providers,
                )?
            }
            None => 0,
        };
        materialized += crate::monomorphization::generate_dynamic_family_specializations(
            &mut program,
            selected_boundary_families,
        )?;
        if materialized == 0 {
            break;
        }
        static_machine_selections =
            specialize_static_machine_calls_with_selections(&mut program, true)?;
        normalize_open_index_identities(&mut program)?;
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
    // A spelled use that selects a token-bearing machine is supplied by that
    // declaration's own body: bind it to an ordinary call now, after every
    // specialization and call-identity rebinding above, so validation and
    // every executing consumer see the same call edge a named call would make.
    crate::operators::bind_token_bound_machine_calls(&mut program)?;
    crate::monomorphization::validate_selected_attached_method_bounds(&program)?;
    let validated = validate_typed_program(
        &program,
        opaque_property_receipts,
        mode.allows_pending_opaque_copy(),
    )?;
    // Caller-visible mutation summaries depend only on the immutable typed
    // program and the borrow facts assembled once here. Fact construction, the
    // direct-borrow resource closures, and the independent check replay all
    // query the same (program, borrow) pair, so one lazily-filled table serves
    // the whole check pass instead of each consumer rebuilding it.
    let mutation_summaries = crate::flow::StateMutationSummaryCache::default();
    let mut facts = build_check_facts(
        &program,
        &validated.proof_plan,
        validated.operational,
        validated.service_reaches,
        &validated.validation_facts,
        static_machine_selections,
        &mutation_summaries,
    )?;
    checks::initialize_checked_direct_borrow_resources(&program, &mut facts, &mutation_summaries)?;

    // MP5: specialization selection happens before checked contract plans
    // exist. Bind the selected machines' normalized contract identities now,
    // validate the recorded relation, and make those identities part of the
    // instance fingerprint used by caches and artifacts.
    crate::monomorphization::bind_specialization_contract_identities(
        &mut program,
        &facts.contract_plans,
    )?;

    match mode {
        CheckingMode::Complete
        | CheckingMode::PreliminaryPackage
        | CheckingMode::SettledPackage => {
            checks::check_checked_facts_recording_with_mutation_summaries(
                &program,
                &mut facts,
                &mutation_summaries,
            )?;
        }
        #[cfg(test)]
        CheckingMode::CrashFactInspection => {
            checks::check_checked_facts_recording_without_crash_admission(&program, &mut facts)?;
        }
    }
    crate::facts::refresh_realized_contract_envelopes(&mut facts);

    let mut facts = crate::execution::finalize_execution::finalize_execution(&program, facts)?;
    facts.flow.semantic_dependencies =
        crate::flow::derive_checked_semantic_dependencies(&program, &facts);

    crate::authored_selections::bind_checked_intrinsic_call_facts(&program, &mut facts)
        .map_err(|diagnostic| vec![diagnostic])?;
    if mode.allows_unresolved_toolchain_selections() {
        crate::authored_selections::finalize_preliminary_checked_authored_selections(
            &mut program,
            &facts,
        )
    } else {
        crate::authored_selections::finalize_checked_authored_selections(&mut program, &facts)
    }
    .map_err(|diagnostic| vec![diagnostic])?;
    validation::validate_reserved_cleanup_selections(&program)?;
    validation::validate_declaration_visibility(&program)?;

    Ok(CheckedTrees::with_roots(program, facts))
}

/// Lower a pre-settlement package checkpoint. Unresolved selections are
/// retained only for compiler-owned toolchain source; the caller must reject
/// unresolved ordinary-package selections before granting build authority.
pub fn lower_preliminary_typed_trees(
    program: typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    check_program(program, CheckingMode::PreliminaryPackage, &[], &[], &[])
}

/// One Omega-selected generic checked body that must be specialized for the
/// exact closed applications of its boundary-operator requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedGenericOperatorProviderSpecialization {
    pub requirement_operator: symbols::SymbolHandle,
    pub realization_machine: symbols::SymbolHandle,
}

/// One selected boundary adapter row whose requirement declares a complete
/// finite family: the chosen provider template must own a checked
/// specialization per declared roster tuple, the same commitment a `dyn`
/// selection derives from its conformance row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedBoundaryFamilySpecialization {
    /// The boundary requirement's exact signature symbol; its `where` clause
    /// owns the roster through `TypedTrees::finite_signature_family`.
    pub requirement_signature: symbols::SymbolHandle,
    /// The selected provider's generic machine template.
    pub realization_machine: symbols::SymbolHandle,
}

/// Lower with exact selected generic operator providers supplied by the
/// orchestration owner. Psi derives applications from authored uses and uses
/// ordinary authoritative specialization; the request carries no application
/// strings, capability assertions, or provider-selection policy.
pub fn lower_typed_trees_with_selected_generic_operator_providers(
    program: typed_trees::TypedTrees,
    selected: &[SelectedGenericOperatorProviderSpecialization],
    selected_boundary_families: &[SelectedBoundaryFamilySpecialization],
    opaque_property_receipts: &[::validation::OpaqueDataPropertyReceipt],
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    check_program(
        program,
        CheckingMode::SettledPackage,
        selected,
        selected_boundary_families,
        opaque_property_receipts,
    )
}

/// Final package-aware lowering keeps ordinary package selections strict while
/// permitting unresolved compiler-owned toolchain selections to remain TCB
/// input. The compiler must run its package declaration-authority gate over
/// the result before issuing package evidence.
pub fn lower_package_typed_trees_with_selected_generic_operator_providers(
    program: typed_trees::TypedTrees,
    selected: &[SelectedGenericOperatorProviderSpecialization],
    selected_boundary_families: &[SelectedBoundaryFamilySpecialization],
    opaque_property_receipts: &[::validation::OpaqueDataPropertyReceipt],
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    check_program(
        program,
        CheckingMode::SettledPackage,
        selected,
        selected_boundary_families,
        opaque_property_receipts,
    )
}

/// These are distinct checking checkpoints, not freely combinable permissions.
#[derive(Clone, Copy)]
enum CheckingMode {
    /// Strictly finalized checking retained for callers that must reject
    /// toolchain late bindings; standalone and package routes currently both
    /// settle toolchain-owned selections at build-time evaluation.
    #[allow(dead_code)]
    Complete,
    PreliminaryPackage,
    SettledPackage,
    #[cfg(test)]
    CrashFactInspection,
}

impl CheckingMode {
    fn allows_pending_opaque_copy(self) -> bool {
        matches!(self, Self::PreliminaryPackage)
    }

    fn allows_pending_const_range_endpoints(self) -> bool {
        matches!(self, Self::PreliminaryPackage)
    }

    fn allows_unresolved_toolchain_selections(self) -> bool {
        matches!(self, Self::PreliminaryPackage | Self::SettledPackage)
    }
}

#[cfg(test)]
pub(crate) fn lower_typed_trees_for_crash_fact_inspection(
    program: typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<diagnostics::Diagnostic>> {
    check_program(program, CheckingMode::CrashFactInspection, &[], &[], &[])
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
    let operational = ::validation::infer_operational_may(program);
    ::validation::validate_static_machine_call_contracts(program, &operational)
        .map_err(|diagnostic| vec![diagnostic])?;
    Ok(selections)
}

#[cfg(test)]
mod tests {
    use super::CheckingMode;

    #[test]
    fn checking_modes_preserve_package_settlement_permissions() {
        for mode in [CheckingMode::Complete, CheckingMode::CrashFactInspection] {
            assert!(!mode.allows_pending_opaque_copy());
            assert!(!mode.allows_pending_const_range_endpoints());
            assert!(!mode.allows_unresolved_toolchain_selections());
        }
        assert!(CheckingMode::PreliminaryPackage.allows_pending_opaque_copy());
        assert!(CheckingMode::PreliminaryPackage.allows_pending_const_range_endpoints());
        assert!(CheckingMode::PreliminaryPackage.allows_unresolved_toolchain_selections());
        assert!(!CheckingMode::SettledPackage.allows_pending_opaque_copy());
        assert!(!CheckingMode::SettledPackage.allows_pending_const_range_endpoints());
        assert!(CheckingMode::SettledPackage.allows_unresolved_toolchain_selections());
    }

    #[test]
    fn mathematical_declarations_refuse_at_checked_lowering() {
        use source_files_to_tokens::Lexer;
        use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
        use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
        use tokens_to_syntax_trees::parse_syntax_trees;

        let tokens = Lexer::new("let double(x: u64): u64 = x;")
            .tokenize()
            .expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");

        let diagnostics =
            crate::lower_typed_trees(typed).expect_err("checked elaboration is pending");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("PROOF-CONTRACT-MIGRATION")),
            "unexpected diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn unelaboratable_mathematical_declarations_fail_with_their_own_diagnostic() {
        use source_files_to_tokens::Lexer;
        use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
        use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
        use tokens_to_syntax_trees::parse_syntax_trees;

        let tokens = Lexer::new("let bad<A: core::Type<u, v>>(x: A): A = x;")
            .tokenize()
            .expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");

        let diagnostics = crate::lower_typed_trees(typed)
            .expect_err("a malformed `core::Type` carrier is unelaboratable");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("core::Type")),
            "unexpected diagnostics: {diagnostics:?}"
        );
    }
}
