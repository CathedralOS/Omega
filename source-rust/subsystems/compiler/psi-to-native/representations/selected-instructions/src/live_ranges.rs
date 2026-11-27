/*
  Durable selected-instruction live-range facts (`LiveRangePlan`,
  `FunctionLiveRanges`, `VirtualLiveRange`, `ArchitecturalUnitLiveRange`) and
  compressed virtual interference tables (`VirtualInterferences`). A plan names
  the program it describes through the shared `selected` plan; its liveness is
  a function of that program alone, so it records no separate liveness name.
*/

mod interference {

    use std::cmp::Ordering;
    use std::fmt;

    use super::VirtualInterference;
    use crate::VirtualRegisterId;

    #[derive(Clone, Default)]
    pub struct VirtualInterferences {
        lower_ids: Vec<VirtualRegisterId>,
        run_ends: Vec<usize>,
        higher_ids: Vec<VirtualRegisterId>,
    }

    impl VirtualInterferences {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn len(&self) -> usize {
            self.higher_ids.len()
        }

        pub fn is_empty(&self) -> bool {
            self.higher_ids.is_empty()
        }

        pub fn clear(&mut self) {
            self.lower_ids.clear();
            self.run_ends.clear();
            self.higher_ids.clear();
        }

        pub fn push(&mut self, pair: VirtualInterference) {
            self.higher_ids.push(pair.higher);
            let end = self.len();
            if self.lower_ids.last() == Some(&pair.lower) {
                *self.run_ends.last_mut().expect("lower run has an end") = end;
            } else {
                self.lower_ids.push(pair.lower);
                self.run_ends.push(end);
            }
        }

        pub fn get(&self, index: usize) -> Option<VirtualInterference> {
            let higher = *self.higher_ids.get(index)?;
            let run = self.run_ends.partition_point(|end| *end <= index);
            Some(VirtualInterference {
                lower: self.lower_ids[run],
                higher,
            })
        }

        pub fn iter(&self) -> Interferences<'_> {
            Interferences {
                rows: self,
                index: 0,
                run: 0,
            }
        }

        pub fn binary_search(&self, pair: &VirtualInterference) -> Result<usize, usize> {
            let mut size = self.len();
            if size == 0 {
                return Err(0);
            }
            let mut base = 0;
            while size > 1 {
                let half = size / 2;
                let mid = base + half;
                let cmp = self
                    .get(mid)
                    .expect("search position is in range")
                    .cmp(pair);
                base = if cmp == Ordering::Greater { base } else { mid };
                size -= half;
            }
            let cmp = self
                .get(base)
                .expect("search position is in range")
                .cmp(pair);
            if cmp == Ordering::Equal {
                Ok(base)
            } else {
                Err(base + usize::from(cmp == Ordering::Less))
            }
        }

        pub fn sort_lower_tail(&mut self, start: usize) {
            assert!(start <= self.len());
            if start == self.len() {
                return;
            }
            let run = self.run_ends.partition_point(|end| *end <= start);
            assert_eq!(run + 1, self.lower_ids.len(), "tail must have one lower ID");
            self.higher_ids[start..].sort_unstable();
        }

