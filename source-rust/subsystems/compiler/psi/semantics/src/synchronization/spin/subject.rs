//! A waiting cell is a projection of one borrowed argument. Its indices must
//! retain the same meanings across retries, calls, and independent artifacts.
use super::{Facts, Operand};
use semantic_vocabulary::{PlaceId, ValueId};
use terminal_psi::{
    StructuralPathSegment as Segment, StructuralTypeShape, TerminalMachine, TerminalModule,
};

pub(in super::super) fn fixed_extent(
    module: &TerminalModule,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[Segment],
) -> Option<u64> {
    let tip = crate::runtime_structural_path_tip(module.types.structural_types.iter(), root, path)?;
    let StructuralTypeShape::FixedArray { length, .. } = &tip.shape else {
        return None;
    };
    length.literal()
}

/// Every element of a nonempty fixed array has the same type. This path is
/// used only for walking declarations. Storage always uses the actual selector.
pub(in super::super) fn type_path(path: &[Segment]) -> Vec<Segment> {
    path.iter()
        .map(|part| match part {
            Segment::FixedRange { start: 0, .. } => Segment::FixedIndex(0),
            part => part.clone(),
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in super::super) struct Subject {
    pub(in super::super) parameter: usize,
    pub(in super::super) path: Vec<Segment>,
    /// Each FixedRange denotes one chosen element. The corresponding operand
    /// is in signature coordinates and never contains a body-local ValueId.
    indices: Vec<Operand>,
}

impl Subject {
    pub(in super::super) fn from_fixed(parameter: usize, path: &[Segment]) -> Option<Self> {
        path.iter()
            .all(|part| matches!(part, Segment::FixedIndex(_) | Segment::Field(_)))
            .then(|| Self {
                parameter,
                path: path.to_vec(),
                indices: Vec::new(),
            })
    }

    pub(in super::super) fn fixed_path(&self) -> Option<Vec<Segment>> {
        self.indices.is_empty().then(|| self.path.clone())
    }

    pub(in super::super) fn indices(&self) -> impl Iterator<Item = Operand> + '_ {
        self.indices.iter().copied()
    }

    fn index(operand: Operand, extent: u64) -> Option<(Segment, Option<Operand>)> {
        if extent == 0 {
            return None;
        }
        match operand.literal() {
            Some(value) => {
                let index = crate::synchronization::progress::ScalarFact::Literal(value).index()?;
                (index < extent).then_some((Segment::FixedIndex(index), None))
            }
            None => Some((
                Segment::FixedRange {
                    start: 0,
                    end: extent,
                },
                Some(operand),
            )),
        }
    }

    pub(super) fn forwarded(
        &mut self,
        parent: &Self,
        remap: impl Fn(Operand) -> Option<Operand>,
    ) -> Option<()> {
        let mut result = parent.clone();
        let mut indices = self.indices.iter().copied();
        for part in &self.path {
            if let Segment::FixedRange { start: 0, end } = part {
                let (part, operand) = Self::index(remap(indices.next()?)?, *end)?;
                result.path.push(part);
                result.indices.extend(operand);
            } else {
                result.path.push(part.clone());
            }
        }
        *self = result;
        Some(())
    }
}

#[derive(Default)]
pub(super) struct Subjects {
    pub(super) places: Facts<PlaceId, Subject>,
    pub(super) borrows: Facts<PlaceId, Subject>,
}

impl Subjects {
    pub(super) fn at_entry(machine: &TerminalMachine) -> Self {
        let mut state = Self::default();
        for (parameter, formal) in machine.structural_parameters.iter().enumerate() {
            // A borrowed machine formal already denotes the caller's referent.
            // Its first Field projects that storage; a Referent would follow a
            // pointer stored inside it.
            if !matches!(formal.access, terminal_psi::reading_borrow_pair_pattern!()) {
                continue;
            }
            state.places.insert(
                formal.place,
                Subject {
                    parameter,
                    path: Vec::new(),
                    indices: Vec::new(),
                },
            );
        }
        state
    }

    pub(super) fn subject(
        &self,
        module: &TerminalModule,
        machine: &TerminalMachine,
        place: PlaceId,
        path: &[Segment],
        scalar: impl Fn(ValueId) -> Option<Operand>,
    ) -> Option<Subject> {
        let (mut subject, path) = if let Some(borrow) = self.borrows.get(&place) {
            let [Segment::Referent, rest @ ..] = path else {
                return None;
            };
            (borrow.clone(), rest)
        } else {
            (self.places.get(&place)?.clone(), path)
        };
        if let Some(fixed) = Subject::from_fixed(subject.parameter, path) {
            subject.path.extend(fixed.path);
            return Some(subject);
        }
        let root = machine
            .structural_parameters
            .get(subject.parameter)?
            .structural_type;
        for segment in path {
            let part = match segment {
                Segment::Field(_) | Segment::FixedIndex(_) => segment.clone(),
                Segment::RuntimeIndex { index, .. } => {
                    let extent = fixed_extent(module, root, &type_path(&subject.path))?;
                    let (part, selector) = Subject::index(scalar(*index)?, extent)?;
                    subject.indices.extend(selector);
                    part
                }
                _ => return None,
            };
            subject.path.push(part);
        }
        Some(subject)
    }
}
