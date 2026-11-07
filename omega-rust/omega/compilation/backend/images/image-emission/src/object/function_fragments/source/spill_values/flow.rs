//! Must-agree last writers over a finite control graph. Uninitialized entry
//! paths participate in joins; a store on a backedge cannot initialize the
//! first iteration. Stores kill incoming writers, including disagreement.
use selected_instructions::SelectedInstructionId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Writer {
    Unreachable,
    Missing,
    Store(SelectedInstructionId),
    Different,
}
impl Writer {
    fn merge(self, incoming: Self) -> Self {
        if self == incoming || incoming == Self::Unreachable {
            self
        } else if self == Self::Unreachable {
            incoming
        } else {
            Self::Different
        }
    }
}
pub(super) fn entries(
    entry: usize,
    last: &[Option<SelectedInstructionId>],
    successors: &[Vec<usize>],
) -> Vec<Writer> {
    let mut entries = vec![Writer::Unreachable; successors.len()];
    entries[entry] = Writer::Missing;
    let mut pending = std::collections::VecDeque::from([entry]);
    let mut queued = vec![false; entries.len()];
    queued[entry] = true;
    while let Some(block) = pending.pop_front() {
        queued[block] = false;
        let outgoing = last[block].map_or(entries[block], Writer::Store);
        for &next in &successors[block] {
            let merged = entries[next].merge(outgoing);
            if merged != entries[next] {
                entries[next] = merged;
                if !queued[next] {
                    queued[next] = true;
                    pending.push_back(next);
                }
            }
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: SelectedInstructionId = SelectedInstructionId(10);
    const B: SelectedInstructionId = SelectedInstructionId(20);
    #[test]
    fn initialized_word_reaches_straight_line_and_loop_reads() {
        let result = entries(
            0,
            &[Some(A), None, None, None],
            &[vec![1], vec![2, 3], vec![1], vec![]],
        );
        assert_eq!(
            result,
            vec![
                Writer::Missing,
                Writer::Store(A),
                Writer::Store(A),
                Writer::Store(A)
            ]
        );
    }
    #[test]
    fn a_branch_without_initialization_does_not_inherit_its_sibling_store() {
        let result = entries(
            0,
            &[None, Some(A), None, None],
            &[vec![1, 2], vec![3], vec![3], vec![]],
        );
        assert_eq!(result[3], Writer::Different);
    }
    #[test]
    fn different_branch_writers_remain_distinct() {
        let result = entries(
            0,
            &[None, Some(A), Some(B), None],
            &[vec![1, 2], vec![3], vec![3], vec![]],
        );
        assert_eq!(result[3], Writer::Different);
    }
    #[test]
    fn a_backedge_store_does_not_initialize_the_first_iteration() {
        let result = entries(0, &[None, None, Some(A)], &[vec![1], vec![2], vec![1]]);
        assert_eq!(result[1], Writer::Different);
    }
    #[test]
    fn unreachable_stores_do_not_publish_values() {
        let result = entries(0, &[None, Some(A), None], &[vec![], vec![2], vec![]]);
        assert_eq!(result[1], Writer::Unreachable);
        assert_eq!(result[2], Writer::Unreachable);
    }
    #[test]
    fn reentered_definition_cannot_bypass_its_store_before_a_reload() {
        // Block 1 introduces a new dynamic value. Block 2 stores it and loops.
        // Bypassing that store to read in block 3 also permits the first visit
        // without a store, so no static store ID certifies a stale iteration.
        let result = entries(
            0,
            &[None, None, Some(A), None],
            &[vec![1], vec![2, 3], vec![1], vec![]],
        );
        assert_eq!(result[3], Writer::Different);
    }
}
