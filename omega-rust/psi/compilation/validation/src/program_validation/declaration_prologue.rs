//! The declaration validators that run before the machine pool, on the
//! checking pool at once instead of one after another on the compile thread.
//!
//! Each validator judges declarations of the frozen program and appends to
//! the diagnostics and formation obligations it is handed. Run in order, a
//! validator also sees what every earlier one appended; run apart, it sees
//! only its own. The two differ only where a validator reads those vectors
//! beyond its own appends. An audit of `validation` found three such reads
//! reachable from these validators, all de-duplication:
//! - `cast_ranges::validate_range_cast_at_use` and
//!   `expression_analysis::cast::require_exact_integer_fit` skip a diagnostic
//!   equal to one already present;
//! - `operand_fits::CarrierFit::retain` skips an obligation already present.
//!
//! Every other read compares against a length the validator took itself.
//!
//! So the parallel run is GUARDED by its outcome. It stands only when no
//! validator reported any diagnostic, so every diagnostic read saw an empty
//! prefix both ways, and when no obligation of one validator equals an
//! obligation of an earlier one, so no `contains` check would have answered
//! differently. Otherwise the validators run again in order on the real
//! vectors, exactly as before, and the speculative results are dropped. A
//! program with a declaration error therefore takes the serial path.
//!
//! The validators' remaining shared state is the checking memos, which hold
//! pure answers over the frozen program and already serve the machine pool
//! concurrently.

use super::*;
use checked_trees::IntegerRangeObligation;

/// What every prologue validator reads besides the program.
pub(super) struct PrologueInputs<'program> {
    pub(super) program: &'program dyn crate::CheckingInput,
    pub(super) effect_admission: EffectAdmission,
    pub(super) quotient_admission: QuotientRequestAdmission,
    pub(super) opaque_properties: OpaquePropertyValidation<'program>,
    pub(super) symbols: TopLevelSymbols<'program>,
    /// What building `symbols` reported, in its place among the validators.
    pub(super) symbol_diagnostics: Vec<Diagnostic>,
    pub(super) fact_plan: facts::FactPlan,
    pub(super) conformance_operational: flow_effects::OperationalPlan,
    pub(super) conformance_service_reaches: flow_effects::ServiceReachInferencePlan,
    pub(super) proof_only: std::sync::Arc<typed_trees::proof_only::ProofOnlyClassification>,
    pub(super) call_frames: Option<calls::CallFrameResolver<'program>>,
}

impl<'program> PrologueInputs<'program> {
    pub(super) fn build(
        program: &'program dyn crate::CheckingInput,
        effect_admission: EffectAdmission,
        quotient_admission: QuotientRequestAdmission,
        opaque_properties: OpaquePropertyValidation<'program>,
    ) -> Self {
        let mut symbol_diagnostics = Vec::new();
        let symbols = TopLevelSymbols::build(program, &mut symbol_diagnostics);
        let fact_plan = build_definition_fact_plan(program);
        let conformance_operational = infer_operational_may(program);
        let conformance_service_reaches = infer_service_reaches(program, &conformance_operational);
        let proof_only = crate::proof_only_classification(program);
        let call_frames = calls::CallFrameResolver::new(program);
        Self {
            program,
            effect_admission,
            quotient_admission,
            opaque_properties,
            symbols,
            symbol_diagnostics,
            fact_plan,
            conformance_operational,
            conformance_service_reaches,
            proof_only,
            call_frames,
        }
    }
}

/// What a validator returns beside its diagnostics and obligations.
pub(super) enum PrologueValue {
    None,
    IntegerEmbeddingCalls(Vec<ValidatedIntegerEmbeddingCall>),
    QuotientFormations(Vec<crate::ValidatedQuotientFormation>),
    FactCallProjections(
        Vec<crate::machine_calls::fact_call_projections::ValidatedFactCallProjection>,
    ),
    SpecificationCalls(Vec<crate::SpecificationCallRequest>),
    CallComponents(Vec<ValidatedProofCallComponent>),
    StructLiterals(struct_literals::StructLiteralValidation),
}

