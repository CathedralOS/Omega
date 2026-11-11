/*
Whole-program semantic validation coordinator (`validate_program` and
`prepare_specialized_program`), declaration prologue, statement and transition
validation, contract entailment scheduling, and checking worker pool.
*/

use crate::declarations::standard_declarations;
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
use crate::proof_contracts::domains::validate_domain_definitions;
use crate::proof_contracts::proof_facts::validate_proposition_definitions;
use crate::proof_contracts::quotients::QuotientRequestAdmission;
use crate::value_custody::data::validate_data_field_types;
use crate::value_custody::locals::WritableRoots;
use crate::value_custody::locals::validate_local_data_names;
use crate::{
    OpaqueDataPropertyReceipt, TopLevelSymbols, ValidatedBoundaryOperatorApplication,
    ValidatedFloatMeaningEqualityProposition, ValidatedFloatMeaningProjectionInvocation,
    ValidatedIntegerEmbeddingCall, ValidatedProofCallComponent, build_definition_fact_plan,
    collect_dynamic_implementation_selections, declarations::declaration_visibility,
    declarations::operators, declarations::transitions, infer_operational_may,
    infer_service_reaches, machine_calls::call_cycles, machine_calls::callable_overloads,
    machine_calls::calls, machine_calls::fact_call_projections, machine_calls::invocations,
    machine_calls::machine_parameters, proof_contracts::arithmetic_domains,
    proof_contracts::default_domains, proof_contracts::float_projection_bindings,
    proof_contracts::float_projection_invocations, proof_contracts::proof_embeddings,
    proof_contracts::proof_only_faces, proof_contracts::properties,
    proof_contracts::proposition_entailment, proof_contracts::qualification_evidence,
    proof_contracts::quotients, proof_contracts::relevance, value_custody::access_views,
    value_custody::cleanup, value_custody::constants, value_custody::content_conservation,
    value_custody::content_projections, value_custody::destructure, value_custody::literals,
    value_custody::plan_laid, value_custody::recasts, value_custody::struct_literals,
    value_custody::wire, value_custody::write_only_borrows,
};
use diagnostics::Diagnostic;
use rayon::prelude::*;
use typed_trees::expression::ExpressionHandle;
use typed_trees::statement::StatementNode;

pub(crate) mod completion_results {
    use crate::machine_calls::calls;
    use diagnostics::Diagnostic;
    use typed_trees::machine::Machine;
    use typed_trees::state::State;
    use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

    pub(crate) fn validate(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !machine.supplies_checked_body() || calls::unit_type(program, state.return_type) {
            return;
        }
        if crate::declarations::standard_declarations::is_core_vector_surface_state(
            program, machine, state,
        ) {
            return;
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        let Ok(Some(order)) = program
            .statement_table
            .finite_execution_order(state.statement_nodes)
        else {
            return;
        };
        for (position, statement_index) in order.iter().copied().enumerate() {
            match &statements[statement_index] {
                StatementNode::Transition(transition) => {
                    if transition.exit == TransitionExit::Ordinary {
                        for target in transition.targets() {
                            if matches!(
                                program.statement_table.transition_target(target),
                                TransitionTargetNode::Terminal
                            ) {
                                report(program, machine, state, "a terminal arm", diagnostics);
                            }
                        }
                    }
                    if crate::declarations::transitions::admitted_dispatch_ends(
                        transition,
                        order.get(position + 1).map(|next| &statements[*next]),
                    ) {
                        return;
                    }
                }
                StatementNode::Expression(expression) => {
                    if position + 1 == order.len()
                        || calls::expression_normal_completion_possible(program, *expression)
                            == Some(false)
                    {
                        return;
                    }
                }
                StatementNode::Call(call)
                    if calls::target_normal_completion_possible(program, call.target_symbol)
                        == Some(false) =>
                {
                    return;
                }
                StatementNode::LocalData(local)
                    if local.ordinary_value_initializer().is_some_and(|value| {
                        calls::expression_normal_completion_possible(program, value) == Some(false)
                    }) =>
                {
                    return;
                }
                StatementNode::Assignment(assignment)
                    if calls::expression_normal_completion_possible(program, assignment.value)
                        == Some(false) =>
                {
                    return;
                }
                _ => {}
            }
        }
        if statements.is_empty() {
            diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` declares a return type but its body is empty -- return a value or drop the `-> T`",
            machine.name, state.name,
        )));
        } else {
            report(
                program,
                machine,
                state,
                "implicit body completion",
                diagnostics,
            );
        }
    }

    fn report(
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        completion: &str,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` {completion} supplies Unit, but the invocation returns `{}`; every normal completion must supply its declared result",
        machine.name, state.name, program.display_type_reference_with_constraints(state.return_type),
    )));
    }
}
pub(crate) mod contract_queries {
    use crate::program_validation::finish_diagnostics;
    use crate::proof_contracts::contract_entailment;
    use crate::proof_contracts::contract_entailment::validate_machine_contract_entailment;
    use diagnostics::Diagnostic;
    use typed_trees::data::TypeParameterKind;

    pub fn validate_checked_operator_realization_contract(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        operator: &typed_trees::operator::OperatorDefinition,
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

    pub fn validate_generic_machine_contract_entailment(
        program: &dyn crate::CheckingInput,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();
        for machine in program.machines() {
            if program
                .machine_type_parameters(machine)
                .iter()
                .any(|parameter| matches!(parameter.kind, TypeParameterKind::Machine { .. }))
            {
                validate_machine_contract_entailment(
                    program,
                    machine,
                    &mut diagnostics,
                    &mut crate::ProvenContractExpressions::default(),
                );
            }
        }
        finish_diagnostics(diagnostics)
    }
}
pub(crate) mod declaration_prologue {
    use super::*;
    use checked_trees::IntegerRangeObligation;

    pub(crate) struct PrologueInputs<'program> {
        pub(crate) program: &'program dyn crate::CheckingInput,
        pub(crate) effect_admission: EffectAdmission,
        pub(crate) quotient_admission: QuotientRequestAdmission,
        pub(crate) opaque_properties: OpaquePropertyValidation<'program>,
        pub(crate) symbols: TopLevelSymbols<'program>,
        pub(crate) symbol_diagnostics: Vec<Diagnostic>,
        pub(crate) fact_plan: facts::FactPlan,
        pub(crate) conformance_operational: flow_effects::OperationalPlan,
        pub(crate) conformance_service_reaches: flow_effects::ServiceReachInferencePlan,
        pub(crate) proof_only: std::sync::Arc<typed_trees::proof_only::ProofOnlyClassification>,
        pub(crate) call_frames: Option<calls::CallFrameResolver<'program>>,
    }

