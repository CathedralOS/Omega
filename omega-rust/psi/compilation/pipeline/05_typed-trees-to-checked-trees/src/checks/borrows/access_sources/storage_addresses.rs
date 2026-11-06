//! Writes retain their evaluated receiver and the field or index selected below
//! it, even when a returned view has no absolute source address. Relative paths
//! are produced with the operand, rather than reconstructed from newer storage.
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
    let result = project_address(
        program,
        facts,
        state,
        capture,
        replaces_descriptor,
        ancestors,
    );
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
    let mut resolved = Vec::new();
    if let Some(projection) = operand.address_projection {
        if !control.operands.is_valid(operand.backing) {
            return Err("the captured address projection has no completed receiver");
        }
        let receiver = control.operands.get(operand.backing);
        if receiver.state_symbol != operand.state_symbol
            || receiver.statement_index != operand.statement_index
        {
            return Err("the completed receiver belongs to another evaluation");
        }
        let projection = control
            .operand_child_paths
            .span(projection)
            .ok_or("the completed address has no captured receiver projection")?;
        let backings = address(program, facts, state, operand.backing, false, ancestors)?;
        let projectable = validation::completed_storage::projectable_reference_places(
            program,
            control,
            operand.backing,
            &[],
        );
        for backing in backings {
            let mut projected = backing.place;
            // An authority footprint can enclose another representation. Only
            // independently retained typed coordinates admit a field/index
            // suffix; otherwise the whole source enclosure may be written.
            // In particular Viewed::field cannot name Original::field merely
            // because their byte layouts agree.
            if projectable.as_ref().is_some_and(|places| {
                places.iter().any(|place| {
                    place.root == projected.root && place.segments == projected.segments
                })
            }) {
                projected.segments =
                    facts::project_captured_view_segments(&projected.segments, projection)
                        .ok_or("the captured address projection exceeds its receiver window")?;
            }
            append_distinct(
                &mut resolved,
                ResolvedAlias {
                    selected: backing.selected,
                    place: projected,
                    lineage: backing.lineage,
                },
            );
        }
    } else {
        // A completed value can depend on an earlier value while owning its
        // own storage, as a copied match subject does. Only a positive address
        // relation above redirects a write away from this operand's own home.
        let places = control
            .operand_referents
            .span(operand.referents)
            .ok_or("the completed storage address has no complete place roster")?;
        if places.is_empty() {
            return Err("the completed storage address has no exact places");
        }
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
                        selected: crate::checks::borrows::aliases::unloaned_storage_source(
                            program, facts, state, &place,
                        ),
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
        previous.place == candidate.place
            && previous.selected == candidate.selected
            && previous.lineage == candidate.lineage
    }) {
        resolved.push(candidate);
    }
}