/// The prologue's results, as the serial code left them.
#[derive(Default)]
pub(super) struct PrologueResults {
    pub(super) integer_embedding_calls: Vec<ValidatedIntegerEmbeddingCall>,
    pub(super) quotient_formations: Vec<crate::ValidatedQuotientFormation>,
    pub(super) fact_call_projections:
        Vec<crate::machine_calls::fact_call_projections::ValidatedFactCallProjection>,
    pub(super) specification_calls: Vec<crate::SpecificationCallRequest>,
    pub(super) proof_recursive_components: Vec<ValidatedProofCallComponent>,
    pub(super) struct_literals: Option<struct_literals::StructLiteralValidation>,
}

impl PrologueResults {
    fn take(&mut self, value: PrologueValue) {
        match value {
            PrologueValue::None => {}
            PrologueValue::IntegerEmbeddingCalls(calls) => self.integer_embedding_calls = calls,
            PrologueValue::QuotientFormations(formations) => self.quotient_formations = formations,
            PrologueValue::FactCallProjections(projections) => {
                self.fact_call_projections = projections
            }
            PrologueValue::SpecificationCalls(calls) => self.specification_calls = calls,
            PrologueValue::CallComponents(components) => {
                self.proof_recursive_components = components
            }
            PrologueValue::StructLiterals(literals) => self.struct_literals = Some(literals),
        }
    }
}