    impl<'program> PrologueInputs<'program> {
        pub(crate) fn build(
            program: &'program dyn crate::CheckingInput,
            effect_admission: EffectAdmission,
            quotient_admission: QuotientRequestAdmission,
            opaque_properties: OpaquePropertyValidation<'program>,
        ) -> Self {
            let mut symbol_diagnostics = Vec::new();
            let symbols = TopLevelSymbols::build(program, &mut symbol_diagnostics);
            let fact_plan = build_definition_fact_plan(program);
            let conformance_operational = infer_operational_may(program);
            let conformance_service_reaches =
                infer_service_reaches(program, &conformance_operational);
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

    pub(crate) enum PrologueValue {
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

    #[derive(Default)]
    pub(crate) struct PrologueResults {
        pub(crate) integer_embedding_calls: Vec<ValidatedIntegerEmbeddingCall>,
        pub(crate) quotient_formations: Vec<crate::ValidatedQuotientFormation>,
        pub(crate) fact_call_projections:
            Vec<crate::machine_calls::fact_call_projections::ValidatedFactCallProjection>,
        pub(crate) specification_calls: Vec<crate::SpecificationCallRequest>,
        pub(crate) proof_recursive_components: Vec<ValidatedProofCallComponent>,
        pub(crate) struct_literals: Option<struct_literals::StructLiteralValidation>,
    }

    impl PrologueResults {
        fn take(&mut self, value: PrologueValue) {
            match value {
                PrologueValue::None => {}
                PrologueValue::IntegerEmbeddingCalls(calls) => self.integer_embedding_calls = calls,
                PrologueValue::QuotientFormations(formations) => {
                    self.quotient_formations = formations
                }
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

    pub(crate) type Validator = dyn Fn(
            &PrologueInputs<'_>,
            &mut Vec<Diagnostic>,
            &mut Vec<IntegerRangeObligation>,
        ) -> PrologueValue
        + Sync;

    fn validators() -> Vec<Box<Validator>> {
        fn none(
            validate: impl Fn(
                &PrologueInputs<'_>,
                &mut Vec<Diagnostic>,
                &mut Vec<IntegerRangeObligation>,
            ) + Sync
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
                validate_trait_requirements(
                    inputs.program,
                    &inputs.symbols,
                    diagnostics,
                    obligations,
                )
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
            none(|inputs, diagnostics, _| {
                access_views::validate_plans(inputs.program, diagnostics)
            }),
            none(|inputs, diagnostics, _| {
                relevance::validate_relevance(inputs.program, diagnostics)
            }),
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
            none(|inputs, diagnostics, _| {
                constants::validate_constants(inputs.program, diagnostics)
            }),
            Box::new(|inputs, diagnostics, obligations| {
                PrologueValue::StructLiterals(struct_literals::validate_struct_literal_fields(
                    inputs.program,
                    diagnostics,
                    obligations,
                ))
            }),
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

    pub(crate) fn run(
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
}
pub(crate) mod dependency_bodies {
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::signature::SignatureContractKind;

    use super::ProvenContractExpressions;
    use super::statements::statement_source_span;
    use crate::proof_contracts::contract_entailment::structural_prefix::private_invocation_index;

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct FrozenDependencyBody {
        pub identity: String,
        pub body_hash: u64,
        pub dependency_context: u64,
    }

    #[derive(Debug, Clone, Copy)]
    pub(crate) struct BodyAdmission {
        body_hash: u64,
        dependency_context: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
    pub struct DependencyBodyJudgment {
        pub identity: String,
        pub body_hash: u64,
        pub dependency_context: u64,
        pub ensures_len: u32,
        pub proved_normal_return_ordinals: Vec<u32>,
        pub proved_entry_ordinals: Vec<u32>,
    }

    pub(crate) struct CapturedEntailment {
        pub(crate) ensures_len: u32,
        pub(crate) proved_normal_return_ordinals: Vec<u32>,
        pub(crate) proved_entry_ordinals: Vec<u32>,
    }

    pub(crate) struct PriorJudgment {
        identity: String,
        body_hash: u64,
        dependency_context: u64,
        ensures_len: u32,
        proved_normal_return_ordinals: Vec<u32>,
        proved_entry_ordinals: Vec<u32>,
    }

    pub(crate) struct DependencyBodyIndex {
        active: bool,
        record: bool,
        identities: Vec<Option<String>>,
        admissions: Vec<Option<BodyAdmission>>,
        callees: Vec<Vec<u32>>,
        priors: Vec<PriorJudgment>,
        frozen_priors: Vec<FrozenDependencyBody>,
    }

    impl DependencyBodyIndex {
        pub(crate) fn inactive() -> Self {
            Self {
                active: false,
                record: false,
                identities: Vec::new(),
                admissions: Vec::new(),
                callees: Vec::new(),
                priors: Vec::new(),
                frozen_priors: Vec::new(),
            }
        }

        pub(crate) fn prepare(
            program: &dyn crate::CheckingInput,
            priors: &[DependencyBodyJudgment],
            frozen_priors: &[FrozenDependencyBody],
            record: bool,
        ) -> Self {
            if priors.is_empty() && frozen_priors.is_empty() && !record {
                return Self::inactive();
            }
            let machines = program.machines();
            let machine_count = machines.len();
            let mut machine_of_symbol = Vec::new();
            for (machine_index, machine) in machines.iter().enumerate() {
                let machine_index = u32::try_from(machine_index).unwrap_or(u32::MAX);
                for symbol in std::iter::once(machine.symbol).chain(
                    program
                        .machine_states(machine)
                        .iter()
                        .map(|state| state.symbol),
                ) {
                    let symbol_index = symbol.arena_index() as usize;
                    if machine_of_symbol.len() <= symbol_index {
                        machine_of_symbol.resize(symbol_index + 1, u32::MAX);
                    }
                    machine_of_symbol[symbol_index] = machine_index;
                }
            }
            let mut identities = Vec::with_capacity(machine_count);
            for machine in machines {
                identities.push(
                    crate::machine_calls::contract_identities::normalized_machine_identity(
                        program, machine,
                    ),
                );
            }
            let sources = program.symbols.source_map();
            let mut body_hashes = Vec::with_capacity(machine_count);
            let mut callees = Vec::with_capacity(machine_count);
            for machine in machines {
                let (body_hash, machine_callees) =
                    local_body_hash(program, machine, sources.as_deref(), &machine_of_symbol);
                body_hashes.push(body_hash);
                callees.push(machine_callees);
            }
            let mut admissions = Vec::with_capacity(machine_count);
            let mut other_symbols = Vec::new();
            for (machine_index, machine) in machines.iter().enumerate() {
                let admission = body_hashes[machine_index].map(|body_hash| BodyAdmission {
                    body_hash,
                    dependency_context: dependency_context(
                        program,
                        machine,
                        &body_hashes,
                        &callees[machine_index],
                        &identities,
                        &machine_of_symbol,
                        &mut other_symbols,
                    ),
                });
                let admission = admission.and_then(|admission| {
                    substitution_context(
                        program,
                        machine,
                        admission,
                        &identities,
                        &machine_of_symbol,
                        &mut other_symbols,
                    )
                });
                admissions.push(admission);
            }
            exclude_private_arrival_dependencies(program, &callees, &mut admissions);
            Self {
                active: true,
                record,
                identities,
                admissions,
                callees,
                priors: collapse_prior_judgments(priors),
                frozen_priors: frozen_priors.to_vec(),
            }
        }

        pub(crate) fn admits_frozen_body(&self, machine_index: usize) -> bool {
            let Some(admission) = self.admissions.get(machine_index).and_then(Option::as_ref)
            else {
                return false;
            };
            let Some(identity) = self.identities.get(machine_index).and_then(Option::as_ref) else {
                return false;
            };
            self.frozen_priors
                .iter()
                .any(|prior| prior.identity == *identity && same_body_and_context(prior, admission))
                && self.frozen_callees_match(machine_index)
        }

        pub(crate) fn frozen_bodies(&self) -> Vec<FrozenDependencyBody> {
            if !self.record {
                return Vec::new();
            }
            self.identities
                .iter()
                .zip(&self.admissions)
                .filter_map(|(identity, admission)| {
                    let admission = admission.as_ref()?;
                    Some(FrozenDependencyBody {
                        identity: identity.as_ref()?.clone(),
                        body_hash: admission.body_hash,
                        dependency_context: admission.dependency_context,
                    })
                })
                .collect()
        }

        fn frozen_callees_match(&self, machine_index: usize) -> bool {
            let Some(callees) = self.callees.get(machine_index) else {
                return false;
            };
            for callee in callees {
                let callee = *callee as usize;
                let Some(admission) = self.admissions.get(callee).and_then(Option::as_ref) else {
                    return false;
                };
                let Some(identity) = self.identities.get(callee).and_then(Option::as_ref) else {
                    return false;
                };
                if !self.frozen_priors.iter().any(|prior| {
                    prior.identity == *identity && same_body_and_context(prior, admission)
                }) {
                    return false;
                }
            }
            true
        }

        pub(crate) fn active_observation(&self) -> bool {
            self.active
        }

        pub(crate) fn reuse(
            &self,
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            machine_index: usize,
        ) -> Option<ProvenContractExpressions> {
            if !self.active || self.priors.is_empty() {
                return None;
            }
            let admission = self.admissions.get(machine_index)?.as_ref()?;
            let identity = self.identities.get(machine_index)?.as_ref()?;
            let prior = find_prior(&self.priors, identity)?;
            if prior.body_hash != admission.body_hash
                || prior.dependency_context != admission.dependency_context
                || !self.callees_match(machine_index)
            {
                return None;
            }
            let ensures = ensures_expressions(program, machine);
            if ensures.len() != prior.ensures_len as usize {
                return None;
            }
            proved_from_ordinals(
                &ensures,
                &prior.proved_entry_ordinals,
                &prior.proved_normal_return_ordinals,
            )
        }

        pub(crate) fn capture(
            &self,
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            machine_index: usize,
            proven: &ProvenContractExpressions,
        ) -> Option<CapturedEntailment> {
            if !self.record {
                return None;
            }
            if !proven.conditional_content.is_empty() {
                return None;
            }
            if self.admissions.get(machine_index)?.is_none() {
                return None;
            }
            if self.identities.get(machine_index)?.is_none() {
                return None;
            }
            let ensures = ensures_expressions(program, machine);
            let ensures_len = u32::try_from(ensures.len()).ok()?;
            Some(CapturedEntailment {
                ensures_len,
                proved_entry_ordinals: ordinals(&ensures, &proven.entry)?,
                proved_normal_return_ordinals: ordinals(&ensures, &proven.normal_returns)?,
            })
        }

        pub(crate) fn judgments(
            &self,
            captured: &[Option<CapturedEntailment>],
        ) -> Vec<DependencyBodyJudgment> {
            if !self.record {
                return Vec::new();
            }
            let mut judgments = Vec::new();
            for (machine_index, captured) in captured.iter().enumerate() {
                let Some(captured) = captured else {
                    continue;
                };
                let Some(identity) = self.identities.get(machine_index).and_then(Option::as_ref)
                else {
                    continue;
                };
                let Some(admission) = self.admissions.get(machine_index).and_then(Option::as_ref)
                else {
                    continue;
                };
                judgments.push(DependencyBodyJudgment {
                    identity: identity.clone(),
                    body_hash: admission.body_hash,
                    dependency_context: admission.dependency_context,
                    ensures_len: captured.ensures_len,
                    proved_normal_return_ordinals: captured.proved_normal_return_ordinals.clone(),
                    proved_entry_ordinals: captured.proved_entry_ordinals.clone(),
                });
            }
            collapse_prior_judgments(&judgments)
                .into_iter()
                .map(|prior| DependencyBodyJudgment {
                    identity: prior.identity,
                    body_hash: prior.body_hash,
                    dependency_context: prior.dependency_context,
                    ensures_len: prior.ensures_len,
                    proved_normal_return_ordinals: prior.proved_normal_return_ordinals,
                    proved_entry_ordinals: prior.proved_entry_ordinals,
                })
                .collect()
        }

        fn callees_match(&self, machine_index: usize) -> bool {
            let Some(callees) = self.callees.get(machine_index) else {
                return false;
            };
            for callee in callees {
                let callee = *callee as usize;
                let Some(admission) = self.admissions.get(callee).and_then(Option::as_ref) else {
                    return false;
                };
                let Some(identity) = self.identities.get(callee).and_then(Option::as_ref) else {
                    return false;
                };
                match find_prior(&self.priors, identity) {
                    Some(prior)
                        if prior.body_hash == admission.body_hash
                            && prior.dependency_context == admission.dependency_context => {}
                    _ => return false,
                }
            }
            true
        }
    }

    fn machine_slot(machine_of_symbol: &[u32], symbol: symbols::SymbolHandle) -> Option<usize> {
        let slot = *machine_of_symbol.get(symbol.arena_index() as usize)?;
        (slot != u32::MAX).then_some(slot as usize)
    }

    fn find_prior<'a>(priors: &'a [PriorJudgment], identity: &str) -> Option<&'a PriorJudgment> {
        let index = priors
            .binary_search_by(|prior| prior.identity.as_str().cmp(identity))
            .ok()?;
        Some(&priors[index])
    }

    fn collapse_prior_judgments(judgments: &[DependencyBodyJudgment]) -> Vec<PriorJudgment> {
        let mut ordered = judgments.to_vec();
        ordered.sort_by(|left, right| left.identity.cmp(&right.identity));
        let mut collapsed = Vec::new();
        let mut index = 0;
        while index < ordered.len() {
            let mut end = index + 1;
            while end < ordered.len() && ordered[end].identity == ordered[index].identity {
                end += 1;
            }
            let group = &ordered[index..end];
            let first = &group[0];
            let same = group.iter().all(|judgment| {
                judgment.body_hash == first.body_hash
                    && judgment.dependency_context == first.dependency_context
                    && judgment.ensures_len == first.ensures_len
                    && judgment.proved_normal_return_ordinals == first.proved_normal_return_ordinals
                    && judgment.proved_entry_ordinals == first.proved_entry_ordinals
            });
            if same {
                collapsed.push(PriorJudgment {
                    identity: first.identity.clone(),
                    body_hash: first.body_hash,
                    dependency_context: first.dependency_context,
                    ensures_len: first.ensures_len,
                    proved_normal_return_ordinals: first.proved_normal_return_ordinals.clone(),
                    proved_entry_ordinals: first.proved_entry_ordinals.clone(),
                });
            }
            index = end;
        }
        collapsed
    }

    fn same_body_and_context(prior: &FrozenDependencyBody, admission: &BodyAdmission) -> bool {
        prior.body_hash == admission.body_hash
            && prior.dependency_context == admission.dependency_context
    }

    fn substitution_context(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        admission: BodyAdmission,
        identities: &[Option<String>],
        machine_of_symbol: &[u32],
        other_symbols: &mut Vec<Option<String>>,
    ) -> Option<BodyAdmission> {
        let mut matched = None;
        for specialization in &program.machine_specializations {
            if specialization.instance != machine.symbol {
                continue;
            }
            if matched.is_some() {
                return None;
            }
            matched = Some(specialization);
        }
        let Some(specialization) = matched else {
            return Some(admission);
        };
        let mut hash = semantic_vocabulary::Fnv1a::resume(admission.dependency_context);
        hash.byte(9);
        hash.string_terminated(&specialization.normalized_template_identity);
        for text in specialization
            .type_argument_identities
            .iter()
            .chain(specialization.const_argument_identities.iter())
        {
            hash.string_terminated(text);
        }
        for symbol in specialization
            .machine_arguments
            .iter()
            .chain(specialization.conformance_arguments.iter())
        {
            hash_target(
                &mut hash,
                program,
                *symbol,
                identities,
                machine_of_symbol,
                other_symbols,
            );
        }
        for application in &specialization.conformance_applications {
            for text in application
                .lifetime_arguments
                .iter()
                .chain(application.type_arguments.iter())
                .chain(application.trait_lifetime_arguments.iter())
                .chain(application.trait_arguments.iter())
            {
                hash.string_terminated(text);
            }
            for symbol in std::iter::once(application.declaration)
                .chain(std::iter::once(application.trait_definition))
                .chain(application.machine_arguments.iter().copied())
            {
                hash_target(
                    &mut hash,
                    program,
                    symbol,
                    identities,
                    machine_of_symbol,
                    other_symbols,
                );
            }
            for row in &application.rows {
                for symbol in [
                    row.declaring_trait,
                    row.requirement,
                    row.realization_machine,
                    row.realization_state,
                ] {
                    hash_target(
                        &mut hash,
                        program,
                        symbol,
                        identities,
                        machine_of_symbol,
                        other_symbols,
                    );
                }
            }
        }
        Some(BodyAdmission {
            body_hash: admission.body_hash,
            dependency_context: hash.finish(),
        })
    }

    fn exclude_private_arrival_dependencies(
        program: &dyn crate::CheckingInput,
        callees: &[Vec<u32>],
        admissions: &mut [Option<BodyAdmission>],
    ) {
        let index = private_invocation_index(program);
        let mut callers = vec![Vec::new(); callees.len()];
        for (caller, targets) in callees.iter().enumerate() {
            for &target in targets {
                callers[target as usize].push(caller);
            }
        }
        let mut excluded = program
            .machines()
            .iter()
            .map(|machine| index.has_complete_private_uses(program, machine))
            .collect::<Vec<_>>();
        let mut pending = excluded
            .iter()
            .enumerate()
            .filter_map(|(machine, &exclude)| exclude.then_some(machine))
            .collect::<Vec<_>>();
        while let Some(machine) = pending.pop() {
            admissions[machine] = None;
            for &caller in &callers[machine] {
                if !excluded[caller] {
                    excluded[caller] = true;
                    pending.push(caller);
                }
            }
        }
    }

    fn dependency_context(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        body_hashes: &[Option<u64>],
        callees: &[u32],
        identities: &[Option<String>],
        machine_of_symbol: &[u32],
        other_symbols: &mut Vec<Option<String>>,
    ) -> u64 {
        let mut context = semantic_vocabulary::Fnv1a::new();
        hash_producer_context(&mut context, program, machine.symbol);
        for callee in callees {
            let callee = *callee as usize;
            context.byte(1);
            if let Some(identity) = identities.get(callee).and_then(Option::as_ref) {
                context.string_terminated(identity);
            } else {
                context.byte(0);
            }
            match body_hashes.get(callee).and_then(Option::as_ref) {
                Some(body_hash) => {
                    context.byte(1);
                    context.u64(*body_hash);
                }
                None => context.byte(0),
            }
            if let Some(machine) = program.machines().get(callee) {
                hash_producer_context(&mut context, program, machine.symbol);
            }
        }
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            for (statement_index, _) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                for call in
                    crate::collect_call_sites_in_statement(program, machine, state, statement_index)
                {
                    let target = call.target_symbol(program);
                    if machine_slot(machine_of_symbol, target).is_none() {
                        hash_target(
                            &mut context,
                            program,
                            target,
                            identities,
                            machine_of_symbol,
                            other_symbols,
                        );
                    }
                }
                let _ = state_index;
            }
        }
        context.finish()
    }

    fn local_body_hash(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        sources: Option<&source::SourceMap>,
        machine_of_symbol: &[u32],
    ) -> (Option<u64>, Vec<u32>) {
        let Some(sources) = sources else {
            return (None, Vec::new());
        };
        let mut body = semantic_vocabulary::Fnv1a::new();
        hash_supply(&mut body, machine.supply_mode);
        let mut authored_statements = 0u32;
        let mut statement_count = 0u32;
        let mut callees = Vec::new();
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                statement_count = statement_count.saturating_add(1);
                match statement_source_span(program, statement) {
                    Some(span) => {
                        authored_statements = authored_statements.saturating_add(1);
                        body.byte(1);
                        body.string_terminated(sources.text_at(span));
                    }
                    None => body.byte(0),
                }
                for call in
                    crate::collect_call_sites_in_statement(program, machine, state, statement_index)
                {
                    let target = call.target_symbol(program);
                    if let Some(callee) = machine_slot(machine_of_symbol, target)
                        && let Ok(callee) = u32::try_from(callee)
                    {
                        callees.push(callee);
                    }
                }
                let _ = state_index;
            }
        }
        if machine.supplies_checked_body() && statement_count > 0 && authored_statements == 0 {
            return (None, callees);
        }
        for contract in program.machine_contracts(machine) {
            body.byte(match contract.kind {
                SignatureContractKind::Requires => 1,
                SignatureContractKind::Ensures => 2,
                SignatureContractKind::EnsuresForResultCase { .. } => 3,
                SignatureContractKind::Crashes { .. } => 4,
            });
            for fact in contract.facts.handles() {
                if !hash_fact(&mut body, program, sources, fact) {
                    return (None, callees);
                }
            }
        }
        (Some(body.finish()), callees)
    }

    fn hash_producer_context(
        hash: &mut semantic_vocabulary::Fnv1a,
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
    ) {
        let Some(span) = program.symbols.symbol_provenance_source_span(symbol) else {
            hash.byte(0);
            return;
        };
        let Some(file) = program.symbols.source_file(span) else {
            hash.byte(0);
            return;
        };
        hash.byte(match file.dependency_scope {
            source::DependencyScope::Product => 1,
            source::DependencyScope::Build => 2,
        });
        match &file.package_identity {
            Some(package) => {
                hash.byte(1);
                hash.bytes(&package.digest());
            }
            None => hash.byte(0),
        }
    }

    fn hash_fact(
        hash: &mut semantic_vocabulary::Fnv1a,
        program: &dyn crate::CheckingInput,
        sources: &source::SourceMap,
        fact: arena::Handle<typed_trees::domain::ProofFact>,
    ) -> bool {
        let authored = program
            .proof_fact_source_span(fact)
            .filter(|span| *span != source::SourceSpan::default());
        let text = if let Some(span) = authored {
            sources.text_at(span)
        } else if let ProofFact::Expression(expression) = program.proof_facts.get(fact) {
            let span = program.expression_table.source_span(*expression);
            if span == source::SourceSpan::default() {
                return false;
            }
            sources.text_at(span)
        } else {
            return false;
        };
        hash.byte(1);
        hash.string_terminated(text);
        true
    }

    fn hash_target(
        hash: &mut semantic_vocabulary::Fnv1a,
        program: &dyn crate::CheckingInput,
        symbol: symbols::SymbolHandle,
        identities: &[Option<String>],
        machine_of_symbol: &[u32],
        other_symbols: &mut Vec<Option<String>>,
    ) {
        if let Some(machine_index) = machine_slot(machine_of_symbol, symbol)
            && let Some(identity) = identities.get(machine_index).and_then(Option::as_ref)
        {
            hash.byte(1);
            hash.string_terminated(identity);
            return;
        }
        let symbol_index = symbol.arena_index() as usize;
        if other_symbols.len() <= symbol_index {
            other_symbols.resize(symbol_index + 1, None);
        }
        if other_symbols[symbol_index].is_none() {
            other_symbols[symbol_index] = Some(
                crate::machine_calls::contract_identities::conformance_symbol_identity(
                    program, symbol,
                ),
            );
        }
        hash.byte(2);
        hash.string_terminated(other_symbols[symbol_index].as_deref().unwrap_or(""));
    }

    fn hash_supply(
        hash: &mut semantic_vocabulary::Fnv1a,
        mode: language_semantics::MachineSupplyMode,
    ) {
        match mode {
            language_semantics::MachineSupplyMode::CheckedBody => hash.byte(0),
            language_semantics::MachineSupplyMode::Requirement => hash.byte(1),
            language_semantics::MachineSupplyMode::TopLevelRequirement => hash.byte(2),
            language_semantics::MachineSupplyMode::Boundary => hash.byte(3),
            language_semantics::MachineSupplyMode::AdmissionClaim => hash.byte(4),
            language_semantics::MachineSupplyMode::ExternalRealization { binding, mechanism } => {
                hash.byte(5);
                hash.u32(binding.map(|binding| binding.0).unwrap_or(0));
                hash.byte(
                    mechanism
                        .map(language_semantics::ExternalBindingMechanism::identity_tag)
                        .unwrap_or(0),
                );
            }
        }
    }

    pub(crate) fn ensures_expressions(
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
    ) -> Vec<ExpressionHandle> {
        let mut ensures = Vec::new();
        for contract in program.machine_contracts(machine) {
            if !matches!(contract.kind, SignatureContractKind::Ensures) {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let ProofFact::Expression(expression) = fact {
                    ensures.push(*expression);
                }
            }
        }
        ensures
    }

    fn ordinals(ensures: &[ExpressionHandle], proved: &[ExpressionHandle]) -> Option<Vec<u32>> {
        let mut used = vec![false; ensures.len()];
        let mut ordinals = Vec::with_capacity(proved.len());
        for handle in proved {
            let ordinal = ensures
                .iter()
                .enumerate()
                .position(|(index, candidate)| !used[index] && candidate == handle)?;
            used[ordinal] = true;
            ordinals.push(u32::try_from(ordinal).ok()?);
        }
        Some(ordinals)
    }

    fn proved_from_ordinals(
        ensures: &[ExpressionHandle],
        entry: &[u32],
        normal_returns: &[u32],
    ) -> Option<ProvenContractExpressions> {
        let mut proven = ProvenContractExpressions::default();
        for ordinal in entry {
            proven
                .entry
                .push(*ensures.get(usize::try_from(*ordinal).ok()?)?);
        }
        for ordinal in normal_returns {
            proven
                .normal_returns
                .push(*ensures.get(usize::try_from(*ordinal).ok()?)?);
        }
        Some(proven)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn index_for(context: u64) -> DependencyBodyIndex {
            let body_hash = 41;
            DependencyBodyIndex {
                active: true,
                record: false,
                identities: vec![Some("package:aa::wrapper".to_owned())],
                admissions: vec![Some(BodyAdmission {
                    body_hash,
                    dependency_context: context,
                })],
                callees: vec![Vec::new()],
                priors: Vec::new(),
                frozen_priors: vec![FrozenDependencyBody {
                    identity: "package:aa::wrapper".to_owned(),
                    body_hash,
                    dependency_context: 7,
                }],
            }
        }

        #[test]
        fn same_body_hash_with_different_dependency_context_is_refused() {
            assert!(!index_for(9).admits_frozen_body(0));
            assert!(index_for(7).admits_frozen_body(0));
        }
    }
}
pub(crate) mod statements {
    pub(crate) mod assembly_fact_statements {
        use super::{StatementOutputs, StatementScope};
        use crate::proof_contracts::proof_facts;
        use diagnostics::Diagnostic;
        use typed_trees::statement::StatementNode;

        pub(crate) fn validate(
            scope: &StatementScope<'_>,
            outputs: &mut StatementOutputs<'_>,
            statement: &StatementNode,
        ) {
            let Some(fact) = statement.as_assembly_fact() else {
                unreachable!("dispatched assembly_fact_statements")
            };
            let (program, machine, state_name, state, symbols, writable_roots) = scope.reads();
            let diagnostics = &mut *outputs.diagnostics;
            if let Some(state) = state {
                for expression in program.tables.proof_fact_expression_roots(fact.fact) {
                    crate::value_custody::locals::StateValueScope {
                        program,
                        machine,
                        state,
                        symbols,
                        prior_statements: writable_roots.statements,
                        context: "asm assertion",
                    }
                    .expression(expression, diagnostics);
                }
            }
            let proof_fact = program.proof_facts.get(fact.fact);
            if let Some(state) = state {
                proof_facts::validate_proof_facts(
                    program,
                    std::slice::from_ref(proof_fact),
                    diagnostics,
                    proof_facts::ProofFactOwner::StatementContract {
                        machine,
                        state,
                        statement_index: scope.statement_index,
                        kind: "asm",
                    },
                );
            }
            if let typed_trees::domain::ProofFact::Expression(expression) = proof_fact
                && !proof_facts::is_boolean_asm_fact_expression(
                    program,
                    machine,
                    state,
                    *expression,
                )
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` state `{state_name}` asm fact `{}` is not boolean-shaped",
                    machine.name,
                    program.expression_table.display_name(*expression)
                )));
            }
        }
    }
    pub(crate) mod assignment_statements {

        use super::{StatementOutputs, StatementScope};
        use crate::value_custody::places::validate_assignment_target_handle;
        use crate::{
            machine_calls::calls, proof_contracts::arithmetic_domains,
            proof_contracts::domain_weakening, value_custody::access_views,
            value_custody::atomic_operations, value_custody::expression_types,
            value_custody::places, value_custody::struct_literals,
        };
        use numerics::arithmetic::ArithmeticDomain;
        use typed_trees::statement::StatementNode;

        pub(crate) fn validate<'a, 'p>(
            scope: &StatementScope<'a>,
            outputs: &mut StatementOutputs<'_>,
            statement: &StatementNode,
            direct_written: Option<Vec<String>>,
            bound_lookup: &mut (
                &'p dyn crate::CheckingInput,
                Option<crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'p>>,
            ),
        ) {
            let Some(assignment) = statement.as_assignment() else {
                unreachable!("dispatched assignment_statements")
            };
            let (program, machine, state_name, current_state, _, writable_roots) = scope.reads();
            let value_environment = &mut *outputs.value_environment;
            let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
            let diagnostics = &mut *outputs.diagnostics;
            let state = current_state;
            if !state.is_some_and(|state| {
                access_views::assignment_is_access_atomic_operation(
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
                    atomic_operations::assignment_receiver_polarity(
                        program, machine, state, assignment,
                    ),
                );
            }
            calls::validate_asm_value_destination(program, machine, state, assignment, diagnostics);
            let atomic = atomic_operations::atomic_assignment_carrier(program, assignment);
            let resident_type = |reference| {
                if atomic.is_some() {
                    crate::atomic_resident_type_reference(program, reference).unwrap_or(reference)
                } else {
                    reference
                }
            };
            let assignment_target_type =
                places::declared_place_type(program, machine, state, assignment.target)
                    .or_else(|| {
                        places::declared_indexed_projection_type(
                            program,
                            machine,
                            current_state,
                            assignment.target,
                        )
                    })
                    .map(resident_type);
            let assignment_target_type_raw =
                places::declared_place_type_raw(program, machine, state, assignment.target)
                    .or_else(|| {
                        places::declared_indexed_projection_type_raw(
                            program,
                            machine,
                            current_state,
                            assignment.target,
                        )
                    });
            let assignment_target_primitive =
                assignment_target_type.and_then(|handle| program.primitive_type_reference(handle));
            if let Some(state) = current_state {
                struct_literals::validate_array_window_elements(
                    program,
                    machine,
                    state,
                    assignment.target,
                    assignment.value,
                    bound_lookup,
                    diagnostics,
                    &mut arithmetic_decisions.integer_range_obligations,
                );
            }
            if let Some(current_state) = current_state
                && let Some(target_type) = assignment_target_type
            {
                struct_literals::validate_array_literal_elements(
                    program,
                    machine,
                    current_state,
                    assignment.value,
                    target_type,
                    diagnostics,
                    &mut arithmetic_decisions.integer_range_obligations,
                );
            }
            let owner = format!("machine `{}` state `{state_name}` assignment", machine.name);
            if let Some(state) = current_state {
                crate::value_custody::recasts::pending_views::report_whole_view_write(
                    program,
                    machine,
                    state,
                    scope.statement_index,
                    assignment.target,
                    diagnostics,
                );
                if let Some(expected) = assignment_target_type {
                    crate::value_custody::recasts::pending_views::report_consumption(
                        program,
                        machine,
                        state,
                        scope.statement_index,
                        assignment.value,
                        expected,
                        &owner,
                        diagnostics,
                    );
                }
            }

            if let Some(event) = &atomic
                && !crate::place_is_atomic_storage(program, machine, current_state, event.place)
                && !current_state.is_some_and(|state| {
                    access_views::assignment_is_access_atomic_operation(
                        program, machine, state, assignment,
                    )
                })
            {
                diagnostics.push(diagnostics::Diagnostic::error(format!(
            "{owner} requires dedicated atomic storage or an admitted placed atomic accessor; ordinary write authority does not authorize an atomic event"
        )));
            }
            let ordinary_value = [assignment.value];
            let delivered_values = atomic.as_ref().map_or(ordinary_value.as_slice(), |atomic| {
                atomic.operands.as_slice()
            });
            let resident_replacement = atomic.as_ref().and_then(|event| {
                use access_plans::AtomicAccessOperation as A;
                match event.operation {
                    A::Store(_) | A::Swap(_) => event.operands.first().copied(),
                    A::CompareExchange { .. } => event.operands.get(1).copied(),
                    _ => None,
                }
            });
            let atomic_fetch = atomic.as_ref().and_then(|event| {
                use access_plans::AtomicAccessOperation as A;
                use typed_trees::expression::BinaryOperator;
                let (operator, operand) = match event.operation {
                    A::FetchAdd(_) => (BinaryOperator::Add, event.operands.first().copied()?),
                    A::FetchSub(_) => (BinaryOperator::Subtract, event.operands.first().copied()?),
                    A::FetchXor(_) => {
                        (BinaryOperator::BitwiseXor, event.operands.first().copied()?)
                    }
                    A::FetchOr(_) => (BinaryOperator::BitwiseOr, event.operands.first().copied()?),
                    A::FetchAnd(_) => {
                        (BinaryOperator::BitwiseAnd, event.operands.first().copied()?)
                    }
                    _ => return None,
                };
                Some((operator, operand))
            });
            let established_value = resident_replacement
                .or(atomic_fetch.map(|(_, operand)| operand))
                .unwrap_or(assignment.value);
            for &value in delivered_values {
                if let Some(target_primitive) = assignment_target_primitive {
                    expression_types::report_cross_class_store(
                        program,
                        Some(machine),
                        current_state,
                        value,
                        target_primitive,
                        &owner,
                        "place",
                        diagnostics,
                    );
                }
                if let Some(target_type) = assignment_target_type {
                    expression_types::report_data_type_conflict(
                        program,
                        machine,
                        current_state,
                        value,
                        target_type,
                        &owner,
                        "place",
                        diagnostics,
                    );
                    expression_types::report_delivered_type_mismatch_with_component(
                        program,
                        machine.symbol,
                        value,
                        target_type,
                        program.pattern_component(assignment.target),
                        &owner,
                        diagnostics,
                    );
                    expression_types::report_array_scalar_shape_mismatch(
                        program,
                        machine,
                        current_state,
                        value,
                        target_type,
                        &owner,
                        "place",
                        diagnostics,
                    );
                    expression_types::report_scalar_data_shape_mismatch(
                        program,
                        machine,
                        current_state,
                        value,
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
                        current_state,
                        value,
                        target_type,
                        &owner,
                        diagnostics,
                    );
                }
                if atomic.is_some()
                    && value != established_value
                    && let Some(target_primitive) = assignment_target_primitive
                {
                    arithmetic_domains::check_value_narrowing(
                        program,
                        machine,
                        current_state,
                        value,
                        target_primitive,
                        assignment_target_type_raw.map_or(ArithmeticDomain::Exact, |reference| {
                            program.arithmetic_domain_for_type_reference(
                                places::assignment_value_type(program, reference),
                            )
                        }),
                        value_environment,
                        &owner,
                        diagnostics,
                    );
                }
            }
            if let Some((projections, _)) = crate::queries::expression_traversal::projection_climb(
                program,
                assignment.target,
                usize::MAX,
            ) {
                for projection in projections {
                    if let typed_trees::expression::ExpressionNode::Indexed(indexed) =
                        program.expression_table.expression(projection)
                    {
                        arithmetic_domains::validate_value_range_retaining_decisions(
                            program,
                            machine,
                            current_state,
                            indexed.index,
                            value_environment,
                            None,
                            ArithmeticDomain::Exact,
                            &owner,
                            diagnostics,
                            arithmetic_decisions,
                        );
                    }
                }
            }
            let before = diagnostics.len();
            let first_checked_interval = arithmetic_decisions.expression_values.len();
            let assignment_target_domain = assignment_target_type
                .map(|handle| program.arithmetic_domain_for_type_reference(handle))
                .unwrap_or(ArithmeticDomain::Exact);
            let (interval, source_primitive, source_domain) = assignment_target_type_raw
                .and_then(|destination| {
                    arithmetic_domains::validate_anonymous_integer_range(
                        program,
                        places::assignment_value_type(program, destination),
                        established_value,
                        &owner,
                        diagnostics,
                    )
                })
                .unwrap_or_else(|| {
                    arithmetic_domains::validate_value_range_retaining_decisions(
                        program,
                        machine,
                        current_state,
                        established_value,
                        value_environment,
                        assignment_target_primitive,
                        assignment_target_domain,
                        &owner,
                        diagnostics,
                        arithmetic_decisions,
                    )
                });
            let declared_target_domain = assignment_target_type_raw
                .map(|handle| {
                    program.arithmetic_domain_for_type_reference(places::assignment_value_type(
                        program, handle,
                    ))
                })
                .unwrap_or(ArithmeticDomain::Exact);
            let (source_primitive, source_domain) = arithmetic_domains::stored_value_carrier(
                program,
                machine,
                current_state,
                established_value,
                source_primitive,
                source_domain,
            );
            arithmetic_domains::check_narrowing_assignment(
                assignment_target_primitive,
                declared_target_domain,
                interval,
                source_primitive,
                source_domain,
                &owner,
                diagnostics,
            );
            let interval = if let Some((operator, _)) = atomic_fetch {
                let resident_interval = current_state
                    .and_then(|state| {
                        arithmetic_domains::atomic_resident_interval(
                            program,
                            machine,
                            Some(state),
                            assignment.target,
                        )
                    })
                    .or_else(|| {
                        assignment_target_primitive.and_then(arithmetic_domains::primitive_range)
                    })
                    .unwrap_or(arithmetic_domains::Interval::UNBOUNDED);
                use typed_trees::expression::BinaryOperator;
                match operator {
                    BinaryOperator::Add => resident_interval.add(interval),
                    BinaryOperator::Subtract => resident_interval.subtract(interval),
                    BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor => {
                        let primitive = assignment_target_primitive
                            .or(source_primitive)
                            .unwrap_or(typed_trees::types::PrimitiveType::U32);
                        let resident_known = if resident_interval.low == resident_interval.high
                            && resident_interval.low >= Some(0)
                        {
                            resident_interval.low.map(|v| v as u64)
                        } else {
                            None
                        };
                        let operand_known =
                            if interval.low == interval.high && interval.low >= Some(0) {
                                interval.low.map(|v| v as u64)
                            } else {
                                None
                            };
                        arithmetic_domains::bitwise_binary(
                            operator,
                            primitive,
                            resident_interval,
                            interval,
                            resident_known,
                            operand_known,
                        )
                    }
                    _ => arithmetic_domains::Interval::UNBOUNDED,
                }
            } else {
                interval
            };
            if diagnostics.len() == before {
                let observation = arithmetic_domains::IntegerValueObservation {
                    machine,
                    state: current_state,
                    expression: established_value,
                    environment: value_environment,
                    interval,
                };
                let writes = arithmetic_domains::validate_stored_integer_write(
                    program,
                    machine,
                    current_state,
                    assignment.target,
                    established_value,
                    scope.local_write_origins,
                    interval,
                    source_primitive,
                    value_environment,
                    &owner,
                    diagnostics,
                );
                outputs.stored_integer_writes.extend(writes);
                if atomic.is_some()
                    && let Some(stated) = current_state.and_then(|state| {
                        crate::proof_contracts::default_domains::stored_field_requires_interval(
                            program,
                            machine,
                            state,
                            assignment.target,
                        )
                    })
                {
                    let contained = arithmetic_domains::check_requires_interval_containment(
                        program,
                        stated,
                        observation,
                        &owner,
                        diagnostics,
                    );
                    if contained
                        && diagnostics.len() == before
                        && let (Some(state), Some(target_type)) =
                            (current_state, assignment_target_type_raw)
                        && let Some(decision) =
                            super::integer_establishment::checked_integer_establishment(
                                program,
                                machine,
                                state,
                                scope.statement_index,
                                assignment.target,
                                assignment.value,
                                target_type,
                                stated,
                            )
                    {
                        arithmetic_decisions.integer_assignments.push(decision);
                    }
                }
            }
            let mut copied_value = arithmetic_domains::CopiedIntegerValue::capture(
                program,
                machine,
                current_state,
                assignment.value,
                assignment_target_type_raw
                    .map(|reference| places::assignment_value_type(program, reference))
                    .unwrap_or_default(),
                value_environment,
                &arithmetic_decisions.expression_values[first_checked_interval..],
            );
            let copied_float = arithmetic_domains::capture_float_assignment(
                program,
                machine,
                current_state,
                value_environment,
                assignment_target_primitive,
                assignment.value,
            );
            if let Some(written) = direct_written {
                if let Some(state) = current_state {
                    copied_value.capture_assignment_relations(
                        program,
                        machine,
                        state,
                        statement,
                        scope.call_frames,
                        &written,
                        value_environment,
                    );
                }
                value_environment.invalidate_assignment_paths(
                    program,
                    machine,
                    current_state,
                    statement,
                    scope.call_frames,
                    &written,
                );
            } else {
                value_environment.invalidate_unknown_writes(program, current_state);
            }
            if !scope.call_frames.is_some_and(|frames| {
                frames.assignment_destination_remains_current(machine, statement)
            }) {
                return;
            }
            if let Some(event) = atomic.as_ref().filter(|event| event.result.is_valid()) {
                let observed = arithmetic_domains::atomic_resident_interval(
                    program,
                    machine,
                    current_state,
                    event.place,
                )
                .unwrap_or(arithmetic_domains::Interval::UNBOUNDED);
                arithmetic_domains::record_assignment(
                    value_environment,
                    arithmetic_domains::place_path(program, event.result),
                    observed,
                    None,
                );
            }
            let assignment_path = arithmetic_domains::place_path(program, assignment.target);
            arithmetic_domains::record_assignment(
                value_environment,
                assignment_path.clone(),
                if atomic.is_some() {
                    current_state
                        .and_then(|state| {
                            crate::proof_contracts::default_domains::stored_field_requires_interval(
                                program,
                                machine,
                                state,
                                assignment.target,
                            )
                        })
                        .unwrap_or(arithmetic_domains::Interval::UNBOUNDED)
                } else {
                    interval
                },
                places::declared_place_type_raw(program, machine, current_state, assignment.target)
                    .and_then(|handle| {
                        arithmetic_domains::enforced_declared_range(program, handle)
                    }),
            );
            if let Some(path) = assignment_path.as_deref() {
                arithmetic_domains::publish_slice_length(
                    program,
                    machine,
                    current_state,
                    value_environment,
                    path,
                    assignment_root_symbol(program, assignment.target),
                    assignment.value,
                );
            }
            if diagnostics.len() == before {
                let copied_target_path = assignment_path
                    .clone()
                    .or_else(|| arithmetic_domains::element_place_path(program, assignment.target));
                copied_value.record(
                    value_environment,
                    copied_target_path,
                    places::canonical_place_from_expression(program, assignment.target),
                );
                arithmetic_domains::record_boolean_assignment(
                    program,
                    machine,
                    current_state,
                    value_environment,
                    assignment.target,
                    assignment.value,
                );
                arithmetic_domains::record_unsigned_literal_assignment(
                    program,
                    value_environment,
                    assignment_path.clone(),
                    assignment_target_primitive,
                    assignment.value,
                );
                arithmetic_domains::record_float_assignment(
                    value_environment,
                    assignment_path.clone(),
                    copied_float,
                );
                if let (Some(path), Some(reference)) =
                    (assignment_path.as_deref(), assignment_target_type_raw)
                {
                    let reference = places::assignment_value_type(program, reference);
                    if crate::reference_shell(program, reference).is_none()
                        && let Some(state) = current_state
                        && let Some(destination) = arithmetic_domains::scalar_operand(
                            program,
                            machine,
                            state,
                            assignment.target,
                            None,
                        )
                    {
                        arithmetic_domains::record_admitted_integer_fields(
                            program,
                            value_environment,
                            &destination,
                            reference,
                        );
                    }
                    arithmetic_domains::record_declared_float_fields(
                        program,
                        value_environment,
                        path,
                        reference,
                    );
                }
            }
        }

        fn assignment_root_symbol(
            program: &dyn crate::CheckingInput,
            target: typed_trees::expression::ExpressionHandle,
        ) -> symbols::SymbolHandle {
            let Some(place) = places::canonical_place_from_expression(program, target) else {
                return symbols::SymbolHandle::default();
            };
            let facts::PlaceRoot::Symbol(symbol) = place.root else {
                return symbols::SymbolHandle::default();
            };
            if !place.segments.is_empty() {
                return symbols::SymbolHandle::default();
            }
            symbol
        }
    }
    pub(crate) mod atomic_establishment {
        use super::StatementScope;
        use crate::proof_contracts::arithmetic_domains;
        use crate::value_custody::places;
        use diagnostics::Diagnostic;
        use std::borrow::Cow;

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_once_resident(
            scope: &StatementScope<'_>,
            local: &typed_trees::statement::TableLocalData,
            event: &crate::value_custody::atomic_operations::AtomicCarrierAccess,
            environment: &arithmetic_domains::ValueEnvironment,
            decisions: &mut arithmetic_domains::ArithmeticDecisions,
            owner: &str,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let program = scope.program;
            let Some(state) = scope.current_state else {
                return;
            };
            let Some(stated) =
                crate::proof_contracts::default_domains::stored_field_requires_interval(
                    program,
                    scope.machine,
                    state,
                    event.place,
                )
            else {
                return;
            };
            let Some(target_type) =
                places::declared_place_type_raw(program, scope.machine, Some(state), event.place)
            else {
                return;
            };
            let Some(resident_type) = crate::atomic_resident_type_reference(program, target_type)
            else {
                return;
            };
            let Some(replacement) = event.operands.get(1).copied() else {
                return;
            };
            let primitive = program.primitive_type_reference(resident_type);
            let domain = program.arithmetic_domain_for_type_reference(resident_type);
            let before = diagnostics.len();
            let anonymous = arithmetic_domains::validate_anonymous_integer_range(
                program,
                resident_type,
                replacement,
                owner,
                diagnostics,
            );
            let observed = if anonymous.is_some() {
                Cow::Borrowed(environment)
            } else if let Some(observed) = arithmetic_domains::occurrence_value_environment(
                program,
                scope.machine.symbol,
                state.symbol,
                replacement,
            ) {
                Cow::Owned(observed)
            } else {
                diagnostics.push(Diagnostic::error(format!(
                    "{owner} cannot establish atomic resident validity at replacement completion"
                )));
                return;
            };
            let (interval, source_primitive, source_domain) = anonymous.unwrap_or_else(|| {
                arithmetic_domains::validate_value_range(
                    program,
                    scope.machine,
                    Some(state),
                    replacement,
                    &observed,
                    primitive,
                    domain,
                    owner,
                    diagnostics,
                )
            });
            let (source_primitive, source_domain) = arithmetic_domains::stored_value_carrier(
                program,
                scope.machine,
                Some(state),
                replacement,
                source_primitive,
                source_domain,
            );
            arithmetic_domains::check_narrowing_assignment(
                primitive,
                domain,
                interval,
                source_primitive,
                source_domain,
                owner,
                diagnostics,
            );
            let contained = if anonymous.is_some() {
                arithmetic_domains::check_requires_interval_containment(
                    program,
                    stated,
                    arithmetic_domains::IntegerValueObservation {
                        machine: scope.machine,
                        state: Some(state),
                        expression: replacement,
                        environment: &observed,
                        interval,
                    },
                    owner,
                    diagnostics,
                )
            } else {
                let proven =
            crate::proof_contracts::contract_entailment::completed_value_fits_integer_interval(
                program,
                scope.machine,
                state,
                scope.statement_index,
                replacement,
                stated,
            );
                if !proven {
                    diagnostics.push(Diagnostic::error(format!(
                "{owner} stores a value not provably within its data's `requires` facts at replacement completion"
            )));
                }
                proven
            };
            if contained
                && diagnostics.len() == before
                && let Some(decision) = super::integer_establishment::checked_integer_establishment(
                    program,
                    scope.machine,
                    state,
                    scope.statement_index,
                    event.place,
                    local.initial_value,
                    target_type,
                    stated,
                )
            {
                decisions.integer_assignments.push(decision);
            }
        }
    }
    pub(crate) mod call_statements {
        use super::{StatementOutputs, StatementScope};
        use crate::machine_calls::calls::validate_call_node;
        use crate::proof_contracts::arithmetic_domains;
        use typed_trees::statement::StatementNode;

        pub(crate) fn validate(
            scope: &StatementScope<'_>,
            outputs: &mut StatementOutputs<'_>,
            statement: &StatementNode,
            statement_handle: typed_trees::statement::StatementHandle,
            direct_written: Option<Vec<String>>,
        ) {
            let Some(call) = statement.as_call() else {
                unreachable!("dispatched call_statements")
            };
            let (program, machine, state_name, current_state, symbols, writable_roots) =
                scope.reads();
            let value_environment = &mut *outputs.value_environment;
            let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
            let boundary_operator_applications = &mut *outputs.boundary_operator_applications;
            let diagnostics = &mut *outputs.diagnostics;
            if let Some(state) = current_state {
                crate::declarations::operators::validate_named_statement_operator_application(
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
                current_state,
                symbols,
                writable_roots,
                value_environment,
                diagnostics,
                &mut arithmetic_decisions.integer_range_obligations,
            );
            let arguments = program.statement_table.expression_handles(call.arguments);
            if !arguments.is_empty() {
                let owner = format!(
                    "machine `{}` state `{state_name}` call argument",
                    machine.name
                );
                for argument in arguments {
                    arithmetic_domains::validate_value_range_retaining_decisions(
                        program,
                        machine,
                        current_state,
                        *argument,
                        value_environment,
                        None,
                        numerics::arithmetic::ArithmeticDomain::Exact,
                        &owner,
                        diagnostics,
                        arithmetic_decisions,
                    );
                }
            }
            if let Some(written) = direct_written.as_deref() {
                value_environment.invalidate_call_paths(program, current_state, written);
            } else {
                value_environment.invalidate_unknown_writes(program, current_state);
            }
            if let Some(signature) =
                crate::machine_calls::calls::boundary_trait_signature(program, machine, call)
            {
                arithmetic_domains::seed_out_param_ensures(
                    program,
                    machine,
                    current_state,
                    call,
                    signature,
                    value_environment,
                );
            } else {
                arithmetic_domains::seed_cited_element_bounds(
                    program,
                    machine,
                    current_state,
                    call,
                    direct_written.as_deref(),
                    value_environment,
                );
            }
        }
    }
    pub(crate) mod expression_statements {
        use super::{StatementOutputs, StatementScope};
        use crate::{machine_calls::calls, proof_contracts::arithmetic_domains};
        use diagnostics::Diagnostic;
        use typed_trees::statement::StatementNode;
        use typed_trees::types::TypeReferenceNode;

        pub(crate) fn validate(
            scope: &StatementScope<'_>,
            outputs: &mut StatementOutputs<'_>,
            statement: &StatementNode,
        ) {
            let Some(expression) = statement.as_expression() else {
                unreachable!("dispatched expression_statements")
            };
            let (program, machine, state_name, current_state, _, _) = scope.reads();
            let value_environment = &mut *outputs.value_environment;
            let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
            let diagnostics = &mut *outputs.diagnostics;
            let Some(state) = current_state else {
                return;
            };

            if calls::unit_statement_call_is_admitted(program, machine, state, *expression) {
                arithmetic_domains::validate_value_range_retaining_decisions(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    value_environment,
                    None,
                    numerics::arithmetic::ArithmeticDomain::Exact,
                    &format!(
                        "machine `{}` state `{state_name}` call operands",
                        machine.name
                    ),
                    diagnostics,
                    arithmetic_decisions,
                );
                return;
            }
            if scope.statement_index.checked_add(1)
                != Some(
                    program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len(),
                )
            {
                let result_type =
                    crate::expression_result_type_reference(program, machine, state, *expression);
                let permits_statement = result_type.is_some_and(|result| {
                    let carrier =
                        crate::value_custody::places::peel_declared_constraints(program, result);
                    match program.type_reference_table.type_reference(carrier) {
                        TypeReferenceNode::Unit => true,
                        TypeReferenceNode::Named { symbol, .. } => {
                            program.symbols.builtin_type_atom(*symbol)
                                == Some(symbols::BuiltinTypeAtom::Never)
                        }
                        _ => false,
                    }
                });
                if !permits_statement {
                    diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{state_name}` expression statement discards its non-Unit value; consume the value or discard a call explicitly with `_ = call(...);`",
                machine.name,
            )));
                }
                let primitive =
                    result_type.and_then(|result| program.primitive_type_reference(result));
                let domain = result_type
                    .map_or(numerics::arithmetic::ArithmeticDomain::Exact, |result| {
                        program.arithmetic_domain_for_type_reference(result)
                    });
                arithmetic_domains::validate_value_range_retaining_decisions(
                    program,
                    machine,
                    Some(state),
                    *expression,
                    value_environment,
                    primitive,
                    domain,
                    &format!(
                        "machine `{}` state `{state_name}` expression statement",
                        machine.name
                    ),
                    diagnostics,
                    arithmetic_decisions,
                );
                return;
            }
            if !state.return_type.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{state_name}` has a terminal expression but no return type",
            machine.name
        )));
                return;
            }

            super::validate_completion_value(
                scope,
                state,
                *expression,
                &mut arithmetic_decisions.integer_range_obligations,
                diagnostics,
            );
            let owner = format!(
                "machine `{}` state `{state_name}` terminal expression",
                machine.name
            );
            let return_primitive = program.primitive_type_reference(state.return_type);
            let return_domain = program.arithmetic_domain_for_type_reference(state.return_type);
            let (return_interval, source_primitive, source_domain) =
                arithmetic_domains::validate_anonymous_integer_range(
                    program,
                    state.return_type,
                    *expression,
                    &owner,
                    diagnostics,
                )
                .unwrap_or_else(|| {
                    arithmetic_domains::validate_value_range_retaining_decisions(
                        program,
                        machine,
                        Some(state),
                        *expression,
                        value_environment,
                        return_primitive,
                        return_domain,
                        &owner,
                        diagnostics,
                        arithmetic_decisions,
                    )
                });
            let (source_primitive, source_domain) = arithmetic_domains::stored_value_carrier(
                program,
                machine,
                Some(state),
                *expression,
                source_primitive,
                source_domain,
            );
            arithmetic_domains::check_narrowing_assignment(
                return_primitive,
                return_domain,
                return_interval,
                source_primitive,
                source_domain,
                &owner,
                diagnostics,
            );
        }
    }
    pub(crate) mod integer_establishment {
        use crate::proof_contracts::arithmetic_domains;
        use typed_trees::expression::ExpressionHandle;
        use typed_trees::statement::StatementNode;

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn checked_integer_establishment(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: usize,
            target: ExpressionHandle,
            value: ExpressionHandle,
            target_type: typed_trees::types::TypeReferenceHandle,
            stated: arithmetic_domains::Interval,
        ) -> Option<checked_trees::CheckedIntegerAssignmentDecision> {
            let statement = program
                .statement_table
                .statements(state.statement_nodes)
                .get(statement_index)?;
            let atomic_resident = match statement {
                StatementNode::Assignment(assignment)
                    if assignment.target == target && assignment.value == value =>
                {
                    crate::atomic_assignment_carrier(program, assignment).is_some()
                }
                StatementNode::LocalData(local) if local.initial_value == value => {
                    let event = crate::atomic_once_carrier(program, value)?;
                    if event.place != target {
                        return None;
                    }
                    true
                }
                _ => return None,
            };
            let primitive = program.primitive_type_reference(target_type)?;
            let (mut minimum, mut maximum) =
                crate::proof_contracts::proof_embeddings::primitive_range(primitive)?;
            for interval in std::iter::once(stated).chain(
                arithmetic_domains::enforced_declared_range(program, target_type),
            ) {
                if let Some(lower) = interval.low() {
                    minimum = minimum.max(numerics::bignum::BigInt::from_i64(lower));
                }
                if let Some(upper) = interval.high() {
                    maximum = maximum.min(numerics::bignum::BigInt::from_i64(upper));
                }
            }
            (minimum <= maximum).then_some(checked_trees::CheckedIntegerAssignmentDecision {
                atomic_resident,
                machine: machine.symbol,
                state: state.symbol,
                statement_index,
                target,
                value,
                target_type,
                minimum,
                maximum,
            })
        }
    }
    pub(crate) mod local_data_statements {

        use super::{StatementOutputs, StatementScope};
        use crate::frozen_program::expression_table;
        use crate::value_custody::type_references::validate_type_reference_handle;
        use crate::value_custody::type_references::{TypeParameterScope, TypeReferenceOwner};
        use crate::{
            machine_calls::calls, proof_contracts::arithmetic_domains,
            proof_contracts::domain_weakening, value_custody::expression_types,
            value_custody::struct_literals,
        };
        use diagnostics::Diagnostic;
        use typed_trees::statement::StatementNode;

        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_type_reference(
            program: &dyn crate::CheckingInput,
            machine: &typed_trees::machine::Machine,
            current_state: Option<&typed_trees::state::State>,
            state_name: &str,
            local_data: &typed_trees::statement::TableLocalData,
            symbols: &crate::TopLevelSymbols<'_>,
            value_environment: &arithmetic_domains::ValueEnvironment,
            diagnostics: &mut Vec<Diagnostic>,
            formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        ) {
            if !local_data.role.is_runtime_binding() {
                return;
            }
            let (type_parameters, lifetime_parameters) = program
                .machine_type_reference_scope(machine, |existing, parameter| {
                    existing.symbol == parameter.symbol
                });
            validate_type_reference_handle(
                program,
                local_data.type_reference,
                symbols,
                diagnostics,
                TypeReferenceOwner::StateLocalData {
                    machine,
                    current_state,
                    state: state_name,
                    local: local_data.name.as_str(),
                    value_environment,
                    generic_depth: 0,
                },
                TypeParameterScope::authored(
                    machine.symbol,
                    &type_parameters,
                    &lifetime_parameters,
                ),
                formation_obligations,
            );
        }

        pub(crate) fn validate(
            scope: &StatementScope<'_>,
            outputs: &mut StatementOutputs<'_>,
            statement: &StatementNode,
        ) {
            let Some(local_data) = statement.as_local_data() else {
                unreachable!("dispatched local_data_statements")
            };
            if !local_data.role.is_runtime_binding() {
                return;
            }
            let (program, machine, state_name, current_state, symbols, _) = scope.reads();
            let statement_index = scope.statement_index;
            let value_environment = &mut *outputs.value_environment;
            let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
            let diagnostics = &mut *outputs.diagnostics;
            if local_data.role != language_core::LocalBindingRole::Value
                || local_data.type_is_inferred
                || local_data.is_operand_capture
            {
                validate_type_reference(
                    program,
                    machine,
                    current_state,
                    state_name,
                    local_data,
                    symbols,
                    value_environment,
                    diagnostics,
                    &mut arithmetic_decisions.integer_range_obligations,
                );
            }
            let local_target_primitive =
                program.primitive_type_reference(local_data.type_reference);
            if let Some(current_state) = current_state {
                struct_literals::validate_array_literal_elements(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    diagnostics,
                    &mut arithmetic_decisions.integer_range_obligations,
                );
            }
            let owner = current_state
                .and_then(|state| value_argument_capture_owner(program, state, local_data))
                .unwrap_or_else(|| {
                    format!(
                        "machine `{}` state `{state_name}` local `{}`",
                        machine.name,
                        local_data.name.as_str()
                    )
                });
            if local_data.role == language_core::LocalBindingRole::AtomicObservedResult {
                let Some(state) = current_state else {
                    diagnostics.push(Diagnostic::error("atomic result requires its source state"));
                    return;
                };
                let Some(event) = crate::atomic_result_placeholder(program, state, local_data)
                else {
                    diagnostics.push(Diagnostic::error(
                        "atomic result reservation lost its completing event",
                    ));
                    return;
                };
                let mut before_event = event.operands.clone();
                before_event.push(event.place);
                crate::queries::expression_traversal::collect_type_expressions(
                    program,
                    local_data.type_reference,
                    &mut Vec::new(),
                    &mut before_event,
                );
                if crate::queries::expression_traversal::visit_pre_order(
                    program,
                    before_event,
                    |_, node| matches!(node, typed_trees::expression::ExpressionNode::Name(path) if path.symbol == local_data.symbol || path.head_symbol == local_data.symbol),
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} cannot consume its atomic result before the event completes"
                    )));
                    return;
                }
                let reference =
                    crate::declared_place_type_raw(program, machine, Some(state), event.place);
                let primitive =
                    reference.and_then(|reference| program.primitive_type_reference(reference));
                if primitive.is_none() || primitive != local_target_primitive {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} requires the exact atomic instruction-observed resident type"
                    )));
                    return;
                }
                domain_weakening::validate_implicit_domain_weakening(
                    program,
                    machine,
                    Some(state),
                    event.place,
                    local_data.type_reference,
                    &owner,
                    diagnostics,
                );
                arithmetic_domains::record_assignment(
                    value_environment,
                    Some(local_data.name.to_string()),
                    arithmetic_domains::Interval::UNBOUNDED,
                    None,
                );
                return;
            }
            if let Some(carrier) = crate::atomic_once_carrier(program, local_data.initial_value) {
                let resident = crate::value_custody::places::declared_place_type(
                    program,
                    machine,
                    current_state,
                    carrier.place,
                )
                .filter(|_| {
                    crate::place_is_atomic_storage(program, machine, current_state, carrier.place)
                })
                .and_then(|cell| program.primitive_type_reference(cell));
                let Some(custody) = carrier.operation.single_attempt_custody() else {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} has no admitted single-attempt result contract"
                    )));
                    return;
                };
                if resident.is_none() {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} requires dedicated atomic storage for its single attempt"
                    )));
                } else if !resident.is_some_and(|primitive| {
                    crate::atomic_once_result_type(
                        program,
                        local_data.type_reference,
                        primitive,
                        custody,
                    )
                }) {
                    diagnostics.push(Diagnostic::error(format!(
                        "{owner} requires the sealed {} with the exact resident type",
                        custody.outcome_identity.name(),
                    )));
                } else {
                    super::atomic_establishment::validate_once_resident(
                        scope,
                        local_data,
                        &carrier,
                        value_environment,
                        arithmetic_decisions,
                        &owner,
                        diagnostics,
                    );
                }
            }
            if local_data.initial_value.is_valid()
                && local_data.type_reference.is_valid()
                && !local_data.type_is_inferred
                && matches!(
                    program
                        .type_reference_table
                        .type_reference(local_data.type_reference),
                    typed_trees::types::TypeReferenceNode::Unit
                )
            {
                diagnostics.push(Diagnostic::error(format!(
            "{owner} has an explicit Unit type and cannot bind an initializer value; use the value's type or explicitly discard the result"
        )));
            }
            if local_data.initial_value.is_valid()
                && let Some(target_primitive) = local_target_primitive
            {
                expression_types::report_cross_class_store(
                    program,
                    Some(machine),
                    current_state,
                    local_data.initial_value,
                    target_primitive,
                    &owner,
                    "local",
                    diagnostics,
                );
            }
            if let Some(state) = current_state
                && local_data.initial_value.is_valid()
                && crate::reference_shell(program, local_data.type_reference)
                    .is_none_or(|shell| shell.access != language_core::ReferenceAccess::Mutable)
            {
                crate::value_custody::recasts::pending_views::report_consumption(
                    program,
                    machine,
                    state,
                    statement_index,
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    diagnostics,
                );
            }
            if local_data.initial_value.is_valid() {
                let prior_statements = current_state.map_or(&[][..], |state| {
                    &program.statement_table.statements(state.statement_nodes)[..statement_index]
                });
                let access_refused =
                    crate::value_custody::reference_access::report_reference_access_conflict(
                        program,
                        machine,
                        current_state,
                        prior_statements,
                        local_data.initial_value,
                        local_data.type_reference,
                        &owner,
                        diagnostics,
                    );
                if !access_refused {
                    expression_types::report_delivered_type_mismatch_with_component(
                        program,
                        machine.symbol,
                        local_data.initial_value,
                        local_data.type_reference,
                        program.pattern_component(local_data.initial_value),
                        &owner,
                        diagnostics,
                    );
                }
                expression_types::report_place_value_mismatches(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    "local",
                    diagnostics,
                );
                domain_weakening::validate_implicit_domain_weakening(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    &owner,
                    diagnostics,
                );
            }
            calls::report_nested_call_in_local_initializer(
                program,
                machine,
                state_name,
                local_data.initial_value,
                diagnostics,
            );
            let before = diagnostics.len();
            let first_checked_interval = arithmetic_decisions.expression_values.len();
            let local_target_domain =
                program.arithmetic_domain_for_type_reference(local_data.type_reference);
            let (interval, source_primitive, source_domain) =
                arithmetic_domains::validate_anonymous_integer_range(
                    program,
                    local_data.type_reference,
                    local_data.initial_value,
                    &owner,
                    diagnostics,
                )
                .unwrap_or_else(|| {
                    arithmetic_domains::validate_value_range_retaining_decisions(
                        program,
                        machine,
                        current_state,
                        local_data.initial_value,
                        value_environment,
                        local_target_primitive,
                        local_target_domain,
                        &owner,
                        diagnostics,
                        arithmetic_decisions,
                    )
                });
            if local_data.initial_value.is_valid() {
                let (source_primitive, source_domain) = arithmetic_domains::stored_value_carrier(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    source_primitive,
                    source_domain,
                );
                arithmetic_domains::check_narrowing_assignment(
                    local_target_primitive,
                    local_target_domain,
                    interval,
                    source_primitive,
                    source_domain,
                    &owner,
                    diagnostics,
                );
            }
            if local_data.initial_value.is_valid() {
                let mut completed_values = Vec::new();
                let aggregate_with_writes =
                    matches!(
                        expression_table(program).expression(local_data.initial_value),
                        typed_trees::expression::ExpressionNode::StructLiteral(_)
                            | typed_trees::expression::ExpressionNode::ArrayLiteral(_)
                    ) && calls::expression_has_calls(program, local_data.initial_value)
                        && scope
                            .call_frames
                            .and_then(|frames| {
                                frames.expression_may_write_paths(machine, local_data.initial_value)
                            })
                            .is_none_or(|written| !written.is_empty());
                let captured_values = if aggregate_with_writes
                    && let Some(state) = current_state
                    && let Some(captured) = arithmetic_domains::occurrence_integer_child_values(
                        program,
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        local_data.initial_value,
                    ) {
                    let keys = captured
                        .iter()
                        .map(|value| {
                            (
                                value.expression.arena_index(),
                                value.expression.generation(),
                            )
                        })
                        .collect::<std::collections::BTreeSet<_>>();
                    completed_values.extend(
                        arithmetic_decisions.expression_values[first_checked_interval..]
                            .iter()
                            .filter(|value| {
                                !keys.contains(&(
                                    value.expression.arena_index(),
                                    value.expression.generation(),
                                ))
                            })
                            .cloned(),
                    );
                    completed_values.extend(captured);
                    &completed_values[..]
                } else {
                    &arithmetic_decisions.expression_values[first_checked_interval..]
                };
                let mut copied_value = arithmetic_domains::CopiedIntegerValue::capture(
                    program,
                    machine,
                    current_state,
                    local_data.initial_value,
                    local_data.type_reference,
                    value_environment,
                    captured_values,
                );
                copied_value.capture_local_relations(
                    program,
                    machine,
                    current_state,
                    local_data,
                    value_environment,
                );
                let copied_float = arithmetic_domains::capture_float_assignment(
                    program,
                    machine,
                    current_state,
                    value_environment,
                    local_target_primitive,
                    local_data.initial_value,
                );
                let local_name_owned = local_data.name.as_str().to_owned();
                arithmetic_domains::record_assignment(
                    value_environment,
                    Some(local_name_owned.clone()),
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
                arithmetic_domains::publish_slice_length(
                    program,
                    machine,
                    current_state,
                    value_environment,
                    local_data.name.as_str(),
                    local_data.symbol,
                    local_data.initial_value,
                );
                if diagnostics.len() == before {
                    copied_value.record(
                        value_environment,
                        Some(local_name_owned.clone()),
                        Some(crate::value_custody::places::CanonicalPlace {
                            root: facts::PlaceRoot::Symbol(local_data.symbol),
                            segments: Vec::new(),
                        }),
                    );
                    arithmetic_domains::record_boolean_local(
                        program,
                        machine,
                        current_state,
                        value_environment,
                        local_data,
                    );
                    arithmetic_domains::record_unsigned_literal_assignment(
                        program,
                        value_environment,
                        Some(local_name_owned.clone()),
                        local_target_primitive,
                        local_data.initial_value,
                    );
                    arithmetic_domains::record_float_assignment(
                        value_environment,
                        Some(local_name_owned),
                        copied_float,
                    );
                    if crate::reference_shell(program, local_data.type_reference).is_none() {
                        arithmetic_domains::record_admitted_integer_fields(
                            program,
                            value_environment,
                            &arithmetic_domains::ScalarOperand::Place {
                                root: local_data.symbol,
                                path: local_data.name.as_str().to_owned(),
                            },
                            local_data.type_reference,
                        );
                    }
                    arithmetic_domains::record_declared_float_fields(
                        program,
                        value_environment,
                        local_data.name.as_str(),
                        local_data.type_reference,
                    );
                }
            }
        }

        fn value_argument_capture_owner(
            program: &dyn crate::CheckingInput,
            state: &typed_trees::state::State,
            local_data: &typed_trees::statement::TableLocalData,
        ) -> Option<String> {
            if !local_data.name.as_str().starts_with("__value_index#") {
                return None;
            }
            let mut seen = Vec::new();
            for statement in program.statement_table.statements(state.statement_nodes) {
                let typed_trees::statement::StatementNode::LocalData(owner) = statement else {
                    continue;
                };
                if let Some(phrase) = value_argument_phrase(
                    program,
                    owner.type_reference,
                    local_data.symbol,
                    &mut seen,
                ) {
                    return Some(phrase);
                }
            }
            None
        }

        fn value_argument_phrase(
            program: &dyn crate::CheckingInput,
            reference: typed_trees::types::TypeReferenceHandle,
            symbol: symbols::SymbolHandle,
            seen: &mut Vec<typed_trees::types::TypeReferenceHandle>,
        ) -> Option<String> {
            if !reference.is_valid() || seen.contains(&reference) {
                return None;
            }
            seen.push(reference);
            let typed_trees::types::TypeReferenceNode::Generic {
                base_symbol,
                arguments,
                ..
            } = program.type_reference_table.type_reference(reference)
            else {
                return None;
            };
            let actuals = program
                .type_reference_table
                .type_reference_handles(*arguments);
            let data = program.unique_data_definition_by_symbol(*base_symbol)?;
            let parameters = program.data_type_parameters(data);
            if actuals.len() != parameters.len() {
                return None;
            }
            for (parameter, actual) in parameters.iter().zip(actuals.iter().copied()) {
                if matches!(
                    parameter.kind,
                    typed_trees::data::TypeParameterKind::Value { .. }
                ) && matches!(
                    program.type_reference_table.type_reference(actual),
                    typed_trees::types::TypeReferenceNode::Named { symbol: named, .. } if *named == symbol
                ) {
                    return Some(format!(
                        "value argument `{}` of `{}`",
                        parameter.name.as_str(),
                        data.name.as_str()
                    ));
                }
                if let Some(phrase) = value_argument_phrase(program, actual, symbol, seen) {
                    return Some(phrase);
                }
            }
            None
        }
    }
    pub(crate) use local_data_statements::validate_type_reference as validate_local_type_reference;
    pub(crate) mod transition_statements {
        use super::{StatementOutputs, StatementScope};
        use crate::declarations::transitions::validate_transition_target_node;
        use crate::proof_contracts::arithmetic_domains;
        use diagnostics::Diagnostic;
        use typed_trees::statement::StatementNode;
        use typed_trees::statement::TransitionGuardNode;
        use typed_trees::statement::TransitionTargetNode;

        pub(crate) fn validate(
            scope: &StatementScope<'_>,
            outputs: &mut StatementOutputs<'_>,
            statement: &StatementNode,
        ) {
            let Some(transition) = statement.as_transition() else {
                unreachable!("dispatched transition_statements")
            };
            let (program, machine, state_name, current_state, symbols, writable_roots) =
                scope.reads();
            let transition_values = scope.transition_values;
            let value_environment = &mut *outputs.value_environment;
            let arithmetic_decisions = &mut *outputs.arithmetic_decisions;
            let diagnostics = &mut *outputs.diagnostics;
            if let Some(state) = current_state {
                for retained in program
                    .statement_table
                    .transition_patterns(transition.patterns)
                {
                    crate::value_custody::type_references::validate_expression_type_application(
                        program,
                        machine,
                        state,
                        retained.pattern.application_type,
                        symbols,
                        value_environment,
                        diagnostics,
                        &mut arithmetic_decisions.integer_range_obligations,
                    );
                }
                crate::value_custody::expression_types::validate_transition_patterns(
                    program,
                    machine,
                    state,
                    transition,
                    diagnostics,
                );
            }
            if let TransitionGuardNode::When(guard) = transition.guard {
                let owner = format!(
                    "machine `{}` state `{state_name}` transition guard",
                    machine.name
                );
                arithmetic_domains::validate_value_range_retaining_decisions(
                    program,
                    machine,
                    current_state,
                    guard,
                    value_environment,
                    Some(typed_trees::types::PrimitiveType::Bool),
                    numerics::arithmetic::ArithmeticDomain::Exact,
                    &owner,
                    diagnostics,
                    arithmetic_decisions,
                );
            }
            validate_transition_target_node(
                program,
                machine,
                current_state,
                value_environment,
                transition_values.for_target(transition.target),
                transition.target,
                writable_roots,
                diagnostics,
                &mut arithmetic_decisions.integer_range_obligations,
            );

            if transition.continuation.is_valid() {
                validate_transition_target_node(
                    program,
                    machine,
                    current_state,
                    value_environment,
                    transition_values.for_target(transition.continuation),
                    transition.continuation,
                    writable_roots,
                    diagnostics,
                    &mut arithmetic_decisions.integer_range_obligations,
                );
            }

            let guard_is_always = matches!(transition.guard, TransitionGuardNode::Always);
            let narrowed_owned = (!guard_is_always).then(|| {
                arithmetic_domains::guard_narrowed_environment(
                    program,
                    machine,
                    current_state,
                    &transition.guard,
                    value_environment,
                )
            });

            if transition.target.is_valid()
                && !transition.continuation.is_valid()
                && !guard_is_always
            {
                *value_environment = arithmetic_domains::fall_through_narrowed_environment(
                    program,
                    machine,
                    current_state,
                    &transition.guard,
                    value_environment,
                );
            }
            let narrowed = narrowed_owned.as_ref().unwrap_or(&*value_environment);

            if let Some(state) = current_state
                && !state.return_type.is_valid()
            {
                for target in transition.targets() {
                    if let TransitionTargetNode::Value(expression) =
                        program.statement_table.transition_target(target)
                    {
                        if !crate::machine_calls::calls::unit_statement_call_is_admitted(
                            program,
                            machine,
                            state,
                            *expression,
                        ) {
                            diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{state_name}` transition returns a value, but the state declares no result; a terminal arm is `_ -> {{}}`",
                        machine.name
                    )));
                            continue;
                        }
                        let environment = transition_values
                            .for_target(target)
                            .first()
                            .unwrap_or(narrowed);
                        arithmetic_domains::validate_value_range_retaining_decisions(
                            program,
                            machine,
                            Some(state),
                            *expression,
                            environment,
                            None,
                            numerics::arithmetic::ArithmeticDomain::Exact,
                            &format!(
                                "machine `{}` state `{state_name}` terminal call operands",
                                machine.name
                            ),
                            diagnostics,
                            arithmetic_decisions,
                        );
                    }
                }
            }

            if let Some(state) = current_state
                && state.return_type.is_valid()
            {
                let mut return_owner = None;
                for target in transition.targets() {
                    if let TransitionTargetNode::Value(return_expression) =
                        program.statement_table.transition_target(target)
                    {
                        super::validate_completion_value(
                            scope,
                            state,
                            *return_expression,
                            &mut arithmetic_decisions.integer_range_obligations,
                            diagnostics,
                        );
                        let return_owner = return_owner.get_or_insert_with(|| {
                            format!(
                                "machine `{}` state `{state_name}` return value",
                                machine.name
                            )
                        });
                        let return_environment = transition_values
                            .for_target(target)
                            .first()
                            .unwrap_or(narrowed);
                        arithmetic_domains::validate_return_value_range(
                            program,
                            machine,
                            state,
                            *return_expression,
                            return_environment,
                            return_owner,
                            diagnostics,
                            arithmetic_decisions,
                        );
                    }
                }
            }

            let mut argument_owner = None;
            for target in transition.targets() {
                if let TransitionTargetNode::Named { arguments, .. } =
                    program.statement_table.transition_target(target)
                {
                    let arguments = program.statement_table.expression_handles(*arguments);
                    if !arguments.is_empty() {
                        let argument_owner = argument_owner.get_or_insert_with(|| {
                            format!(
                                "machine `{}` state `{state_name}` transition argument",
                                machine.name
                            )
                        });
                        for (argument_index, argument) in arguments.iter().enumerate() {
                            let argument_environment = transition_values
                                .for_target(target)
                                .get(argument_index)
                                .unwrap_or(narrowed);
                            arithmetic_domains::validate_value_range_retaining_decisions(
                                program,
                                machine,
                                current_state,
                                *argument,
                                argument_environment,
                                None,
                                numerics::arithmetic::ArithmeticDomain::Exact,
                                argument_owner,
                                diagnostics,
                                arithmetic_decisions,
                            );
                        }
                    }
                }
            }
        }
    }
    pub(crate) mod unreachable_branches {
        use crate::proof_contracts::arithmetic_domains::{
            self, ValueEnvironment, contradicting_premises, dispatch_arm_environments,
        };
        use diagnostics::Diagnostic;
        use typed_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
        use typed_trees::machine::Machine;
        use typed_trees::state::State;
        use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

        pub(crate) fn warn(
            program: &dyn crate::CheckingInput,
            machine: &Machine,
            state: &State,
            statement: &StatementNode,
            environment: &ValueEnvironment,
            diagnostics: &mut Vec<Diagnostic>,
        ) {
            let report = Report {
                program,
                machine,
                state,
            };
            match statement {
                StatementNode::Expression(expression) => {
                    report.matches(*expression, environment, diagnostics);
                }
                StatementNode::LocalData(local) => {
                    report.matches(local.initial_value, environment, diagnostics);
                }
                StatementNode::Transition(transition) => {
                    report.transition(transition, environment, diagnostics);
                }
                _ => {}
            }
        }

        pub(crate) struct Report<'a> {
            program: &'a dyn crate::CheckingInput,
            machine: &'a Machine,
            state: &'a State,
        }

        impl Report<'_> {
            fn transition(
                &self,
                transition: &typed_trees::statement::TableTransition,
                environment: &ValueEnvironment,
                diagnostics: &mut Vec<Diagnostic>,
            ) {
                let arms = [
                    (transition.target, true, "`true` arm"),
                    (transition.continuation, false, "fall-back arm"),
                ];
                for (target, positive, arm) in arms {
                    if !target.is_valid() {
                        continue;
                    }
                    let TransitionGuardNode::When(guard) = transition.guard else {
                        self.target(target, environment, diagnostics);
                        continue;
                    };
                    let narrowed = if positive {
                        arithmetic_domains::guard_narrowed_environment(
                            self.program,
                            self.machine,
                            Some(self.state),
                            &transition.guard,
                            environment,
                        )
                    } else {
                        arithmetic_domains::fall_through_narrowed_environment(
                            self.program,
                            self.machine,
                            Some(self.state),
                            &transition.guard,
                            environment,
                        )
                    };
                    match contradicting_premises(self.program, self.machine, environment, &narrowed)
                    {
                        Some(premises) => diagnostics.push(self.warning(
                            &format!("the {arm} of a transition"),
                            "guard",
                            &premises,
                            guard,
                        )),
                        None => self.target(target, &narrowed, diagnostics),
                    }
                }
            }

            fn target(
                &self,
                target: typed_trees::statement::TransitionTargetHandle,
                environment: &ValueEnvironment,
                diagnostics: &mut Vec<Diagnostic>,
            ) {
                if let TransitionTargetNode::Value(value) =
                    self.program.statement_table.transition_target(target)
                {
                    self.matches(*value, environment, diagnostics);
                }
            }

            fn matches(
                &self,
                expression: ExpressionHandle,
                environment: &ValueEnvironment,
                diagnostics: &mut Vec<Diagnostic>,
            ) {
                let ExpressionNode::Match(dispatch) =
                    self.program.expression_table.expression(expression)
                else {
                    return;
                };
                let arms = self.program.expression_table.match_arms(dispatch.arms);
                let environments = dispatch_arm_environments(
                    self.program,
                    self.machine,
                    Some(self.state),
                    expression,
                    dispatch,
                    environment,
                );
                for (position, (arm, selected)) in arms.iter().zip(&environments).enumerate() {
                    match contradicting_premises(self.program, self.machine, environment, selected)
                    {
                        Some(premises) => {
                            let name = match arm.pattern {
                                MatchPattern::Value(pattern) => {
                                    match self.program.expression_table.boolean_node(pattern) {
                                        Some(value) => format!("`{value}` arm"),
                                        None => format!("arm {}", position + 1),
                                    }
                                }
                                _ => format!("arm {}", position + 1),
                            };
                            diagnostics.push(self.warning(
                                &format!("the {name} of a match"),
                                "pattern",
                                &premises,
                                arm.value,
                            ));
                        }
                        None => self.matches(arm.value, selected, diagnostics),
                    }
                }
            }

            fn warning(
                &self,
                branch: &str,
                selector: &str,
                premises: &[String],
                location: ExpressionHandle,
            ) -> Diagnostic {
                let reason = if premises.is_empty() {
                    format!("its {selector} can never select it")
                } else {
                    format!(
                        "its {selector} contradicts the facts in force here: {}",
                        premises.join(", ")
                    )
                };
                Diagnostic::warning(format!(
            "unreachable branch: {branch} in machine `{}` state `{}` can never run; {reason}",
            self.machine.name, self.state.name,
        ))
        .with_source_span(self.program.expression_table.source_span(location))
            }
        }
    }

    use crate::value_custody::expression_types::{
        self, ExpressionTypeOwner, validate_expression_type_handle,
    };
    use crate::value_custody::locals::WritableRoots;
    use crate::{
        TopLevelSymbols, ValidatedBoundaryOperatorApplication, declarations::transitions,
        proof_contracts::arithmetic_domains, proof_contracts::domain_weakening,
        value_custody::access_views, value_custody::reference_access,
        value_custody::struct_literals,
    };
    use diagnostics::Diagnostic;
    use typed_trees::expression::ExpressionHandle;
    use typed_trees::statement::StatementNode;

    #[allow(clippy::too_many_arguments)]
    #[derive(Clone, Copy)]
    pub(crate) struct StatementScope<'a> {
        pub(crate) program: &'a dyn crate::CheckingInput,
        pub(crate) machine: &'a typed_trees::machine::Machine,
        pub(crate) state_name: &'a str,
        pub(crate) current_state: Option<&'a typed_trees::state::State>,
        pub(crate) statement_index: usize,
        pub(crate) symbols: &'a TopLevelSymbols<'a>,
        pub(crate) writable_roots: &'a WritableRoots<'a, 'a>,
        pub(crate) local_write_origins: Option<&'a [crate::LocalWriteOrigin]>,
        pub(crate) call_frames: Option<&'a crate::CallFrameResolver<'a>>,
        pub(crate) transition_values: &'a transitions::TransitionValueEnvironments,
    }

    pub(crate) struct StatementOutputs<'a> {
        pub(crate) stored_integer_writes: &'a mut Vec<checked_trees::CheckedStoredIntegerWrite>,
        pub(crate) value_environment: &'a mut arithmetic_domains::ValueEnvironment,
        pub(crate) arithmetic_decisions: &'a mut arithmetic_domains::ArithmeticDecisions,
        pub(crate) boundary_operator_applications:
            &'a mut Vec<ValidatedBoundaryOperatorApplication>,
        pub(crate) diagnostics: &'a mut Vec<Diagnostic>,
    }

    impl<'a> StatementScope<'a> {
        pub(crate) fn reads(
            &self,
        ) -> (
            &'a dyn crate::CheckingInput,
            &'a typed_trees::machine::Machine,
            &'a str,
            Option<&'a typed_trees::state::State>,
            &'a TopLevelSymbols<'a>,
            &'a WritableRoots<'a, 'a>,
        ) {
            (
                self.program,
                self.machine,
                self.state_name,
                self.current_state,
                self.symbols,
                self.writable_roots,
            )
        }
    }

    pub(crate) fn validate_completion_value(
        scope: &StatementScope<'_>,
        state: &typed_trees::state::State,
        value: ExpressionHandle,
        formation_obligations: &mut Vec<checked_trees::IntegerRangeObligation>,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let (program, machine, state_name, _, _, _) = scope.reads();
        let return_owner = format!(
            "machine `{}` state `{state_name}` return value",
            machine.name
        );
        let slot_context = format!("machine `{}` state `{state_name}`", machine.name);
        reference_access::report_reference_access_conflict(
            program,
            machine,
            Some(state),
            &program.statement_table.statements(state.statement_nodes)[..scope.statement_index],
            value,
            state.return_type,
            &return_owner,
            diagnostics,
        );
        crate::value_custody::recasts::pending_views::report_consumption(
            program,
            machine,
            state,
            scope.statement_index,
            value,
            state.return_type,
            &return_owner,
            diagnostics,
        );
        struct_literals::validate_array_literal_elements(
            program,
            machine,
            state,
            value,
            state.return_type,
            diagnostics,
            formation_obligations,
        );
        let delivered_mark = diagnostics.len();
        expression_types::report_delivered_type_mismatch(
            program,
            machine.symbol,
            value,
            state.return_type,
            &return_owner,
            diagnostics,
        );
        let shape_mark = diagnostics.len();
        if shape_mark == delivered_mark {
            validate_expression_type_handle(
                program,
                machine.symbol,
                value,
                state.return_type,
                diagnostics,
                ExpressionTypeOwner::StateTerminalExpression {
                    machine: machine.name.as_str(),
                    state: state_name,
                },
            );
        }
        if diagnostics.len() != shape_mark {
            return;
        }
        if let Some(target) = program.primitive_type_reference(state.return_type) {
            expression_types::report_cross_class_store(
                program,
                Some(machine),
                Some(state),
                value,
                target,
                &slot_context,
                "return value",
                diagnostics,
            );
        }
        expression_types::report_place_value_mismatches(
            program,
            machine,
            Some(state),
            value,
            state.return_type,
            &slot_context,
            "return value",
            diagnostics,
        );
        domain_weakening::validate_return_domain_weakening(
            program,
            machine,
            state,
            value,
            &slot_context,
            diagnostics,
        );
    }

    pub(crate) fn validate_state_statement_node<'p>(
        program: &'p dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state_name: &str,
        current_state: Option<&typed_trees::state::State>,
        symbols: &TopLevelSymbols<'_>,
        writable_roots: &WritableRoots<'_, '_>,
        statement_handle: typed_trees::statement::StatementHandle,
        statement_index: usize,
        statement: &StatementNode,
        value_environment: &mut arithmetic_domains::ValueEnvironment,
        transition_values: &transitions::TransitionValueEnvironments,
        direct_written: Option<Vec<String>>,
        local_write_origins: Option<&[crate::LocalWriteOrigin]>,
        call_frames: Option<&crate::CallFrameResolver<'_>>,
        arithmetic_decisions: &mut arithmetic_domains::ArithmeticDecisions,
        stored_integer_writes: &mut Vec<checked_trees::CheckedStoredIntegerWrite>,
        boundary_operator_applications: &mut Vec<ValidatedBoundaryOperatorApplication>,
        bound_lookup: &mut (
            &'p dyn crate::CheckingInput,
            Option<crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'p>>,
        ),
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let diagnostic_mark = diagnostics.len();
        if let Some(state) = current_state {
            access_views::validate_statement(program, machine, state, statement, diagnostics);
        }
        let scope = StatementScope {
            program,
            machine,
            state_name,
            current_state,
            statement_index,
            symbols,
            writable_roots,
            transition_values,
            local_write_origins,
            call_frames,
        };
        if let Some(state) = current_state {
            unreachable_branches::warn(
                program,
                machine,
                state,
                statement,
                value_environment,
                diagnostics,
            );
        }
        let mut outputs = StatementOutputs {
            stored_integer_writes,
            value_environment,
            arithmetic_decisions,
            boundary_operator_applications,
            diagnostics,
        };
        match statement {
            StatementNode::Assembly(_) => {}
            StatementNode::AssemblyFact(_) => {
                assembly_fact_statements::validate(&scope, &mut outputs, statement)
            }
            StatementNode::Assignment(_) => assignment_statements::validate(
                &scope,
                &mut outputs,
                statement,
                direct_written,
                bound_lookup,
            ),
            StatementNode::Call(_) => call_statements::validate(
                &scope,
                &mut outputs,
                statement,
                statement_handle,
                direct_written,
            ),
            StatementNode::Expression(_) => {
                expression_statements::validate(&scope, &mut outputs, statement)
            }
            StatementNode::LocalData(_) => {
                local_data_statements::validate(&scope, &mut outputs, statement)
            }
            StatementNode::Transition(_) => {
                transition_statements::validate(&scope, &mut outputs, statement)
            }
        }
        if outputs.diagnostics.len() > diagnostic_mark
            && let Some(source_span) = statement_source_span(program, statement)
        {
            for diagnostic in &mut outputs.diagnostics[diagnostic_mark..] {
                if diagnostic.source_span.is_none() {
                    diagnostic.source_span = Some(source_span);
                }
            }
        }
    }

    pub fn statement_source_span(
        program: &dyn crate::CheckingInput,
        statement: &StatementNode,
    ) -> Option<source::SourceSpan> {
        let authored = |source_span: source::SourceSpan| -> Option<source::SourceSpan> {
            (source_span != source::SourceSpan::default()).then_some(source_span)
        };
        let expression_span = |handle: typed_trees::expression::ExpressionHandle| {
            handle
                .is_valid()
                .then(|| program.expression_table.source_span(handle))
                .and_then(authored)
        };
        match statement {
            StatementNode::Assembly(_) => None,
            StatementNode::AssemblyFact(fact) => {
                program.proof_fact_source_span(fact.fact).and_then(authored)
            }
            StatementNode::Assignment(assignment) => {
                expression_span(assignment.value).or_else(|| expression_span(assignment.target))
            }
            StatementNode::Call(call) => authored(call.source_span),
            StatementNode::Expression(expression) => expression_span(*expression),
            StatementNode::LocalData(local) => expression_span(local.initial_value),
            StatementNode::Transition(transition) => authored(transition.source_span),
        }
    }
}
pub use contract_queries::{
    validate_checked_operator_realization_contract, validate_generic_machine_contract_entailment,
};
pub use dependency_bodies::{DependencyBodyJudgment, FrozenDependencyBody};
pub use statements::statement_source_span;
use statements::validate_state_statement_node;

