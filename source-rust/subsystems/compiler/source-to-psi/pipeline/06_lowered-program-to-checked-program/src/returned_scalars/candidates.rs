//! Describe returned storage using only names available in the next program.
//! Keep the weakest integer bounds across exits and compare homes that share
//! a definition or occur together in a known comparison.
use super::{
    ReturnedComparison as Comparison, ReturnedOperand as Operand, ReturnedScalarFact as Fact,
};
use crate::checking::{
    Hypotheses,
    known::{Known, TransportInputs},
    prove::Prover,
    result_summaries::widest,
};
use semantic_vocabulary::{
    IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm, ScalarType,
};
use std::cmp::Ordering;
use terminal_check_tables::MachineTables;

#[derive(Clone)]
pub(super) struct Bounds {
    low: Option<Option<IntegerValue>>,
    high: Option<Option<IntegerValue>>,
}
impl Default for Bounds {
    fn default() -> Self {
        Self {
            low: Some(None),
            high: Some(None),
        }
    }
}
impl Bounds {
    pub(super) fn include(
        &mut self,
        ty: IntegerType,
        value: Option<&ScalarTerm>,
        prover: &mut Prover<'_>,
    ) {
        let (low, high) = match value {
            Some(ScalarTerm::Integer { value, .. }) => (Some(*value), Some(*value)),
            Some(ScalarTerm::Value { id, .. }) => prover.literal_bounds(*id),
            _ => (None, None),
        };
        self.low = widest(
            self.low,
            low.or_else(|| ty.minimum_value()),
            ty,
            Ordering::Less,
        );
        self.high = widest(
            self.high,
            high.or_else(|| ty.maximum_value()),
            ty,
            Ordering::Greater,
        );
    }

    pub(super) fn propose(self, candidates: &mut Vec<Fact>, position: u32, ty: ScalarType) {
        let home = Operand::Parameter(position);
        let mut push = |left, comparison, right| {
            candidates.push(Fact {
                ty,
                operands: [left, right],
                comparison,
            })
        };
        match ty {
            ScalarType::Integer(integer) => {
                let (low, high) = (self.low.flatten(), self.high.flatten());
                if let Some(n) = low
                    && high == Some(n)
                {
                    push(home, Comparison::Equal, Operand::Integer(n));
                }
                if let Some(n) = low
                    && low != integer.minimum_value()
                {
                    push(Operand::Integer(n), Comparison::LessOrEqual, home);
                }
                if let Some(n) = high
                    && high != integer.maximum_value()
                {
                    push(home, Comparison::LessOrEqual, Operand::Integer(n));
                }
            }
            ScalarType::Boolean => {
                for value in [true, false] {
                    push(home, Comparison::Equal, Operand::Boolean(value));
                }
            }
            _ => {}
        }
    }
}

pub(super) fn comparisons(
    candidates: &mut Vec<Fact>,
    contents: &[Option<ScalarTerm>],
    hypotheses: &Hypotheses,
    context: &PropositionContext,
    tables: &MachineTables,
) {
    if contents.iter().flatten().take(2).count() < 2 {
        return;
    }
    let known = Known::new(
        context,
        &hypotheses.assumptions,
        &hypotheses.axioms,
        &tables.values,
    );
    let mut transport = TransportInputs::new(known.equations.len());
    let mut denotation = |term: &ScalarTerm| match term {
        ScalarTerm::Value { id, .. } => {
            known.expand_value(context, *id, term.scalar_type(), &mut transport)
        }
        other => other.clone(),
    };
    let mut names = Vec::new();
    for (position, term) in contents.iter().enumerate() {
        if let Some(term) = term {
            let position = position as u32;
            names.push((term.clone(), position));
            let expanded = denotation(term);
            if expanded != *term {
                names.push((expanded, position));
            }
        }
    }
    names.sort_unstable();
    let positions = |term: &ScalarTerm| {
        let start = names.partition_point(|(name, _)| name < term);
        let count = names[start..].partition_point(|(name, _)| name == term);
        &names[start..start + count]
    };
    let mut compare = |ty, a: u32, comparison, b: u32| {
        if a == b {
            return;
        }
        let operands = if comparison == Comparison::Equal && a > b {
            [b, a]
        } else {
            [a, b]
        };
        candidates.push(Fact {
            ty,
            operands: operands.map(Operand::Parameter),
            comparison,
        });
    };
    for group in names.chunk_by(|(a, _), (b, _)| a == b) {
        let term = &group[0].0;
        if matches!(term, ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_)) {
            continue;
        }
        for (i, (_, a)) in group.iter().enumerate() {
            for (_, b) in &group[i + 1..] {
                compare(term.scalar_type(), *a, Comparison::Equal, *b);
            }
        }
    }
    for fact in &known.facts {
        let (left, comparison, right) = match &fact.proposition {
            Proposition::Equal(a, b) => (a, Comparison::Equal, b),
            Proposition::LessThan(a, b) => (a, Comparison::LessThan, b),
            Proposition::LessOrEqual(a, b) => (a, Comparison::LessOrEqual, b),
            _ => continue,
        };
        if matches!(left, ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_))
            || matches!(right, ScalarTerm::Integer { .. } | ScalarTerm::Boolean(_))
        {
            continue;
        }
        let left = denotation(left);
        let right = denotation(right);
        if left == right {
            continue;
        }
        for (_, a) in positions(&left) {
            for (_, b) in positions(&right) {
                compare(left.scalar_type(), *a, comparison, *b);
            }
        }
    }
}
