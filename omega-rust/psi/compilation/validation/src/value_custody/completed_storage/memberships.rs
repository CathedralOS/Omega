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
