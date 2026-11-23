use super::{
    ComputedRankError, error,
    numbers::{self, Number},
};
use num_bigint::BigInt;
use omega_psi::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Comparison {
    left: ValueId,
    right: ValueId,
    op: PredicateOperator,
}

#[derive(Clone, Default)]
pub(super) struct State {
    pub numbers: BTreeMap<ValueId, Number>,
    guards: BTreeMap<ValueId, Comparison>,
}
impl State {
    pub fn merge(
        &mut self,
        other: &Self,
    ) {
        self.numbers.retain(|id, n| {
            other.numbers.get(id).is_some_and(|b| {
                n.merge(b);
                true
            })
        });
        self.guards.retain(|id, g| other.guards.get(id) == Some(g));
    }
    pub fn branch(
        &mut self,
        id: ValueId,
        positive: bool,
    ) {
        let Some(g) = self.guards.get(&id) else { return };
        let (Some(left), Some(right)) = (self.numbers.get(&g.left).cloned(), self.numbers.get(&g.right).cloned()) else {
            return;
        };
        let cmp = if positive {
            g.op
        } else {
            match g.op {
                PredicateOperator::LessThan => PredicateOperator::GreaterOrEqual,
                PredicateOperator::LessOrEqual => PredicateOperator::GreaterThan,
                PredicateOperator::GreaterThan => PredicateOperator::LessOrEqual,
                PredicateOperator::GreaterOrEqual => PredicateOperator::LessThan,
                PredicateOperator::Equal => PredicateOperator::NotEqual,
                PredicateOperator::NotEqual => PredicateOperator::Equal,
                _ => return,
            }
        };
        if right.range.lo == right.range.hi {
            restrict(self.numbers.get_mut(&g.left).unwrap(), cmp, &right.range.lo);
        }
        if left.range.lo == left.range.hi {
            restrict(self.numbers.get_mut(&g.right).unwrap(), reverse(cmp), &left.range.lo);
        }
    }
    pub fn bind_edge(
        &mut self,
        body: &MachineBody,
        e: &SuccessorEdge,
        dest: &BranchBlock,
    ) -> Result<(), String> {
        if e.arguments.len() != dest.scalar_parameters.len() {
            return Err("wrong successor arity in rank proof".into());
        }
        let prior = self.clone();
        for (&arg, &id) in e.arguments.iter().zip(&dest.scalar_parameters) {
            let row = body.values.get(id.index()).ok_or("missing joined rank value")?;
            if row.origin != ValueOrigin::BlockParameter(dest.id) || body.values.get(arg.index()).is_none_or(|v| v.ty != row.ty) {
                return Err("invalid joined value in rank proof".into());
            }
            self.numbers.remove(&id);
            self.guards.remove(&id);
            if let Some(v) = prior.numbers.get(&arg) {
                self.numbers.insert(id, v.clone());
            }
            if let Some(g) = prior.guards.get(&arg) {
                self.guards.insert(id, g.clone());
            }
        }
        Ok(())
    }
    pub fn operation(
        &mut self,
        unit: &LoweredPackage,
        m: &LoweredMachine,
        op: &PsiOperation,
    ) -> Result<(), String> {
        let OperationResult::Scalar(id) = op.result else { return Ok(()) };
        let row = m
            .body
            .values
            .get(id.index())
            .ok_or("missing scalar operation result")?;
        if row.origin != ValueOrigin::Operation(op.id) {
            return Err("rank evidence names another value's definition".into());
        }
        let Some(TypeShape::PrimitiveScalar(ty)) = unit.types.get(row.ty) else {
            return Ok(());
        };
        match &op.kind {
            OperationKind::Constant(ScalarLiteral::Integer(n)) => {
                self.numbers.insert(id, Number::literal(*ty, n.clone())?);
            }
            OperationKind::Integer { operation, operands } => {
                if *ty == ScalarType::Boolean {
                    let [left, right] = operands.as_slice() else { return Ok(()) };
                    let (Some(a), Some(b)) = (self.numbers.get(left), self.numbers.get(right)) else {
                        return Ok(());
                    };
                    if a.ty != b.ty || operation.policy != ArithmeticPolicy::Exact || !operation.obligations.is_empty() {
                        return Ok(());
                    }
                    let op = match operation.primitive {
                        IntegerPrimitive::Equal => PredicateOperator::Equal,
                        IntegerPrimitive::NotEqual => PredicateOperator::NotEqual,
                        IntegerPrimitive::LessThan => PredicateOperator::LessThan,
                        IntegerPrimitive::GreaterThan => PredicateOperator::GreaterThan,
                        IntegerPrimitive::LessOrEqual => PredicateOperator::LessOrEqual,
                        IntegerPrimitive::GreaterOrEqual => PredicateOperator::GreaterOrEqual,
                        _ => return Ok(()),
                    };
                    self.guards.insert(
                        id,
                        Comparison {
                            left: *left,
                            right: *right,
                            op,
                        },
                    );
                } else {
                    let (a, b) = match operands.as_slice() {
                        [a] => (self.numbers.get(a), None),
                        [a, b] => {
                            let Some(b) = self.numbers.get(b) else { return Ok(()) };
                            (self.numbers.get(a), Some(b))
                        }
                        _ => return Ok(()),
                    };
                    // Only a proven equation for a dominating definition may become rank evidence.
                    // Unrelated operations keep their own safety obligations.
                    if let Some(a) = a
                        && a.ty == *ty
                        && let Ok(n) = numbers::apply(operation, a, b, &self.numbers)
                    {
                        self.numbers.insert(id, n);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn reverse(op: PredicateOperator) -> PredicateOperator {
    use PredicateOperator::*;
    match op {
        LessThan => GreaterThan,
        LessOrEqual => GreaterOrEqual,
        GreaterThan => LessThan,
        GreaterOrEqual => LessOrEqual,
        x => x,
    }
}

fn restrict(
    n: &mut Number,
    op: PredicateOperator,
    c: &BigInt,
) {
    use PredicateOperator::*;
    let mut r = n.range.clone();
    match op {
        LessThan => r.hi = r.hi.min(c - 1),
        LessOrEqual => r.hi = r.hi.min(c.clone()),
        GreaterThan => r.lo = r.lo.max(c + 1),
        GreaterOrEqual => r.lo = r.lo.max(c.clone()),
        Equal => {
            r.lo = r.lo.max(c.clone());
            r.hi = r.hi.min(c.clone());
        }
        NotEqual => {
            if &r.lo == c {
                r.lo += 1;
            }
            if &r.hi == c {
                r.hi -= 1;
            }
        }
        _ => return,
    }
    // An inconsistent branch/contract could be discharged separately. Keeping
    // the old approximation is conservative and never manufactures a fact.
    if r.lo <= r.hi {
        n.range = r;
    }
}

pub(super) fn entry(
    unit: &LoweredPackage,
    m: &LoweredMachine,
) -> Result<State, ComputedRankError> {
    let fail = |s: &str| error(&m.body.diagnostics.body, s);
    if m.parameter_types.len() != m.body.parameters.len() {
        return Err(fail("rank body's formal arity differs from its signature"));
    }
    let mut state = State::default();
    let mut ids = BTreeSet::new();
    for (arg, &ty) in m.body.parameters.iter().zip(&m.parameter_types) {
        let Parameter::Scalar(id) = *arg else { continue };
        let row = m
            .body
            .values
            .get(id.index())
            .ok_or_else(|| fail("missing entry formal for rank"))?;
        if row.origin != ValueOrigin::Parameter || row.ty != ty || !ids.insert(id) {
            return Err(fail("invalid rank entry formal"));
        }
        if let Some(TypeShape::PrimitiveScalar(ty @ ScalarType::Integer { .. })) = unit.types.get(ty)
            && let Ok(n) = Number::input(*ty, id)
        {
            state.numbers.insert(id, n);
        }
    }
    for clause in &m.contract.requires {
        // Extract conjunctions only. An unknown, negated or disjunctive clause
        // cannot contribute one of its operands as a standalone premise.
        enum Term<'a> {
            Formal(usize),
            Literal(&'a BigInt),
            Bounds(Vec<(usize, PredicateOperator, &'a BigInt)>),
        }
        let mut stack = Vec::new();
        for step in &clause.predicate {
            let term = match step {
                PredicateStep::Formal(i) => Term::Formal(*i),
                PredicateStep::Integer(n) => Term::Literal(n),
                PredicateStep::Apply(op) => {
                    let (Some(r), Some(l)) = (stack.pop(), stack.pop()) else {
                        stack.clear();
                        break;
                    };
                    match (l, r, op) {
                        (Term::Bounds(mut a), Term::Bounds(b), PredicateOperator::And) => {
                            a.extend(b);
                            Term::Bounds(a)
                        }
                        (Term::Formal(i), Term::Literal(c), op) | (Term::Literal(c), Term::Formal(i), op)
                            if matches!(op, PredicateOperator::Equal | PredicateOperator::NotEqual) =>
                        {
                            Term::Bounds(vec![(i, *op, c)])
                        }
                        (Term::Formal(i), Term::Literal(c), op) if ordered(*op) => Term::Bounds(vec![(i, *op, c)]),
                        (Term::Literal(c), Term::Formal(i), op) if ordered(*op) => Term::Bounds(vec![(i, reverse(*op), c)]),
                        _ => {
                            stack.clear();
                            break;
                        }
                    }
                }
                _ => {
                    stack.clear();
                    break;
                }
            };
            stack.push(term);
        }
        if let [Term::Bounds(bounds)] = stack.as_slice() {
            for &(i, op, c) in bounds {
                if let Some(Parameter::Scalar(id)) = m.body.parameters.get(i)
                    && let Some(n) = state.numbers.get_mut(id)
                {
                    restrict(n, op, c);
                }
            }
        }
    }
    Ok(state)
}
fn ordered(op: PredicateOperator) -> bool {
    matches!(
        op,
        PredicateOperator::LessThan | PredicateOperator::GreaterThan | PredicateOperator::LessOrEqual | PredicateOperator::GreaterOrEqual
    )
}
