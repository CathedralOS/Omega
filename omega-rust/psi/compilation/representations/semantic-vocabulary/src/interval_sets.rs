//! Canonical finite, disjoint half-open intervals over natural endpoints.

use bignum::BigInt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NaturalInterval {
    start: BigInt,
    end: BigInt,
}

impl NaturalInterval {
    pub fn new(start: BigInt, end: BigInt) -> Result<Self, IntervalSetError> {
        if start.is_negative() || end.is_negative() {
            return Err(IntervalSetError::NegativeBound);
        }
        if start > end {
            return Err(IntervalSetError::ReversedBounds);
        }
        Ok(Self { start, end })
    }

    pub const fn start(&self) -> &BigInt {
        &self.start
    }

    pub const fn end(&self) -> &BigInt {
        &self.end
    }

    fn is_empty(&self) -> bool {
        self.start == self.end
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CanonicalIntervalSet {
    members: Vec<NaturalInterval>,
}

impl CanonicalIntervalSet {
    pub fn new(
        members: impl IntoIterator<Item = NaturalInterval>,
    ) -> Result<Self, IntervalSetError> {
        let mut members = members
            .into_iter()
            .filter(|member| !member.is_empty())
            .collect::<Vec<_>>();
        members.sort();

        let mut normalized: Vec<NaturalInterval> = Vec::with_capacity(members.len());
        for member in members {
            let Some(previous) = normalized.last_mut() else {
                normalized.push(member);
                continue;
            };
            if member.start < previous.end {
                return Err(IntervalSetError::OverlappingMembers);
            }
            if member.start == previous.end {
                previous.end = member.end;
            } else {
                normalized.push(member);
            }
        }
        Ok(Self {
            members: normalized,
        })
    }

    pub fn singleton(start: BigInt, end: BigInt) -> Result<Self, IntervalSetError> {
        Self::new([NaturalInterval::new(start, end)?])
    }

    pub fn members(&self) -> &[NaturalInterval] {
        &self.members
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    pub fn separate<'a>(
        sets: impl IntoIterator<Item = &'a Self>,
    ) -> Result<Self, IntervalSetError> {
        Self::new(sets.into_iter().flat_map(|set| set.members.iter().cloned()))
    }

    pub fn contains(&self, kept: &Self) -> bool {
        let mut whole_index = 0;
        for member in &kept.members {
            while whole_index < self.members.len() && self.members[whole_index].end <= member.start
            {
                whole_index += 1;
            }
            let Some(whole) = self.members.get(whole_index) else {
                return false;
            };
            if member.start < whole.start || member.end > whole.end {
                return false;
            }
        }
        true
    }

    pub fn residual(&self, kept: &Self) -> Result<Self, IntervalSetError> {
        if !self.contains(kept) {
            return Err(IntervalSetError::NotContained);
        }
        let mut residual = Vec::new();
        for whole in &self.members {
            let mut cursor = whole.start.clone();
            for member in kept
                .members
                .iter()
                .filter(|member| member.start >= whole.start && member.end <= whole.end)
            {
                if cursor < member.start {
                    residual.push(NaturalInterval {
                        start: cursor,
                        end: member.start.clone(),
                    });
                }
                cursor = member.end.clone();
            }
            if cursor < whole.end {
                residual.push(NaturalInterval {
                    start: cursor,
                    end: whole.end.clone(),
                });
            }
        }
        Self::new(residual)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntervalSetError {
    NegativeBound,
    ReversedBounds,
    OverlappingMembers,
    NotContained,
}