type Validator = dyn Fn(&PrologueInputs<'_>, &mut Vec<Diagnostic>, &mut Vec<IntegerRangeObligation>) -> PrologueValue
    + Sync;

/// Every prologue validator, in the order the serial path runs them.
fn validators() -> Vec<Box<Validator>> {
    fn none(
        validate: impl Fn(&PrologueInputs<'_>, &mut Vec<Diagnostic>, &mut Vec<IntegerRangeObligation>)
        + Sync
        + 'static,
    ) -> Box<Validator> {
        Box::new(move |inputs, diagnostics, obligations| {
            validate(inputs, diagnostics, obligations);
            PrologueValue::None
        })
    }
    let complete =
        |inputs: &PrologueInputs<'_>| inputs.effect_admission == EffectAdmission::Complete;
    vec![
        none(|inputs, diagnostics, _| {
            callable_overloads::validate_named_callable_overload_declarations(
                inputs.program,
                diagnostics,
            )
        }),
        none(|inputs, diagnostics, _| {
            float_projection_bindings::validate_float_projection_operator_bindings(
                inputs.program,
                diagnostics,
            )
        }),
        none(|inputs, diagnostics, _| {
            let program = inputs.program;
            for machine in program.machines() {
                if typed_trees::float_providers::is_catalog_declaration(
                    program.typed_program(),
                    machine,
                ) && let Err(reason) =
                    typed_trees::float_providers::authenticate_intrinsic_member(
                        program.typed_program(),
                        machine,
                    )
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "float provider member `{}` is not authenticated: {reason}",
                        machine.name,
                    )));
                }
            }
        }),
        none(|inputs, diagnostics, _| {
            crate::value_custody::atomic_fences::validate_atomic_fence_declarations(
                inputs.program,
                diagnostics,
            )
        }),
        none(|inputs, diagnostics, _| {
            diagnostics.extend(inputs.symbol_diagnostics.iter().cloned())
        }),
        none(|inputs, diagnostics, _| {
            standard_declarations::validate_calling_vocabulary_uniqueness(
                inputs.program,
                diagnostics,
            )
        }),
        none(|inputs, diagnostics, _| {
            declaration_visibility::collect_declaration_visibility_diagnostics(
                inputs.program,
                diagnostics,
            )
        }),
        none(|inputs, diagnostics, _| {
            cleanup::collect_reserved_cleanup_selection_diagnostics(inputs.program, diagnostics)
        }),
        none(|inputs, diagnostics, _| {
            validate_proposition_definitions(inputs.program, diagnostics)
        }),
        none(|inputs, diagnostics, _| {
            proposition_entailment::validate_proposition_entailment(inputs.program, diagnostics)
        }),
        none(|inputs, diagnostics, _| {
            literals::validate_suffix_landings(inputs.program, diagnostics)
        }),
        none(|inputs, diagnostics, _| {
            literals::validate_suffix_magnitudes(inputs.program, diagnostics)
        }),
        // Abstract calls consume the same scoped content formation that later
        // fact construction uses. Keep its diagnostic-producing owner first.
        none(|inputs, diagnostics, obligations| {
            content_projections::validate_content_projection_conformances(
                inputs.program,
                diagnostics,
                obligations,
            )
        }),
        none(|inputs, diagnostics, _| {
            content_conservation::validate_content_conservation_contracts(
                inputs.program,
                diagnostics,
            )
        }),
        none(|inputs, diagnostics, obligations| {
            validate_domain_definitions(
                inputs.program,
                &inputs.symbols,
                &inputs.fact_plan,
                diagnostics,
                obligations,
            )
        }),
        none(|inputs, diagnostics, obligations| {
            validate_callable_state_signatures(
                inputs.program,
                &inputs.symbols,
                diagnostics,
                obligations,
            )
        }),
        none(|inputs, diagnostics, _| {
            write_only_borrows::validate_checked_write_only_slice(inputs.program, diagnostics)
        }),
        none(|inputs, diagnostics, obligations| {
            arithmetic_domains::validate_abstract_total_specification_arithmetic(
                inputs.program,
                diagnostics,
                obligations,
            )
        }),
        none(|inputs, diagnostics, _| {
            cleanup::validate_cleanup_machine_declarations(inputs.program, diagnostics)
        }),
        none(|inputs, diagnostics, obligations| {
            validate_trait_requirements(inputs.program, &inputs.symbols, diagnostics, obligations)
        }),
        none(|inputs, diagnostics, obligations| {
            for trait_definition in inputs.program.traits() {
                validate_trait_conformance_bounds(
                    inputs.program,
                    trait_definition,
                    &inputs.symbols,
                    diagnostics,
                    obligations,
                );
            }
        }),
        none(|inputs, diagnostics, _| {
            qualification_evidence::validate_qualification_authorization(
                inputs.program,
                diagnostics,
            )
        }),
        none(move |inputs, diagnostics, obligations| {
            if complete(inputs) {
                validate_conformances(
                    inputs.program,
                    &inputs.conformance_service_reaches,
                    &inputs.symbols,
                    diagnostics,
                    obligations,
                );
                if let Err(mut dynamic_diagnostics) =
                    collect_dynamic_implementation_selections(inputs.program)
                {
                    diagnostics.append(&mut dynamic_diagnostics);
                }
            }
        }),
        none(|inputs, diagnostics, obligations| {
            crate::declarations::type_aliases::validate(
                inputs.program,
                &inputs.symbols,
                diagnostics,
                obligations,
            )
        }),
        none(|inputs, diagnostics, obligations| {
            validate_data_field_types(inputs.program, &inputs.symbols, diagnostics, obligations)
        }),
        none(|inputs, diagnostics, _| plan_laid::validate_plans(inputs.program, diagnostics)),
        none(|inputs, diagnostics, _| access_views::validate_plans(inputs.program, diagnostics)),
        none(|inputs, diagnostics, _| relevance::validate_relevance(inputs.program, diagnostics)),
        // Math roster N1: recursive data is legal and PROOF-ONLY (computed, never
        // spelled); every runtime consumption face refuses with the
        // classification named.
        none(|inputs, diagnostics, _| {
            proof_embeddings::validate_proof_embeddings(
                inputs.program,
                &inputs.proof_only,
                diagnostics,
            )
        }),
        Box::new(move |inputs, diagnostics, _| {
            PrologueValue::IntegerEmbeddingCalls(if complete(inputs) {
                proof_embeddings::validate_integer_embedding_calls(inputs.program, diagnostics)
            } else {
                Vec::new()
            })
        }),
        Box::new(|inputs, diagnostics, _| {
            PrologueValue::QuotientFormations(quotients::validate_quotients(
                inputs.program,
                &inputs.proof_only,
                inputs.quotient_admission,
                diagnostics,
            ))
        }),
        Box::new(move |inputs, diagnostics, _| {
            PrologueValue::FactCallProjections(if complete(inputs) {
                fact_call_projections::validate_fact_call_projections(
                    inputs.program,
                    &inputs.conformance_operational,
                    &inputs.conformance_service_reaches,
                    diagnostics,
                )
            } else {
                Vec::new()
            })
        }),
        Box::new(move |inputs, diagnostics, _| {
            PrologueValue::SpecificationCalls(if complete(inputs) {
                crate::machine_calls::denotational_calls::validate_specification_calls(
                    inputs.program,
                    &inputs.conformance_operational,
                    &inputs.conformance_service_reaches,
                    diagnostics,
                )
            } else {
                Vec::new()
            })
        }),
        none(|inputs, diagnostics, _| {
            proof_only_faces::validate_proof_only_consumption(
                inputs.program,
                &inputs.proof_only,
                diagnostics,
            )
        }),
        // Chapter 3 / MR4: runtime call cycles require the constant-stack tail
        // admission; erased proof-only SCCs instead require strict structural descent.
        Box::new(|inputs, diagnostics, _| {
            PrologueValue::CallComponents(call_cycles::validate_machine_call_cycles(
                inputs.program,
                diagnostics,
            ))
        }),
        none(|inputs, diagnostics, _| {
            let (opaque_property_receipts, allow_pending_opaque_copy) =
                match inputs.opaque_properties {
                    OpaquePropertyValidation::Required(receipts) => (receipts, false),
                    OpaquePropertyValidation::PendingBuildSelection => (&[][..], true),
                };
            properties::validate_data_properties(
                inputs.program,
                opaque_property_receipts,
                allow_pending_opaque_copy,
                diagnostics,
            )
        }),
        none(|inputs, diagnostics, _| constants::validate_constants(inputs.program, diagnostics)),
        // Bare-payload-case `==` (decision 11) is checked on the RESOLVED trees,
        // before membership lowering synthesizes its internal tag compares; see
        // 03_symbol-resolved-trees-to-typed-trees/src/equality.rs.
        Box::new(|inputs, diagnostics, obligations| {
            PrologueValue::StructLiterals(struct_literals::validate_struct_literal_fields(
                inputs.program,
                diagnostics,
                obligations,
            ))
        }),
        // Record patterns in LET (owner spec 2026-07-18): the exhaustiveness
        // law on the parse-minted `__destructure#*` marker.
        none(|inputs, diagnostics, _| {
            destructure::validate_destructure_exhaustiveness(inputs.program, diagnostics)
        }),
        none(|inputs, diagnostics, _| {
            recasts::validate_recasts(inputs.program, inputs.call_frames.as_ref(), diagnostics)
        }),
        none(|inputs, diagnostics, _| {
            wire::validate_wire_schemas(inputs.program, &inputs.symbols, diagnostics)
        }),
        none(|inputs, diagnostics, obligations| {
            operators::validate_operator_declarations(
                inputs.program,
                &inputs.symbols,
                diagnostics,
                obligations,
            )
        }),
    ]
}

