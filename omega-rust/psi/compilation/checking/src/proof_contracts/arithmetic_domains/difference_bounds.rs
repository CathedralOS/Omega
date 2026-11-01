//! Indexed storage for the difference graph; arithmetic stays in its owner.
//!
//! Stable row indices avoid repeated string-key tree walks during closure.
//! A separate lexical inventory retains the old graph's iteration order,
//! including inconsistent closure. Sparse rows allocate only recorded edges.
//! Names, ordering and rows clone/restore as one value for speculative updates.

use numerics::bignum::BigInt;

#[derive(Clone, Default)]
pub(crate) struct DifferenceBounds {
    names: Vec<String>,
    lexical_order: Vec<usize>,
    rows: Vec<Vec<(usize, BigInt)>>,
}

impl DifferenceBounds {
    pub(crate) fn atom_names(&self) -> impl Iterator<Item = &str> {
        self.lexical_order
            .iter()
            .map(|index| self.names[*index].as_str())
    }

    pub(crate) fn lexical_indices(&self) -> &[usize] {
        &self.lexical_order
    }

    pub(crate) fn position(&self, name: &str) -> Option<usize> {
        self.lexical_order
            .binary_search_by(|index| self.names[*index].as_str().cmp(name))
            .ok()
            .map(|position| self.lexical_order[position])
    }

    pub(crate) fn intern(&mut self, name: &str) -> usize {
        match self
            .lexical_order
            .binary_search_by(|index| self.names[*index].as_str().cmp(name))
        {
            Ok(position) => self.lexical_order[position],
            Err(position) => {
                let index = self.names.len();
                self.names.push(name.to_owned());
                self.rows.push(Vec::new());
                self.lexical_order.insert(position, index);
                index
            }
        }
    }

    pub(crate) fn stored_bound_indices(&self, from: usize, to: usize) -> Option<&BigInt> {
        let row = &self.rows[from];
        row.binary_search_by_key(&to, |(destination, _)| *destination)
            .ok()
            .map(|position| &row[position].1)
    }

    pub(crate) fn stored_targets(&self, from: usize) -> impl Iterator<Item = usize> + '_ {
        self.rows[from].iter().map(|(destination, _)| *destination)
    }

    pub(crate) fn record(&mut self, from: &str, to: &str, bound: BigInt) -> bool {
        // Both vertices existed after every old record attempt, even when a
        // stored bound did not improve or the seed was a negative diagonal.
        let from = self.intern(from);
        let to = self.intern(to);
        self.record_indices(from, to, bound)
    }

    pub(crate) fn record_indices(&mut self, from: usize, to: usize, bound: BigInt) -> bool {
        let row = &mut self.rows[from];
        match row.binary_search_by_key(&to, |(destination, _)| *destination) {
            Ok(position) => {
                if bound > row[position].1 {
                    row[position].1 = bound;
                    true
                } else {
                    false
                }
            }
            Err(position) => {
                row.insert(position, (to, bound));
                true
            }
        }
    }
    pub(crate) fn bound_indices(&self, from: usize, to: usize, zero_atom: &str) -> Option<BigInt> {
        if from == to {
            return Some(BigInt::zero());
        }
        if let Some(bound) = self.stored_bound_indices(from, to) {
            return Some(bound.clone());
        }
        let zero = self.position(zero_atom)?;
        if from == zero || to == zero {
            return None;
        }
        let through_zero = self.stored_bound_indices(from, zero)?;
        Some(through_zero.add(self.stored_bound_indices(zero, to)?))
    }

    pub(crate) fn close(&mut self, zero_atom: &str) -> bool {
        let mut unsatisfiable = false;
        let zero = self.position(zero_atom);
        let atoms: Vec<usize> = self
            .lexical_indices()
            .iter()
            .copied()
            .filter(|index| Some(*index) != zero)
            .collect();
        let mut lexical_position = vec![0; self.lexical_indices().len()];
        for (position, atom) in atoms.iter().enumerate() {
            lexical_position[*atom] = position;
        }
        let mut parents: Vec<usize> = (0..atoms.len()).collect();
        fn component_root(parents: &mut [usize], mut index: usize) -> usize {
            while parents[index] != index {
                parents[index] = parents[parents[index]];
                index = parents[index];
            }
            index
        }
        for from in &atoms {
            let mut destinations: Vec<usize> = self
                .stored_targets(*from)
                .filter(|index| Some(*index) != zero)
                .collect();
            destinations.sort_unstable_by_key(|index| lexical_position[*index]);
            for to in destinations {
                let left = component_root(&mut parents, lexical_position[*from]);
                let right = component_root(&mut parents, lexical_position[to]);
                if left != right {
                    parents[left] = right;
                }
            }
        }
        let mut components = vec![Vec::new(); atoms.len()];
        for (position, atom) in atoms.into_iter().enumerate() {
            components[component_root(&mut parents, position)].push(atom);
        }
        for mut members in components.into_iter().filter(|members| !members.is_empty()) {
            if let Some(zero) = zero {
                members.push(zero);
            }
            self.close_members(&members, &mut unsatisfiable);
        }
        unsatisfiable
    }

    fn close_members(&mut self, members: &[usize], unsatisfiable: &mut bool) {
        for via in members {
            for from in members {
                let Some(first) = self.stored_bound_indices(*from, *via).cloned() else {
                    continue;
                };
                for to in members {
                    let Some(second) = self.stored_bound_indices(*via, *to).cloned() else {
                        continue;
                    };
                    let combined = first.add(&second);
                    if from == to {
                        if !combined.is_negative() && !combined.is_zero() {
                            *unsatisfiable = true;
                        }
                        continue;
                    }
                    self.record_indices(*from, *to, combined);
                }
            }
        }
    }
}
