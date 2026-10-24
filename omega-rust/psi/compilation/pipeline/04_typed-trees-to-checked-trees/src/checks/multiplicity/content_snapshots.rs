//! Preserve the projected contents at every edge of a custody derivation.
//!
//! Replaying claim replacements proves ancestry, not byte stability. Each
//! consumed argument must still denote its predecessor's projection, and each
//! final live child must still denote its recorded producer. Following the
//! captured store ledger, including every alternative and intervening write,
//! composes the callee's conservation theorem without inventing scalar promises
//! about how the callee chose the individual child geometries.

use arena::Handle;
use checked_trees::facts::{
    ContentBodyClaimBinding, ContentClaimTransformation, ContentClaimTransformationSource,
};
use checked_trees::{CheckFacts, FlowOperandFact};
use facts::PlaceSegment;
use validation::CanonicalPlace;

pub(super) enum Observation<'capture> {
    Place {
        at: &'capture FlowOperandFact,
        place: CanonicalPlace,
    },
    Captured {
        value: Handle<FlowOperandFact>,
        path: Vec<PlaceSegment>,
    },
}

enum Snapshot {
    Entry(CanonicalPlace),
    Produced {
        state: symbols::SymbolHandle,
        statement: usize,
        expression: typed_trees::expression::ExpressionHandle,
        path: Vec<PlaceSegment>,
    },
}

type CaptureProjection = (Handle<FlowOperandFact>, Vec<PlaceSegment>);

/// One expected snapshot owns this scratch: a cached match must never answer
/// a question about a different producer or field. Completed matches share
/// reconvergent paths; explicit limits refuse before exhausting the host stack
/// or spending unbounded work on a source-controlled graph.
#[derive(Default)]
struct SnapshotTraversal {
    active: Vec<CaptureProjection>,
    completed: Vec<(CaptureProjection, bool)>,
    visits: usize,
}

pub(super) fn preserve_replacement(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    transformation: Handle<ContentClaimTransformation>,
    expected: Handle<ContentBodyClaimBinding>,
    consumed: Handle<ContentBodyClaimBinding>,
) -> Result<(), &'static str> {
    let content = &facts.qualifications.content;
    let transformation = content.claim_transformations.get(transformation);
    let binding = content.body_claim_bindings.get(consumed);
    let observation = match transformation.source {
        ContentClaimTransformationSource::Call { .. } => Observation::Captured {
            value: binding.observation,
            path: facts
                .flow
                .ownership
                .segments
                .span_or_empty(binding.segments)
                .to_vec(),
        },
        ContentClaimTransformationSource::Body { conservation } => {
            let conservation = content.body_conservations.get(conservation);
            let available = content
                .body_available_inputs
                .span_or_empty(conservation.available_inputs)
                .iter()
                .find(|available| {
                    content
                        .body_claim_bindings
                        .get(available.binding)
                        .claim_identity
                        == binding.claim_identity
                })
                .ok_or("body replacement has no observed parent")?;
            Observation::Place {
                at: facts
                    .flow
                    .control
                    .operands
                    .get(conservation.completed_value),
                place: CanonicalPlace {
                    root: available.root,
                    segments: facts
                        .flow
                        .ownership
                        .segments
                        .span_or_empty(available.segments)
                        .to_vec(),
                },
            }
        }
        ContentClaimTransformationSource::Unknown => {
            return Err("replacement has no completed producer");
        }
    };
    preserve_projection(program, facts, expected, observation)
}

pub(super) fn preserve_projection(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    expected: Handle<ContentBodyClaimBinding>,
    observed: Observation<'_>,
) -> Result<(), &'static str> {
    let content = &facts.qualifications.content;
    let binding = content.body_claim_bindings.get(expected);
    let projection = content
        .plans
        .iter()
        .find(|projection| {
            projection.domain == binding.domain
                && projection.semantic_domain == binding.semantic_domain
                && projection.machine == binding.projection_machine
        })
        .ok_or("missing exact owner projection")?;
    let observations = crate::checks::content::projection_observations(&projection.expression)
        .ok_or("unresolved projection dependencies")?;
    let path = facts
        .flow
        .ownership
        .segments
        .span_or_empty(binding.segments);
    let mut producers = content
        .claim_transformations
        .iter()
        .filter(|(_, transformation)| {
            transformation
                .outputs
                .handles()
                .any(|output| output == expected)
        });
    let producer = producers.next().map(|(_, producer)| producer);
    if producers.next().is_some() {
        return Err("claim snapshot has more than one producer");
    }
    for field in observations {
        let mut expected_path = path.to_vec();
        expected_path.extend_from_slice(&field);
        let snapshot = if let Some(producer) = producer {
            Snapshot::Produced {
                state: producer.state_symbol,
                statement: producer.statement_index,
                expression: producer.expression,
                path: expected_path,
            }
        } else if binding.parameter_symbol.is_valid() {
            Snapshot::Entry(CanonicalPlace {
                root: facts::PlaceRoot::Symbol(binding.parameter_symbol),
                segments: expected_path,
            })
        } else {
            return Err("claim snapshot has neither an entry nor a completed producer");
        };
        let mut traversal = SnapshotTraversal::default();
        let preserved = match &observed {
            Observation::Place { at, place } => {
                let mut selected = place.clone();
                selected.segments.extend_from_slice(&field);
                if let Snapshot::Entry(entry) = &snapshot {
                    let machine = program
                        .machine_holding_state(at.state_symbol)
                        .ok_or("missing snapshot machine")?;
                    let state = program
                        .state_by_symbol(at.state_symbol)
                        .ok_or("missing snapshot state")?;
                    validation::completed_value_preserves_entry_projection(
                        program,
                        machine,
                        state,
                        at.statement_index,
                        at.expression,
                        &selected,
                        entry,
                    )
                } else {
                    place_matches(program, facts, at, &selected, &snapshot, &mut traversal)?
                }
            }
            Observation::Captured { value, path } => {
                let mut path = path.clone();
                path.extend_from_slice(&field);
                capture_matches(program, facts, *value, &path, &snapshot, &mut traversal)?
            }
        };
        if !preserved {
            return Err("qualified claim projection changed since its recorded custody snapshot");
        }
    }
    Ok(())
}