        pub fn sort_unstable_and_dedup(&mut self) {
            let mut pairs: Vec<_> = self.iter().collect();
            pairs.sort_unstable();
            pairs.dedup();
            *self = pairs.into_iter().collect();
        }
    }

    impl FromIterator<VirtualInterference> for VirtualInterferences {
        fn from_iter<T: IntoIterator<Item = VirtualInterference>>(rows: T) -> Self {
            let mut result = Self::new();
            for row in rows {
                result.push(row);
            }
            result
        }
    }

    impl From<Vec<VirtualInterference>> for VirtualInterferences {
        fn from(rows: Vec<VirtualInterference>) -> Self {
            rows.into_iter().collect()
        }
    }

    impl PartialEq for VirtualInterferences {
        fn eq(&self, other: &Self) -> bool {
            self.len() == other.len() && self.iter().eq(other.iter())
        }
    }

    impl Eq for VirtualInterferences {}

    impl fmt::Debug for VirtualInterferences {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_list().entries(self.iter()).finish()
        }
    }

    pub struct Interferences<'a> {
        rows: &'a VirtualInterferences,
        index: usize,
        run: usize,
    }

    impl Iterator for Interferences<'_> {
        type Item = VirtualInterference;

        fn next(&mut self) -> Option<Self::Item> {
            let higher = *self.rows.higher_ids.get(self.index)?;
            while self.index >= self.rows.run_ends[self.run] {
                self.run += 1;
            }
            let lower = self.rows.lower_ids[self.run];
            self.index += 1;
            Some(VirtualInterference { lower, higher })
        }

        fn size_hint(&self) -> (usize, Option<usize>) {
            let remaining = self.rows.len() - self.index;
            (remaining, Some(remaining))
        }
    }

    impl ExactSizeIterator for Interferences<'_> {}
    impl std::iter::FusedIterator for Interferences<'_> {}

    impl<'a> IntoIterator for &'a VirtualInterferences {
        type Item = VirtualInterference;
        type IntoIter = Interferences<'a>;

        fn into_iter(self) -> Self::IntoIter {
            self.iter()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn pair(lower: u32, higher: u32) -> VirtualInterference {
            VirtualInterference {
                lower: VirtualRegisterId(lower),
                higher: VirtualRegisterId(higher),
            }
        }

        fn original_bytes(rows: &[VirtualInterference]) -> Vec<u8> {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&(rows.len() as u64).to_le_bytes());
            for row in rows {
                bytes.extend_from_slice(&row.lower.0.to_le_bytes());
                bytes.extend_from_slice(&row.higher.0.to_le_bytes());
            }
            bytes
        }

        fn check(rows: &[VirtualInterference]) {
            let compact: VirtualInterferences = rows.iter().copied().collect();
            assert_eq!(compact.len(), rows.len());
            assert_eq!(compact.is_empty(), rows.is_empty());
            assert_eq!(compact.iter().collect::<Vec<_>>(), rows);
            assert_eq!(format!("{compact:?}"), format!("{rows:?}"));
            assert_eq!(format!("{compact:#?}"), format!("{rows:#?}"));
            assert_eq!(compact, compact.clone());
            assert_eq!(compact, VirtualInterferences::from(rows.to_vec()));
            for index in 0..=rows.len() {
                assert_eq!(compact.get(index), rows.get(index).copied());
            }
            let mut encoded = Vec::new();
            encoded.extend_from_slice(&(compact.len() as u64).to_le_bytes());
            for row in &compact {
                encoded.extend_from_slice(&row.lower.0.to_le_bytes());
                encoded.extend_from_slice(&row.higher.0.to_le_bytes());
            }
            assert_eq!(encoded, original_bytes(rows));
            for lower in [0, 1, 2, u32::MAX] {
                for higher in [0, 1, 2, u32::MAX] {
                    let target = pair(lower, higher);
                    assert_eq!(compact.binary_search(&target), rows.binary_search(&target));
                }
            }
            let mut changed = compact.clone();
            changed.push(pair(7, 8));
            assert_ne!(changed, compact);
            changed.clear();
            assert!(changed.is_empty());
            changed.push(pair(u32::MAX, 0));
            assert_eq!(changed.iter().collect::<Vec<_>>(), vec![pair(u32::MAX, 0)]);
            let mut iter = compact.iter();
            for (index, row) in rows.iter().enumerate() {
                assert_eq!(iter.len(), rows.len() - index);
                assert_eq!(iter.next(), Some(*row));
            }
            assert_eq!(iter.next(), None);
            assert_eq!(iter.next(), None);
            assert_eq!(iter.len(), 0);
        }

        #[test]
        fn compact_edges_match_original_rows_search_debug_identity_and_construction() {
            let alphabet = [pair(0, 0), pair(0, 1), pair(1, 0), pair(1, 1)];
            for len in 0..=5 {
                for mut code in 0..4usize.pow(len) {
                    let rows = (0..len)
                        .map(|_| {
                            let row = alphabet[code % alphabet.len()];
                            code /= alphabet.len();
                            row
                        })
                        .collect::<Vec<_>>();
                    check(&rows);
                }
            }
            check(&[
                pair(u32::MAX, 0),
                pair(0, u32::MAX),
                pair(u32::MAX, u32::MAX),
            ]);
            check(&[pair(4, 3), pair(4, 3), pair(2, 8), pair(4, 9), pair(4, 0)]);

            let split = VirtualInterferences {
                lower_ids: vec![VirtualRegisterId(4), VirtualRegisterId(4)],
                run_ends: vec![1, 3],
                higher_ids: vec![
                    VirtualRegisterId(9),
                    VirtualRegisterId(2),
                    VirtualRegisterId(2),
                ],
            };
            let merged: VirtualInterferences = vec![pair(4, 9), pair(4, 2), pair(4, 2)].into();
            assert_eq!(split, merged);
            assert_eq!(format!("{split:?}"), format!("{merged:?}"));

            let mut old = Vec::new();
            let mut compact = VirtualInterferences::new();
            for (lower, higher) in [
                (4, vec![9, 1, 1]),
                (4, vec![8, 2]),
                (0, vec![]),
                (2, vec![3, 0]),
            ] {
                let first = old.len();
                for higher in higher {
                    let row = pair(lower, higher);
                    old.push(row);
                    compact.push(row);
                }
                old[first..].sort_unstable();
                compact.sort_lower_tail(first);
                assert_eq!(compact.iter().collect::<Vec<_>>(), old);
            }
            old.sort_unstable();
            old.dedup();
            compact.sort_unstable_and_dedup();
            assert_eq!(compact.iter().collect::<Vec<_>>(), old);

            let dense: VirtualInterferences = (0..128)
                .flat_map(|lower| (lower + 1..128).map(move |higher| pair(lower, higher)))
                .collect();
            assert_eq!(dense.len(), 128 * 127 / 2);
            assert_eq!(dense.lower_ids.len(), 127);
            assert_eq!(std::mem::size_of::<VirtualInterference>(), 8);
            let payload = dense.higher_ids.len() * std::mem::size_of::<VirtualRegisterId>()
                + dense.lower_ids.len() * std::mem::size_of::<VirtualRegisterId>()
                + dense.run_ends.len() * std::mem::size_of::<usize>();
            assert!(payload < dense.len() * std::mem::size_of::<VirtualInterference>());
        }
    }
}
pub use interference::VirtualInterferences;