/// Run every prologue validator, appending to `diagnostics` and
/// `obligations` exactly as running them one after another would.
pub(super) fn run(
    inputs: &PrologueInputs<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    obligations: &mut Vec<IntegerRangeObligation>,
) -> PrologueResults {
    let validators = validators();
    if diagnostics.is_empty()
        && checking_thread_pool().current_num_threads() >= 2
        && let Some(results) = run_apart(inputs, &validators, obligations)
    {
        return results;
    }
    let mut results = PrologueResults::default();
    for validator in &validators {
        let value = validator(inputs, diagnostics, obligations);
        results.take(value);
    }
    results
}

/// Every validator on its own fresh vectors on the checking pool. `None`,
/// with `obligations` untouched, when the outcome shows that some validator
/// may have read what an earlier one appended (see the module comment).
fn run_apart(
    inputs: &PrologueInputs<'_>,
    validators: &[Box<Validator>],
    obligations: &mut Vec<IntegerRangeObligation>,
) -> Option<PrologueResults> {
    let ran = checking_thread_pool().install(|| {
        validators
            .par_iter()
            .map(|validator| {
                let mut diagnostics = Vec::new();
                let mut obligations = Vec::new();
                let value = validator(inputs, &mut diagnostics, &mut obligations);
                (diagnostics, obligations, value)
            })
            .collect::<Vec<_>>()
    });
    if ran
        .iter()
        .any(|(diagnostics, _, _)| !diagnostics.is_empty())
    {
        return None;
    }
    // The only obligation read is a `contains` de-duplication. It would have
    // answered differently only for an obligation equal to one an earlier
    // validator appended; with none, every validator saw the same.
    let starts_before = obligations.len();
    for (position, (_, own, _)) in ran.iter().enumerate() {
        let earlier = obligations[..starts_before]
            .iter()
            .chain(ran[..position].iter().flat_map(|(_, earlier, _)| earlier));
        for earlier in earlier {
            if own.contains(earlier) {
                return None;
            }
        }
    }
    let mut results = PrologueResults::default();
    for (_, own, value) in ran {
        obligations.extend(own);
        results.take(value);
    }
    Some(results)
}