pub use checked_trees::operators::ExactIntegerCastFact;

use crate::proof_contracts::arithmetic_domains::ExpressionInterval;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramValidationFacts {
    pending_integer_formations:
        Vec<arithmetic_domains::storage_formations::PendingIntegerFormation>,
    pending_construction_predicates: Vec<struct_literals::PendingConstructionPredicates>,
    pub quotient_formations: Vec<quotients::ValidatedQuotientFormation>,
    pub integer_range_obligations: Vec<checked_trees::IntegerRangeObligation>,
    pub static_machine_selections: machine_parameters::PreparedStaticMachineSelections,
    formed_static_machine_selections: machine_parameters::FormedStaticMachineSelections,
    pub stored_integer_writes: Vec<checked_trees::CheckedStoredIntegerWrite>,
    pub integer_assignments: Vec<checked_trees::CheckedIntegerAssignmentDecision>,
    pub boundary_operator_applications: Vec<ValidatedBoundaryOperatorApplication>,
    pub exact_integer_casts: Vec<ExactIntegerCastFact>,
    pub decided_formations: Vec<typed_trees::expression::ExpressionHandle>,
    pub trap_free_expressions: Vec<typed_trees::expression::ExpressionHandle>,
    pub expression_intervals: Vec<ExpressionInterval>,
    pub float_meaning_projection_invocations: Vec<ValidatedFloatMeaningProjectionInvocation>,
    pub float_meaning_equality_propositions: Vec<ValidatedFloatMeaningEqualityProposition>,
    pub fact_call_projections: Vec<fact_call_projections::ValidatedFactCallProjection>,
    pub specification_calls:
        Vec<crate::machine_calls::denotational_calls::SpecificationCallRequest>,
    pub integer_embedding_calls: Vec<ValidatedIntegerEmbeddingCall>,
    pub proof_recursive_components: Vec<ValidatedProofCallComponent>,
    pub self_call_descents: Vec<crate::ValidatedSelfCallDescent>,
}

