//! Assignment addresses follow completed receivers and captured projections.
//! Loading an intermediate pointer uses the ordinary descriptor authority
//! resolver at that receiver's own cut. The final field's address does not load
//! its old pointer when the operation replaces that field. Captured projections
//! also keep an index's evaluated coordinate when a later operand changes it.

use arena::Handle;
use checked_trees::{CapturedPlace, CheckFacts, FlowOperandFact, FlowStateFact};

use super::super::aliases::ResolvedAlias;

pub(super) fn resolve(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: Handle<FlowOperandFact>,
    replaces_descriptor: bool,
) -> Result<Vec<ResolvedAlias>, &'static str> {
    address(
        program,
        facts,
        state,
        capture,
        replaces_descriptor,
        &mut Vec::new(),
    )
}

fn address(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: Handle<FlowOperandFact>,
    replaces_descriptor: bool,
    ancestors: &mut Vec<Handle<FlowOperandFact>>,
) -> Result<Vec<ResolvedAlias>, &'static str> {
    let control = &facts.flow.control;
    if !control.operands.is_valid(capture) || ancestors.contains(&capture) {
        return Err("the completed storage address is missing or cyclic");
    }
    let operand = control.operands.get(capture);
    if operand.state_symbol != state.state_symbol {
        return Err("the completed storage address belongs to another state");
    }
    if !replaces_descriptor
        && validation::reference_shell(program, operand.type_reference).is_some()
    {
        return super::super::completed_backings::resolve(program, facts, state, capture)
            .map(|backings| backings.places);
    }
    ancestors.push(capture);
    let result = project_address(program, facts, state, capture, replaces_descriptor, ancestors);
    ancestors.pop();
    result
}

fn project_address(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: Handle<FlowOperandFact>,
    replaces_descriptor: bool,
    ancestors: &mut Vec<Handle<FlowOperandFact>>,
) -> Result<Vec<ResolvedAlias>, &'static str> {
    let control = &facts.flow.control;
    let operand = control.operands.get(capture);
    let places = control
        .operand_referents
        .span(operand.referents)
        .ok_or("the completed storage address has no complete place roster")?;
    if places.is_empty() {
        return Err("the completed storage address has no exact places");
    }
    let mut resolved = Vec::new();
    if control.operands.is_valid(operand.backing) {
        let receiver = control.operands.get(operand.backing);
        if receiver.state_symbol != operand.state_symbol
            || receiver.statement_index != operand.statement_index
        {
            return Err("the completed receiver belongs to another evaluation");
        }
        let receivers = control
            .operand_referents
            .span(receiver.referents)
            .ok_or("the completed receiver has no complete place roster")?;
        let backings = address(program, facts, state, operand.backing, false, ancestors)?;
        for place in places {
            let segments = control
                .operand_referent_segments
                .span(place.segments)
                .ok_or("the completed address has an incomplete projection")?;
            let mut suffixes = Vec::new();
            for parent in receivers {
                let prefix = control
                    .operand_referent_segments
                    .span(parent.segments)
                    .ok_or("the completed receiver has an incomplete projection")?;
                if parent.root == place.root
                    && let Some(suffix) = segments.strip_prefix(prefix)
                    && !suffixes.contains(&suffix)
                {
                    suffixes.push(suffix);
                }
            }
            if suffixes.is_empty() {
                return Err("the completed address is not projected from its receiver");
            }
            for suffix in suffixes {
                for backing in &backings {
                    let mut projected = backing.place.clone();
                    projected.segments.extend_from_slice(suffix);
                    append_distinct(
                        &mut resolved,
                        ResolvedAlias {
                            place: projected,
                            lineage: backing.lineage.clone(),
                        },
                    );
                }
            }
        }
    } else {
        for place in places {
            let place = CapturedPlace {
                root: place.root,
                segments: control
                    .operand_referent_segments
                    .span(place.segments)
                    .ok_or("the completed address has an incomplete projection")?
                    .to_vec(),
                ..Default::default()
            };
            if replaces_descriptor {
                append_distinct(
                    &mut resolved,
                    ResolvedAlias {
                        place,
                        lineage: Vec::new(),
                    },
                );
            } else {
                for backing in super::resolve_captured_all(
                    program,
                    facts,
                    state,
                    operand.statement_index,
                    operand.constraints,
                    place,
                )
                .ok_or("the completed address has no retained access lineage")?
                {
                    append_distinct(&mut resolved, backing);
                }
            }
        }
    }
    if resolved.is_empty() {
        Err("the completed address has no resolved storage alternatives")
    } else {
        Ok(resolved)
    }
}

fn append_distinct(resolved: &mut Vec<ResolvedAlias>, candidate: ResolvedAlias) {
    if !resolved.iter().any(|previous| {
        previous.place == candidate.place && previous.lineage == candidate.lineage
    }) {
        resolved.push(candidate);
    }
}
