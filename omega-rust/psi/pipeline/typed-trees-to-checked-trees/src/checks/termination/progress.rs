use checked_trees::{
    BuildBoundProgressDemand, FlowCallFact, FlowFacts, FlowStateFact, ProgressDemandCallSite,
};
use diagnostics::Diagnostic;
use facts::{FactPayload, FactPlace, PlaceRoot, PlaceSegment};
use language_semantics::{ProgressPremise, ProgressSubject, TerminationGuarantee};
use symbols::SymbolHandle;

use crate::{call_site_argument_expressions, call_target_parameters, find_call_site};

mod components;

/// Derive checked termination premises from exact selected call contracts.
///
/// Published operation contracts remain the caller-facing authority. Private
/// checked callees contribute their derived summary through a fixed point, so
/// mentioning a qualified value does nothing while actually invoking a
/// premise-bearing operation instantiates exactly that premise.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CheckedProgressSummary {
    pub(crate) machine: SymbolHandle,
    pub(crate) guarantee: TerminationGuarantee,
    pub(crate) build_bound_demands: Vec<BuildBoundProgressDemand>,
}

pub(crate) fn analyze_checked_progress(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
) -> Result<Vec<CheckedProgressSummary>, Vec<Diagnostic>> {
    let correspondence_diagnostics = validate_qualification_correspondences(program, semantic);
    if !correspondence_diagnostics.is_empty() {
        return Err(correspondence_diagnostics);
    }
    let summaries = components::derive_summaries(program, flow, semantic);

    let mut diagnostics = Vec::new();
    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody)
    {
        // Ranking diagnostics own local control-flow failure and can name the
        // missing or invalid witness precisely. Progress coverage begins only
        // after that independent obligation succeeds.
        if !super::infer_machine_checked_summary(program, machine).promises_termination() {
            continue;
        }
        let Some(checked) = summaries
            .iter()
            .find(|summary| summary.machine == machine.symbol)
        else {
            continue;
        };
        let language_semantics::TerminationInterface::Published(published) =
            &machine.termination_plan.interface
        else {
            continue;
        };
        let TerminationGuarantee::Terminates {
            premises: published_premises,
        } = published
        else {
            continue;
        };
        let TerminationGuarantee::Terminates {
            premises: checked_premises,
        } = &checked.guarantee
        else {
            diagnostics.push(Diagnostic::error(format!(
                "cannot prove published termination for machine `{}`: its checked body reaches an operation without a usable termination guarantee",
                machine.name
            )));
            continue;
        };
        for premise in checked_premises {
            if !published_premises.contains(premise) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` derives progress premise `{}` for `{}` from a selected call, but its published termination contract does not cover that exact subject",
                    machine.name,
                    profile_label(program, premise.profile),
                    subject_label(program, &premise.subject),
                )));
            }
        }
    }

    if diagnostics.is_empty() {
        Ok(summaries)
    } else {
        Err(diagnostics)
    }
}

fn validate_qualification_correspondences(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut retained = Vec::new();
    for (_, correspondence) in semantic.qualification_correspondences.iter() {
        if retained.contains(correspondence) {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence is duplicated",
            ));
            continue;
        }
        retained.push(*correspondence);
        if !semantic.facts.is_valid(correspondence.source_fact)
            || !semantic.facts.is_valid(correspondence.destination_fact)
            || correspondence.source_fact == correspondence.destination_fact
            || correspondence.source_fact.arena_index()
                >= correspondence.destination_fact.arena_index()
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence fact identity or construction order drifted",
            ));
            continue;
        }
        let facts::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            statement_index,
        } = correspondence.formation
        else {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence formation is not an exact statement point",
            ));
            continue;
        };
        if !machine_symbol.is_valid()
            || !state_symbol.is_valid()
            || program.symbols.get(machine_symbol).kind != symbols::SymbolKind::Machine
            || program.symbols.get(state_symbol).kind != symbols::SymbolKind::State
            || program.symbols.get(state_symbol).parent != machine_symbol
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence formation owner identity drifted",
            ));
            continue;
        }
        if !exact_correspondence_place(
            program,
            semantic,
            correspondence.source_place,
            machine_symbol,
            state_symbol,
            statement_index,
        ) || !exact_correspondence_place(
            program,
            semantic,
            correspondence.source_occurrence_place,
            machine_symbol,
            state_symbol,
            statement_index,
        ) || !exact_correspondence_place(
            program,
            semantic,
            correspondence.destination_place,
            machine_symbol,
            state_symbol,
            statement_index,
        ) || correspondence.source_place == correspondence.destination_place
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence place is not an exact formation-owned structural symbol place",
            ));
            continue;
        }
        let source = semantic.facts.get(correspondence.source_fact);
        let destination = semantic.facts.get(correspondence.destination_fact);
        if source.place != FactPlace::Place(correspondence.source_place)
            || destination.place != FactPlace::Place(correspondence.destination_place)
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence place handle drifted from its fact row",
            ));
            continue;
        }
        if !semantic.places_equal(
            correspondence.source_place,
            correspondence.source_occurrence_place,
        ) {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence source occurrence drifted from its fact place",
            ));
            continue;
        }
        if destination.origin != facts::FactOrigin::StatementTransfer
            || destination.point != correspondence.formation
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence destination is not its exact statement transfer",
            ));
            continue;
        }
        if source.evidence != correspondence.evidence
            || destination.evidence != correspondence.evidence
            || correspondence.evidence.origin
                != language_semantics::QualificationEvidenceOrigin::CheckedTransformation
            || !exact_correspondence_evidence_source(program, correspondence.evidence)
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence evidence identity drifted",
            ));
            continue;
        }
        if facts::QualificationPayloadIdentity::from_fact_payload(source.payload)
            != Some(correspondence.payload)
            || facts::QualificationPayloadIdentity::from_fact_payload(destination.payload)
                != Some(correspondence.payload)
            || !exact_correspondence_payload(program, correspondence.payload)
        {
            diagnostics.push(Diagnostic::error(
                "qualification correspondence payload or domain identity drifted",
            ));
        }
    }
    diagnostics
}

fn exact_correspondence_payload(
    program: &typed_trees::TypedTrees,
    payload: facts::QualificationPayloadIdentity,
) -> bool {
    match payload {
        facts::QualificationPayloadIdentity::DomainMembership {
            domain,
            domain_symbol,
        } => {
            domain_symbol.is_valid()
                && program.symbols.get(domain_symbol).kind == symbols::SymbolKind::Domain
                && program.domain_path_members.span(domain).is_some()
        }
        facts::QualificationPayloadIdentity::CarryPermission { .. }
        | facts::QualificationPayloadIdentity::CarryOrigin => true,
    }
}

fn exact_correspondence_evidence_source(
    program: &typed_trees::TypedTrees,
    evidence: facts::QualificationEvidence,
) -> bool {
    evidence.source_symbol.is_valid()
        && evidence.requirement_symbol == SymbolHandle::invalid()
        && evidence.receipt_identity == 0
        && matches!(
            program.symbols.get(evidence.source_symbol).kind,
            symbols::SymbolKind::Machine | symbols::SymbolKind::Operator
        )
}

fn exact_correspondence_place(
    program: &typed_trees::TypedTrees,
    semantic: &facts::FactPlan,
    handle: facts::PlaceHandle,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    formation_statement_index: usize,
) -> bool {
    if !semantic.places.is_valid(handle) {
        return false;
    }
    let place = semantic.places.get(handle);
    let PlaceRoot::Symbol(root) = place.root else {
        return false;
    };
    if !root.is_valid() {
        return false;
    }
    let Some(segments) = semantic.place_segments.span(place.segments) else {
        return false;
    };
    let Some(mut current) = replay_root_type_reference(
        program,
        machine_symbol,
        state_symbol,
        formation_statement_index,
        root,
    ) else {
        return false;
    };
    let mut selected_variant = None;
    for segment in segments {
        match segment {
            PlaceSegment::Field { symbol } => {
                if !symbol.is_valid()
                    || program.symbols.get(*symbol).kind != symbols::SymbolKind::Field
                {
                    return false;
                }
                let Some(data) = replay_data_type(program, current, machine_symbol) else {
                    return false;
                };
                let field = if let Some(variant_symbol) = selected_variant.take() {
                    program.data_members(data).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Variant(variant) = member else {
                            return None;
                        };
                        (variant.symbol == variant_symbol).then(|| {
                            program
                                .data_payload_fields(variant)
                                .iter()
                                .find(|field| field.symbol == *symbol)
                        })?
                    })
                } else {
                    program.data_members(data).iter().find_map(|member| {
                        let typed_trees::data::DataMember::Field(field) = member else {
                            return None;
                        };
                        (field.symbol == *symbol).then_some(field)
                    })
                };
                let Some(field) = field else {
                    return false;
                };
                current = field.type_reference;
            }
            PlaceSegment::Case { variant } => {
                if selected_variant.is_some()
                    || !variant.is_valid()
                    || program.symbols.get(*variant).kind != symbols::SymbolKind::Variant
                {
                    return false;
                }
                let Some(data) = replay_data_type(program, current, machine_symbol) else {
                    return false;
                };
                if !program.data_members(data).iter().any(|member| {
                    matches!(member, typed_trees::data::DataMember::Variant(candidate)
                        if candidate.symbol == *variant)
                }) {
                    return false;
                }
                selected_variant = Some(*variant);
            }
            PlaceSegment::FixedIndex { index } => {
                if selected_variant.is_some() {
                    return false;
                }
                loop {
                    match program.type_reference_table.type_reference(current) {
                        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
                        | typed_trees::types::TypeReferenceNode::Constrained {
                            base_type: referee,
                            ..
                        } => current = *referee,
                        typed_trees::types::TypeReferenceNode::FixedArray {
                            element_type,
                            length: typed_trees::types::FixedArrayLength::Literal(length),
                        } if *index < *length => {
                            current = *element_type;
                            break;
                        }
                        _ => return false,
                    }
                }
            }
            PlaceSegment::FixedRange { .. } | PlaceSegment::Index { .. } => return false,
        }
    }
    true
}

fn replay_root_type_reference(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    formation_statement_index: usize,
    root: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)?;
    match program.symbols.get(root).kind {
        symbols::SymbolKind::Parameter
            if matches!(
                program.symbols.get(root).parent,
                parent if parent == machine_symbol || parent == state_symbol
            ) =>
        {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == root)
                .map(|parameter| parameter.type_reference)
        }
        symbols::SymbolKind::Local
            if program.symbols.get(root).parent == state_symbol
                && formation_statement_index
                    < program
                        .statement_table
                        .statements(state.statement_nodes)
                        .len() =>
        {
            let mut declarations = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(formation_statement_index)
                .filter_map(|statement| {
                    let typed_trees::statement::StatementNode::LocalData(local) = statement else {
                        return None;
                    };
                    (local.symbol == root).then_some(local.type_reference)
                });
            let declared_type = declarations.next()?;
            declarations.next().is_none().then_some(declared_type)
        }
        _ => None,
    }
}

fn replay_data_type(
    program: &typed_trees::TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
    machine_symbol: SymbolHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    match program.type_reference_table.type_reference(type_reference) {
        typed_trees::types::TypeReferenceNode::Reference { referee, .. }
        | typed_trees::types::TypeReferenceNode::Constrained {
            base_type: referee, ..
        } => replay_data_type(program, *referee, machine_symbol),
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if symbol.is_valid()
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::Data =>
        {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == *symbol && definition.name == *name)
        }
        typed_trees::types::TypeReferenceNode::Named { symbol, name }
            if *symbol == machine_symbol && name.as_str() == "Self" =>
        {
            let machine = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)?;
            machine.attached_data_symbol.is_valid().then_some(())?;
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.symbol == machine.attached_data_symbol)
        }
        _ => None,
    }
}

fn derive_machine_summary(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    machine: &typed_trees::machine::Machine,
    summaries: &[CheckedProgressSummary],
) -> Option<CheckedProgressSummary> {
    if !super::infer_machine_checked_summary(program, machine).promises_termination() {
        return Some(no_guarantee(machine.symbol));
    }

    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody {
        return Some(no_guarantee(machine.symbol));
    }

    let parameter_lineage = state_parameter_lineage(program, flow, machine);
    let entry_parameter_roots = program
        .machine_states(machine)
        .first()
        .map(|state| {
            program
                .state_parameters(state)
                .iter()
                .map(|parameter| parameter.symbol)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let mut premises = Vec::new();
    let mut build_bound_demands: Vec<BuildBoundProgressDemand> = Vec::new();
    for (_, state_flow) in flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == machine.symbol)
    {
        for call in flow.control.calls.span_or_empty(state_flow.calls) {
            if is_local_state_transition(program, machine, state_flow, call) {
                // Named transitions are edges within this activation, not
                // nested calls. Their argument correspondence is folded into
                // `parameter_lineage` above.
                continue;
            }
            let selected = selected_call_summary(program, call.target_symbol, summaries)?;
            for demand in selected.build_bound_demands {
                if !build_bound_demands.contains(demand) {
                    build_bound_demands.push(demand.clone());
                }
            }
            let TerminationGuarantee::Terminates {
                premises: callee_premises,
            } = selected.guarantee
            else {
                return Some(no_guarantee(machine.symbol));
            };
            for callee_premise in callee_premises {
                let build_bound_requirement = provider_receiver_requirement(
                    program,
                    call.receiver_symbol,
                    call.target_symbol,
                    callee_premise,
                );
                let local_instance =
                    instantiate_call_premise(program, machine, state_flow, call, callee_premise)?;
                if admitted_receipt_covers(
                    program,
                    flow,
                    semantic,
                    state_flow,
                    call,
                    &local_instance,
                ) {
                    continue;
                }
                if let Some((
                    provider_service_package_identity,
                    provider_service_identity,
                    requirement_owner_package_identity,
                    requirement_identity,
                )) = &build_bound_requirement
                {
                    // A requirement receiver remains build-bound even when a
                    // checked wrapper happens to hold it in one of its own
                    // parameters. Rewriting it into that caller parameter
                    // would lose the exact selected-provider subject class.
                    let demand = BuildBoundProgressDemand {
                        provider_service_identity: provider_service_identity.clone(),
                        provider_service_package_identity: *provider_service_package_identity,
                        requirement_identity: requirement_identity.clone(),
                        requirement_owner_package_identity: *requirement_owner_package_identity,
                        profile_identity: profile_label(program, callee_premise.profile),
                        subject_projections: callee_premise
                            .subject
                            .projections
                            .iter()
                            .map(|symbol| program.symbols.display_path(*symbol, "::"))
                            .collect(),
                        origin: ProgressDemandCallSite {
                            machine: machine.symbol,
                            state: state_flow.state_symbol,
                            statement_ordinal: call.statement_index,
                            call_ordinal: call.call_ordinal,
                        },
                    };
                    if !build_bound_demands.contains(&demand) {
                        build_bound_demands.push(demand);
                    }
                    continue;
                }
                let instances = resolve_through_state_lineage(&parameter_lineage, local_instance)?;
                for instance in instances {
                    if !entry_parameter_roots.contains(&instance.subject.root) {
                        // Arbitrary local values still cannot become caller
                        // premises. Only the exact provider receiver handled
                        // above may defer its coverage to composition.
                        return Some(no_guarantee(machine.symbol));
                    }
                    if !premises.contains(&instance) {
                        premises.push(instance);
                    }
                }
            }
        }
    }

    Some(CheckedProgressSummary {
        machine: machine.symbol,
        guarantee: TerminationGuarantee::Terminates { premises },
        build_bound_demands,
    })
}

fn no_guarantee(machine: SymbolHandle) -> CheckedProgressSummary {
    CheckedProgressSummary {
        machine,
        guarantee: TerminationGuarantee::NoGuarantee,
        build_bound_demands: Vec::new(),
    }
}

struct SelectedCallProgress<'a> {
    guarantee: &'a TerminationGuarantee,
    build_bound_demands: &'a [BuildBoundProgressDemand],
}

fn selected_call_summary<'a>(
    program: &'a typed_trees::TypedTrees,
    target_symbol: SymbolHandle,
    summaries: &'a [CheckedProgressSummary],
) -> Option<SelectedCallProgress<'a>> {
    if let Some((_, signature)) = program.machine_parameter_signature(target_symbol) {
        return Some(SelectedCallProgress {
            guarantee: &signature.termination_guarantee,
            build_bound_demands: &[],
        });
    }
    if let Some(signature) = program.traits().iter().find_map(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
    }) {
        return Some(SelectedCallProgress {
            guarantee: &signature.termination_guarantee,
            build_bound_demands: &[],
        });
    }
    let target_machine = program.machines().iter().find(|candidate| {
        candidate.symbol == target_symbol
            || program
                .machine_states(candidate)
                .iter()
                .any(|state| state.symbol == target_symbol)
    })?;
    match &target_machine.termination_plan.interface {
        language_semantics::TerminationInterface::Published(guarantee) => {
            let build_bound_demands = summaries
                .iter()
                .find(|summary| summary.machine == target_machine.symbol)
                .map_or(&[] as &[BuildBoundProgressDemand], |summary| {
                    summary.build_bound_demands.as_slice()
                });
            Some(SelectedCallProgress {
                guarantee,
                build_bound_demands,
            })
        }
        language_semantics::TerminationInterface::InternalDerived => {
            let summary = summaries
                .iter()
                .find(|summary| summary.machine == target_machine.symbol)?;
            Some(SelectedCallProgress {
                guarantee: &summary.guarantee,
                build_bound_demands: &summary.build_bound_demands,
            })
        }
    }
}

fn provider_receiver_requirement(
    program: &typed_trees::TypedTrees,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    premise: &ProgressPremise,
) -> Option<(
    Option<semantic_vocabulary::PackageKeyIdentity>,
    String,
    Option<semantic_vocabulary::PackageKeyIdentity>,
    String,
)> {
    program.traits().iter().find_map(|owner| {
        program
            .trait_machine_signatures(owner)
            .iter()
            .find(|requirement| requirement.symbol == target_symbol)
            .and_then(|requirement| {
                program
                    .state_signature_parameters(requirement)
                    .iter()
                    .find(|parameter| parameter.symbol == premise.subject.root)
                    .filter(|parameter| parameter.is_self)
                    .map(|_| {
                        let provider_service =
                            crate::flow::symbol_type_symbol(program, receiver_symbol)
                                .and_then(|symbol| {
                                    program.traits().iter().find(|candidate| {
                                        candidate.is_boundary && candidate.symbol == symbol
                                    })
                                })
                                .unwrap_or(owner);
                        (
                            program
                                .symbols
                                .symbol_package_identity(provider_service.symbol),
                            provider_service.name.as_str().to_owned(),
                            program.symbols.symbol_package_identity(requirement.symbol),
                            program
                                .normalized_trait_requirement_overload_identity(owner, requirement)
                                .identity(),
                        )
                    })
            })
    })
}

fn instantiate_call_premise(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    premise: &ProgressPremise,
) -> Option<ProgressPremise> {
    let mut subject =
        call_argument_subject(program, machine, state_flow, call, premise.subject.root)?;
    subject
        .projections
        .extend(premise.subject.projections.iter().copied());
    Some(ProgressPremise {
        profile: premise.profile,
        subject,
    })
}

fn call_argument_subject(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    parameter_symbol: SymbolHandle,
) -> Option<ProgressSubject> {
    let parameters = call_target_parameters(program, call.target_symbol)?;
    call_argument_subject_with_parameters(
        program,
        machine,
        state_flow,
        call,
        parameters,
        parameter_symbol,
    )
}

fn call_argument_subject_with_parameters(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state_flow: &FlowStateFact,
    call: &FlowCallFact,
    parameters: &[typed_trees::signature::StateParameter],
    parameter_symbol: SymbolHandle,
) -> Option<ProgressSubject> {
    let call_site = find_call_site(
        program,
        machine.symbol,
        state_flow.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let parameter_index = parameters
        .iter()
        .position(|parameter| parameter.symbol == parameter_symbol)?;
    let arguments = call_site_argument_expressions(program, &call_site);
    let non_self_count = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let uses_receiver =
        parameters.iter().any(|parameter| parameter.is_self) && arguments.len() == non_self_count;
    let parameter = &parameters[parameter_index];
    let place = if parameter.is_self && uses_receiver {
        crate::flow::canonical_receiver_place_for_call_site(
            program,
            machine.symbol,
            state_flow.state_symbol,
            &call_site,
        )?
    } else {
        let argument_index = if uses_receiver {
            parameters[..parameter_index]
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count()
        } else {
            parameter_index
        };
        crate::flow::canonical_place_from_expression_in_state(
            program,
            state_flow.state_symbol,
            call.statement_index,
            *arguments.get(argument_index)?,
        )?
    };
    subject_from_place(place.root, &place.segments)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ParameterLineage {
    Unseen,
    Exact(Vec<ProgressSubject>),
    Ambiguous,
}

fn state_parameter_lineage(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    machine: &typed_trees::machine::Machine,
) -> Vec<(SymbolHandle, ParameterLineage)> {
    let states = program.machine_states(machine);
    let mut lineage = states
        .iter()
        .flat_map(|state| program.state_parameters(state))
        .map(|parameter| (parameter.symbol, ParameterLineage::Unseen))
        .collect::<Vec<_>>();
    if let Some(entry) = states.first() {
        for parameter in program.state_parameters(entry) {
            set_parameter_lineage(
                &mut lineage,
                parameter.symbol,
                ParameterLineage::Exact(vec![ProgressSubject {
                    root: parameter.symbol,
                    projections: Vec::new(),
                }]),
            );
        }
    }

    loop {
        let previous = lineage.clone();
        for (_, state_flow) in flow
            .control
            .states
            .iter()
            .filter(|(_, state)| state.machine_symbol == machine.symbol)
        {
            for call in flow.control.calls.span_or_empty(state_flow.calls) {
                let Some(target) =
                    local_state_transition_target(program, machine, state_flow, call)
                else {
                    continue;
                };
                let target_parameters = program.state_parameters(target);
                for parameter in target_parameters {
                    let incoming = call_argument_subject_with_parameters(
                        program,
                        machine,
                        state_flow,
                        call,
                        target_parameters,
                        parameter.symbol,
                    )
                    .map(|subject| resolve_subject_lineage(&previous, subject))
                    .unwrap_or(ParameterLineage::Ambiguous);
                    merge_parameter_lineage(&mut lineage, parameter.symbol, incoming);
                }
            }
        }
        if lineage == previous {
            break;
        }
    }

    lineage
}

fn is_local_state_transition(
    program: &typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> bool {
    local_state_transition_target(program, machine, state, call).is_some()
}

fn local_state_transition_target<'program>(
    program: &'program typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> Option<&'program typed_trees::state::State> {
    let call_site = find_call_site(
        program,
        machine.symbol,
        state.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    if !matches!(call_site, crate::CallSite::TransitionNamed { .. }) {
        return None;
    }
    let target_index =
        super::graph::named_transition_target_state_index(program, machine, call.target_symbol)?;
    program.machine_states(machine).get(target_index)
}

fn resolve_subject_lineage(
    lineage: &[(SymbolHandle, ParameterLineage)],
    subject: ProgressSubject,
) -> ParameterLineage {
    let Some((_, root)) = lineage.iter().find(|(symbol, _)| *symbol == subject.root) else {
        return ParameterLineage::Ambiguous;
    };
    match root {
        ParameterLineage::Unseen => ParameterLineage::Unseen,
        ParameterLineage::Ambiguous => ParameterLineage::Ambiguous,
        ParameterLineage::Exact(roots) => ParameterLineage::Exact(
            roots
                .iter()
                .map(|root| {
                    let mut resolved = root.clone();
                    resolved
                        .projections
                        .extend(subject.projections.iter().copied());
                    resolved
                })
                .collect(),
        ),
    }
}

fn resolve_through_state_lineage(
    lineage: &[(SymbolHandle, ParameterLineage)],
    premise: ProgressPremise,
) -> Option<Vec<ProgressPremise>> {
    let ParameterLineage::Exact(subjects) = resolve_subject_lineage(lineage, premise.subject)
    else {
        return None;
    };
    Some(
        subjects
            .into_iter()
            .map(|subject| ProgressPremise {
                profile: premise.profile,
                subject,
            })
            .collect(),
    )
}

fn set_parameter_lineage(
    lineage: &mut [(SymbolHandle, ParameterLineage)],
    symbol: SymbolHandle,
    value: ParameterLineage,
) {
    if let Some((_, retained)) = lineage
        .iter_mut()
        .find(|(candidate, _)| *candidate == symbol)
    {
        *retained = value;
    }
}

fn merge_parameter_lineage(
    lineage: &mut [(SymbolHandle, ParameterLineage)],
    symbol: SymbolHandle,
    incoming: ParameterLineage,
) {
    let Some((_, retained)) = lineage
        .iter_mut()
        .find(|(candidate, _)| *candidate == symbol)
    else {
        return;
    };
    match (&*retained, incoming) {
        (_, ParameterLineage::Unseen) => {}
        (ParameterLineage::Unseen, value) => *retained = value,
        (ParameterLineage::Exact(_), ParameterLineage::Exact(right)) => {
            let ParameterLineage::Exact(retained) = retained else {
                unreachable!()
            };
            for subject in right {
                if !retained.contains(&subject) {
                    retained.push(subject);
                }
            }
        }
        (ParameterLineage::Exact(_), ParameterLineage::Ambiguous) => {
            *retained = ParameterLineage::Ambiguous;
        }
        (ParameterLineage::Ambiguous, _) => {}
    }
}

fn admitted_receipt_covers(
    program: &typed_trees::TypedTrees,
    flow: &FlowFacts,
    semantic: &facts::FactPlan,
    state: &FlowStateFact,
    call: &FlowCallFact,
    premise: &ProgressPremise,
) -> bool {
    let Some(domain_symbol) = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.semantic_id == premise.profile)
        .map(|domain| domain.symbol)
    else {
        return false;
    };
    flow.state_call_entry_semantic_contexts(
        state,
        call.statement_index,
        call.call_ordinal,
        call.target_symbol,
        call.receiver_symbol,
    )
    .any(|context| {
        semantic
            .context_view(semantic.contexts.get(context))
            .facts()
            .any(|fact| {
                fact.evidence.origin
                    == language_semantics::QualificationEvidenceOrigin::AdmittedReceipt
                    && fact_domain(fact.payload) == Some(domain_symbol)
                    && fact_subject(semantic, fact.place).as_ref() == Some(&premise.subject)
            })
    })
}

fn fact_domain(payload: FactPayload) -> Option<SymbolHandle> {
    match payload {
        FactPayload::DomainMembership { domain_symbol, .. }
        | FactPayload::ContractDomainMembership { domain_symbol, .. } => Some(domain_symbol),
        _ => None,
    }
}

fn fact_subject(semantic: &facts::FactPlan, place: FactPlace) -> Option<ProgressSubject> {
    match place {
        FactPlace::Place(handle) => {
            let place = *semantic.places.get(handle);
            subject_from_place(
                place.root,
                semantic.place_segments.span_or_empty(place.segments),
            )
        }
        FactPlace::Symbol(symbol) => Some(ProgressSubject {
            root: symbol,
            projections: Vec::new(),
        }),
        _ => None,
    }
}

fn subject_from_place(root: PlaceRoot, segments: &[PlaceSegment]) -> Option<ProgressSubject> {
    let PlaceRoot::Symbol(root) = root else {
        return None;
    };
    let mut projections = Vec::new();
    for segment in segments {
        match segment {
            PlaceSegment::Field { symbol } => projections.push(*symbol),
            // The field symbol already carries exact variant identity, matching
            // authored member-path normalization.
            PlaceSegment::Case { .. } => {}
            PlaceSegment::FixedIndex { .. }
            | PlaceSegment::FixedRange { .. }
            | PlaceSegment::Index { .. } => return None,
        }
    }
    Some(ProgressSubject { root, projections })
}

fn profile_label(
    program: &typed_trees::TypedTrees,
    profile: language_semantics::SemanticDomainId,
) -> String {
    program
        .semantic_domains
        .name(profile)
        .unwrap_or("<unknown-progress-profile>")
        .to_owned()
}

fn subject_label(program: &typed_trees::TypedTrees, subject: &ProgressSubject) -> String {
    let mut label = program.symbols.display_path(subject.root, "::");
    for projection in &subject.projections {
        label.push('.');
        label.push_str(&program.symbols.display_path(*projection, "::"));
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::HandleSpan;
    use facts::{
        Fact, FactOrigin, QualificationCorrespondence, QualificationEvidence,
        QualificationPayloadIdentity,
    };
    use symbols::{SymbolKind, SymbolNameRef, SymbolTableBuilder};

    fn correspondence_fixture() -> (
        typed_trees::TypedTrees,
        facts::FactPlan,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
        SymbolHandle,
    ) {
        let mut symbols = SymbolTableBuilder::new();
        let root = symbols.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        let roots = symbols.insert_children(
            root,
            [
                (SymbolKind::Machine, SymbolNameRef::Static("worker")),
                (SymbolKind::Domain, SymbolNameRef::Static("Ready")),
                (SymbolKind::Data, SymbolNameRef::Static("Pair")),
                (SymbolKind::Local, SymbolNameRef::Static("excluded_local")),
                (
                    SymbolKind::TypeParameter,
                    SymbolNameRef::Static("ExcludedType"),
                ),
                (SymbolKind::Machine, SymbolNameRef::Static("foreign_worker")),
            ],
        );
        let roots = SymbolTableBuilder::child_handles(roots).collect::<Vec<_>>();
        let machine = roots[0];
        let domain = roots[1];
        let data_symbol = roots[2];
        let excluded_local = roots[3];
        let excluded_generic = roots[4];
        let foreign_machine = roots[5];
        let fields = SymbolTableBuilder::child_handles(symbols.insert_children(
            data_symbol,
            [
                (SymbolKind::Field, SymbolNameRef::Static("source")),
                (SymbolKind::Field, SymbolNameRef::Static("destination")),
            ],
        ))
        .collect::<Vec<_>>();
        let source_field = fields[0];
        let destination_field = fields[1];
        let machine_members = symbols.insert_children(
            machine,
            [
                (SymbolKind::State, SymbolNameRef::Static("entry")),
                (SymbolKind::Parameter, SymbolNameRef::Static("self")),
                (SymbolKind::State, SymbolNameRef::Static("sibling")),
            ],
        );
        let machine_members =
            SymbolTableBuilder::child_handles(machine_members).collect::<Vec<_>>();
        let state = machine_members[0];
        let self_parameter = machine_members[1];
        let sibling_state = machine_members[2];
        let sibling_members = SymbolTableBuilder::child_handles(symbols.insert_children(
            sibling_state,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("sibling_self")),
                (SymbolKind::Local, SymbolNameRef::Static("sibling_local")),
            ],
        ))
        .collect::<Vec<_>>();
        let sibling_state_parameter = sibling_members[0];
        let sibling_state_local = sibling_members[1];
        let foreign_members = SymbolTableBuilder::child_handles(symbols.insert_children(
            foreign_machine,
            [
                (SymbolKind::Parameter, SymbolNameRef::Static("foreign_self")),
                (SymbolKind::State, SymbolNameRef::Static("foreign_entry")),
            ],
        ))
        .collect::<Vec<_>>();
        let foreign_parameter = foreign_members[0];
        let foreign_state = foreign_members[1];
        let exact_local = SymbolTableBuilder::child_handles(
            symbols.insert_children(state, [(SymbolKind::Local, SymbolNameRef::Static("local"))]),
        )
        .next()
        .expect("exact state local");
        let foreign_local = SymbolTableBuilder::child_handles(symbols.insert_children(
            foreign_state,
            [(SymbolKind::Local, SymbolNameRef::Static("foreign_local"))],
        ))
        .next()
        .expect("foreign state local");
        let mut program = typed_trees::TypedTrees {
            symbols: symbols.finish(),
            ..typed_trees::TypedTrees::default()
        };
        let unit = program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Unit);
        let pair_type =
            program
                .type_reference_table
                .insert(typed_trees::types::TypeReferenceNode::Named {
                    symbol: data_symbol,
                    name: typed_trees::name::Identifier::generated("Pair"),
                });
        let mut data = typed_trees::data::DataDefinition {
            symbol: data_symbol,
            name: typed_trees::name::Identifier::generated("Pair"),
            ..Default::default()
        };
        for (symbol, name) in [(source_field, "source"), (destination_field, "destination")] {
            program.push_data_member(
                &mut data,
                typed_trees::data::DataMember::Field(typed_trees::data::DataField {
                    symbol,
                    name: typed_trees::name::Identifier::generated(name),
                    type_reference: unit,
                    ..Default::default()
                }),
            );
        }
        program.push_data_definition(data);
        let mut state_node = typed_trees::state::State {
            symbol: state,
            name: typed_trees::name::Identifier::generated("entry"),
            ..Default::default()
        };
        program.push_state_parameter(
            &mut state_node,
            typed_trees::signature::StateParameter {
                symbol: self_parameter,
                name: typed_trees::name::Identifier::generated("self"),
                type_reference: pair_type,
                is_self: true,
                ..Default::default()
            },
        );
        program.statement_table.push_statement(
            &mut state_node.statement_nodes,
            typed_trees::statement::StatementNode::LocalData(
                typed_trees::statement::TableLocalData {
                    symbol: exact_local,
                    name: typed_trees::name::Identifier::generated("local"),
                    type_reference: pair_type,
                    initial_value: typed_trees::expression::ExpressionHandle::invalid(),
                    is_mutable: true,
                },
            ),
        );
        program.statement_table.push_statement(
            &mut state_node.statement_nodes,
            typed_trees::statement::StatementNode::Expression(
                typed_trees::expression::ExpressionHandle::invalid(),
            ),
        );
        program.statement_table.push_statement(
            &mut state_node.statement_nodes,
            typed_trees::statement::StatementNode::Expression(
                typed_trees::expression::ExpressionHandle::invalid(),
            ),
        );
        let mut machine_node = typed_trees::machine::Machine {
            symbol: machine,
            name: typed_trees::name::Identifier::generated("worker"),
            ..Default::default()
        };
        program.push_machine_state(&mut machine_node, state_node);
        program.push_machine(machine_node);

        let mut semantic = facts::FactPlan::default();
        let source_place = semantic.append_symbol_place(self_parameter);
        semantic.push_place_segment(
            source_place,
            PlaceSegment::Field {
                symbol: source_field,
            },
        );
        let destination_place = semantic.append_symbol_place(self_parameter);
        semantic.push_place_segment(
            destination_place,
            PlaceSegment::Field {
                symbol: destination_field,
            },
        );
        let formation = facts::ProgramPoint::Statement {
            machine_symbol: machine,
            state_symbol: state,
            statement_index: 1,
        };
        let payload = FactPayload::DomainMembership {
            value: typed_trees::expression::ExpressionHandle::invalid(),
            domain: HandleSpan::empty(),
            domain_symbol: domain,
        };
        let evidence = QualificationEvidence::from_origin(
            language_semantics::QualificationEvidenceOrigin::CheckedTransformation,
            machine,
        );
        let source_fact = semantic.append_fact(Fact {
            place: FactPlace::Place(source_place),
            point: facts::ProgramPoint::CallEnsures {
                machine_symbol: machine,
                state_symbol: state,
                statement_index: 0,
                call_ordinal: 0,
            },
            origin: FactOrigin::CallEnsures,
            evidence,
            payload,
        });
        let destination_fact = semantic.append_fact(Fact {
            place: FactPlace::Place(destination_place),
            point: formation,
            origin: FactOrigin::StatementTransfer,
            evidence,
            payload,
        });
        semantic.append_qualification_correspondence(QualificationCorrespondence {
            source_fact,
            destination_fact,
            source_occurrence_place: source_place,
            source_place,
            destination_place,
            formation,
            payload: QualificationPayloadIdentity::DomainMembership {
                domain: HandleSpan::empty(),
                domain_symbol: domain,
            },
            evidence,
        });
        (
            program,
            semantic,
            excluded_local,
            excluded_generic,
            foreign_parameter,
            sibling_state_parameter,
            exact_local,
            foreign_local,
            sibling_state_local,
        )
    }

    fn set_replay_parameter_type(
        program: &mut typed_trees::TypedTrees,
        semantic: &facts::FactPlan,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) {
        let correspondence = semantic
            .qualification_correspondences
            .iter()
            .next()
            .map(|(_, correspondence)| correspondence)
            .expect("correspondence");
        let facts::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            ..
        } = correspondence.formation
        else {
            unreachable!("correspondence fixture formation")
        };
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
            .expect("formation machine");
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
            .expect("formation state");
        let parameter = state.parameters.start();
        program.state_parameters.get_mut(parameter).type_reference = type_reference;
    }

    fn install_replay_paths(
        semantic: &mut facts::FactPlan,
        source: &[PlaceSegment],
        destination: &[PlaceSegment],
    ) {
        let row = semantic
            .qualification_correspondences
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .expect("correspondence");
        let retained = *semantic.qualification_correspondences.get(row);
        let root = semantic.places.get(retained.source_place).root;
        let source_place = semantic.append_place(facts::Place {
            root,
            segments: HandleSpan::empty(),
        });
        for segment in source {
            semantic.push_place_segment(source_place, *segment);
        }
        let source_occurrence_place = semantic.append_place(facts::Place {
            root,
            segments: HandleSpan::empty(),
        });
        for segment in source {
            semantic.push_place_segment(source_occurrence_place, *segment);
        }
        let destination_place = semantic.append_place(facts::Place {
            root,
            segments: HandleSpan::empty(),
        });
        for segment in destination {
            semantic.push_place_segment(destination_place, *segment);
        }
        semantic.facts.get_mut(retained.source_fact).place = FactPlace::Place(source_place);
        semantic.facts.get_mut(retained.destination_fact).place =
            FactPlace::Place(destination_place);
        let retained = semantic.qualification_correspondences.get_mut(row);
        retained.source_place = source_place;
        retained.source_occurrence_place = source_occurrence_place;
        retained.destination_place = destination_place;
    }

    fn set_replay_roots(
        semantic: &mut facts::FactPlan,
        source_root: SymbolHandle,
        destination_root: SymbolHandle,
    ) {
        let row = semantic
            .qualification_correspondences
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .expect("correspondence");
        let retained = *semantic.qualification_correspondences.get(row);
        semantic.places.get_mut(retained.source_place).root = PlaceRoot::Symbol(source_root);
        semantic
            .places
            .get_mut(retained.source_occurrence_place)
            .root = PlaceRoot::Symbol(source_root);
        semantic.places.get_mut(retained.destination_place).root =
            PlaceRoot::Symbol(destination_root);
    }

    fn replay_state_statement_span(
        program: &typed_trees::TypedTrees,
        semantic: &facts::FactPlan,
    ) -> HandleSpan<typed_trees::statement::StatementNode> {
        let formation = semantic
            .qualification_correspondences
            .iter()
            .next()
            .map(|(_, row)| row.formation)
            .expect("correspondence");
        let facts::ProgramPoint::Statement {
            machine_symbol,
            state_symbol,
            ..
        } = formation
        else {
            unreachable!("statement formation")
        };
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_symbol)
            .expect("formation machine");
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
            .map(|state| state.statement_nodes)
            .expect("formation state")
    }

    fn nested_fixed_array_correspondence_fixture() -> (typed_trees::TypedTrees, facts::FactPlan) {
        let (mut program, mut semantic, _, _, _, _, _, _, _) = correspondence_fixture();
        let unit = program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Unit);
        let inner = program.type_reference_table.insert(
            typed_trees::types::TypeReferenceNode::FixedArray {
                element_type: unit,
                length: typed_trees::types::FixedArrayLength::Literal(2),
            },
        );
        let outer = program.type_reference_table.insert(
            typed_trees::types::TypeReferenceNode::FixedArray {
                element_type: inner,
                length: typed_trees::types::FixedArrayLength::Literal(2),
            },
        );
        set_replay_parameter_type(&mut program, &semantic, outer);
        install_replay_paths(
            &mut semantic,
            &[
                PlaceSegment::FixedIndex { index: 0 },
                PlaceSegment::FixedIndex { index: 1 },
            ],
            &[
                PlaceSegment::FixedIndex { index: 1 },
                PlaceSegment::FixedIndex { index: 0 },
            ],
        );
        (program, semantic)
    }

    #[test]
    fn checked_progress_replays_exact_qualification_correspondence() {
        let (program, semantic, _, _, _, _, _, _, _) = correspondence_fixture();
        assert!(validate_qualification_correspondences(&program, &semantic).is_empty());
    }

    #[test]
    fn checked_progress_replays_exact_prior_state_local_as_either_endpoint() {
        let (program, mut local_source, _, _, _, _, exact_local, _, _) = correspondence_fixture();
        let parameter = match local_source
            .places
            .get(
                local_source
                    .qualification_correspondences
                    .iter()
                    .next()
                    .map(|(_, row)| row.destination_place)
                    .expect("destination place"),
            )
            .root
        {
            PlaceRoot::Symbol(root) => root,
            _ => unreachable!("symbol root"),
        };
        set_replay_roots(&mut local_source, exact_local, parameter);
        assert!(validate_qualification_correspondences(&program, &local_source).is_empty());

        let mut local_destination = local_source.clone();
        set_replay_roots(&mut local_destination, parameter, exact_local);
        assert!(validate_qualification_correspondences(&program, &local_destination).is_empty());
    }

    #[test]
    fn checked_progress_rejects_local_declaration_type_missing_duplicate_and_order_drift() {
        let (program, mut semantic, excluded_local, _, _, _, exact_local, _, _) =
            correspondence_fixture();
        let parameter = match semantic
            .places
            .get(
                semantic
                    .qualification_correspondences
                    .iter()
                    .next()
                    .map(|(_, row)| row.destination_place)
                    .expect("destination place"),
            )
            .root
        {
            PlaceRoot::Symbol(root) => root,
            _ => unreachable!("symbol root"),
        };
        set_replay_roots(&mut semantic, exact_local, parameter);

        let mut wrong_type = program.clone();
        let unit = wrong_type
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Unit);
        let statements = replay_state_statement_span(&wrong_type, &semantic);
        let typed_trees::statement::StatementNode::LocalData(local) =
            &mut wrong_type.statement_table.statements_mut(statements)[0]
        else {
            unreachable!("local declaration")
        };
        local.type_reference = unit;
        assert!(
            validate_qualification_correspondences(&wrong_type, &semantic)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );

        let mut missing = program.clone();
        let statements = replay_state_statement_span(&missing, &semantic);
        let typed_trees::statement::StatementNode::LocalData(local) =
            &mut missing.statement_table.statements_mut(statements)[0]
        else {
            unreachable!("local declaration")
        };
        local.symbol = excluded_local;
        assert!(
            validate_qualification_correspondences(&missing, &semantic)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("formation-owned"))
        );

        let mut duplicate = program.clone();
        let statements = replay_state_statement_span(&duplicate, &semantic);
        let duplicate_declaration = duplicate.statement_table.statements(statements)[0].clone();
        duplicate.statement_table.statements_mut(statements)[1] = duplicate_declaration;
        let mut duplicate_semantic = semantic.clone();
        let row = duplicate_semantic
            .qualification_correspondences
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .expect("correspondence");
        let formation = match duplicate_semantic
            .qualification_correspondences
            .get(row)
            .formation
        {
            facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                ..
            } => facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index: 2,
            },
            _ => unreachable!("statement formation"),
        };
        duplicate_semantic
            .qualification_correspondences
            .get_mut(row)
            .formation = formation;
        let destination_fact = duplicate_semantic
            .qualification_correspondences
            .get(row)
            .destination_fact;
        duplicate_semantic.facts.get_mut(destination_fact).point = formation;
        assert!(
            validate_qualification_correspondences(&duplicate, &duplicate_semantic)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("formation-owned"))
        );

        let mut reordered = semantic;
        let row = reordered
            .qualification_correspondences
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .expect("correspondence");
        let formation = match reordered.qualification_correspondences.get(row).formation {
            facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                ..
            } => facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index: 0,
            },
            _ => unreachable!("statement formation"),
        };
        reordered
            .qualification_correspondences
            .get_mut(row)
            .formation = formation;
        let destination_fact = reordered
            .qualification_correspondences
            .get(row)
            .destination_fact;
        reordered.facts.get_mut(destination_fact).point = formation;
        assert!(
            validate_qualification_correspondences(&program, &reordered)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("formation-owned"))
        );
    }

    #[test]
    fn checked_progress_replays_nested_in_bounds_literal_fixed_indexes() {
        let (program, semantic) = nested_fixed_array_correspondence_fixture();
        assert!(validate_qualification_correspondences(&program, &semantic).is_empty());
    }

    #[test]
    fn checked_progress_rejects_fixed_index_bounds_runtime_type_and_length_tamper() {
        let (program, semantic) = nested_fixed_array_correspondence_fixture();

        let mut out_of_bounds = semantic.clone();
        install_replay_paths(
            &mut out_of_bounds,
            &[PlaceSegment::FixedIndex { index: 2 }],
            &[PlaceSegment::FixedIndex { index: 0 }],
        );
        assert!(
            validate_qualification_correspondences(&program, &out_of_bounds)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );

        let mut runtime = semantic.clone();
        install_replay_paths(
            &mut runtime,
            &[PlaceSegment::Index {
                expression: typed_trees::expression::ExpressionHandle::invalid(),
            }],
            &[PlaceSegment::FixedIndex { index: 0 }],
        );
        assert!(
            validate_qualification_correspondences(&program, &runtime)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );

        let mut range = semantic.clone();
        install_replay_paths(
            &mut range,
            &[PlaceSegment::FixedRange { start: 0, end: 1 }],
            &[PlaceSegment::FixedIndex { index: 0 }],
        );
        assert!(
            validate_qualification_correspondences(&program, &range)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );

        let mut wrong_type_program = program.clone();
        let unit = wrong_type_program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Unit);
        set_replay_parameter_type(&mut wrong_type_program, &semantic, unit);
        assert!(
            validate_qualification_correspondences(&wrong_type_program, &semantic)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );

        let mut nonliteral_program = program;
        let unit = nonliteral_program
            .type_reference_table
            .insert(typed_trees::types::TypeReferenceNode::Unit);
        let formation = semantic
            .qualification_correspondences
            .iter()
            .next()
            .map(|(_, row)| row.formation)
            .expect("correspondence");
        let facts::ProgramPoint::Statement { machine_symbol, .. } = formation else {
            unreachable!("statement formation")
        };
        let nonliteral = nonliteral_program.type_reference_table.insert(
            typed_trees::types::TypeReferenceNode::FixedArray {
                element_type: unit,
                length: typed_trees::types::FixedArrayLength::ConstParameter {
                    symbol: machine_symbol,
                    name: typed_trees::name::Identifier::generated("N"),
                },
            },
        );
        set_replay_parameter_type(&mut nonliteral_program, &semantic, nonliteral);
        assert!(
            validate_qualification_correspondences(&nonliteral_program, &semantic)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );
    }

    #[test]
    fn checked_progress_rejects_generic_and_label_only_data_traversal() {
        let (mut generic_program, semantic, _, _, _, _, _, _, _) = correspondence_fixture();
        let row = semantic
            .qualification_correspondences
            .iter()
            .next()
            .map(|(_, row)| row)
            .expect("correspondence");
        let (machine_symbol, state_symbol) = match row.formation {
            facts::ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                ..
            } => (machine_symbol, state_symbol),
            _ => unreachable!("statement formation"),
        };
        let parameter_root = match semantic.places.get(row.source_place).root {
            PlaceRoot::Symbol(root) => root,
            _ => unreachable!("symbol root"),
        };
        let data_symbol = match generic_program.type_reference_table.type_reference(
            replay_root_type_reference(
                &generic_program,
                machine_symbol,
                state_symbol,
                1,
                parameter_root,
            )
            .expect("root type"),
        ) {
            typed_trees::types::TypeReferenceNode::Named { symbol, .. } => *symbol,
            _ => unreachable!("named fixture type"),
        };
        let arguments = generic_program
            .type_reference_table
            .insert_type_reference_handles([]);
        let generic = generic_program.type_reference_table.insert(
            typed_trees::types::TypeReferenceNode::Generic {
                base_symbol: data_symbol,
                base_name: typed_trees::name::Identifier::generated("Pair"),
                lifetime_arguments: Vec::new(),
                arguments,
            },
        );
        set_replay_parameter_type(&mut generic_program, &semantic, generic);
        assert!(
            validate_qualification_correspondences(&generic_program, &semantic)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );

        let mut label_only_program = generic_program;
        let wrong_symbol = match row.formation {
            facts::ProgramPoint::Statement { machine_symbol, .. } => machine_symbol,
            _ => unreachable!("statement formation"),
        };
        let wrong = label_only_program.type_reference_table.insert(
            typed_trees::types::TypeReferenceNode::Named {
                symbol: wrong_symbol,
                name: typed_trees::name::Identifier::generated("Pair"),
            },
        );
        set_replay_parameter_type(&mut label_only_program, &semantic, wrong);
        assert!(
            validate_qualification_correspondences(&label_only_program, &semantic)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );
    }

    #[test]
    fn checked_progress_rejects_correspondence_payload_place_and_order_drift() {
        let (program, semantic, _, _, _, _, _, _, _) = correspondence_fixture();

        let mut payload = semantic.clone();
        let row = payload
            .qualification_correspondences
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .expect("correspondence");
        payload.qualification_correspondences.get_mut(row).payload =
            QualificationPayloadIdentity::CarryOrigin;
        assert!(
            validate_qualification_correspondences(&program, &payload)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("payload or domain identity"))
        );

        let mut occurrence = semantic.clone();
        let row = occurrence
            .qualification_correspondences
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .expect("correspondence");
        let destination_place = occurrence
            .qualification_correspondences
            .get(row)
            .destination_place;
        occurrence
            .qualification_correspondences
            .get_mut(row)
            .source_occurrence_place = destination_place;
        assert!(
            validate_qualification_correspondences(&program, &occurrence)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("source occurrence drifted"))
        );

        let mut indexed = semantic.clone();
        let source_place = indexed
            .qualification_correspondences
            .iter()
            .next()
            .map(|(_, row)| row.source_place)
            .expect("source place");
        let root = indexed.places.get(source_place).root;
        let indexed_place = indexed.append_place(facts::Place {
            root,
            segments: HandleSpan::empty(),
        });
        indexed.push_place_segment(indexed_place, PlaceSegment::FixedIndex { index: 0 });
        let row = indexed
            .qualification_correspondences
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .expect("correspondence");
        indexed
            .qualification_correspondences
            .get_mut(row)
            .source_place = indexed_place;
        let source_fact = indexed.qualification_correspondences.get(row).source_fact;
        indexed.facts.get_mut(source_fact).place = FactPlace::Place(indexed_place);
        assert!(
            validate_qualification_correspondences(&program, &indexed)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
        );

        let mut reversed = semantic;
        let row = reversed
            .qualification_correspondences
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .expect("correspondence");
        let retained = reversed.qualification_correspondences.get_mut(row);
        std::mem::swap(&mut retained.source_fact, &mut retained.destination_fact);
        assert!(
            validate_qualification_correspondences(&program, &reversed)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("construction order drifted"))
        );
    }

    #[test]
    fn checked_progress_rejects_excluded_roots_and_malformed_formation() {
        let (
            program,
            semantic,
            excluded_local,
            excluded_generic,
            foreign_parameter,
            sibling_state_parameter,
            _,
            foreign_local,
            sibling_state_local,
        ) = correspondence_fixture();
        let row = semantic
            .qualification_correspondences
            .iter()
            .next()
            .map(|(handle, _)| handle)
            .expect("correspondence");
        let source_place = semantic.qualification_correspondences.get(row).source_place;

        for excluded_root in [
            PlaceRoot::Unknown,
            PlaceRoot::Expression(typed_trees::expression::ExpressionHandle::invalid()),
            PlaceRoot::TypeReference(typed_trees::types::TypeReferenceHandle::invalid()),
        ] {
            let mut drifted = semantic.clone();
            drifted.places.get_mut(source_place).root = excluded_root;
            assert!(
                validate_qualification_correspondences(&program, &drifted)
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
            );
        }

        for excluded_symbol in [excluded_local, excluded_generic] {
            let mut drifted = semantic.clone();
            drifted.places.get_mut(source_place).root = PlaceRoot::Symbol(excluded_symbol);
            assert!(
                validate_qualification_correspondences(&program, &drifted)
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("structural symbol place"))
            );
        }

        for excluded_symbol in [foreign_parameter, sibling_state_parameter] {
            for destination in [false, true] {
                let mut drifted = semantic.clone();
                let place = if destination {
                    drifted
                        .qualification_correspondences
                        .get(row)
                        .destination_place
                } else {
                    source_place
                };
                drifted.places.get_mut(place).root = PlaceRoot::Symbol(excluded_symbol);
                assert!(
                    validate_qualification_correspondences(&program, &drifted)
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("formation-owned"))
                );
            }

            let mut occurrence = semantic.clone();
            let occurrence_place = occurrence.append_symbol_place(excluded_symbol);
            occurrence
                .qualification_correspondences
                .get_mut(row)
                .source_occurrence_place = occurrence_place;
            assert!(
                validate_qualification_correspondences(&program, &occurrence)
                    .iter()
                    .any(|diagnostic| diagnostic.message.contains("formation-owned"))
            );
        }

        for excluded_symbol in [foreign_local, sibling_state_local] {
            for destination in [false, true] {
                let mut drifted = semantic.clone();
                let place = if destination {
                    drifted
                        .qualification_correspondences
                        .get(row)
                        .destination_place
                } else {
                    source_place
                };
                drifted.places.get_mut(place).root = PlaceRoot::Symbol(excluded_symbol);
                assert!(
                    validate_qualification_correspondences(&program, &drifted)
                        .iter()
                        .any(|diagnostic| diagnostic.message.contains("formation-owned"))
                );
            }
        }

        let mut formation = semantic;
        formation
            .qualification_correspondences
            .get_mut(row)
            .formation = facts::ProgramPoint::Statement {
            machine_symbol: SymbolHandle::invalid(),
            state_symbol: SymbolHandle::invalid(),
            statement_index: 1,
        };
        assert!(
            validate_qualification_correspondences(&program, &formation)
                .iter()
                .any(|diagnostic| diagnostic.message.contains("owner identity drifted"))
        );
    }
}
