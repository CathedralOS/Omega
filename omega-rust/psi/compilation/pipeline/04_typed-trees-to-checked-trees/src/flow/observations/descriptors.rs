//! A selected descriptor has one value observation and several possible
//! backing places. A tag on that value is not a tag on every backing. Rejoin
//! the actual carrier capture, its binding version and all live contents
//! before lending that observation namespace to another completed read.

use arena::HandleSpan;
use checked_trees::{FlowControlFacts, FlowOperandFact};
use facts::FactContextHandle;
use validation::CanonicalPlace;

/// The producer's exact descriptor-load namespace, independently of its
/// pointee addresses. A type, source expression or address equality alone is
/// insufficient: the actual source access and complete retention must exist.
pub(super) fn descriptor_namespace(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
) -> Option<CanonicalPlace> {
    let shell = validation::reference_shell(program, operand.type_reference)?;
    if !shell.access.is_readable()
        || !control.case_accesses.is_valid(operand.case_access)
        || control
            .operand_referents
            .span(operand.referents)?
            .is_empty()
        || !same_retention(program, control, operand, operand)
    {
        return None;
    }
    let access = control.case_accesses.get(operand.case_access);
    if access.expression != operand.expression
        || access.state_symbol != operand.state_symbol
        || access.statement_index != operand.statement_index
        || access.constraints != operand.constraints
    {
        return None;
    }
    let [source] = control.operand_referents.span(access.places)? else {
        return None;
    };
    // A saved historical namespace has its own formation obligation. The
    // current descriptor-load relation cannot reinterpret that old namespace.
    if source.formation.is_valid() {
        return None;
    }
    Some(CanonicalPlace {
        root: source.root,
        segments: control
            .operand_referent_segments
            .span(source.segments)?
            .to_vec(),
    })
}

/// Each actual descriptor load adds a formation endpoint for its own source
/// access. Compare the retained descriptor beneath only those authenticated
/// current-read wrappers; saved-owner and earlier formation identities remain.
fn descriptor_source<'facts>(
    control: &'facts FlowControlFacts,
    operand: &FlowOperandFact,
    mut referent: &'facts checked_trees::FlowOperandReferent,
) -> Option<&'facts checked_trees::FlowOperandReferent> {
    let mut visited = Vec::new();
    while referent.formation.is_valid() {
        if !control.referent_formations.is_valid(referent.formation)
            || visited.contains(&referent.formation)
        {
            return None;
        }
        visited.push(referent.formation);
        let formation = control.referent_formations.get(referent.formation);
        if formation.owner.is_valid()
            || !formation.path.is_empty()
            || !control.case_accesses.is_valid(formation.source_access)
        {
            break;
        }
        let access = control.case_accesses.get(formation.source_access);
        if access.expression != operand.expression
            || access.state_symbol != operand.state_symbol
            || access.statement_index != operand.statement_index
            || access.constraints != operand.constraints
        {
            break;
        }
        let (Some(actual_path), Some(formed_path)) = (
            control.operand_referent_segments.span(referent.segments),
            control.operand_referent_segments.span(formation.segments),
        ) else {
            return None;
        };
        if formation.root != referent.root
            || formed_path != actual_path
            || !control.operand_referents.is_valid(formation.source)
            || !access
                .places
                .handles()
                .any(|source| source == formation.source)
        {
            return None;
        }
        let source = control.operand_referents.get(formation.source);
        if source.root != referent.root
            || control.operand_referent_segments.span(source.segments)? != actual_path
        {
            return None;
        }
        referent = source;
    }
    Some(referent)
}