impl ProgramValidationFacts {
    pub fn discharge_storage_formations(
        &mut self,
        program: &dyn crate::CheckingInput,
    ) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
        let discharged = arithmetic_domains::storage_formations::discharge(
            program,
            &self.pending_integer_formations,
        )?;
        if let Err(errors) = struct_literals::discharge_pending_predicates(
            program,
            &self.pending_construction_predicates,
        ) {
            let mut diagnostics = discharged.overflow;
            diagnostics.extend(errors);
            return Err(diagnostics);
        }
        self.decided_formations.extend(discharged.proven);
        self.decided_formations
            .sort_by_key(|expression| checked_trees::operators::handle_position(*expression));
        self.decided_formations.dedup();
        self.pending_integer_formations.clear();
        self.pending_construction_predicates.clear();
        Ok(discharged.overflow)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProvenContractExpressions {
    pub entry: Vec<ExpressionHandle>,
    pub normal_returns: Vec<ExpressionHandle>,
    pub conditional_content: Vec<crate::ConditionalContentGuarantee>,
}

#[derive(Debug, Clone, Copy)]
pub enum OpaquePropertyValidation<'a> {
    Required(&'a [OpaqueDataPropertyReceipt]),
    PendingBuildSelection,
}

#[derive(Debug)]
pub struct ProgramValidation {
    pub facts: ProgramValidationFacts,
    pub operational: flow_effects::OperationalPlan,
    pub service_reaches: flow_effects::ServiceReachInferencePlan,
    pub deferred_entailment: DeferredEntailment,
    pub frozen_dependency_bodies: Vec<FrozenDependencyBody>,
    pub deferred_overflow: Vec<Diagnostic>,
}

pub fn validate_program(program: &dyn crate::CheckingInput) -> Result<(), Vec<Diagnostic>> {
    validate(
        program,
        OpaquePropertyValidation::Required(&[]),
        QuotientRequestAdmission::AtValidation,
        EffectAdmission::Complete,
        &[],
        &[],
        false,
    )
    .and_then(|mut validated| {
        let overflow = validated.facts.discharge_storage_formations(program)?;
        match judge_contract_entailment(program, validated.deferred_entailment) {
            Ok(_) if overflow.is_empty() => Ok(()),
            Ok(_) => Err(overflow),
            Err(errors) => {
                let mut diagnostics = overflow;
                diagnostics.extend(errors);
                Err(diagnostics)
            }
        }
    })
}

pub fn prepare_specialized_program(
    program: &dyn crate::CheckingInput,
    opaque_properties: OpaquePropertyValidation<'_>,
    prior_dependency_bodies: &[DependencyBodyJudgment],
    prior_frozen_bodies: &[FrozenDependencyBody],
    record_dependency_bodies: bool,
) -> Result<ProgramValidation, Vec<Diagnostic>> {
    validate(
        program,
        opaque_properties,
        QuotientRequestAdmission::AfterCheckedFacts,
        EffectAdmission::AfterOwnership,
        prior_dependency_bodies,
        prior_frozen_bodies,
        record_dependency_bodies,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EffectAdmission {
    Complete,
    AfterOwnership,
}

static DEPENDENCY_ENTAILMENT_REUSED: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);
static DEPENDENCY_ENTAILMENT_JUDGED: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

pub fn dependency_entailment_totals() -> (u64, u64) {
    (
        DEPENDENCY_ENTAILMENT_REUSED.load(std::sync::atomic::Ordering::Relaxed),
        DEPENDENCY_ENTAILMENT_JUDGED.load(std::sync::atomic::Ordering::Relaxed),
    )
}

#[derive(Clone, Copy)]
pub(crate) enum DependencyBodyRead {
    ReturnOrigin,
    BindingStability,
    ObservationFreedom,
    ReturnInterval,
}

static FROZEN_BODY_READS: [std::sync::atomic::AtomicU64; 4] = [
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
];
static SOURCE_BODY_READS: [std::sync::atomic::AtomicU64; 4] = [
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
    std::sync::atomic::AtomicU64::new(0),
];

pub fn dependency_body_read_totals() -> [(u64, u64); 4] {
    let load =
        |counter: &std::sync::atomic::AtomicU64| counter.load(std::sync::atomic::Ordering::Relaxed);
    std::array::from_fn(|index| {
        (
            load(&FROZEN_BODY_READS[index]),
            load(&SOURCE_BODY_READS[index]),
        )
    })
}

pub(crate) fn note_dependency_body_read(
    program: &dyn crate::CheckingInput,
    machine: symbols::SymbolHandle,
    read: DependencyBodyRead,
) {
    let frozen = program.checking_memos().is_some_and(|memos| {
        memos
            .admitted_dependency_bodies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(&machine)
    });
    let counters = if frozen {
        &FROZEN_BODY_READS
    } else {
        &SOURCE_BODY_READS
    };
    counters[read as usize].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn record_dependency_entailment(reused: bool) {
    let counter = if reused {
        &DEPENDENCY_ENTAILMENT_REUSED
    } else {
        &DEPENDENCY_ENTAILMENT_JUDGED
    };
    counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn validate<'program>(
    program: &'program dyn crate::CheckingInput,
    opaque_properties: OpaquePropertyValidation<'_>,
    quotient_admission: QuotientRequestAdmission,
    effect_admission: EffectAdmission,
    prior_dependency_bodies: &[DependencyBodyJudgment],
    prior_frozen_bodies: &[FrozenDependencyBody],
    record_dependency_bodies: bool,
) -> Result<ProgramValidation, Vec<Diagnostic>> {
    let mut formation_obligations = Vec::new();
    let mut pending_construction_predicates = Vec::new();
    let mut diagnostics = Vec::new();
    if let Err(mut errors) =
        crate::conformance_applications::validate_conformance_applications(program)
    {
        diagnostics.append(&mut errors);
    }
    if let Err(message) = crate::validate_boundary_completion_declarations(program) {
        diagnostics.push(Diagnostic::error(message));
    }
    literals::validate_anonymous_numeric_formation(program, &mut diagnostics);
    crate::declarations::traits::validate_member_formations(program, &mut diagnostics);
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let mut boundary_operator_applications = Vec::new();
    let mut arithmetic_decisions = arithmetic_domains::ArithmeticDecisions::default();
    let mut stored_integer_writes = Vec::new();
    let prologue = declaration_prologue::PrologueInputs::build(
        program,
        effect_admission,
        quotient_admission,
        opaque_properties,
    );
    let declaration_prologue::PrologueResults {
        integer_embedding_calls,
        quotient_formations,
        fact_call_projections,
        specification_calls,
        proof_recursive_components,
        struct_literals: literals,
    } = declaration_prologue::run(&prologue, &mut diagnostics, &mut formation_obligations);
    if let Some(literals) = literals {
        stored_integer_writes.extend(literals.stored_integer_writes);
        pending_construction_predicates.extend(literals.pending_predicates);
    }
    let declaration_prologue::PrologueInputs {
        symbols,
        conformance_operational,
        conformance_service_reaches,
        proof_only,
        call_frames,
        ..
    } = prologue;
    let mut self_call_descents = Vec::new();
    let mut static_machine_selections =
        machine_parameters::PreparedStaticMachineSelections::default();
    let formation_diagnostic_start = diagnostics.len();
    if let Err(mut errors) = crate::static_arguments::validate_authored_static_arguments(
        program,
        &mut formation_obligations,
    ) {
        diagnostics.append(&mut errors);
    }
    if let Err(mut errors) = crate::static_bindings::validate_static_machine_bindings(
        program,
        &mut formation_obligations,
    ) {
        diagnostics.append(&mut errors);
    }
    let mut formed_static_machine_selections =
        machine_parameters::form_static_machine_selections(program, &mut diagnostics);
    if diagnostics[formation_diagnostic_start..]
        .iter()
        .any(Diagnostic::is_error)
    {
        return Err(diagnostics);
    }
    if effect_admission == EffectAdmission::Complete {
        match machine_parameters::admit_static_machine_effects(
            program,
            &conformance_operational,
            std::mem::take(&mut formed_static_machine_selections),
        ) {
            Ok(selections) => static_machine_selections = selections,
            Err(mut errors) => diagnostics.append(&mut errors),
        }
        invocations::validate_invocation_contracts(program, &mut diagnostics);
    }

    let body_index = dependency_bodies::DependencyBodyIndex::prepare(
        program,
        prior_dependency_bodies,
        prior_frozen_bodies,
        record_dependency_bodies,
    );
    if let Some(memos) = program.checking_memos() {
        let mut admitted = memos
            .admitted_dependency_bodies
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for (machine_index, machine) in program.machines().iter().enumerate() {
            if body_index.admits_frozen_body(machine_index) {
                admitted.insert(machine.symbol, ());
            }
        }
    }
    let has_plan_laid_integer_fields = program.plan_laid_layouts.iter().any(|plan| {
        plan.geometries(program.typed_program())
            .any(|geometry| !geometry.integer_fields.is_empty())
    });
    let validate_machine = |machine: &typed_trees::machine::Machine,
                            bound_lookup: &mut (
        &'program dyn crate::CheckingInput,
        Option<crate::proof_contracts::immutable_integer_bounds::ImmutableBoundLookup<'program>>,
    )|
     -> MachineValidation {
        let mut diagnostics = Vec::new();
        let mut arithmetic_decisions = arithmetic_domains::ArithmeticDecisions::default();
        let mut formation_obligations = Vec::new();
        let mut stored_integer_writes = Vec::new();
        let mut boundary_operator_applications = Vec::new();
        let mut self_call_descents = Vec::new();
        let machine_diagnostic_mark = diagnostics.len();
        let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);

        validate_owned_data(
            program,
            machine,
            &symbols,
            &mut diagnostics,
            &mut formation_obligations,
        );
        validate_generic_conformance_bounds(
            program,
            machine,
            &symbols,
            &mut diagnostics,
            &mut formation_obligations,
        );
        validate_machine_contracts(program, machine, &mut diagnostics);
        arithmetic_decisions.extend(
            arithmetic_domains::validate_machine_total_specification_arithmetic(
                program,
                machine,
                &mut diagnostics,
                &mut formation_obligations,
            ),
        );
        if effect_admission == EffectAdmission::Complete {
            validate_machine_trait_conformances(
                program,
                &conformance_service_reaches,
                machine,
                &symbols,
                &mut diagnostics,
                &mut formation_obligations,
            );
        }

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
                     BODYLESS external realization (a composite lowering is an \
                     ordinary checked body; an accepted axiom carries no \
                     realization)",
                    machine.name,
                )));
            }
            if via_count == 1 && is_external {
                validate_external_leaf_native_shapes(program, machine, &mut diagnostics);
            }
        }

        let mut incoming_environments = None;
        let is_content_projection_machine =
            content_projections::is_content_projection_machine(program, machine);
        let is_proof_machine = proof_only.is_proof_machine(program, machine);
        let entry_judge =
            crate::proof_contracts::contract_entailment::CallerEntryJudge::new(program, machine);
        for (state_index, state) in program.machine_states(machine).iter().enumerate() {
            {
                crate::value_custody::reference_local_establishment::validate(
                    program,
                    machine,
                    state,
                    &mut diagnostics,
                );
            };
            completion_results::validate(program, machine, state, &mut diagnostics);
            validate_local_data_names(
                program.statement_table.statements(state.statement_nodes),
                &machine_symbols,
                program.state_parameters(state),
                machine.name.as_str(),
                state.name.as_str(),
                &mut diagnostics,
            );
            let mut value_environment = {
                if state_index == 0 {
                    arithmetic_domains::requires_value_environment(program, machine, state)
                } else {
                    incoming_environments
                        .get_or_insert_with(|| {
                            arithmetic_domains::incoming_guard_environments(
                                program,
                                machine,
                                call_frames.as_ref(),
                            )
                        })
                        .iter()
                        .find_map(|(symbol, environment)| {
                            (*symbol == state.symbol).then(|| environment.clone())
                        })
                        .unwrap_or_default()
                }
            };
            let statements = program.statement_table.statements(state.statement_nodes);
            let mut order = program
                .statement_table
                .finite_execution_order(state.statement_nodes)
                .ok()
                .flatten()
                .unwrap_or_default();
            let executed_count = order.len();
            let mut executed = vec![false; statements.len()];
            for &statement_index in &order {
                executed[statement_index] = true;
            }
            order.extend((0..statements.len()).filter(|index| !executed[*index]));
            for (position, statement_index) in order.into_iter().enumerate() {
                if position >= executed_count {
                    value_environment.clear();
                }
                let statement_handle = state.statement_nodes.handle_at(statement_index as u32);
                let statement = &statements[statement_index];
                value_environment.clear_numeric_observations();
                let statement_diagnostic_mark = diagnostics.len();
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
                    &value_environment,
                    call_frames.as_ref(),
                );
                if let Some(local) = statement.as_local_data()
                    && local.role == language_core::LocalBindingRole::Value
                    && !local.type_is_inferred
                    && !local.is_operand_capture
                {
                    statements::validate_local_type_reference(
                        program,
                        machine,
                        Some(state),
                        &state.name,
                        local,
                        &symbols,
                        &value_environment,
                        &mut diagnostics,
                        &mut arithmetic_decisions.integer_range_obligations,
                    );
                }
                {
                    value_environment.prepare_numeric_observations(
                        program,
                        machine,
                        state,
                        statement_index,
                    );
                };
                let value_written = {
                    call_frames.as_ref().and_then(|frames| {
                        frames.statement_value_may_write_paths_for_symbols(
                            machine,
                            &machine_symbols,
                            statement,
                        )
                    })
                };
                if crate::trace_flags::struct_trace() {
                    eprintln!(
                        "STRUCT completed-value-writes caller={} state={} statement={} written={value_written:?}",
                        machine.name, state.name, statement_index
                    );
                }
                if let Some(written) = value_written {
                    value_environment.invalidate_call_paths(program, Some(state), &written);
                } else if call_frames.is_some()
                    || calls::statement_value_expression_roots(program, statement)
                        .iter()
                        .any(|expression| calls::expression_has_calls(program, *expression))
                {
                    value_environment.invalidate_unknown_writes(program, Some(state));
                }
                if !is_content_projection_machine {
                    {
                        validate_value_position_calls(
                            program,
                            machine,
                            state,
                            statement,
                            &symbols,
                            &writable_roots,
                            &value_environment,
                            &transition_values,
                            &entry_judge,
                            &mut boundary_operator_applications,
                            &mut diagnostics,
                            &mut formation_obligations,
                        );
                    };
                }
                if is_proof_machine {
                    self_call_descents.extend(validate_proof_machine_recursion(
                        program,
                        machine,
                        state,
                        statement_index,
                        statement,
                        &mut diagnostics,
                    ));
                } else {
                    validate_self_recursive_call_positions(
                        program,
                        machine,
                        state,
                        statement,
                        &mut diagnostics,
                    );
                }
                let (direct_written, local_write_origins) = {
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
                    let local_write_origins = if matches!(statement, StatementNode::Assignment(_))
                        && has_plan_laid_integer_fields
                    {
                        call_frames.as_ref().and_then(|frames| {
                            frames.local_write_origins_before_statement(machine, statement)
                        })
                    } else {
                        Some(Vec::new())
                    };
                    (direct_written, local_write_origins)
                };
                {
                    validate_state_statement_node(
                        program,
                        machine,
                        &state.name,
                        Some(state),
                        &symbols,
                        &writable_roots,
                        statement_handle,
                        statement_index,
                        statement,
                        &mut value_environment,
                        &transition_values,
                        direct_written,
                        local_write_origins.as_deref(),
                        call_frames.as_ref(),
                        &mut arithmetic_decisions,
                        &mut stored_integer_writes,
                        &mut boundary_operator_applications,
                        bound_lookup,
                        &mut diagnostics,
                    );
                };
                if diagnostics.len() > statement_diagnostic_mark
                    && let Some(source_span) = statements::statement_source_span(program, statement)
                {
                    for diagnostic in &mut diagnostics[statement_diagnostic_mark..] {
                        if diagnostic.source_span.is_none() {
                            diagnostic.source_span = Some(source_span);
                        }
                    }
                }
            }
        }
        if diagnostics.len() > machine_diagnostic_mark {
            let machine_source_span = program
                .symbols
                .symbol_provenance_source_span(machine.symbol)
                .filter(|source_span| *source_span != source::SourceSpan::default());
            for diagnostic in &mut diagnostics[machine_diagnostic_mark..] {
                if diagnostic.source_span.is_none() {
                    diagnostic.source_span = machine_source_span;
                }
            }
        }
        MachineValidation {
            diagnostics,
            arithmetic_decisions,
            formation_obligations,
            stored_integer_writes,
            boundary_operator_applications,
            self_call_descents,
        }
    };
    let machines = program.machines();
    let judged = checking_thread_pool().install(|| {
        machines
            .par_iter()
            .map_init(
                || (program, None),
                |bound_lookup, machine| validate_machine(machine, bound_lookup),
            )
            .collect::<Vec<_>>()
    });
    for machine in judged {
        diagnostics.extend(machine.diagnostics);
        arithmetic_decisions.extend(machine.arithmetic_decisions);
        formation_obligations.extend(machine.formation_obligations);
        stored_integer_writes.extend(machine.stored_integer_writes);
        for application in machine.boundary_operator_applications {
            crate::declarations::operators::retain_validated_boundary_operator_application(
                &mut boundary_operator_applications,
                application,
                &mut diagnostics,
            );
        }
        self_call_descents.extend(machine.self_call_descents);
    }
    let frozen_dependency_bodies = body_index.frozen_bodies();

    let arithmetic_domains::ArithmeticDecisions {
        allows_pending_formations: _,
        pending_formations: pending_integer_formations,
        mut integer_range_obligations,
        mut exact_integer_casts,
        mut integer_assignments,
        mut decided_formations,
        mut trapping_decisions,
        expression_values: walked_intervals,
    } = arithmetic_decisions;
    integer_range_obligations.extend(formation_obligations);
    let expression_intervals = arithmetic_domains::collect_expression_intervals(walked_intervals);
    crate::value_custody::qualification_casts::validate(
        program,
        &expression_intervals,
        &mut diagnostics,
    );
    let mut deferred_overflow = Vec::new();
    if diagnostics.iter().any(Diagnostic::is_error) {
        let mut retained = Vec::new();
        for diagnostic in diagnostics.drain(..) {
            if is_exact_overflow_diagnostic(&diagnostic) {
                deferred_overflow.push(diagnostic);
            } else {
                retained.push(diagnostic);
            }
        }
        diagnostics = retained;
        if diagnostics.iter().any(Diagnostic::is_error) {
            diagnostics.append(&mut judge_machines(program, &body_index).diagnostics);
            diagnostics.append(&mut deferred_overflow);
        }
    }
    finish_diagnostics(diagnostics)?;
    let mut anonymous_bitwise = Vec::new();
    literals::validate_anonymous_bitwise(program, &mut anonymous_bitwise);
    finish_diagnostics(anonymous_bitwise)?;
    finish_diagnostics(literals::anonymous_integer_landing_errors(program))?;
    trapping_decisions
        .sort_by_key(|(expression, _)| checked_trees::operators::handle_position(*expression));
    let mut trapping_verdicts = Vec::<(ExpressionHandle, bool)>::new();
    for (expression, proven) in trapping_decisions {
        match trapping_verdicts.last_mut() {
            Some((previous, all_proven)) if *previous == expression => *all_proven &= proven,
            _ => trapping_verdicts.push((expression, proven)),
        }
    }
    let trap_free_expressions = trapping_verdicts
        .into_iter()
        .filter_map(|(expression, proven)| proven.then_some(expression))
        .collect();
    decided_formations
        .sort_by_key(|expression| checked_trees::operators::handle_position(*expression));
    decided_formations.dedup();
    integer_assignments.sort_by_key(|decision| {
        (
            (
                decision.machine.arena_index(),
                decision.machine.generation(),
            ),
            (decision.state.arena_index(), decision.state.generation()),
            decision.statement_index,
        )
    });
    integer_assignments.dedup();
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
            pending_integer_formations,
            pending_construction_predicates,
            quotient_formations,
            integer_range_obligations,
            static_machine_selections,
            formed_static_machine_selections,
            stored_integer_writes,
            integer_assignments,
            boundary_operator_applications,
            exact_integer_casts,
            decided_formations,
            trap_free_expressions,
            expression_intervals,
            float_meaning_projection_invocations,
            float_meaning_equality_propositions,
            fact_call_projections,
            specification_calls,
            integer_embedding_calls,
            proof_recursive_components,
            self_call_descents,
        },
        deferred_entailment: DeferredEntailment { body_index },
        frozen_dependency_bodies,
        deferred_overflow,
    })
}

