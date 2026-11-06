//! Prepare the known parent relation once before comparing published loans.
//! This index supplies no permission: callers provide their already checked
//! occurrence graph. Missing edges prove nothing; cycles never grant ancestry.

pub struct ReferenceAncestryIndex<I> {
    identities: Vec<I>,
    parents: Vec<Vec<usize>>,
    ancestors: Vec<Option<Vec<u64>>>,
    cyclic: Vec<bool>,
}

impl<I: Ord + Clone> ReferenceAncestryIndex<I> {
    pub fn new(rows: impl IntoIterator<Item = (I, Vec<I>)>) -> Self {
        let mut rows = rows.into_iter().collect::<Vec<_>>();
        for (_, parents) in &mut rows {
            parents.sort();
            parents.dedup();
        }
        rows.sort_by(|left, right| left.0.cmp(&right.0));
        let mut unique: Vec<(I, Vec<I>, bool)> = Vec::with_capacity(rows.len());
        for (identity, parents) in rows {
            if let Some(previous) = unique.last_mut()
                && previous.0 == identity
            {
                previous.2 |= previous.1 != parents;
            } else {
                unique.push((identity, parents, false));
            }
        }
        let identities = unique.iter().map(|row| row.0.clone()).collect::<Vec<_>>();
        let parents = unique
            .iter()
            .map(|(_, parents, _)| {
                parents
                    .iter()
                    .filter_map(|parent| identities.binary_search(parent).ok())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut cyclic = unique.iter().map(|row| row.2).collect::<Vec<_>>();
        let mut states = vec![0u8; identities.len()];
        let ancestors = vec![None; identities.len()];
        for start in 0..identities.len() {
            if states[start] == 2 {
                continue;
            }
            let mut pending = vec![(start, 0usize)];
            states[start] = 1;
            while let Some((index, next)) = pending.last_mut() {
                let index = *index;
                if let Some(&parent) = parents[index].get(*next) {
                    *next += 1;
                    match states[parent] {
                        0 => {
                            states[parent] = 1;
                            pending.push((parent, 0));
                        }
                        1 => cyclic[index] = true,
                        _ => {}
                    }
                    continue;
                }
                cyclic[index] |= parents[index]
                    .iter()
                    .any(|parent| states[*parent] != 2 || cyclic[*parent]);
                states[index] = 2;
                pending.pop();
            }
        }
        Self {
            identities,
            parents,
            ancestors,
            cyclic,
        }
    }

    pub fn is_ancestor(&mut self, ancestor: &I, child: &I) -> bool {
        let (Ok(ancestor), Ok(child)) = (
            self.identities.binary_search(ancestor),
            self.identities.binary_search(child),
        ) else {
            return false;
        };
        if ancestor == child || self.cyclic[child] {
            return false;
        }
        let words = self.identities.len().div_ceil(64);
        let row = self.ancestors[child].get_or_insert_with(|| {
            let mut bits = vec![0u64; words];
            let mut pending = self.parents[child].clone();
            while let Some(parent) = pending.pop() {
                let mask = 1u64 << (parent % 64);
                if bits[parent / 64] & mask != 0 {
                    continue;
                }
                bits[parent / 64] |= mask;
                pending.extend_from_slice(&self.parents[parent]);
            }
            bits
        });
        row[ancestor / 64] & (1u64 << (ancestor % 64)) != 0
    }
}