fn same_referents(
    control: &FlowControlFacts,
    first: &FlowOperandFact,
    second: &FlowOperandFact,
) -> bool {
    let (Some(first_referents), Some(second_referents)) = (
        control.operand_referents.span(first.referents),
        control.operand_referents.span(second.referents),
    ) else {
        return false;
    };
    let same = |first_source: &checked_trees::FlowOperandReferent,
                second_source: &checked_trees::FlowOperandReferent| {
        let (Some(first), Some(second)) = (
            descriptor_source(control, first, first_source),
            descriptor_source(control, second, second_source),
        ) else {
            return false;
        };
        let (Some(first_path), Some(second_path)) = (
            control.operand_referent_segments.span(first.segments),
            control.operand_referent_segments.span(second.segments),
        ) else {
            return false;
        };
        first.formation == second.formation
            && first.root == second.root
            && first_path == second_path
    };
    !first_referents.is_empty()
        && first_referents
            .iter()
            .all(|first| second_referents.iter().any(|second| same(first, second)))
        && second_referents
            .iter()
            .all(|second| first_referents.iter().any(|first| same(first, second)))
}

fn same_retention(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    first: &FlowOperandFact,
    second: &FlowOperandFact,
) -> bool {
    type Retention = arena::Handle<checked_trees::FlowOperandRetention>;

    // A projection is a deferred selection from an actual completed capture.
    // Resolve every alternative through the common owner before reading its
    // local roster. An unavailable selection cannot prove an empty lifetime.
    fn selected_profiles(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        retention: Retention,
    ) -> Option<Vec<Retention>> {
        if !control.operand_retentions.is_valid(retention) {
            return None;
        }
        let profile = control.operand_retentions.get(retention);
        let path = control
            .operand_retained_paths
            .span(profile.stored_projection)?;
        if path.is_empty() {
            return Some(vec![retention]);
        }
        if !control.operands.is_valid(profile.stored_value) {
            return None;
        }
        super::super::operand_values::captured_retention_profiles_at_path(
            program,
            control,
            control.operands.get(profile.stored_value),
            path,
        )
    }

    fn alternatives_agree(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        first: &[Retention],
        second: &[Retention],
        visited: &mut Vec<(Retention, Retention)>,
    ) -> bool {
        !first.is_empty()
            && !second.is_empty()
            && first.iter().all(|first| {
                second
                    .iter()
                    .any(|second| profiles_agree(program, control, *first, *second, visited))
            })
            && second.iter().all(|second| {
                first
                    .iter()
                    .any(|first| profiles_agree(program, control, *first, *second, visited))
            })
    }

    fn profiles_agree(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        first: Retention,
        second: Retention,
        visited: &mut Vec<(Retention, Retention)>,
    ) -> bool {
        if !control.operand_retentions.is_valid(first)
            || !control.operand_retentions.is_valid(second)
            || visited.contains(&(first, second))
        {
            return false;
        }
        visited.push((first, second));
        let agreed = (|| {
            let first_profile = control.operand_retentions.get(first);
            let second_profile = control.operand_retentions.get(second);
            if !first_profile.stored_projection.is_empty()
                || !second_profile.stored_projection.is_empty()
            {
                let (Some(first), Some(second)) = (
                    selected_profiles(program, control, first),
                    selected_profiles(program, control, second),
                ) else {
                    return false;
                };
                return alternatives_agree(program, control, &first, &second, visited);
            }
            let (Some(first_roots), Some(second_roots)) = (
                control.retained_roots(first),
                control.retained_roots(second),
            ) else {
                return false;
            };
            if (first_profile.stored_value.is_valid()
                && !control.operands.is_valid(first_profile.stored_value))
                || (second_profile.stored_value.is_valid()
                    && !control.operands.is_valid(second_profile.stored_value))
                || first_profile.stored_value != second_profile.stored_value
                || !first_roots.iter().all(|root| second_roots.contains(root))
                || !second_roots.iter().all(|root| first_roots.contains(root))
            {
                return false;
            }
            let (Some(first_leaves), Some(second_leaves)) = (
                control.operand_retained_leaves.span(first_profile.leaves),
                control.operand_retained_leaves.span(second_profile.leaves),
            ) else {
                return false;
            };
            if first_leaves.len() != second_leaves.len()
                || !first_leaves
                    .iter()
                    .zip(second_leaves)
                    .all(|(first, second)| {
                        let (Some(first_path), Some(second_path)) = (
                            control.operand_retained_paths.span(first.path),
                            control.operand_retained_paths.span(second.path),
                        ) else {
                            return false;
                        };
                        first_path == second_path
                            && profiles_agree(
                                program,
                                control,
                                first.retention,
                                second.retention,
                                visited,
                            )
                    })
            {
                return false;
            }
            // The union roster does not replace each alternative's debts.
            // Keep and authenticate every possible completed profile.
            let first_alternatives: Vec<_> = first_profile.alternatives.handles().collect();
            let second_alternatives: Vec<_> = second_profile.alternatives.handles().collect();
            if first_alternatives.is_empty() && second_alternatives.is_empty() {
                true
            } else {
                alternatives_agree(
                    program,
                    control,
                    &first_alternatives,
                    &second_alternatives,
                    visited,
                )
            }
        })();
        visited.pop();
        agreed
    }
    let (Some(first), Some(second)) = (
        super::super::operand_values::captured_retention_profiles_at_path(
            program,
            control,
            first,
            &[],
        ),
        super::super::operand_values::captured_retention_profiles_at_path(
            program,
            control,
            second,
            &[],
        ),
    ) else {
        return false;
    };
    alternatives_agree(program, control, &first, &second, &mut Vec::new())
}