pub(crate) fn is_exact_overflow_diagnostic(diagnostic: &Diagnostic) -> bool {
    diagnostic.is_error()
        && diagnostic.message.starts_with("exact arithmetic in ")
        && diagnostic.message.contains("may overflow")
}

pub struct DeferredEntailment {
    body_index: dependency_bodies::DependencyBodyIndex,
}

impl std::fmt::Debug for DeferredEntailment {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeferredEntailment")
            .finish_non_exhaustive()
    }
}

pub struct ContractEntailment {
    pub proven_machine_contracts: Vec<(symbols::SymbolHandle, ProvenContractExpressions)>,
    pub dependency_body_judgments: Vec<DependencyBodyJudgment>,
}

pub fn judge_contract_entailment(
    program: &dyn crate::CheckingInput,
    deferred: DeferredEntailment,
) -> Result<ContractEntailment, Vec<Diagnostic>> {
    let call_frames = calls::CallFrameResolver::new(program);
    let mut diagnostics = Vec::new();
    {
        default_domains::validate_default_domain_writes(
            program,
            call_frames.as_ref(),
            &mut diagnostics,
        );
    };
    let judged = judge_machines(program, &deferred.body_index);
    diagnostics.extend(judged.diagnostics);
    finish_diagnostics(diagnostics)?;
    Ok(ContractEntailment {
        proven_machine_contracts: judged.proven_machine_contracts,
        dependency_body_judgments: deferred.body_index.judgments(&judged.captured_entailments),
    })
}