fn place_matches(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    at: &FlowOperandFact,
    place: &CanonicalPlace,
    expected: &Snapshot,
    traversal: &mut SnapshotTraversal,
) -> Result<bool, &'static str> {
    let Some(_) = program.state_by_symbol(at.state_symbol) else {
        return Ok(false);
    };
    let Some(machine) = program.machine_holding_state(at.state_symbol) else {
        return Ok(false);
    };
    let Some(_) = facts.flow.control.state(machine.symbol, at.state_symbol) else {
        return Ok(false);
    };
    match crate::flow::stored_values::at_snapshot(
        program,
        &facts.flow.control,
        at.storage_snapshot,
        place,
        crate::flow::stored_values::StoredObservation::Contents,
    ) {
        Some(crate::flow::stored_values::StoredOperand::Value {
            capture,
            projection,
            ..
        }) => capture_matches(program, facts, capture, &projection, expected, traversal),
        Some(stored @ crate::flow::stored_values::StoredOperand::Alternatives(_)) => {
            let Some(captures) = stored.captures() else {
                return Ok(false);
            };
            for selected in captures {
                if !capture_matches(
                    program,
                    facts,
                    selected.capture,
                    &selected.projection,
                    expected,
                    traversal,
                )? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Some(crate::flow::stored_values::StoredOperand::Unavailable) => Ok(false),
        None => Ok(matches!(expected, Snapshot::Entry(entry) if entry == place)),
    }
}

fn capture_matches(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    expected: &Snapshot,
    traversal: &mut SnapshotTraversal,
) -> Result<bool, &'static str> {
    if traversal.visits >= 65_536 {
        return Err("content snapshot work budget exhausted");
    }
    traversal.visits += 1;
    if !facts.flow.control.operands.is_valid(value) {
        return Ok(false);
    }
    if let Some((_, preserved)) = traversal
        .completed
        .iter()
        .find(|((previous, projection), _)| *previous == value && projection == path)
    {
        return Ok(*preserved);
    }
    if traversal
        .active
        .iter()
        .any(|(previous, projection)| *previous == value && projection == path)
    {
        return Err("content snapshot contains a capture cycle");
    }
    if traversal.active.len() >= 128 {
        return Err("content snapshot depth budget exhausted");
    }
    let capture = facts.flow.control.operands.get(value);
    if matches!(expected, Snapshot::Produced { state, statement, expression, path: expected_path }
        if capture.state_symbol == *state && capture.statement_index == *statement
            && capture.expression == *expression && path == expected_path)
    {
        return Ok(true);
    }
    let key = (value, path.to_vec());
    traversal.active.push(key.clone());
    let preserved = (|| {
        if let Some(predecessors) =
            crate::flow::completed_values::predecessors(program, facts, value, path)
        {
            if predecessors.is_empty() {
                return Ok(false);
            }
            for (value, path) in predecessors {
                if !capture_matches(program, facts, value, &path, expected, traversal)? {
                    return Ok(false);
                }
            }
            Ok(true)
        } else if let Some(mut place) = validation::canonical_place_from_expression_in_state(
            program,
            capture.state_symbol,
            capture.statement_index,
            capture.expression,
        ) {
            place.segments.extend_from_slice(path);
            place_matches(program, facts, capture, &place, expected, traversal)
        } else {
            Ok(false)
        }
    })();
    traversal.active.pop();
    if let Ok(result) = preserved {
        traversal.completed.push((key, result));
    }
    preserved
}
