//! Resolve a bounded runtime array selection against sparsely stored elements.
use super::{Segment, Stored};
impl Stored {
    /// A fixed range in a selected location denotes one unknown index. Visit
    /// every explicitly written element and the shared default at most once.
    fn index_keys(&self, end: u64) -> Vec<Segment> {
        let mut keys = match self {
            Self::Aggregate(array) => array
                .fields
                .iter()
                .filter_map(|(key, _)| match key {
                    Segment::FixedIndex(i) if *i < end => Some(key.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            _ => Vec::new(),
        };
        if (keys.len() as u128) < u128::from(end) {
            keys.push(Segment::AllElements);
        }
        keys
    }

    pub(in super::super) fn read(&self, path: &[Segment]) -> Self {
        let Some(position) = path
            .iter()
            .position(|part| matches!(part, Segment::FixedRange { start: 0, .. }))
        else {
            return self.project(path).clone();
        };
        let array = self.project(&path[..position]);
        let Segment::FixedRange { end, .. } = path[position] else {
            unreachable!()
        };
        let tail = &path[position + 1..];
        let mut values = array
            .index_keys(end)
            .into_iter()
            .map(|key| array.field(&key).read(tail));
        let mut result = values.next().unwrap_or_default();
        for other in values {
            result.join(&other);
        }
        result
    }

    pub(in super::super) fn write(&mut self, path: &[Segment], new: &Self, weak: bool) {
        let Some(position) = path
            .iter()
            .position(|part| matches!(part, Segment::FixedRange { start: 0, .. }))
        else {
            let old = self.project_mut(path);
            if weak {
                old.join(new);
            } else {
                *old = new.clone();
            }
            return;
        };
        let array = self.project_mut(&path[..position]);
        let Segment::FixedRange { end, .. } = path[position] else {
            unreachable!()
        };
        for key in array.index_keys(end) {
            array
                .field_mut(&key)
                .write(&path[position + 1..], new, true);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_values::TerminalScalarValue as Scalar;
    use crate::synchronization::progress::ScalarFact;
    use semantic_vocabulary::ScalarType;

    #[test]
    fn a_sparse_array_preserves_its_unwritten_elements_after_an_unknown_selection() {
        let mut array = Stored::ZERO;
        let held = Stored::Scalar(ScalarFact::Literal(Scalar::Boolean(true)));
        let element = Segment::FixedIndex(19);
        array.write(std::slice::from_ref(&element), &held, false);
        let all = Segment::FixedRange {
            start: 0,
            end: u64::MAX,
        };
        let values = |stored: Stored| {
            let mut bits = Vec::new();
            stored
                .visit_scalars(ScalarType::Boolean, |v| bits.push(v.literal().unwrap()))
                .unwrap();
            (
                bits.contains(&Scalar::Boolean(false)),
                bits.contains(&Scalar::Boolean(true)),
            )
        };
        assert_eq!(values(array.read(std::slice::from_ref(&all))), (true, true));
        array.write(&[all], &held, false);
        assert_eq!(values(array.read(&[element])), (false, true));
        assert_eq!(values(array.read(&[Segment::FixedIndex(20)])), (true, true));
    }
}
