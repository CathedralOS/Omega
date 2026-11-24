//! Read integer storage on arrival at a join when branches may have written
//! different values. Each arm passes its observed value to an ordinary
//! block parameter. The checker can then establish a bound across the
//! arrivals while checking each read against its own reaching store.
//! See `wiki/spec/terminal-psi/fact_validity.md`, “Premise availability.”

use arena::NumericId;
use semantic_vocabulary::{BlockId, EdgeId, PlaceId, ScalarType, ValueId};
use terminal_check_tables::bindings::PlaceOrigin;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{Checked, ExactPlaceId, ProgramPoint};
use terminal_psi::{OperationKind, Terminator};

use super::Lowering;
use super::places::Step;
use crate::body::Qualification;
use crate::{SourceOrigin, Unpositioned};

/// Only roots and selectors already in scope before the split can be
/// observed on all arrivals. Later writes tell us which places may differ.
pub(super) struct BeforeArms {
    roots: usize,
    values: usize,
    writes: usize,
}

impl BeforeArms {
    pub(super) fn capture(lowering: &Lowering<'_>) -> Self {
        Self {
            roots: lowering.roots.len(),
            values: lowering.tables.values.len(),
            writes: lowering.tables.writes.len(),
        }
    }
}

pub(super) struct JoinReads {
    block: BlockId,
    before: BeforeArms,
    arrivals: Vec<BlockId>,
    split: bool,
}

impl JoinReads {
    pub(super) fn new(
        block: BlockId,
        before: BeforeArms,
        writes_after: usize,
        arrivals: impl ExactSizeIterator<Item = BlockId>,
    ) -> Option<Self> {
        (arrivals.len() > 1 && writes_after > before.writes).then(|| Self {
            block,
            before,
            arrivals: arrivals.collect(),
            split: false,
        })
    }
}

impl Lowering<'_> {
    pub(super) fn incoming_read(
        &mut self,
        root: PlaceId,
        mut leaf: ExactPlaceId,
        steps: &[Step],
        (carrier, qualification): (ScalarType, Qualification),
        source: SourceOrigin,
    ) -> Checked<Option<ValueId>> {
        let Some(join) = &self.join_reads else {
            return Ok(None);
        };
        if !matches!(carrier, ScalarType::Integer(_))
            || self.current != Some(join.block)
            || root.get() > join.before.roots as u64
        {
            return Ok(None);
        }
        // The original observation has already captured its selectors. A
        // runtime selector evaluated at the join does not dominate its arms.
        if steps.iter().any(|step| matches!(
            step,
            Step::Index { selector, fixed: None, .. } if selector.get() > join.before.values as u64
        )) {
            return Ok(None);
        }
        let writes_before = join.before.writes;
        // An incoming owned storage argument may consume the place being
        // read. Preserve its transfer and any preceding effects in order.
        // Constants can precede the read (as in `1 + stock.value`).
        let block = self.draft()?;
        if !block.structural_parameters.is_empty()
            || block.operations.iter().any(|operation| {
                !matches!(
                    operation.kind,
                    OperationKind::IntegerConstant { .. }
                        | OperationKind::BooleanConstant { .. }
                        | OperationKind::IeeeFloatConstant { .. }
                )
            })
        {
            self.join_reads = None;
            return Ok(None);
        }
        loop {
            let Some(place) = self.tables.places.get(leaf) else {
                return Err(self.internal("a join read names no place"));
            };
            if place
                .writes
                .last()
                .is_some_and(|write| write.get() > writes_before as u64)
            {
                break;
            }
            match place.origin {
                PlaceOrigin::Root(_) => return Ok(None),
                PlaceOrigin::Projection { parent, .. } => leaf = parent,
            }
        }

        let mut join = self
            .join_reads
            .take()
            .expect("the current join was checked");
        for arrival in &mut join.arrivals {
            self.resume_block(*arrival)?;
            let Some(mut transfer) = self.draft()?.terminator.take() else {
                return Err(self.internal("a match arrival has no transfer"));
            };
            let Terminator::Jump {
                edge,
                target,
                arguments,
                ..
            } = &mut transfer
            else {
                return Err(self.internal("a match arrival is not a jump"));
            };
            if *target != join.block {
                return Err(self.internal("a match arrival has another destination"));
            }
            if !join.split {
                *arrival = self.split_read_arrival(*edge)?;
            }
            let actual = self.primitive_read(root, steps, carrier, source)?;
            self.qualify(actual, qualification)?;
            arguments.push(actual);
            self.terminate(transfer)?;
        }
        join.split = true;
        self.resume_block(join.block)?;
        let position = self.draft()?.parameters.len();
        let value = self.value(
            carrier,
            ValueOrigin::BlockParameter {
                block: join.block,
                position: self.position(position)?,
            },
        );
        self.parameter(join.block, value, carrier)?;
        self.join_reads = Some(join);
        Ok(Some(value))
    }

    fn resume_block(&mut self, block: BlockId) -> Checked<()> {
        self.enter(block);
        if let Some(operation) = self.draft()?.operations.last() {
            self.point = Some(ProgramPoint::AfterOperation(operation.id));
        }
        Ok(())
    }

    /// End arm-local loans before observing storage still visible at the
    /// join. The existing jump keeps its scalar and structural arguments.
    fn split_read_arrival(&mut self, outgoing: EdgeId) -> Checked<BlockId> {
        let source = SourceOrigin::Unpositioned(Unpositioned::MatchJoin);
        let observed = self.block(source);
        let Some(edge) = self.tables.edges.get_mut(outgoing) else {
            return Err(self.internal("a match arrival has no edge row"));
        };
        edge.source = observed;
        self.jump(observed, Vec::new(), source)?;
        self.enter(observed);
        Ok(observed)
    }
}
