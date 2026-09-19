use crate::contracts::invocations::lower_authored_invocations;
use crate::contracts::proof_facts::lower_proof_facts;
use crate::declarations::state::lower_state;
use crate::expressions::expression::lower_expression_handle;
use crate::lowerer::Lowerer;
use crate::signatures::type_parameters::lower_type_parameters;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

mod generic_data;

pub(crate) fn lower_machine(
    lowerer: &mut Lowerer,
    machine: &resolved::machine::Machine,
) -> Result<typed::machine::Machine, Diagnostic> {
    let derived = generic_data::is_derived(lowerer.source_trees, machine)?;
    let exposure = if derived {
        None
    } else {
        lowerer.type_reference_exposure
    };
    lowerer
        .with_type_reference_exposure(exposure, |lowerer| lower_machine_contents(lowerer, machine))
}

/// The operator-signature view of a token-bearing machine, or `None` for a
/// named machine and for a generic-data clone (its template already carries
/// the binding; a clone entering the candidate set would only duplicate it).
///
/// The view is how `machine + Vec2::add(...)` reaches operand-directed
/// selection: `typed_trees::operator::resolve_spelling` enumerates
/// `OperatorDefinition` records, so the machine's entry-state parameters and
/// return type, its head contracts, and its type parameters are exposed under
/// the machine's own symbol. Every span is shared with the machine record;
/// nothing is copied and no second declaration or executable identity exists.
/// The path members follow the authored `operator` convention
/// (`[owner, leaf]` for an attached machine) so path-based diagnostics and
/// `satisfies Owner::leaf` matching read the same shape.
pub(crate) fn lower_token_binding_view(
    lowerer: &mut Lowerer,
    machine: &resolved::machine::Machine,
    typed_machine: &typed::machine::Machine,
) -> Result<Option<typed::operator::OperatorDefinition>, Diagnostic> {
    let Some(spelling) = typed_machine.spelling else {
        return Ok(None);
    };
    if generic_data::is_derived(lowerer.source_trees, machine)? {
        return Ok(None);
    }
    let entry = lowerer
        .typed_trees
        .machine_states(typed_machine)
        .first()
        .ok_or_else(|| {
            Diagnostic::error(format!(
                "`{}` binds the fixed operator token `{}` but declares no entry state",
                typed_machine.name,
                spelling.symbol()
            ))
        })?;
    let mut view = typed::operator::OperatorDefinition {
        is_public: typed_machine.is_public,
        is_boundary: typed_machine.supply_mode == language_semantics::MachineSupplyMode::Boundary,
        symbol: typed_machine.symbol,
        name: Default::default(),
        lifetime_parameters: typed_machine.lifetime_parameters.clone(),
        type_parameters: typed_machine.type_parameters,
        parameters: entry.parameters,
        return_type: entry.return_type,
        contracts: typed_machine.contracts,
        spelling: Some(spelling),
        // Authored `operator` declarations count their own tokens for review
        // fingerprints; the machine's identity is fingerprinted as a machine.
        token_count: 0,
        home_domain: attached_domain_symbol(lowerer, typed_machine),
    };
    let leaf = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or_default()
        .to_owned();
    if let Some(attached_data) = &typed_machine.attached_data {
        lowerer
            .typed_trees
            .push_operator_path_member(&mut view, attached_data.clone());
    }
    lowerer
        .typed_trees
        .push_operator_path_member(&mut view, typed::name::Identifier::generated(&leaf));
    Ok(Some(view))
}

