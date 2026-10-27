//! Ordinary runtime proof inputs bind declaration-owned typed subjects to exact
//! evaluated call snapshots. Public proposition labels identify applications;
//! they never prove equality of mutable values. Payload history owns copies,
//! stores and predecessor intersection; this reader grants no access or custody.
use crate::descriptor_lifetimes::{DescriptorLocation, DescriptorPosition};
use crate::descriptor_results::{
    self, DescriptorAnalysis, DescriptorCasePredicate, DescriptorMachine, PayloadMembershipHistory,
    PayloadScalarObservation,
};
mod inputs;
pub use inputs::{
    RuntimeEvidenceBody, RuntimeEvidenceInputs, RuntimeEvidenceOperation, RuntimeEvidenceResult,
};

use semantic_vocabulary::{EvidenceTermId, MachineId, PropositionId, ValueId};
use terminal_psi::{
    DirectCallParts, EvidenceArgumentSubject, EvidenceContractLane, EvidenceContractLaneKind,
    ProofOutputCall, StructuralAccess, StructuralMultiplicity, TerminalModule,
};

#[derive(Clone)]
enum ArgumentValue {
    Closed(String),
    Scalar(PayloadScalarObservation),
}

/// Check the complete ordinary Machine input lane at every actual direct call.
/// Applied requirement dispatch keeps its distinct requirement-owned verifier;
/// it cannot substitute a private implementation's nominal lane here.
pub fn validate_runtime_evidence_inputs(module: &TerminalModule) -> Result<(), &'static str> {
    terminal_inputs::validate(module)
}
mod terminal_inputs;

