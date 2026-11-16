/*
    The interpreter stores each actual resident value once. This flat rank index
    only names those existing resident slots; it grants no ownership and never
    reconstructs a cell from its type, byte length or count.

    A split composes a visible interval without traversing cells or introducing
    slice nodes. Merge normalizes the two visible intervals and joins a balanced
    index. Counts are u64: an AVL tree of nonempty leaves has height below 128.
    Each operation has an explicit fixed-word metadata budget, including all
    index allocations. These are host index operations, never payload reads.
    Old index nodes contain arena handles rather than recursive Arc ownership,
    so retiring the final descriptor cannot trigger a deferred history walk.
*/

use crate::boundary::errors::{TerminalInterpretError, malformed};
use crate::storage::provider_storage::ResidentStorageSlot;
use arena::{Arena, Handle};
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};

const HEIGHT_BOUND: usize = 128;
const METADATA_STEP_BOUND: usize = 64 * HEIGHT_BOUND;
const NEW_NODE_BOUND: usize = 16 * HEIGHT_BOUND;

type NodeId = Handle<IndexNode>;
type CellId = Handle<ResidentStorageSlot>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ResidentRunRange {
    root: NodeId,
    start: u64,
    count: u64,
}

impl ResidentRunRange {
    pub(crate) fn count(self) -> u64 {
        self.count
    }

