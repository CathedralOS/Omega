//! Payload access requires the live tag of its exact source place.
//! Flow records the access after receiver effects and before subsequent
//! operands. Readable access and a declared payload type supply no tag proof;
//! consume the ordinary live-case judgment, independently of scalar format,
//! qualification, or whether the accessed payload is copied or borrowed.

use arena::Handle;
use checked_trees::{CheckFacts, FlowCaseAccessFact, FlowOperandReferent};
use diagnostics::Diagnostic;
use facts::{PlaceRoot, PlaceSegment};
use validation::CanonicalPlace;

pub(super) fn check(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (_, state) in facts.flow.control.states.iter() {
        for access in facts
            .flow
            .control
            .case_accesses
            .span_or_empty(state.case_accesses)
        {
            let subjects = facts
                .flow
                .control
                .operand_referents
                .span_or_empty(access.places);
            let unavailable_subject = subjects.iter().find_map(|subject| {
                subject_cases_are_selected(program, facts, access, subject, &mut Vec::new())
                    .err()
                    .map(|reason| {
                        (
                            canonical_subject(facts, subject).unwrap_or(CanonicalPlace {
                                root: subject.root,
                                segments: Vec::new(),
                            }),
                            reason,
                        )
                    })
            });
            if subjects.is_empty() || unavailable_subject.is_some() {
                let location = program
                    .machine_by_symbol(state.machine_symbol)
                    .map(|machine| {
                        let state_name = program
                            .state_in_machine(machine, state.state_symbol)
                            .map(|state| state.name.as_str())
                            .unwrap_or("<unknown>");
                        format!(
                            "machine `{}` state `{state_name}` statement {}: ",
                            machine.name, access.statement_index
                        )
                    })
                    .unwrap_or_default();
                let observation = unavailable_subject.map_or_else(
                    || "no captured source place is available".to_owned(),
                    |(subject, reason)| {
                        format!(
                            "captured source `{}` has no live case evidence: {reason}",
                            facts::canonical_place_label_from_parts(
                                program,
                                subject.root,
                                &subject.segments,
                            )
                        )
                    },
                );
                diagnostics.push(Diagnostic::error(format!(
                    "{location}cannot prove selected case for payload access `{}`: {observation}",
                    program.expression_table.display_name(access.expression),
                )));
            }
        }
    }
}

fn canonical_subject(facts: &CheckFacts, subject: &FlowOperandReferent) -> Option<CanonicalPlace> {
    super::places::place_in(
        &facts.flow.control.operand_referent_segments,
        subject.root,
        subject.segments,
    )
}

/// Reuse the exact earlier access obligation, never its ambient context as
/// current evidence. Cyclic or malformed provenance cannot discharge a case.
fn recorded_access_is_selected(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    handle: Handle<FlowCaseAccessFact>,
    expected: &FlowOperandReferent,
    ancestors: &mut Vec<Handle<FlowCaseAccessFact>>,
) -> Result<(), &'static str> {
    let control = &facts.flow.control;
    if !control.case_accesses.is_valid(handle) || ancestors.contains(&handle) {
        return Err("the recorded source access is missing or cyclic");
    }
    let access = control.case_accesses.get(handle);
    let Some(subjects) = control.operand_referents.span(access.places) else {
        return Err("the recorded source access has no valid source list");
    };
    // The producer associates the actual source coordinates with its read;
    // a successful unrelated access is not a substitute for this prefix.
    if subjects.is_empty()
        || !subjects.iter().any(|subject| {
            subject.formation == expected.formation
                && canonical_subject(facts, subject) == canonical_subject(facts, expected)
        })
    {
        return Err("the recorded source access does not name this captured source");
    }
    ancestors.push(handle);
    let selected = subjects.iter().try_for_each(|subject| {
        subject_cases_are_selected(program, facts, access, subject, ancestors)
    });
    ancestors.pop();
    selected
}

fn subject_cases_are_selected(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    access: &FlowCaseAccessFact,
    subject: &FlowOperandReferent,
    ancestors: &mut Vec<Handle<FlowCaseAccessFact>>,
) -> Result<(), &'static str> {
    let Some(place) = canonical_subject(facts, subject) else {
        return Err("the captured source has no canonical place");
    };
    let control = &facts.flow.control;
    let contexts = facts
        .flow
        .semantic_constraint_contexts(access.constraints)
        .collect::<Vec<_>>();
    let mut prefix_length = 0;
    if subject.formation.is_valid() {
        if !control.referent_formations.is_valid(subject.formation) {
            return Err("the captured formation is missing");
        }
        let formation = control.referent_formations.get(subject.formation);
        let Some(namespace) = control.operand_referent_segments.span(formation.segments) else {
            return Err("the captured formation has no valid namespace");
        };
        // Only the proven Case prefixes are historical. Non-case view-window
        // geometry may be projected by the ordinary captured-view operation;
        // no containment or old pointee facts establish any later Case.
        let case_prefix_length = namespace
            .iter()
            .rposition(|segment| matches!(segment, PlaceSegment::Case { .. }))
            .map_or(0, |position| position + 1);
        if formation.root != place.root
            || !place.segments.starts_with(&namespace[..case_prefix_length])
        {
            return Err("the captured formation namespace does not prefix its source");
        }
        let Some(path) = control.operand_child_paths.span(formation.path) else {
            return Err("the captured formation has no valid carrier path");
        };
        if formation.owner.is_valid() {
            if !control.operands.is_valid(formation.owner) {
                return Err("the completed carrier occurrence is missing");
            }
            let owner = control.operands.get(formation.owner);
            if formation.source_access.is_valid() {
                if !control.case_accesses.is_valid(formation.source_access) {
                    return Err("the carrier source access is missing");
                }
                let source = control.case_accesses.get(formation.source_access);
                if source.expression != owner.expression
                    || source.state_symbol != owner.state_symbol
                    || source.statement_index != owner.statement_index
                {
                    return Err("the carrier source access belongs to a different occurrence");
                }
            }
            // Conditional inventory does not select an inner sum. This exact
            // completed carrier must supply its own current case evidence at
            // the descriptor load, independently of the source-copy prefix.
            if !crate::flow::place_cases_are_selected(
                program,
                &facts.semantic,
                &contexts,
                access.machine_symbol,
                access.state_symbol,
                access.statement_index,
                &CanonicalPlace {
                    root: PlaceRoot::Expression(owner.expression),
                    segments: path.to_vec(),
                },
            ) {
                return Err("the completed carrier does not select its required case");
            }
        } else {
            if !path.is_empty() {
                return Err("a carrier-less formation has a nonempty carrier path");
            }
        }
        if !control.operand_referents.is_valid(formation.source) {
            return Err("the captured formation source is missing");
        }
        let source = control.operand_referents.get(formation.source);
        if formation.source_access.is_valid() {
            recorded_access_is_selected(
                program,
                facts,
                formation.source_access,
                source,
                ancestors,
            )?;
        } else {
            let Some(source_place) = canonical_subject(facts, source) else {
                return Err("the historical source has no canonical place");
            };
            if source.formation.is_valid()
                || source_place
                    .segments
                    .iter()
                    .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
            {
                return Err("a historical case source has no completed access evidence");
            }
        }
        prefix_length = case_prefix_length;
    }
    if crate::flow::place_cases_are_selected_after(
        program,
        &facts.semantic,
        &contexts,
        access.machine_symbol,
        access.state_symbol,
        access.statement_index,
        &place,
        prefix_length,
    ) {
        Ok(())
    } else {
        Err("the current source does not select its required case")
    }
}