pub fn validate_runtime_evidence_inputs_in(
    module: &RuntimeEvidenceInputs<'_>,
) -> Result<(), &'static str> {
    validate_subject_rosters(module)?;
    if !module
        .evidence_contract_lanes
        .iter()
        .any(|lane| lane.kind == EvidenceContractLaneKind::Requires)
        && !module
            .proof_output_calls
            .iter()
            .any(|call| call.runtime_call.is_some() && !call.evidence_arguments.is_empty())
    {
        return Ok(());
    }
    let graph = module.graph;
    let mut retained = Vec::new();
    for caller in module.machines {
        if module.machine(caller.id).is_none() {
            return Err("evidence input body identity is ambiguous");
        }
        let mut history = None;
        for operation in caller.operations() {
            let parts = operation.parts();
            let expected = input_lanes(module, parts.callee);
            let mut invocations = module.proof_output_calls.iter().filter(|invocation| {
                invocation.caller == caller.id
                    && invocation.runtime_call.is_some_and(|runtime| {
                        runtime.site == operation.site && runtime.callee == parts.callee
                    })
            });
            let Some(invocation) = invocations.next() else {
                if expected.is_empty() {
                    continue;
                }
                return Err("runtime call omitted its declared evidence inputs");
            };
            if invocation.static_requirement_dispatch.is_some() {
                // This operation invokes an independently authenticated public
                // requirement. Its private realization is not the lane owner.
                continue;
            }
            if invocations.next().is_some() || invocation.evidence_arguments.len() != expected.len()
            {
                return Err("runtime call has ambiguous or incomplete ordinary evidence inputs");
            }
            retained.push((caller.id, operation.site));
            let use_position = position(caller, operation)?;
            let callee = module
                .machine(parts.callee)
                .ok_or("evidence input callee is absent")?;
            let declaration = callee
                .declaration
                .as_ref()
                .ok_or("evidence target has no checked callable declaration")?;
            let callable = module
                .identities
                .text(declaration.callable)
                .ok_or("evidence target callable identity is absent")?;
            let expected_identity =
                terminal_psi::CheckedMachineDeclaration::callable_evidence_identity(
                    callable,
                    declaration
                        .specialization
                        .as_ref()
                        .map(|application| &application.commitment),
                );
            if module.identities.text(invocation.target_machine_identity)
                != Some(expected_identity.as_str())
            {
                return Err("ordinary evidence invocation changed its exact checked callable");
            }
            match (
                parts.role,
                invocation.runtime_result,
                operation.result.scalar(),
            ) {
                (
                    terminal_psi::DirectCallRole::NoNormal,
                    Some(terminal_psi::ProofOutputRuntimeResult::NoNormal),
                    None,
                ) if matches!(operation.result, RuntimeEvidenceResult::NoNormal) => {}
                (
                    terminal_psi::DirectCallRole::Unit,
                    Some(terminal_psi::ProofOutputRuntimeResult::Unit),
                    None,
                ) => {}
                (
                    terminal_psi::DirectCallRole::Scalar,
                    Some(terminal_psi::ProofOutputRuntimeResult::Scalar(expected)),
                    Some((_, actual)),
                ) if actual == expected => {}
                _ => return Err("runtime evidence invocation changed its exact result role"),
            }
            if parts.role == terminal_psi::DirectCallRole::NoNormal {
                if !invocation.outputs.is_empty() {
                    return Err("nonreturning evidence invocation publishes normal outputs");
                }
            } else {
                validate_output_roster(module, invocation, parts.callee)?;
            }
            for (input_position, (argument, lane)) in invocation
                .evidence_arguments
                .iter()
                .zip(expected)
                .enumerate()
            {
                let target_term = term(module, lane.term)?;
                if lane
                    .argument_subjects
                    .iter()
                    .any(|subject| matches!(subject, EvidenceArgumentSubject::Result { .. }))
                {
                    return Err("an evidence Requires lane cannot depend on its future result");
                }
                if lane.position != input_position as u32
                    || argument.input_position != lane.position
                    || argument.callee_proposition != target_term.proposition
                {
                    return Err("runtime evidence argument changed its exact declared input lane");
                }
                if history.is_none() {
                    history = Some(PayloadMembershipHistory::for_current_contracts(
                        module.structural_types,
                        module.structural_domains,
                        graph,
                        caller.id,
                        &caller.structural_parameters,
                        &caller.payload_contracts,
                    )?);
                }
                let history = history
                    .as_mut()
                    .ok_or("runtime evidence payload history is absent")?;
                let target_application = application(module, argument.instantiated_proposition)?;
                let declared = application(module, target_term.proposition)?;
                same_head(declared, target_application)?;
                let target = observe_arguments(
                    module,
                    caller,
                    callee,
                    Some((operation, parts)),
                    use_position,
                    &lane.argument_subjects,
                    &target_application.arguments,
                    &graph,
                    history,
                )?;
                let (source_proposition, source, source_cut) =
                    source_arguments(module, caller, invocation, argument.source, &graph, history)?;
                let source_term = term(module, argument.source)?;
                if source_term.interface != target_term.interface {
                    return Err("runtime evidence argument changed its witness interface");
                }
                same_head(application(module, source_proposition)?, target_application)?;
                if source.len() != target.len() {
                    return Err("runtime evidence argument changed its complete proposition arity");
                }
                let mut captured_values = Vec::new();
                for (source, target) in source.iter().zip(&target) {
                    match (source, target) {
                        (ArgumentValue::Closed(source), ArgumentValue::Closed(target))
                            if source == target => {}
                        (ArgumentValue::Scalar(source), ArgumentValue::Scalar(target)) => {
                            let entry_equal = source
                                .entry
                                .as_ref()
                                .zip(target.entry.as_ref())
                                .is_some_and(|(source, target)| source == target);
                            let value_equal = source
                                .value
                                .as_ref()
                                .zip(target.value.as_ref())
                                .is_some_and(|(source, target)| source == target);
                            if !entry_equal && !value_equal {
                                return Err(
                                    "runtime evidence argument names a different captured value",
                                );
                            }
                            if !entry_equal {
                                if let Some(value) = &source.value {
                                    value.visit_value_ids(|id| {
                                        captured_values.push(id);
                                        true
                                    });
                                }
                            }
                        }
                        _ => {
                            return Err(
                                "runtime evidence argument has no complete typed snapshot correspondence",
                            );
                        }
                    }
                }
                if let Some(source_cut) = source_cut {
                    require_completed_capture(
                        &graph,
                        caller.id,
                        source_cut,
                        use_position,
                        &captured_values,
                    )?;
                }
            }
        }
    }
    if module.proof_output_calls.iter().any(|call| {
        call.static_requirement_dispatch.is_none()
            && call
                .runtime_call
                .is_some_and(|runtime| !retained.contains(&(call.caller, runtime.site)))
    }) {
        return Err("ordinary runtime evidence invocation has no authenticated call operation");
    }
    Ok(())
}

