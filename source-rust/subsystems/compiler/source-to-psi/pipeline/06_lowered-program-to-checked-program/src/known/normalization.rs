//! A child proof keeps the same context and axioms and appends one assumption.
//! It can reuse the parent's simplified premise if its equations still match.
use super::{Fact, Proposition};
use std::ops::Range;

struct ResultRow {
    equations: Range<usize>,
    result: usize,
}

#[derive(Default)]
pub(crate) struct NormalizedPremises {
    assumption_count: usize,
    rows: Vec<Option<ResultRow>>,
    equation_facts: Vec<usize>,
}

pub(crate) struct BranchPreparation<'parent> {
    parent: Option<(&'parent [Fact<'parent>], &'parent NormalizedPremises)>,
    pub(crate) prepared: NormalizedPremises,
}

impl<'parent> BranchPreparation<'parent> {
    pub(crate) fn new(parent: Option<(&'parent [Fact<'parent>], &'parent NormalizedPremises)>) -> Self {
        Self {
            parent,
            prepared: NormalizedPremises::default(),
        }
    }

    pub(super) fn begin(&mut self, assumptions: usize, declared: usize) {
        self.prepared.assumption_count = assumptions;
        self.prepared.rows.resize_with(declared, || None);
    }

    fn parent_declaration(&self, index: usize, parent: &NormalizedPremises) -> Option<usize> {
        if index >= self.prepared.rows.len() {
            return None;
        }
        // The child appends exactly one assumption. Its conjuncts precede the
        // same axioms. Derived facts have no shared declaration position.
        if index < self.prepared.assumption_count {
            return (index < parent.assumption_count).then_some(index);
        }
        Some(parent.assumption_count + (index - self.prepared.assumption_count))
    }

    pub(super) fn inherited(
        &self,
        index: usize,
        facts: &[Fact],
        equations: &[usize],
        selected: &[usize],
    ) -> Option<&'parent Proposition> {
        let (parent, prepared) = self.parent?;
        let old = self.parent_declaration(index, prepared)?;
        let row = prepared.rows.get(old)?.as_ref()?;
        let previous = &prepared.equation_facts[row.equations.clone()];
        if previous.len() != selected.len() {
            return None;
        }
        let same_equations = previous.iter().zip(selected).all(|(&old, &i)| {
            let current = equations[i];
            self.parent_declaration(current, prepared) == Some(old)
                || parent[old].proposition() == facts[current].proposition()
        });
        same_equations.then(|| parent[row.result].proposition())
    }

    pub(super) fn record(
        &mut self,
        index: usize,
        result: usize,
        equations: &[usize],
        selected: &[usize],
    ) {
        let Some(slot @ None) = self.prepared.rows.get_mut(index) else {
            return;
        };
        let inputs = &mut self.prepared.equation_facts;
        let start = inputs.len();
        inputs.extend(selected.iter().map(|&i| equations[i]));
        *slot = Some(ResultRow {
            equations: start..inputs.len(),
            result,
        });
    }
}
