//! Membership supplied by a completed content transformation or owned transfer.
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

/// Ephemeral explanation from an attempted proof, never retained claim evidence.
#[derive(Clone, Copy)]
pub struct CompletedMembershipRefusal {
    pub reason: &'static str,
}

struct MembershipWalk<'observer> {
    visiting: Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
    on_refusal: &'observer mut dyn FnMut(&'static str),
}

pub fn completed_membership(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    domain: SymbolHandle,
    semantic_domain: SemanticDomainId,
) -> bool {
    judge_completed_membership(
        program,
        facts,
        value,
        path,
        domain,
        semantic_domain,
        &mut |_| {},
    )
}

/// Record the first applicable refusal during the same membership proof walk.
pub fn completed_membership_with_refusal(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    domain: SymbolHandle,
    semantic_domain: SemanticDomainId,
    first_refusal: &mut Option<CompletedMembershipRefusal>,
) -> bool {
    judge_completed_membership(
        program,
        facts,
        value,
        path,
        domain,
        semantic_domain,
        &mut |reason| {
            first_refusal.get_or_insert(CompletedMembershipRefusal { reason });
        },
    )
}

fn judge_completed_membership(
    program: &dyn crate::CheckingInput,
    facts: &CheckFacts,
    value: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
    domain: SymbolHandle,
    semantic_domain: SemanticDomainId,
    on_refusal: &mut dyn FnMut(&'static str),
) -> bool {
    fn proves(
        program: &dyn crate::CheckingInput,
        facts: &CheckFacts,
        value: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        domain: SymbolHandle,
        semantic_domain: SemanticDomainId,
        walk: &mut MembershipWalk<'_>,
    ) -> bool {
        if !facts.flow.control.operands.is_valid(value)
            || walk
                .visiting
                .iter()
                .any(|(seen, projection)| *seen == value && projection == path)
        {
            return false;
        }
        if facts
            .qualifications
            .content
            .establishes_completed_membership(&facts.flow, value, path, domain, semantic_domain)
        {
            return true;
        }
        match super::owned_memberships::transferred_membership(
            program,
            facts,
            value,
            path,
            domain,
            semantic_domain,
        ) {
            Some(Ok(())) => return true,
            Some(Err(reason)) => {
                (walk.on_refusal)(reason);
                if std::env::var_os("OMEGA_STRUCT_TRACE").is_some() {
                    let captured = facts.flow.control.operands.get(value);
                    eprintln!(
                        "STRUCT owned-membership value={value:?} state={:?} statement={} lane={:?} path_len={} storage_cut={:?} domain={domain:?} reason={reason}",
                        captured.state_symbol,
                        captured.statement_index,
                        captured.evaluation,
                        path.len(),
                        captured.storage_snapshot,
                    );
                }
            }
            None => {}
        }
        let Some(predecessors) = super::predecessors(program, facts, value, path) else {
            return false;
        };
        walk.visiting.push((value, path.to_vec()));
        let proven = !predecessors.is_empty()
            && predecessors.iter().all(|(previous, projection)| {
                proves(
                    program,
                    facts,
                    *previous,
                    projection,
                    domain,
                    semantic_domain,
                    walk,
                )
            });
        walk.visiting.pop();
        proven
    }
    proves(
        program,
        facts,
        value,
        path,
        domain,
        semantic_domain,
        &mut MembershipWalk {
            visiting: Vec::new(),
            on_refusal,
        },
    )
}