/// The complete declaration roster is checked even if a caller discards an
/// output. Unknown subjects remain explicit but cannot justify a runtime use.
fn validate_subject_rosters(module: &RuntimeEvidenceInputs<'_>) -> Result<(), &'static str> {
    for lane in module.evidence_contract_lanes {
        let owner = module
            .machine(lane.machine)
            .ok_or("evidence subject owner is absent")?;
        let term = term(module, lane.term)?;
        let application = application(module, term.proposition)?;
        if application.evidence_interface.as_ref() != Some(&term.interface)
            || lane.argument_subjects.len() != application.arguments.len()
            || lane
                .argument_subjects
                .iter()
                .enumerate()
                .any(|(position, subject)| subject.argument_position() as usize != position)
        {
            return Err("evidence declaration has no complete exact typed subject roster");
        }
        for subject in &lane.argument_subjects {
            match subject {
                EvidenceArgumentSubject::Parameter(parameter) => {
                    let exists = if parameter.structural_path.is_some() {
                        owner
                            .structural_parameters
                            .get(parameter.parameter_position as usize)
                            .is_some()
                    } else {
                        owner
                            .parameters
                            .get(parameter.parameter_position as usize)
                            .is_some()
                    };
                    if !exists {
                        return Err("evidence declaration subject names an absent formal");
                    }
                }
                EvidenceArgumentSubject::Result { .. }
                    if lane.kind == EvidenceContractLaneKind::Requires =>
                {
                    return Err("an evidence Requires lane cannot depend on its future result");
                }
                _ => {}
            }
        }
    }
    Ok(())
}

fn validate_output_roster(
    module: &RuntimeEvidenceInputs<'_>,
    invocation: &ProofOutputCall,
    callee: MachineId,
) -> Result<(), &'static str> {
    let mut outputs = module
        .evidence_contract_lanes
        .iter()
        .filter(|lane| lane.machine == callee && lane.kind == EvidenceContractLaneKind::Ensures)
        .collect::<Vec<_>>();
    outputs.sort_by_key(|lane| lane.position);
    if invocation.outputs.len() != outputs.len() {
        return Err("ordinary evidence invocation omitted a declared output");
    }
    for (position, (output, lane)) in invocation.outputs.iter().zip(outputs).enumerate() {
        let declared = term(module, lane.term)?;
        if lane.position as usize != position
            || output.output_position != lane.position
            || lane.output_field.as_deref() != Some(output.output_field.as_str())
            || output.callee_proposition != declared.proposition
        {
            return Err("ordinary evidence output changed its exact declaration lane");
        }
        same_head(
            application(module, declared.proposition)?,
            application(module, output.instantiated_proposition)?,
        )?;
        if let Some(input_position) = output.forwarded_input_position {
            let inputs = input_lanes(module, callee);
            let input = inputs
                .get(input_position as usize)
                .ok_or("forwarded output has no declared input")?;
            let source = term(module, input.term)?;
            if output.callee_output.is_some()
                || source.proposition != declared.proposition
                || source.interface != declared.interface
                || input.argument_subjects != lane.argument_subjects
            {
                return Err("forwarded output changed its exact declared input meaning");
            }
        } else if output.callee_output != Some(lane.term) {
            return Err("ordinary evidence output has no exact declared producer");
        }
        if let Some(output_term) = output.output {
            let target = term(module, output_term)?;
            if target.proposition != output.instantiated_proposition
                || target.interface != declared.interface
            {
                return Err("ordinary evidence output changed its receiving witness declaration");
            }
        }
    }
    Ok(())
}