/// Contextual value observations preserve a same-expression descriptor through
/// their actual backing edge. A different expression or detached value cannot
/// be replaced by whichever reference happens to occupy the source slot now.
pub(super) fn descriptor_capture<'facts>(
    program: &dyn validation::CheckingInput,
    control: &'facts FlowControlFacts,
    mut operand: &'facts FlowOperandFact,
) -> Option<(&'facts FlowOperandFact, CanonicalPlace)> {
    let mut visited = Vec::new();
    loop {
        if let Some(namespace) = descriptor_namespace(program, control, operand) {
            return Some((operand, namespace));
        }
        let backing = operand.backing;
        if !control.operands.is_valid(backing) || visited.contains(&backing) {
            return None;
        }
        visited.push(backing);
        let source = control.operands.get(backing);
        if source.expression != operand.expression
            || source.state_symbol != operand.state_symbol
            || source.statement_index != operand.statement_index
        {
            return None;
        }
        operand = source;
    }
}

#[derive(Debug)]
enum DescriptorObservationRefusal {
    MissingObservation,
    MissingReceiver,
    UnavailableObservedDescriptor,
    UnavailableConsumerDescriptor,
    DifferentNamespace,
    DifferentDescriptorSources,
    DifferentRetention,
    UnavailableType,
    DifferentType,
    UnavailableStatementCut,
    ChangedBinding,
    UnavailableDescriptorSources,
    ChangedContents,
}

