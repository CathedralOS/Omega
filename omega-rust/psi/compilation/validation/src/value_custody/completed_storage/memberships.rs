//! Membership supplied by a completed, jointly checked content transformation.
//!
//! The transformation qualifies its result paths, never the raw operands that
//! preceded it. Subsequent reads transport that judgment only through exact
//! preserved completed values. Every alternative must have its own evidence;
//! discovering one successful origin is insufficient at a join.

use arena::Handle;
use checked_trees::{CheckFacts, FlowOperandFact};
use facts::PlaceSegment;
use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;

pub fn completed_membership(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    domain: SymbolHandle,
    semantic_domain: SemanticDomainId,
) -> bool {
    fn proves(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        value: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        domain: SymbolHandle,
        semantic_domain: SemanticDomainId,
        visiting: &mut Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
    ) -> bool {
        if !facts.flow.control.operands.is_valid(value)
            || visiting
                .iter()
                .any(|(seen, projection)| *seen == value && projection == path)
        {
            return false;
        }
        if captured_owned_membership(program, facts, value, path, domain, semantic_domain) {
            return true;
        }
        if facts
            .qualifications
            .content
            .establishes_completed_membership(&facts.flow, value, path, domain, semantic_domain)
        {
            return true;
        }
        let Some(predecessors) = super::predecessors(program, facts, value, path) else {
            return false;
        };
        visiting.push((value, path.to_vec()));
        let proven = !predecessors.is_empty()
            && predecessors.iter().all(|(previous, projection)| {
                proves(
                    program,
                    facts,
                    *previous,
                    projection,
                    domain,
                    semantic_domain,
                    visiting,
                )
            });
        visiting.pop();
        proven
    }
    proves(
        program,
        facts,
        value,
        path,
        domain,
        semantic_domain,
        &mut Vec::new(),
    )
}

/// Owned content cannot disappear when its displayed qualification changes.
/// Read the exact membership present at this capture, not a destination type
/// or a claim identifier. Reference contents remain tied to their own epoch.
fn captured_owned_membership(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    domain: SymbolHandle,
    semantic_domain: SemanticDomainId,
) -> bool {
    if !facts
        .qualifications
        .content
        .for_semantic_domain(semantic_domain)
        .is_some_and(|projection| projection.domain == domain)
    {
        return false;
    }
    let operand = facts.flow.control.operands.get(value);
    for length in 0..=path.len() {
        let Some(reference) = crate::project_type_reference_from_segments(
            program,
            operand.type_reference,
            &path[..length],
        ) else {
            return false;
        };
        if crate::reference_shell(program, reference).is_some()
            || (length == path.len()
                && program.type_multiplicity(reference) != language_semantics::Multiplicity::Linear)
        {
            return false;
        }
    }
    let Some(mut subject) = crate::canonical_place_from_expression_in_state(
        program,
        operand.state_symbol,
        operand.statement_index,
        operand.expression,
    ) else {
        return false;
    };
    subject.segments.extend_from_slice(path);
    let contexts = facts
        .flow
        .semantic_constraint_contexts(operand.constraints)
        .collect::<Vec<_>>();
    crate::membership_evidence::MembershipEvidence::prepare(program, &facts.semantic, &contexts)
        .contains(&subject, domain, semantic_domain)
}