fn input_lanes<'a>(
    module: &'a RuntimeEvidenceInputs<'_>,
    machine: MachineId,
) -> Vec<&'a EvidenceContractLane> {
    let mut lanes = module
        .evidence_contract_lanes
        .iter()
        .filter(|lane| lane.machine == machine && lane.kind == EvidenceContractLaneKind::Requires)
        .collect::<Vec<_>>();
    lanes.sort_by_key(|lane| lane.position);
    lanes
}

fn term<'a>(
    module: &'a RuntimeEvidenceInputs<'_>,
    id: EvidenceTermId,
) -> Result<&'a terminal_psi::EvidenceTermDeclaration, &'static str> {
    let mut rows = module.evidence_terms.iter().filter(|term| term.id == id);
    let term = rows.next().ok_or("runtime evidence term is absent")?;
    if rows.next().is_some() {
        return Err("runtime evidence term identity is ambiguous");
    }
    Ok(term)
}
fn application<'a>(
    module: &'a RuntimeEvidenceInputs<'_>,
    id: PropositionId,
) -> Result<&'a terminal_psi::PropositionApplicationIdentity, &'static str> {
    let mut rows = module
        .proposition_applications
        .iter()
        .filter(|row| row.id == id);
    let row = rows
        .next()
        .ok_or("runtime evidence proposition is absent")?;
    if rows.next().is_some() {
        return Err("runtime evidence proposition identity is ambiguous");
    }
    Ok(row)
}
fn same_head(
    source: &terminal_psi::PropositionApplicationIdentity,
    target: &terminal_psi::PropositionApplicationIdentity,
) -> Result<(), &'static str> {
    if source.declaration != target.declaration
        || source.binder_arguments != target.binder_arguments
        || source.arguments.len() != target.arguments.len()
        || source.evidence_interface != target.evidence_interface
    {
        return Err("runtime evidence transport changed its nominal application head");
    }
    Ok(())
}
fn position(
    machine: &RuntimeEvidenceBody,
    operation: &RuntimeEvidenceOperation,
) -> Result<DescriptorPosition, &'static str> {
    let mut positions = machine
        .calls
        .iter()
        .filter(|candidate| candidate.site == operation.site);
    let current = positions
        .next()
        .ok_or("evidence operation is absent from its caller")?;
    if positions.next().is_some() {
        return Err("evidence operation identity is ambiguous");
    }
    Ok(current.position)
}