fn observation_agrees(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    observation: arena::Handle<checked_trees::FlowValueObservationFact>,
    operand: &FlowOperandFact,
) -> Result<CanonicalPlace, DescriptorObservationRefusal> {
    if !control.value_observations.is_valid(observation) {
        return Err(DescriptorObservationRefusal::MissingObservation);
    }
    let observation = control.value_observations.get(observation);
    if !control.operands.is_valid(observation.receiver) {
        return Err(DescriptorObservationRefusal::MissingReceiver);
    }
    let (observed, namespace) =
        descriptor_capture(program, control, control.operands.get(observation.receiver))
            .ok_or(DescriptorObservationRefusal::UnavailableObservedDescriptor)?;
    let (consumer, actual_namespace) = descriptor_capture(program, control, operand)
        .ok_or(DescriptorObservationRefusal::UnavailableConsumerDescriptor)?;
    if namespace != actual_namespace {
        return Err(DescriptorObservationRefusal::DifferentNamespace);
    }
    if !same_referents(control, observed, consumer) {
        return Err(DescriptorObservationRefusal::DifferentDescriptorSources);
    }
    if !same_retention(program, control, observed, consumer) {
        return Err(DescriptorObservationRefusal::DifferentRetention);
    }
    // Full applied types retain declaration, dependent and region operands.
    // Readability is checked independently by descriptor_capture above.
    let machine = program
        .machine_holding_state(observed.state_symbol)
        .ok_or(DescriptorObservationRefusal::UnavailableType)?
        .symbol;
    if typed_trees::type_identity::applied_type_identity(
        program.typed_program(),
        observed.type_reference,
        machine,
        &[],
    )
    .ok_or(DescriptorObservationRefusal::UnavailableType)?
        != typed_trees::type_identity::applied_type_identity(
            program.typed_program(),
            consumer.type_reference,
            machine,
            &[],
        )
        .ok_or(DescriptorObservationRefusal::UnavailableType)?
    {
        return Err(DescriptorObservationRefusal::DifferentType);
    }
    let statements = if observed.statement_index == consumer.statement_index {
        HandleSpan::empty()
    } else {
        control
            .state_by_symbol(observed.state_symbol)
            .ok_or(DescriptorObservationRefusal::UnavailableStatementCut)?
            .statements
    };
    if !super::super::operand_contexts::operand_bindings_agree_in_state(
        program, control, statements, observed, consumer, &namespace,
    ) {
        return Err(DescriptorObservationRefusal::ChangedBinding);
    }
    // The tag read completes after its receiver. Its own content clock, not
    // merely the pointer load's clock, owns the observed case. Any write to
    // any possible backing (including unknown/atomic effects) expires it.
    let mut completed_read = *observed;
    completed_read.entry_origin_writes = observation.origin_writes;
    completed_read.entry_origin_unknown_write = observation.origin_unknown_write;
    let mut contents = vec![namespace.clone()];
    for referent in control
        .operand_referents
        .span(observed.referents)
        .ok_or(DescriptorObservationRefusal::UnavailableDescriptorSources)?
    {
        contents.push(CanonicalPlace {
            root: referent.root,
            segments: control
                .operand_referent_segments
                .span(referent.segments)
                .ok_or(DescriptorObservationRefusal::UnavailableDescriptorSources)?
                .to_vec(),
        });
    }
    for place in contents {
        if !super::super::operand_contexts::operand_sources_agree_in_state(
            program,
            control,
            statements,
            &completed_read,
            consumer,
            &place,
            super::super::value_origins::ValueObservation::Contents,
        ) {
            return Err(DescriptorObservationRefusal::ChangedContents);
        }
    }
    Ok(namespace)
}

fn rejoin_observation(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    context: FactContextHandle,
    observation: arena::Handle<checked_trees::FlowValueObservationFact>,
    operand: &FlowOperandFact,
) -> Result<CanonicalPlace, DescriptorObservationRefusal> {
    let result = observation_agrees(program, control, observation, operand);
    if let Err(reason) = &result
        && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
    {
        eprintln!(
            "[checked descriptor observation] context={context:?} observation={observation:?} consumer={:?} state={:?} statement={} refusal={reason:?}",
            operand.expression, operand.state_symbol, operand.statement_index
        );
    }
    result
}

pub(crate) fn descriptor_context_agrees(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    context: FactContextHandle,
) -> bool {
    control
        .descriptor_context_observations
        .for_context(context)
        .all(|row| rejoin_observation(program, control, context, row.observation, operand).is_ok())
}

/// A proof namespace for this actual selected value. This grants no address,
/// extent, loan or lifetime authority. Physical view consumers still use the
/// captured referent roster; predicates and payload-case checks share this one
/// rejoin instead of separately guessing aliases from source syntax.
pub(crate) fn descriptor_observation_namespace(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    contexts: &[FactContextHandle],
) -> Option<CanonicalPlace> {
    let mut selected = None;
    for row in contexts.iter().flat_map(|context| {
        control
            .descriptor_context_observations
            .for_context(*context)
    }) {
        let Ok(namespace) =
            rejoin_observation(program, control, row.context, row.observation, operand)
        else {
            continue;
        };
        if selected
            .as_ref()
            .is_some_and(|previous| previous != &namespace)
        {
            return None;
        }
        selected = Some(namespace);
    }
    selected
}