use crate::LivenessPosition;
use crate::{SelectedBlockId, SelectedInstructionId, VirtualRegisterId};
use optimization_core::OptimizationUnitIdentity;
use register_model::{RegisterClassId, RegisterOperandAccess, RegisterUnitId, RegisterViewId};
use semantic_vocabulary::{BlockId, EdgeId, MachineId, WorkScheduleIdentity};
use target::NativeTarget;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LiveRangePoint(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveRangePlan {
    pub selected: std::sync::Arc<crate::SelectedInstructionPlan>,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub target: NativeTarget,
    pub functions: Vec<std::sync::Arc<FunctionLiveRanges>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiveRanges {
    pub machine: MachineId,
    pub block_domains: Vec<BlockPointDomain>,
    pub virtual_registers: Vec<VirtualLiveRange>,
    pub tied_pairs: Vec<DistinctUseDefTie>,
    pub edge_transfers: Vec<EdgeRegisterTransfer>,
    pub copy_affinities: Vec<CopyAffinity>,
    pub early_clobbers: Vec<EarlyClobberConstraint>,
    pub architectural_units: Vec<ArchitecturalUnitLiveRange>,
    pub interference: VirtualInterferences,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EdgeRegisterTransfer {
    pub source: SelectedBlockId,
    pub target: SelectedBlockId,
    pub psi_edge: EdgeId,
    pub argument: VirtualRegisterId,
    pub parameter: VirtualRegisterId,
    pub class: RegisterClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CopyAffinity {
    pub block: SelectedBlockId,
    pub instruction: SelectedInstructionId,
    pub source: VirtualRegisterId,
    pub destination: VirtualRegisterId,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EarlyClobberConstraint {
    pub block: SelectedBlockId,
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub early_point: LiveRangePoint,
    pub def_operand: u16,
    pub def_virtual_register: VirtualRegisterId,
    pub def_class: RegisterClassId,
    pub def_point: LiveRangePoint,
    pub uses: Vec<EarlyClobberUse>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct EarlyClobberUse {
    pub operand: u16,
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DistinctUseDefTie {
    pub block: SelectedBlockId,
    pub position: LivenessPosition,
    pub instruction: SelectedInstructionId,
    pub use_operand: u16,
    pub use_virtual_register: VirtualRegisterId,
    pub use_point: LiveRangePoint,
    pub def_operand: u16,
    pub def_virtual_register: VirtualRegisterId,
    pub def_point: LiveRangePoint,
    pub class: RegisterClassId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockPointDomain {
    pub block: SelectedBlockId,
    pub source_block: BlockId,
    pub start: LiveRangePoint,
    pub end: LiveRangePoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualLiveRange {
    pub virtual_register: VirtualRegisterId,
    pub class: RegisterClassId,
    pub occurrences: Vec<VirtualOccurrence>,
    pub fixed_constraints: Vec<VirtualFixedConstraint>,
    pub fragments: Vec<LiveRangeFragment>,
    pub edge_connectors: Vec<LiveRangeEdgeConnector>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualOccurrence {
    pub position: LivenessPosition,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
    pub access: RegisterOperandAccess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualFixedConstraintSite {
    Entry,
    Operand {
        position: LivenessPosition,
        point: LiveRangePoint,
        instruction: SelectedInstructionId,
        operand: u16,
        access: RegisterOperandAccess,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualFixedConstraint {
    pub site: VirtualFixedConstraintSite,
    pub view: RegisterViewId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeFragment {
    pub block: SelectedBlockId,
    pub start: LiveRangePoint,
    pub end: LiveRangePoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveRangeEdgeConnector {
    pub source: SelectedBlockId,
    pub terminator: SelectedInstructionId,
    pub polarity_ordinal: u8,
    pub psi_edge: Option<EdgeId>,
    pub target: SelectedBlockId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchitecturalUnitLiveRange {
    pub unit: RegisterUnitId,
    pub actions: Vec<ArchitecturalUnitAction>,
    pub fragments: Vec<LiveRangeFragment>,
    pub edge_connectors: Vec<LiveRangeEdgeConnector>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchitecturalUnitAction {
    pub block: SelectedBlockId,
    pub position: LivenessPosition,
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub kind: ArchitecturalUnitActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ArchitecturalUnitActionKind {
    Use,
    Def,
    Clobber,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtualInterference {
    pub lower: VirtualRegisterId,
    pub higher: VirtualRegisterId,
}