#[allow(clippy::too_many_arguments)]
fn observe_arguments(
    module: &RuntimeEvidenceInputs<'_>,
    caller: &RuntimeEvidenceBody,
    callee: &RuntimeEvidenceBody,
    actual: Option<(&RuntimeEvidenceOperation, DirectCallParts<'_>)>,
    cut: DescriptorPosition,
    subjects: &[EvidenceArgumentSubject],
    public_arguments: &[String],
    graph: &[DescriptorMachine],
    history: &mut PayloadMembershipHistory<'_>,
) -> Result<Vec<ArgumentValue>, &'static str> {
    if subjects.len() != public_arguments.len() {
        return Err("evidence declaration omitted a proposition subject");
    }
    subjects.iter().enumerate().map(|(index, subject)| {
        if subject.argument_position() as usize != index { return Err("evidence argument subjects are not complete and positional"); }
        match subject {
            EvidenceArgumentSubject::Closed { .. } => Ok(ArgumentValue::Closed(public_arguments[index].clone())),
            EvidenceArgumentSubject::Unknown { .. } => Err("evidence argument has no retained typed subject"),
            EvidenceArgumentSubject::Result { .. } => {
                let (operation, _) = actual.ok_or("an invocation input cannot refer to its result")?;
                let value = operation.result.scalar().ok_or("evidence result subject is not scalar")?;
                scalar_argument(module, caller, value.0, cut, graph, history)
            }
            EvidenceArgumentSubject::Parameter(parameter) => {
                if let Some(subject_path) = &parameter.structural_path {
                    let formal = callee.structural_parameters.get(parameter.parameter_position as usize)
                        .ok_or("evidence subject has no structural formal")?;
                    if formal.access != StructuralAccess::Owned || formal.multiplicity != StructuralMultiplicity::Unrestricted {
                        return Err("evidence snapshot requires its independent borrowed or affine scope owner");
                    }
                    let (root, prefix) = if let Some((_, parts)) = actual {
                        let source = parts.structural_arguments.get(parameter.parameter_position as usize)
                            .ok_or("evidence subject has no evaluated structural actual")?;
                        if source.access != StructuralAccess::Owned { return Err("evidence actual is not an owned value snapshot"); }
                        (source.place, source.path.as_slice())
                    } else { (formal.place, &[][..]) };
                    let root_type = caller.payload_contract(root)
                        .ok_or("evidence snapshot actual has no current carrier")?.carrier;
                    let mut path = prefix.to_vec(); path.extend_from_slice(subject_path);
                    let selected = crate::descriptor_lifetimes::descriptor_location(
                        &module.structural_types, root, root_type, &path, None,
                    ).ok_or("evidence snapshot has no exact typed storage path")?;
                    Ok(ArgumentValue::Scalar(history.scalar_snapshot(cut, root, &selected.path)?))
                } else {
                    let formal = callee.parameters.get(parameter.parameter_position as usize)
                        .ok_or("evidence subject has no scalar formal")?;
                    let value = actual.map_or(Some(*formal), |(_, parts)| parts.arguments.get(parameter.parameter_position as usize).copied())
                        .ok_or("evidence subject has no evaluated scalar actual")?;
                    scalar_argument(module, caller, value, cut, graph, history)
                }
            }
        }
    }).collect()
}
fn scalar_argument(
    _module: &RuntimeEvidenceInputs<'_>,
    caller: &RuntimeEvidenceBody,
    value: ValueId,
    cut: DescriptorPosition,
    graph: &[DescriptorMachine],
    history: &mut PayloadMembershipHistory<'_>,
) -> Result<ArgumentValue, &'static str> {
    let source_value = value;
    let value = caller
        .scalar_expression(value)
        .ok_or("evidence scalar actual has no exact executable value")?
        .clone();
    let machine = descriptor_results::descriptor_machine(graph, caller.id)
        .ok_or("evidence caller has no current payload graph")?;
    let mut reads = machine
        .scalar_payload_reads
        .iter()
        .flat_map(|(position, reads)| {
            reads.iter().filter_map(|(location, observed)| {
                (observed == &value).then_some((*position, location))
            })
        });
    if let Some((position, location)) = reads.next() {
        if reads.next().is_some() {
            return Err("evidence scalar observation is ambiguous");
        }
        require_completed_capture(graph, caller.id, position, cut, &[source_value])?;
        return Ok(ArgumentValue::Scalar(history.scalar_snapshot(
            position,
            location.root,
            &location.path,
        )?));
    }
    Ok(ArgumentValue::Scalar(PayloadScalarObservation {
        path: Vec::new(),
        value: Some(value),
        entry: None,
    }))
}

#[allow(clippy::too_many_arguments)]
fn source_arguments(
    module: &RuntimeEvidenceInputs<'_>,
    caller: &RuntimeEvidenceBody,
    consumer: &ProofOutputCall,
    source: EvidenceTermId,
    graph: &[DescriptorMachine],
    history: &mut PayloadMembershipHistory<'_>,
) -> Result<
    (
        PropositionId,
        Vec<ArgumentValue>,
        Option<DescriptorPosition>,
    ),
    &'static str,