pub(crate) struct JudgedEntailment {
    diagnostics: Vec<Diagnostic>,
    proven_machine_contracts: Vec<(symbols::SymbolHandle, ProvenContractExpressions)>,
    captured_entailments: Vec<Option<dependency_bodies::CapturedEntailment>>,
}

fn validate_machine_scalar_loan_handbacks(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let frames = crate::CallFrameResolver::new(program);
    for state in program.machine_states(machine) {
        let statements = program.statement_table.statements(state.statement_nodes);
        for (statement_index, statement) in statements.iter().enumerate() {
            let mark = diagnostics.len();
            crate::proof_contracts::contract_entailment::validate_scalar_loan_handbacks(
                program,
                machine,
                state,
                statement_index,
                frames.as_ref(),
                diagnostics,
            );
            if diagnostics.len() > mark
                && let Some(source_span) = statements::statement_source_span(program, statement)
            {
                for diagnostic in &mut diagnostics[mark..] {
                    if diagnostic.source_span.is_none() {
                        diagnostic.source_span = Some(source_span);
                    }
                }
            }
        }
    }
}

fn judge_machines(
    program: &dyn crate::CheckingInput,
    body_index: &dependency_bodies::DependencyBodyIndex,
) -> JudgedEntailment {
    let judge_machine = |machine_index: usize, machine: &typed_trees::machine::Machine| {
        let mut diagnostics = Vec::new();
        let mut proven_machine_contract = None;
        let mut captured_entailment = None;
        {
            if let Some(proven) = body_index.reuse(program, machine, machine_index) {
                record_dependency_entailment(true);
                if machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody {
                    let judged =
                        if crate::proof_contracts::contract_entailment::entailment_covers_all_exits(
                            program, machine,
                        ) {
                            proven
                        } else {
                            ProvenContractExpressions::default()
                        };
                    proven_machine_contract = Some((machine.symbol, judged));
                }
            } else {
                if body_index.active_observation() {
                    record_dependency_entailment(false);
                }
                let mut machine_diagnostics = Vec::new();
                validate_machine_scalar_loan_handbacks(program, machine, &mut machine_diagnostics);
                let mut proven = ProvenContractExpressions::default();
                crate::proof_contracts::contract_entailment::validate_machine_contract_entailment(
                    program,
                    machine,
                    &mut machine_diagnostics,
                    &mut proven,
                );
                captured_entailment = body_index.capture(program, machine, machine_index, &proven);
                if machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody {
                    let judged = if machine_diagnostics.is_empty()
                        && crate::proof_contracts::contract_entailment::entailment_covers_all_exits(
                            program, machine,
                        ) {
                        proven
                    } else {
                        ProvenContractExpressions::default()
                    };
                    proven_machine_contract = Some((machine.symbol, judged));
                }
                diagnostics.append(&mut machine_diagnostics);
            }
        }
        if !diagnostics.is_empty() {
            let machine_source_span = program
                .symbols
                .symbol_provenance_source_span(machine.symbol)
                .filter(|source_span| *source_span != source::SourceSpan::default());
            for diagnostic in &mut diagnostics {
                if diagnostic.source_span.is_none() {
                    diagnostic.source_span = machine_source_span;
                }
            }
        }
        (diagnostics, proven_machine_contract, captured_entailment)
    };
    let machines = program.machines();
    let judged = checking_thread_pool().install(|| {
        machines
            .par_iter()
            .enumerate()
            .map(|(machine_index, machine)| judge_machine(machine_index, machine))
            .collect::<Vec<_>>()
    });
    let mut diagnostics = Vec::new();
    let mut proven_machine_contracts = Vec::new();
    let mut captured_entailments = Vec::with_capacity(judged.len());
    for (mut machine_diagnostics, proven_machine_contract, captured_entailment) in judged {
        captured_entailments.push(captured_entailment);
        diagnostics.append(&mut machine_diagnostics);
        if let Some((symbol, judged)) = proven_machine_contract
            && !proven_machine_contracts
                .iter()
                .any(|(existing, _)| *existing == symbol)
        {
            proven_machine_contracts.push((symbol, judged));
        }
    }
    JudgedEntailment {
        diagnostics,
        proven_machine_contracts,
        captured_entailments,
    }
}