    pub(crate) fn split(self, cut: u64) -> Result<(Self, Self), TerminalInterpretError> {
        if cut > self.count {
            return Err(reject("run split exceeds its retained actual cells"));
        }
        let right_start = self.start.checked_add(cut).ok_or_else(malformed)?;
        Ok((
            Self { count: cut, ..self },
            Self {
                start: right_start,
                count: self.count - cut,
                ..self
            },
        ))
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum IndexKind {
    #[default]
    Empty,
    Cell(CellId),
    Branch {
        left: NodeId,
        right: NodeId,
    },
}

#[derive(Default)]
struct IndexNode {
    kind: IndexKind,
    count: u64,
    height: u8,
    _lease: Option<EvaluationStorageLease>,
}

#[derive(Debug, Clone, Copy)]
struct Metadata {
    kind: IndexKind,
    count: u64,
    height: u8,
}

#[derive(Debug, Clone, Copy, Default)]
struct Work {
    steps: usize,
    nodes: usize,
}

impl Work {
    fn step(&mut self) -> Result<(), TerminalInterpretError> {
        self.steps += 1;
        if self.steps > METADATA_STEP_BOUND {
            return Err(reject(
                "run payload index exceeds the fixed-word metadata step bound",
            ));
        }
        Ok(())
    }

    fn node(&mut self) -> Result<(), TerminalInterpretError> {
        self.nodes += 1;
        if self.nodes > NEW_NODE_BOUND {
            return Err(reject(
                "run payload index exceeds the fixed-word metadata allocation bound",
            ));
        }
        Ok(())
    }
}

pub(crate) struct ResidentRunIndex {
    nodes: Arena<IndexNode>,
    #[cfg(test)]
    last_work: Work,
}

impl ResidentRunIndex {
    pub(crate) fn new() -> Self {
        Self {
            nodes: Arena::default(),
            #[cfg(test)]
            last_work: Work::default(),
        }
    }

    pub(crate) fn singleton(
        &mut self,
        cell: CellId,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<ResidentRunRange, TerminalInterpretError> {
        if cell == CellId::default() {
            return Err(reject("run initialization has no actual resident value"));
        }
        let lease = account
            .map(|account| account.reserve_storage(1, 0))
            .transpose()
            .map_err(TerminalInterpretError::Storage)?;
        let root = self.nodes.insert(IndexNode {
            kind: IndexKind::Cell(cell),
            count: 1,
            height: 1,
            _lease: lease,
        });
        Ok(ResidentRunRange {
            root,
            start: 0,
            count: 1,
        })
    }

    pub(crate) fn validate(&self, range: ResidentRunRange) -> Result<(), TerminalInterpretError> {
        let mut work = Work::default();
        let count = self.read(range.root, &mut work)?.count;
        if range
            .start
            .checked_add(range.count)
            .is_none_or(|end| end > count)
        {
            return Err(reject("run range exceeds its actual indexed cells"));
        }
        Ok(())
    }

    // No resident payload is inspected here. Both operands remain available if
    // reservation, overflow or index validation fails before descriptor commit.
    pub(crate) fn merge(
        &mut self,
        left: ResidentRunRange,
        right: ResidentRunRange,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<ResidentRunRange, TerminalInterpretError> {
        self.validate(left)?;
        self.validate(right)?;
        let count = left
            .count
            .checked_add(right.count)
            .ok_or_else(|| reject("run cell count exceeds u64"))?;
        let mut work = Work::default();
        let mut created = Vec::new();
        let result = (|| {
            let left = self.normalize(left, account, &mut work, &mut created)?;
            let right = self.normalize(right, account, &mut work, &mut created)?;
            let root = self.join(left, right, account, &mut work, &mut created)?;
            if self.read(root, &mut work)?.count != count {
                return Err(malformed());
            }
            Ok(ResidentRunRange {
                root,
                start: 0,
                count,
            })
        })();
        if result.is_err() {
            for node in created {
                self.nodes.free(node);
            }
        }
        #[cfg(test)]
        {
            self.last_work = work;
        }
        result
    }

    pub(crate) fn select(
        &self,
        range: ResidentRunRange,
        index: u64,
    ) -> Result<CellId, TerminalInterpretError> {
        self.validate(range)?;
        if index >= range.count {
            return Err(reject("run extraction has no selected actual cell"));
        }
        let mut index = range.start.checked_add(index).ok_or_else(malformed)?;
        let mut root = range.root;
        let mut work = Work::default();
        for _ in 0..HEIGHT_BOUND {
            match self.read(root, &mut work)?.kind {
                IndexKind::Cell(cell) if index == 0 => return Ok(cell),
                IndexKind::Branch { left, right } => {
                    let left_count = self.read(left, &mut work)?.count;
                    if index < left_count {
                        root = left;
                    } else {
                        index -= left_count;
                        root = right;
                    }
                }
                _ => return Err(malformed()),
            }
        }
        Err(reject("run payload index exceeds its word-bounded height"))
    }

    fn read(&self, node: NodeId, work: &mut Work) -> Result<Metadata, TerminalInterpretError> {
        work.step()?;
        if node == NodeId::default() {
            return Ok(Metadata {
                kind: IndexKind::Empty,
                count: 0,
                height: 0,
            });
        }
        let node = self.nodes.try_get(node).ok_or_else(malformed)?;
        if node.height as usize >= HEIGHT_BOUND || node.count == 0 {
            return Err(malformed());
        }
        Ok(Metadata {
            kind: node.kind,
            count: node.count,
            height: node.height,
        })
    }

    fn branch(
        &mut self,
        left: NodeId,
        right: NodeId,
        account: Option<&EvaluationInvocationAccount>,
        work: &mut Work,
        created: &mut Vec<NodeId>,
    ) -> Result<NodeId, TerminalInterpretError> {
        if left == NodeId::default() {
            return Ok(right);
        }
        if right == NodeId::default() {
            return Ok(left);
        }
        let a = self.read(left, work)?;
        let b = self.read(right, work)?;
        let count = a
            .count
            .checked_add(b.count)
            .ok_or_else(|| reject("run cell count exceeds u64"))?;
        let height = a
            .height
            .max(b.height)
            .checked_add(1)
            .ok_or_else(malformed)?;
        if height as usize >= HEIGHT_BOUND {
            return Err(malformed());
        }
        work.node()?;
        let lease = account
            .map(|account| account.reserve_storage(1, 0))
            .transpose()
            .map_err(TerminalInterpretError::Storage)?;
        let node = self.nodes.insert(IndexNode {
            kind: IndexKind::Branch { left, right },
            count,
            height,
            _lease: lease,
        });
        created.push(node);
        Ok(node)
    }

    fn children(
        &self,
        node: NodeId,
        work: &mut Work,
    ) -> Result<(NodeId, NodeId), TerminalInterpretError> {
        match self.read(node, work)?.kind {
            IndexKind::Branch { left, right } => Ok((left, right)),
            _ => Err(malformed()),
        }
    }

    fn balance(
        &mut self,
        left: NodeId,
        right: NodeId,
        account: Option<&EvaluationInvocationAccount>,
        work: &mut Work,
        created: &mut Vec<NodeId>,
    ) -> Result<NodeId, TerminalInterpretError> {
        let a = self.read(left, work)?;
        let b = self.read(right, work)?;
        if a.height > b.height.saturating_add(1) {
            let (ll, lr) = self.children(left, work)?;
            if self.read(ll, work)?.height >= self.read(lr, work)?.height {
                let tail = self.branch(lr, right, account, work, created)?;
                return self.branch(ll, tail, account, work, created);
            }
            let (lrl, lrr) = self.children(lr, work)?;
            let head = self.branch(ll, lrl, account, work, created)?;
            let tail = self.branch(lrr, right, account, work, created)?;
            return self.branch(head, tail, account, work, created);
        }
        if b.height > a.height.saturating_add(1) {
            let (rl, rr) = self.children(right, work)?;
            if self.read(rr, work)?.height >= self.read(rl, work)?.height {
                let head = self.branch(left, rl, account, work, created)?;
                return self.branch(head, rr, account, work, created);
            }
            let (rll, rlr) = self.children(rl, work)?;
            let head = self.branch(left, rll, account, work, created)?;
            let tail = self.branch(rlr, rr, account, work, created)?;
            return self.branch(head, tail, account, work, created);
        }
        self.branch(left, right, account, work, created)
    }

    fn join(
        &mut self,
        mut left: NodeId,
        mut right: NodeId,
        account: Option<&EvaluationInvocationAccount>,
        work: &mut Work,
        created: &mut Vec<NodeId>,
    ) -> Result<NodeId, TerminalInterpretError> {
        if left == NodeId::default() {
            return Ok(right);
        }
        if right == NodeId::default() {
            return Ok(left);
        }
        let mut path = Vec::new();
        let mut joined;
        loop {
            let a = self.read(left, work)?;
            let b = self.read(right, work)?;
            if a.height > b.height.saturating_add(1) {
                let (outer, inner) = self.children(left, work)?;
                path.push((outer, true));
                left = inner;
            } else if b.height > a.height.saturating_add(1) {
                let (inner, outer) = self.children(right, work)?;
                path.push((outer, false));
                right = inner;
            } else {
                joined = self.branch(left, right, account, work, created)?;
                break;
            }
            if path.len() >= HEIGHT_BOUND {
                return Err(malformed());
            }
        }
        for (outer, was_left) in path.into_iter().rev() {
            joined = if was_left {
                self.balance(outer, joined, account, work, created)?
            } else {
                self.balance(joined, outer, account, work, created)?
            };
        }
        Ok(joined)
    }

    fn split_root(
        &mut self,
        mut root: NodeId,
        mut cut: u64,
        account: Option<&EvaluationInvocationAccount>,
        work: &mut Work,
        created: &mut Vec<NodeId>,
    ) -> Result<(NodeId, NodeId), TerminalInterpretError> {
        if cut > self.read(root, work)?.count {
            return Err(malformed());
        }
        let mut path = Vec::new();
        let (mut left, mut right);
        loop {
            let node = self.read(root, work)?;
            if cut == 0 {
                left = NodeId::default();
                right = root;
                break;
            }
            if cut == node.count {
                left = root;
                right = NodeId::default();
                break;
            }
            let (a, b) = self.children(root, work)?;
            let a_count = self.read(a, work)?.count;
            if cut == a_count {
                left = a;
                right = b;
                break;
            }
            if cut < a_count {
                path.push((b, true));
                root = a;
            } else {
                path.push((a, false));
                root = b;
                cut -= a_count;
            }
            if path.len() >= HEIGHT_BOUND {
                return Err(malformed());
            }
        }
        for (sibling, was_left) in path.into_iter().rev() {
            if was_left {
                right = self.join(right, sibling, account, work, created)?;
            } else {
                left = self.join(sibling, left, account, work, created)?;
            }
        }
        Ok((left, right))
    }

    fn normalize(
        &mut self,
        range: ResidentRunRange,
        account: Option<&EvaluationInvocationAccount>,
        work: &mut Work,
        created: &mut Vec<NodeId>,
    ) -> Result<NodeId, TerminalInterpretError> {
        if range.count == 0 {
            return Ok(NodeId::default());
        }
        let root_count = self.read(range.root, work)?.count;
        if range.start == 0 && range.count == root_count {
            return Ok(range.root);
        }
        let (_, suffix) = self.split_root(range.root, range.start, account, work, created)?;
        let (visible, _) = self.split_root(suffix, range.count, account, work, created)?;
        Ok(visible)
    }
}

fn reject(reason: &'static str) -> TerminalInterpretError {
    TerminalInterpretError::ContentTransformationMismatch(reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(index: u32) -> CellId {
        Handle::from_parts(index, 1)
    }

    #[test]
    fn actual_cells_keep_order_across_arbitrary_cuts_and_both_merge_directions() {
        for prepend in [false, true] {
            let mut index = ResidentRunIndex::new();
            let mut run = ResidentRunRange::default();
            for n in 1..=4096 {
                let leaf = index.singleton(cell(n), None).unwrap();
                run = if prepend {
                    index.merge(leaf, run, None)
                } else {
                    index.merge(run, leaf, None)
                }
                .unwrap();
                assert!(index.last_work.steps <= METADATA_STEP_BOUND);
                assert!(index.last_work.nodes <= NEW_NODE_BOUND);
            }
            for cut in [0, 1, 137, 2048, 4095, 4096] {
                let (left, right) = run.split(cut).unwrap();
                let again = index.merge(left, right, None).unwrap();
                for position in [0, 123, 4095] {
                    let expected = if prepend {
                        4096 - position
                    } else {
                        position + 1
                    };
                    assert_eq!(
                        index.select(again, position).unwrap(),
                        cell(expected as u32)
                    );
                }
            }
        }
    }

    #[test]
    fn fixed_count_slice_concat_history_does_not_accumulate_lookup_depth() {
        let mut index = ResidentRunIndex::new();
        let first = index.singleton(cell(1), None).unwrap();
        let second = index.singleton(cell(2), None).unwrap();
        let mut run = index.merge(first, second, None).unwrap();
        let mut maximum_steps = 0;
        for iteration in 0..20_000 {
            let (left, right) = run.split(1).unwrap();
            run = index.merge(right, left, None).unwrap();
            maximum_steps = maximum_steps.max(index.last_work.steps);
            assert_eq!(
                index.select(run, 0).unwrap(),
                cell(if iteration % 2 == 0 { 2 } else { 1 })
            );
            assert_eq!(
                index.select(run, 1).unwrap(),
                cell(if iteration % 2 == 0 { 1 } else { 2 })
            );
        }
        assert!(
            maximum_steps < 256,
            "small visible ranges retained old history"
        );
    }

    #[test]
    fn split_empty_and_bounds_never_mint_actual_cells() {
        let mut index = ResidentRunIndex::new();
        assert!(index.singleton(CellId::default(), None).is_err());
        let run = index.singleton(cell(3), None).unwrap();
        let (empty, whole) = run.split(0).unwrap();
        assert_eq!(empty.count(), 0);
        assert!(index.select(empty, 0).is_err());
        assert_eq!(index.select(whole, 0).unwrap(), cell(3));
        assert!(run.split(2).is_err());
        assert!(index.select(run, 1).is_err());
    }
}