> {
    let source_term = term(module, source)?;
    let source_application = application(module, source_term.proposition)?;
    let mut entries = input_lanes(module, caller.id)
        .into_iter()
        .filter(|lane| lane.term == source);
    if let Some(entry) = entries.next() {
        if entries.next().is_some() {
            return Err("runtime evidence source has ambiguous entry scope");
        }
        let arguments = observe_arguments(
            module,
            caller,
            caller,
            None,
            (caller.entry, 0),
            &entry.argument_subjects,
            &source_application.arguments,
            graph,
            history,
        )?;
        return Ok((source_term.proposition, arguments, None));
    }
    let mut guarded = caller.operations().filter_map(|operation| {
        let parts = operation.parts();
        let mut rows = parts
            .selected_evidence
            .iter()
            .filter(|row| row.output == source);
        let row = rows.next()?;
        Some((operation, parts, row, rows.next().is_some()))
    });
    if let Some((operation, parts, row, duplicate)) = guarded.next() {
        if duplicate
            || guarded.next().is_some()
            || row.instantiated_proposition != source_term.proposition
            || row.validity.evidence_interface != source_term.interface
        {
            return Err("runtime evidence source has ambiguous guarded provenance");
        }
        let callee = module
            .machine(parts.callee)
            .ok_or("guarded evidence callee is absent")?;
        let mut declarations = callee.outcome_specific_ensures.iter().filter(|ensure| {
            ensure.guard == row.guard
                && ensure.position == row.position
                && ensure.obligation == row.callee_obligation
                && ensure.proposition
                    == semantic_vocabulary::Proposition::Atom(row.callee_proposition)
                && ensure.evidence.as_ref().is_some_and(|evidence| {
                    evidence.term == row.callee_term
                        && evidence.output_field == row.output_field
                        && evidence.argument_subjects == row.argument_substitutions
                })
        });
        if declarations.next().is_none() || declarations.next().is_some() {
            return Err("guarded evidence source changed its exact callee declaration");
        }
        let result = operation
            .result
            .structural()
            .ok_or("guarded evidence source has no structural result")?;
        if row.validity.result != result.place
            || row.guard.result_type != result.structural_type
            || row
                .validity
                .proposition_dependencies
                .iter()
                .chain(&row.validity.interface_dependencies)
                .any(|dependency| *dependency != result.place)
        {
            return Err("guarded evidence source lost its exact result validity cut");
        }
        let runtime = consumer
            .runtime_call
            .ok_or("ordinary evidence consumer has no runtime operation")?;
        let use_operation = caller
            .invocation(runtime.site)
            .ok_or("ordinary evidence consumer operation is absent")?;
        let use_position = position(caller, use_operation)?;
        let source_position = position(caller, operation)?;
        crate::descriptor_results::require_unchanged_produced_payload(
            &module.structural_types,
            graph,
            caller.id,
            source_position,
            use_position,
            &DescriptorLocation::whole(result.place),
        )?;
        let mut analysis = DescriptorAnalysis::new(&module.structural_types, graph);
        if !analysis.case_is_established(
            caller.id,
            use_position,
            &DescriptorCasePredicate {
                subject: DescriptorLocation::whole(result.place),
                case: row.guard.result_case,
            },
        ) {
            return Err("guarded evidence input has no current selected result case");
        }
        let total_uses = row
            .uses
            .len()
            .checked_add(
                module
                    .proof_output_calls
                    .iter()
                    .filter(|invocation| {
                        invocation.caller == caller.id && invocation.runtime_call.is_some()
                    })
                    .map(|invocation| {
                        invocation
                            .evidence_arguments
                            .iter()
                            .filter(|argument| argument.source == source)
                            .count()
                    })
                    .sum::<usize>(),
            )
            .ok_or("guarded evidence use cardinality overflow")?;
        if u32::try_from(total_uses).ok() != Some(row.expected_use_count) {
            return Err("guarded evidence input omitted a selected-term use");
        }
        let declared = application(module, row.callee_proposition)?;
        same_head(declared, source_application)?;
        let mut subjects = Vec::with_capacity(declared.arguments.len());
        for index in 0..declared.arguments.len() {
            let argument_position =
                u32::try_from(index).map_err(|_| "guarded evidence argument exceeds u32")?;
            let mut parameters = row
                .argument_substitutions
                .iter()
                .filter(|subject| subject.argument_position == argument_position);
            if let Some(parameter) = parameters.next() {
                if parameters.next().is_some()
                    || row
                        .result_substitution
                        .is_some_and(|result| result.argument_position == argument_position)
                {
                    return Err("guarded evidence source has duplicate typed subjects");
                }
                subjects.push(EvidenceArgumentSubject::Parameter(parameter.clone()));
            } else if row
                .result_substitution
                .is_some_and(|result| result.argument_position == argument_position)
            {
                subjects.push(EvidenceArgumentSubject::Result { argument_position });
            } else {
                if declared.arguments[index] != source_application.arguments[index] {
                    return Err("guarded evidence changed an unrepresented closed subject");
                }
                subjects.push(EvidenceArgumentSubject::Closed { argument_position });
            }
        }
        let arguments = observe_arguments(
            module,
            caller,
            callee,
            Some((operation, parts)),
            source_position,
            &subjects,
            &source_application.arguments,
            graph,
            history,
        )?;
        return Ok((source_term.proposition, arguments, Some(source_position)));
    }
    let mut producers = module.proof_output_calls.iter().filter_map(|invocation| {
        if invocation.caller != caller.id {
            return None;
        }
        let mut outputs = invocation
            .outputs
            .iter()
            .filter(|output| output.output == Some(source));
        let output = outputs.next()?;
        Some((invocation, output, outputs.next().is_some()))
    });
    let (producer, output, duplicate) = producers
        .next()
        .ok_or("runtime evidence source has no available producer")?;
    if duplicate
        || producers.next().is_some()
        || producer.static_requirement_dispatch.is_some()
        || output.instantiated_proposition != source_term.proposition
        || producer.ordinal >= consumer.ordinal
    {
        return Err("runtime evidence source has ambiguous or unavailable output scope");
    }
    let runtime = producer
        .runtime_call
        .ok_or("erased output has no retained runtime snapshot for this input")?;
    let operation = caller
        .invocation(runtime.site)
        .ok_or("evidence output producer operation is absent")?;
    let parts = operation.parts();
    if parts.callee != runtime.callee {
        return Err("evidence output runtime target changed");
    }
    let callee = module
        .machine(parts.callee)
        .ok_or("evidence output callee is absent")?;
    let mut lanes = module.evidence_contract_lanes.iter().filter(|lane| {
        lane.machine == parts.callee
            && lane.kind == EvidenceContractLaneKind::Ensures
            && lane.position == output.output_position
            && lane.output_field.as_deref() == Some(output.output_field.as_str())
    });
    let lane = lanes
        .next()
        .ok_or("evidence output changed its exact callee lane")?;
    if lanes.next().is_some() || term(module, lane.term)?.proposition != output.callee_proposition {
        return Err("evidence output declaration is ambiguous or substituted");
    }
    if let Some(input_position) = output.forwarded_input_position {
        let inputs = input_lanes(module, parts.callee);
        let input = inputs
            .get(input_position as usize)
            .ok_or("forwarded evidence output has no declared input")?;
        if output.callee_output.is_some()
            || input.argument_subjects != lane.argument_subjects
            || term(module, input.term)?.proposition != term(module, lane.term)?.proposition
            || term(module, input.term)?.interface != term(module, lane.term)?.interface
        {
            return Err("forwarded evidence output changed its exact declared input meaning");
        }
    } else if output.callee_output != Some(lane.term) {
        return Err("evidence output has no exact callee producer term");
    }
    same_head(
        application(module, output.callee_proposition)?,
        source_application,
    )?;
    let source_position = position(caller, operation)?;
    let arguments = observe_arguments(
        module,
        caller,
        callee,
        Some((operation, parts)),
        source_position,
        &lane.argument_subjects,
        &source_application.arguments,
        graph,
        history,
    )?;
    Ok((source_term.proposition, arguments, Some(source_position)))
}

fn require_completed_capture(
    graph: &[DescriptorMachine],
    machine: MachineId,
    source: DescriptorPosition,
    use_position: DescriptorPosition,
    values: &[ValueId],
) -> Result<(), &'static str> {
    descriptor_results::require_completed_scalar_capture(
        graph,
        machine,
        source,
        use_position,
        values,
    )
}
