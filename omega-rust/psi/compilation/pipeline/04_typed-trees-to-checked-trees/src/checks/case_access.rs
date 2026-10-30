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

#[derive(Clone, Copy, Default)]
enum AccessProof {
    #[default]
    Unchecked,
    Checking,
    Selected,
    Rejected,
}

pub(super) fn check(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // The input is frozen for this check. Each access owns one complete
    // obligation, even when many possible backing places refer to it. Check
    // that obligation once, retaining cycle rejection and checking each
    // incoming edge's exact source membership before reusing its result.
    let mut proofs: Vec<_> = facts
        .flow
        .control
        .case_accesses
        .iter()
        .map(|(handle, _)| (handle, AccessProof::Unchecked))
        .collect();
    proofs.sort_unstable_by_key(|(handle, _)| (handle.generation(), handle.arena_index()));
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
            let unavailable_subject = subjects
                .iter()
                .find(|subject| {
                    !subject_cases_are_selected(program, facts, access, subject, &mut proofs)
                })
                .map(|subject| {
                    canonical_subject(facts, subject).unwrap_or(CanonicalPlace {
                        root: subject.root,
                        segments: Vec::new(),
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
                    |subject| {
                        format!(
                            "captured source `{}` has no live case evidence",
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
    proofs: &mut [(Handle<FlowCaseAccessFact>, AccessProof)],
) -> bool {
    let control = &facts.flow.control;
    if !control.case_accesses.is_valid(handle) {
        return false;
    }
    let access = control.case_accesses.get(handle);
    let Some(subjects) = control.operand_referents.span(access.places) else {
        return false;
    };
    // The producer associates the actual source coordinates with its read;
    // a successful unrelated access is not a substitute for this prefix.
    if subjects.is_empty()
        || !subjects.iter().any(|subject| {
            subject.formation == expected.formation
                && canonical_subject(facts, subject) == canonical_subject(facts, expected)
        })
    {
        return false;
    }
    // Page generations are part of identity. The scratch roster contains only
    // this frozen arena's live handles, never an index from another page.
    let Ok(position) = proofs.binary_search_by_key(
        &(handle.generation(), handle.arena_index()),
        |(handle, _)| (handle.generation(), handle.arena_index()),
    ) else {
        return false;
    };
    match proofs[position].1 {
        AccessProof::Selected => return true,
        AccessProof::Checking | AccessProof::Rejected => return false,
        AccessProof::Unchecked => {}
    }
    proofs[position].1 = AccessProof::Checking;
    let selected = subjects
        .iter()
        .all(|subject| subject_cases_are_selected(program, facts, access, subject, proofs));
    proofs[position].1 = if selected {
        AccessProof::Selected
    } else {
        AccessProof::Rejected
    };
    selected
}

fn subject_cases_are_selected(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    access: &FlowCaseAccessFact,
    subject: &FlowOperandReferent,
    proofs: &mut [(Handle<FlowCaseAccessFact>, AccessProof)],
) -> bool {
    let Some(place) = canonical_subject(facts, subject) else {
        return false;
    };
    let control = &facts.flow.control;
    let contexts = facts
        .flow
        .semantic_constraint_contexts(access.constraints)
        .collect::<Vec<_>>();
    let mut prefix_length = 0;
    if subject.formation.is_valid() {
        if !control.referent_formations.is_valid(subject.formation) {
            return false;
        }
        let formation = control.referent_formations.get(subject.formation);
        let Some(namespace) = control.operand_referent_segments.span(formation.segments) else {
            return false;
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
            return false;
        }
        let Some(path) = control.operand_child_paths.span(formation.path) else {
            return false;
        };
        if formation.owner.is_valid() {
            if !control.operands.is_valid(formation.owner) {
                return false;
            }
            let owner = control.operands.get(formation.owner);
            if formation.source_access.is_valid() {
                if !control.case_accesses.is_valid(formation.source_access) {
                    return false;
                }
                let source = control.case_accesses.get(formation.source_access);
                if source.expression != owner.expression
                    || source.state_symbol != owner.state_symbol
                    || source.statement_index != owner.statement_index
                {
                    return false;
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
                return false;
            }
        } else {
            if !path.is_empty() {
                return false;
            }
        }
        if !control.operand_referents.is_valid(formation.source) {
            return false;
        }
        let source = control.operand_referents.get(formation.source);
        if formation.source_access.is_valid() {
            if !recorded_access_is_selected(program, facts, formation.source_access, source, proofs)
            {
                return false;
            }
        } else {
            let Some(source_place) = canonical_subject(facts, source) else {
                return false;
            };
            if source.formation.is_valid()
                || source_place
                    .segments
                    .iter()
                    .any(|segment| matches!(segment, PlaceSegment::Case { .. }))
            {
                return false;
            }
        }
        prefix_length = case_prefix_length;
    }
    crate::flow::place_cases_are_selected_after(
        program,
        &facts.semantic,
        &contexts,
        access.machine_symbol,
        access.state_symbol,
        access.statement_index,
        &place,
        prefix_length,
    )
}