pub(crate) fn finish_diagnostics(diagnostics: Vec<Diagnostic>) -> Result<(), Vec<Diagnostic>> {
    if diagnostics.iter().any(Diagnostic::is_error) {
        return Err(diagnostics);
    }
    for warning in &diagnostics {
        eprintln!("{warning}");
    }
    Ok(())
}

pub(crate) fn finish_strict_diagnostics(
    diagnostics: Vec<Diagnostic>,
) -> Result<(), Vec<Diagnostic>> {
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

pub fn admit_prepared_program_effects(
    program: &dyn crate::CheckingInput,
    operational: &flow_effects::OperationalPlan,
    facts: &mut ProgramValidationFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut formation_obligations = Vec::new();
    let mut diagnostics = Vec::new();
    crate::value_custody::atomic_fences::validate_atomic_fence_declarations(
        program,
        &mut diagnostics,
    );
    let symbols = TopLevelSymbols::build(program, &mut diagnostics);
    let service_reaches = infer_service_reaches(program, operational);
    validate_conformances(
        program,
        &service_reaches,
        &symbols,
        &mut diagnostics,
        &mut formation_obligations,
    );
    if let Err(mut errors) = collect_dynamic_implementation_selections(program) {
        diagnostics.append(&mut errors);
    }
    match machine_parameters::admit_static_machine_effects(
        program,
        operational,
        std::mem::take(&mut facts.formed_static_machine_selections),
    ) {
        Ok(selections) => facts.static_machine_selections = selections,
        Err(mut errors) => diagnostics.append(&mut errors),
    }
    invocations::validate_invocation_contracts(program, &mut diagnostics);
    for machine in program.machines() {
        validate_machine_trait_conformances(
            program,
            &service_reaches,
            machine,
            &symbols,
            &mut diagnostics,
            &mut formation_obligations,
        );
    }
    facts.integer_embedding_calls =
        proof_embeddings::validate_integer_embedding_calls(program, &mut diagnostics);
    facts.fact_call_projections = fact_call_projections::validate_fact_call_projections(
        program,
        operational,
        &service_reaches,
        &mut diagnostics,
    );
    facts.specification_calls =
        crate::machine_calls::denotational_calls::validate_specification_calls(
            program,
            operational,
            &service_reaches,
            &mut diagnostics,
        );
    if let Err(mut errors) = crate::validate_behavior_plan(program, operational) {
        diagnostics.append(&mut errors);
    }
    validate_cleanup_normal_completion(program, operational, &mut diagnostics);
    facts
        .integer_range_obligations
        .extend(formation_obligations);
    finish_diagnostics(diagnostics)
}

fn validate_cleanup_normal_completion(
    program: &dyn crate::CheckingInput,
    operational: &flow_effects::OperationalPlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, dependency) in operational.cleanup_dependencies.iter() {
        let mut pending = vec![dependency.target_machine];
        let mut visited = Vec::new();
        while let Some(target) = pending.pop() {
            if target == dependency.caller_machine {
                diagnostics.push(Diagnostic::error(format!(
                    "selected cleanup dependency from `{}` to `{}` creates a recursive call component without an exact cleanup-edge ranking witness",
                    program.symbols.name(dependency.caller_machine),
                    program.symbols.name(dependency.target_machine),
                )));
                break;
            }
            if visited.contains(&target) {
                continue;
            }
            visited.push(target);
            let Some(index) = operational.machine_index_by_symbol(target) else {
                continue;
            };
            let machine = &operational.machines()[index];
            let exits = operational
                .states
                .span_or_empty(machine.states)
                .iter()
                .any(|state| {
                    operational
                        .calls
                        .span_or_empty(state.calls)
                        .iter()
                        .any(|call| {
                            if call.target_machine_symbol.is_valid() {
                                pending.push(call.target_machine_symbol);
                            }
                            let requirement = crate::exact_external_leaf_boundary_requirement(
                                program,
                                call.target_state_symbol,
                            )
                            .map_or(call.target_state_symbol, |(requirement, _)| requirement);
                            matches!(
                                crate::boundary_requirement_completion(program, requirement),
                                semantic_vocabulary::BoundaryCompletion::External(_)
                            )
                        })
                });
            if exits {
                diagnostics.push(Diagnostic::error(format!(
                    "cleanup machine `{}` must return normally; its selected call closure permits external process exit",
                    program.symbols.name(dependency.target_machine))));
                break;
            }
            pending.extend(
                operational
                    .cleanup_dependencies
                    .iter()
                    .filter_map(|(_, child)| {
                        (child.caller_machine == target).then_some(child.target_machine)
                    }),
            );
        }
    }
}

pub(crate) struct MachineValidation {
    diagnostics: Vec<Diagnostic>,
    arithmetic_decisions: arithmetic_domains::ArithmeticDecisions,
    formation_obligations: Vec<checked_trees::IntegerRangeObligation>,
    stored_integer_writes: Vec<checked_trees::CheckedStoredIntegerWrite>,
    boundary_operator_applications: Vec<ValidatedBoundaryOperatorApplication>,
    self_call_descents: Vec<crate::ValidatedSelfCallDescent>,
}

const CHECKING_STACK_SIZE: usize = 256 * 1024 * 1024;

pub fn checking_thread_pool() -> &'static rayon::ThreadPool {
    static POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .thread_name(|index| format!("omega-check-{index}"))
            .stack_size(CHECKING_STACK_SIZE)
            .build()
            .expect("machine validation worker pool")
    })
}
