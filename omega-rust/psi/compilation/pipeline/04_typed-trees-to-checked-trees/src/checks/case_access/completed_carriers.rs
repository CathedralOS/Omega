//! Current tag evidence follows an exact completed owned subtree.
//! Historical descriptor provenance keeps its original owner and source access;
//! this query relates that owner to the receiver actually used by this read.
use arena::Handle;
use checked_trees::{CheckFacts, FlowCaseAccessFact, FlowControlFacts, FlowOperandFact};
use facts::{PlaceRoot, PlaceSegment};
use validation::CanonicalPlace;

pub(super) fn case_path_is_selected(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    access: &FlowCaseAccessFact,
    owner: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
) -> bool {
    let control = &facts.flow.control;
    let contexts = facts
        .flow
        .semantic_constraint_contexts(access.constraints)
        .collect::<Vec<_>>();
    let Some(evidence) = crate::flow::CaseEvidence::prepare(
        program,
        &facts.semantic,
        &contexts,
        access.machine_symbol,
        access.state_symbol,
        access.statement_index,
    ) else {
        return false;
    };
    let original = CanonicalPlace {
        root: PlaceRoot::Expression(control.operands.get(owner).expression),
        segments: path.to_vec(),
    };
    if evidence.cases_are_selected(&original) {
        return true;
    }
    let Some(state) = control.unique_state(access.machine_symbol, access.state_symbol) else {
        return false;
    };
    let Some(statement) = control
        .statements
        .span_or_empty(state.statements)
        .iter()
        .find(|statement| statement.statement_index == access.statement_index)
    else {
        return false;
    };
    let mut reads = control
        .value_observations
        .span_or_empty(statement.value_observations)
        .iter()
        .filter(|read| {
            read.expression == access.expression
                && read.evaluation == access.evaluation
                && read.constraints == access.constraints
        });
    let Some(read) = reads.next() else {
        return false;
    };
    if reads.next().is_some() || !control.operands.is_valid(read.receiver) {
        return false;
    }
    let receiver = control.operands.get(read.receiver);
    if receiver.state_symbol != access.state_symbol
        || receiver.statement_index != access.statement_index
        || validation::reference_shell(program, receiver.type_reference).is_some()
    {
        return false;
    }
    let Some(mut current) = validation::canonical_place_from_expression_in_state(
        program,
        receiver.state_symbol,
        receiver.statement_index,
        receiver.expression,
    ) else {
        return false;
    };
    // Find an actual owned subtree relation. Geometry, equal scalar contents,
    // names and old initializer syntax are never identity evidence.
    for prefix_length in 0..=path.len() {
        let prefix = &path[..prefix_length];
        let Some(reference) = validation::owned_projection_type_reference(
            program,
            control.operands.get(owner).type_reference,
            prefix,
        ) else {
            continue;
        };
        if validation::reference_shell(program, reference).is_some()
            || program.type_identity(typed_trees::type_identity::TypeIdentityRequest::ordinary(
                reference,
            )) != program.type_identity(
                typed_trees::type_identity::TypeIdentityRequest::ordinary(receiver.type_reference),
            )
        {
            continue;
        }
        if !reaches(
            program,
            control,
            read.receiver,
            &[],
            owner,
            prefix,
            &mut Vec::new(),
        ) {
            continue;
        }
        // A subtree relation cannot justify a case above that subtree.
        if !evidence.cases_are_selected(&CanonicalPlace {
            root: original.root,
            segments: prefix.to_vec(),
        }) {
            continue;
        }
        current.segments.extend_from_slice(&path[prefix_length..]);
        return evidence.cases_are_selected(&current);
    }
    false
}

fn reaches(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    value: Handle<FlowOperandFact>,
    projection: &[PlaceSegment],
    target: Handle<FlowOperandFact>,
    target_projection: &[PlaceSegment],
    active: &mut Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
) -> bool {
    if value == target && projection == target_projection {
        return true;
    }
    let key = (value, projection.to_vec());
    if active.contains(&key) {
        return false;
    }
    active.push(key);
    let answer = validation::completed_storage::predecessors_in_control(
        program,
        control,
        value,
        projection,
        validation::completed_storage::StoredObservation::Contents,
        None,
    )
    .is_some_and(|rows| {
        !rows.is_empty()
            && rows.iter().all(|(value, projection)| {
                reaches(
                    program,
                    control,
                    *value,
                    projection,
                    target,
                    target_projection,
                    active,
                )
            })
    });
    active.pop();
    answer
}