/// The domain a token-bearing machine is attached to, matched by its exact
/// declared path: attachment symbols are assigned against data declarations
/// only, so a domain-attached machine carries an invalid attachment symbol.
/// Symbol resolution already required the path to name exactly one domain.
fn attached_domain_symbol(
    lowerer: &Lowerer,
    typed_machine: &typed::machine::Machine,
) -> symbols::SymbolHandle {
    if typed_machine.attached_data_symbol.is_valid() {
        return symbols::SymbolHandle::invalid();
    }
    let Some(attached) = typed_machine.attached_data.as_ref() else {
        return symbols::SymbolHandle::invalid();
    };
    let mut matches = lowerer
        .typed_trees
        .domain_definitions()
        .iter()
        .filter(|domain| domain.name.as_str() == attached.as_str());
    match (matches.next(), matches.next()) {
        (Some(domain), None) => domain.symbol,
        _ => symbols::SymbolHandle::invalid(),
    }
}

fn lower_machine_contents(
    lowerer: &mut Lowerer,
    machine: &resolved::machine::Machine,
) -> Result<typed::machine::Machine, Diagnostic> {
    if let Some(attached_data) = &machine.attached_data {
        crate::type_reference::retain_type_reference_selection(
            lowerer.source_trees,
            &mut lowerer.typed_trees,
            attached_data,
            machine.attached_data_symbol,
            lowerer.type_reference_exposure,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
        )?;
    }
    let mut typed_machine = typed::machine::Machine {
        symbol: machine.symbol,
        name: crate::lowerer::name::lower_name(&machine.name),
        attached_data: machine
            .attached_data
            .as_ref()
            .map(crate::lowerer::name::lower_name),
        attached_data_symbol: machine.attached_data_symbol,
        attached_data_application: typed::types::TypeReferenceHandle::invalid(),
        // `lower_machine` validates the exact clone origin before entering
        // this conversion; retain it for application-property checking.
        generic_data_template: machine.generic_data_origin.template,
        // Copied, never re-derived from the leaf name.
        spelling: machine.spelling,
        is_public: machine.is_public,
        // Copied, never re-derived.
        supply_mode: machine.supply_mode,
        body_is_present: machine.body_is_present,
        structural_type_equations_pending: machine.structural_type_equations_pending,
        // The authored bit and private witness copy here; the final typed
        // normalization attaches subject-bearing schemas and inherited
        // requirement guarantees after every trait has been lowered.
        termination_plan: machine.termination_plan.clone(),
        service_reach_row: machine.service_reach_row,
        service_reach_is_installation_bound: machine.service_reach_is_installation_bound,
        suspends_keyword_source_spans: machine.suspends_keyword_source_spans.clone(),
        blocks_keyword_source_spans: machine.blocks_keyword_source_spans.clone(),
        lifetime_parameters: machine
            .lifetime_parameters
            .iter()
            .map(crate::lowerer::name::lower_name)
            .collect(),
        type_parameters: arena::HandleSpan::empty(),
        owned_data: arena::HandleSpan::empty(),
        satisfies: arena::HandleSpan::empty(),
        conformance_bounds: Vec::new(),
        invokes: arena::HandleSpan::empty(),
        suspends: machine.suspends,
        blocks: machine.blocks,
        contracts: arena::HandleSpan::empty(),
        states: arena::HandleSpan::empty(),
    };

    typed_machine.type_parameters = lower_type_parameters(lowerer, machine.type_parameters)?;

    if let Some(application) = &machine.attached_data_application {
        typed_machine.attached_data_application =
            lower_type_reference_into_table(lowerer, application)?;
    }

    for owned_data in lowerer.source_trees.machine_owned_data(machine.owned_data) {
        let owned_data = lowerer.with_type_reference_exposure(
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation,
            |lowerer| {
                Ok::<_, Diagnostic>(typed::machine::OwnedData {
                    symbol: owned_data.symbol,
                    name: crate::lowerer::name::lower_name(&owned_data.name),
                    type_reference: lower_type_reference_into_table(
                        lowerer,
                        &owned_data.type_reference,
                    )?,
                    initial_value: owned_data
                        .initial_value
                        .is_valid()
                        .then(|| lower_expression_handle(lowerer, owned_data.initial_value))
                        .transpose()?
                        .unwrap_or_else(typed::expression::ExpressionHandle::invalid),
                })
            },
        )?;
        lowerer
            .typed_trees
            .push_machine_owned_data(&mut typed_machine, owned_data);
    }

    for conformance in lowerer
        .source_trees
        .machine_trait_conformances(machine.satisfies)
    {
        crate::type_reference::retain_type_reference_selection(
            lowerer.source_trees,
            &mut lowerer.typed_trees,
            &conformance.name,
            conformance.symbol,
            lowerer.type_reference_exposure,
            language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::TypeReference,
        )?;
        let mut arguments = arena::HandleSpan::empty();
        for argument in lowerer
            .source_trees
            .child_type_references(conformance.arguments)
        {
            let argument =
                crate::type_reference::lower_type_reference_into_table(lowerer, argument)?;
            lowerer
                .typed_trees
                .type_reference_table
                .push_type_reference_handle(&mut arguments, argument);
        }
        let trait_lifetime_arguments = conformance
            .lifetime_arguments
            .iter()
            .map(|argument| {
                machine
                    .lifetime_parameters
                    .iter()
                    .position(|parameter| parameter.as_str() == argument.as_str())
                    .ok_or_else(|| {
                        Diagnostic::error(format!(
                            "machine `{}` satisfies `{}` with lifetime `'{}'` outside its lifetime telescope",
                            machine.name, conformance.name, argument,
                        ))
                        .with_source_span(argument.source_span())
                    })
                    .and_then(|ordinal| {
                        u32::try_from(ordinal).map_err(|_| {
                            Diagnostic::error(format!(
                                "machine `{}` lifetime telescope exceeds the supported ordinal range",
                                machine.name,
                            ))
                            .with_source_span(argument.source_span())
                        })
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let via_expression = conformance
            .via_expression
            .is_valid()
            .then(|| lower_expression_handle(lowerer, conformance.via_expression))
            .transpose()?
            .unwrap_or_else(typed::expression::ExpressionHandle::invalid);
        lowerer.typed_trees.push_machine_trait_conformance(
            &mut typed_machine,
            typed::machine::TraitConformance {
                symbol: conformance.symbol,
                name: crate::lowerer::name::lower_name(&conformance.name),
                trait_lifetime_arguments,
                arguments,
                requirement: conformance
                    .requirement
                    .as_ref()
                    .map(crate::lowerer::name::lower_name),
                requirement_symbol: symbols::SymbolHandle::invalid(),
                requirement_source_span: conformance
                    .requirement
                    .as_ref()
                    .filter(|requirement| requirement.is_source_backed())
                    .map(|requirement| requirement.source_span()),
                alias: conformance
                    .alias
                    .as_ref()
                    .map(crate::lowerer::name::lower_name),
                external_binding: conformance.external_binding,
                via_expression,
                external_binding_source_span: conformance.external_binding_source_span,
            },
        );
    }

    for bound in &machine.conformance_bounds {
        let mut arguments = Vec::new();
        for argument in lowerer.source_trees.child_type_references(bound.arguments) {
            arguments.push(lower_type_reference_into_table(lowerer, argument)?);
        }
        let selected_conformance = bound
            .selected_conformance
            .as_ref()
            .map(|argument| {
                crate::expressions::expression::lower_static_machine_argument(
                    Some(lowerer.source_trees),
                    &mut lowerer.typed_trees,
                    lowerer.type_reference_exposure,
                    argument,
                )
            })
            .transpose()?;
        typed_machine
            .conformance_bounds
            .push(typed::machine::GenericConformanceBound {
                binder: bound.binder,
                binder_name: bound
                    .binder_name
                    .as_ref()
                    .map(crate::lowerer::name::lower_name),
                subject: bound.subject,
                subject_name: crate::lowerer::name::lower_name(&bound.subject_name),
                carrier: bound.carrier,
                carrier_name: crate::lowerer::name::lower_name(&bound.carrier_name),
                arguments,
                selected_conformance,
            });
    }

    let ranking_subjects = lowerer
        .source_trees
        .tables
        .bodies
        .expressions
        .expression_handles(machine.ranking_subjects)
        .iter()
        .map(|decrease| lower_expression_handle(lowerer, *decrease))
        .collect::<Result<Vec<_>, _>>()?;
    let ranking_view_arguments = lowerer
        .source_trees
        .tables
        .bodies
        .expressions
        .expression_handles(machine.ranking_view_arguments)
        .iter()
        .map(|argument| lower_expression_handle(lowerer, *argument))
        .collect::<Result<Vec<_>, _>>()?;
    let ranking_range = machine
        .ranking_range
        .is_valid()
        .then(|| lower_expression_handle(lowerer, machine.ranking_range))
        .transpose()?;
    if !ranking_subjects.is_empty() || !ranking_view_arguments.is_empty() || ranking_range.is_some()
    {
        lowerer.typed_trees.ranking_expression_custody.push(
            typed::ranking::RankingExpressionCustody {
                machine: machine.symbol,
                subjects: ranking_subjects,
                view_arguments: ranking_view_arguments,
                rank_range: ranking_range,
            },
        );
    }

    let invocation_parameters = lowerer
        .source_trees
        .machine_state_handles(machine.states)
        .first()
        .map(|state| {
            lowerer
                .source_trees
                .state_parameters(lowerer.source_trees.machine_state(*state).parameters)
        })
        .unwrap_or_default();
    let invocations = lower_authored_invocations(
        lowerer.source_trees,
        lowerer.source_trees.machine_invokes(machine),
        invocation_parameters,
        machine.name.as_str(),
    )?;
    for invocation in invocations {
        lowerer
            .typed_trees
            .push_machine_invoke(&mut typed_machine, invocation);
    }

    for contract in lowerer.source_trees.machine_contracts(machine) {
        let facts = lower_proof_facts(lowerer, contract.facts)?;
        lowerer.typed_trees.push_machine_contract(
            &mut typed_machine,
            typed::signature::SignatureContract {
                kind: lower_contract_kind(&contract.kind),
                keyword_source_span: contract.keyword_source_span,
                binding: contract
                    .binding
                    .as_ref()
                    .map(crate::lowerer::name::lower_name),
                facts,
                token_count: contract.token_count,
            },
        );
    }

    for (state_index, state) in lowerer
        .source_trees
        .machine_state_handles(machine.states)
        .iter()
        .enumerate()
    {
        let state = lowerer.source_trees.machine_state(*state);
        // The state body's value-typing scope lets `==` lowering find an
        // operand's data type for structural-equality expansion.
        lowerer.equality_scope = Some(crate::expressions::equatable::EqualityScope::for_state(
            lowerer.source_trees,
            machine,
            state,
        ));
        let publishes_entry_signature = machine.is_public
            || matches!(
                machine.supply_mode,
                language_semantics::MachineSupplyMode::TopLevelRequirement
                    | language_semantics::MachineSupplyMode::Boundary
                    | language_semantics::MachineSupplyMode::AdmissionClaim
            );
        let exposure = lowerer.type_reference_exposure.map(|_| if publishes_entry_signature && state_index == 0 {
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
        } else {
            language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
        });
        let state = lowerer.with_type_reference_exposure(exposure, |lowerer| {
            lower_state(lowerer, machine.attached_data.as_ref(), state)
        })?;
        lowerer.equality_scope = None;
        lowerer
            .typed_trees
            .push_machine_state(&mut typed_machine, state);
    }

    // #66 Phase 1 (returns) NOT YET: synthesizing an `ensures result in Domain`
    // for a `-> T in Domain` return type is VACUOUS today -- the contract
    // entailment proves arithmetic ensures (polynomials over `result`) but does
    // not PROVE a domain-MEMBERSHIP ensures from a machine body, so the obligation
    // is silently skipped (a fail canary returning a raw value compiled). Returns
    // need a direct return-membership check (the #40 return-range parallel, but on
    // the membership engine), not a desugar. Deferred to avoid shipping a vacuous
    // (unsound) obligation. Params (above) work because their `requires` is checked
    // at CALL sites, which is robust.

    Ok(typed_machine)
}

pub(crate) fn settle_satisfied_declarations(
    program: &mut typed::TypedTrees,
) -> Result<(), Diagnostic> {
    settle_satisfied_declarations_from(program, 0)
}

/// Settle only machines appended after a retained typed checkpoint. Existing
/// conformance rows and their authored-selection custody are immutable input
/// to the continuation and must never be recorded a second time.
pub(crate) fn settle_satisfied_declarations_from(
    program: &mut typed::TypedTrees,
    machine_frontier: usize,
) -> Result<(), Diagnostic> {
    let mut updates = Vec::new();
    for machine in program.machines().iter().skip(machine_frontier) {
        let exposure = machine_interface_exposure(machine);
        for (ordinal, conformance) in program
            .machine_trait_conformances(machine)
            .iter()
            .enumerate()
        {
            let Some(declaration) =
                typed::machine::resolve_satisfied_declaration(program, machine, conformance)
            else {
                continue;
            };
            updates.push((
                machine.satisfies,
                ordinal,
                declaration.symbol(),
                conformance.requirement_source_span,
                exposure,
            ));
        }
    }

    for (span, ordinal, requirement_symbol, source_span, exposure) in updates {
        let conformances = program.machine_trait_conformances.span_mut_or_empty(span);
        let Some(conformance) = conformances.get_mut(ordinal) else {
            return Err(Diagnostic::error(
                "failed to settle an exact machine `satisfies` declaration",
            ));
        };
        conformance.requirement_symbol = requirement_symbol;
        if let Some(source_span) = source_span {
            program
                .record_resolved_authored_declaration_selection_once(
                    source_span,
                    exposure,
                    language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::StaticPathSegment,
                    requirement_symbol,
                )
                .map_err(|error| {
                    Diagnostic::error(format!(
                        "failed to retain authored machine `satisfies` requirement selection: {error:?}"
                    ))
                    .with_source_span(source_span)
                })?;
        }
    }
    Ok(())
}

fn machine_interface_exposure(
    machine: &typed::machine::Machine,
) -> language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure {
    let exported_boundary = matches!(
        machine.supply_mode,
        language_semantics::MachineSupplyMode::TopLevelRequirement
            | language_semantics::MachineSupplyMode::Boundary
            | language_semantics::MachineSupplyMode::AdmissionClaim
    );
    if machine.is_public || exported_boundary {
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PublicInterface
    } else {
        language_semantics::declaration_selection::AuthoredDeclarationSelectionExposure::PrivateImplementation
    }
}

fn lower_contract_kind(
    kind: &resolved::signature::SignatureContractKind,
) -> typed::signature::SignatureContractKind {
    match kind {
        resolved::signature::SignatureContractKind::Requires => {
            typed::signature::SignatureContractKind::Requires
        }
        resolved::signature::SignatureContractKind::Ensures => {
            typed::signature::SignatureContractKind::Ensures
        }
        resolved::signature::SignatureContractKind::EnsuresForResultCase {
            result_data,
            result_case,
        } => typed::signature::SignatureContractKind::EnsuresForResultCase {
            result_data: *result_data,
            result_case: *result_case,
        },
        resolved::signature::SignatureContractKind::Crashes { cause } => {
            typed::signature::SignatureContractKind::Crashes {
                cause: match cause {
                    resolved::signature::CrashCause::Trap => typed::signature::CrashCause::Trap,
                    resolved::signature::CrashCause::Abort => typed::signature::CrashCause::Abort,
                },
            }
        }
    }
}
