//! Facts about the final contents of borrowed scalar parameters. Only the
//! lowered-form checker can construct one, after checking every normal return.
mod candidates;

use super::{Bindings, PreparedMachine, ProofSearchActivity, decide, hypotheses, prove::Prover};
use semantic_vocabulary::{
    ContentPlaceVersion, EdgeId, IntegerValue, ObligationId, Proposition, ScalarTerm, ScalarType,
    TargetMachineCallable, ValueId,
};
use terminal_check_tables::{Checked, Construct, MachineTables, ProgramPoint};
use terminal_psi::{
    StructuralAccess, StructuralTypeShape, TerminalMachine, TerminalModule, Terminator,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReturnedOperand {
    Parameter(u32),
    Integer(IntegerValue),
    Boolean(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReturnedComparison {
    Equal,
    LessThan,
    LessOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ReturnedScalarFact {
    ty: ScalarType,
    operands: [ReturnedOperand; 2],
    comparison: ReturnedComparison,
}

impl ReturnedScalarFact {
    pub fn operands(&self) -> [ReturnedOperand; 2] {
        self.operands
    }
    pub fn comparison(&self) -> ReturnedComparison {
        self.comparison
    }

    fn proposition(
        &self,
        mut parameter: impl FnMut(u32) -> Option<ScalarTerm>,
    ) -> Option<Proposition> {
        let mut term = |operand| match operand {
            ReturnedOperand::Parameter(position) => parameter(position),
            ReturnedOperand::Integer(value) => match self.ty {
                ScalarType::Integer(ty) => ScalarTerm::integer(ty, value).ok(),
                _ => None,
            },
            ReturnedOperand::Boolean(value) => Some(ScalarTerm::Boolean(value)),
        };
        let [left, right] = self.operands;
        let (left, right) = (term(left)?, term(right)?);
        Some(match self.comparison {
            ReturnedComparison::Equal => Proposition::Equal(left, right),
            ReturnedComparison::LessThan => Proposition::LessThan(left, right),
            ReturnedComparison::LessOrEqual => Proposition::LessOrEqual(left, right),
        })
    }

    /// The caller matched the frame's declaration identities. Bind each
    /// operand to the corresponding storage in the receiving module.
    pub fn on_entry(&self, module: &TerminalModule) -> Option<Proposition> {
        let Some(TargetMachineCallable::Machine(entry)) = module.entry else {
            return None;
        };
        let parameters = &module.machine(entry)?.structural_parameters;
        let proposition = self.proposition(|position| {
            let parameter = parameters.get(position as usize)?;
            if parameter.position != position
                || parameter.access != StructuralAccess::MutableBorrow
                || module.structural_type(parameter.structural_type)?.shape
                    != StructuralTypeShape::PrimitiveScalar(self.ty)
            {
                return None;
            }
            match self.ty {
                ScalarType::Boolean => {
                    Some(ScalarTerm::boolean_field_path(parameter.place, vec![]))
                }
                ScalarType::Integer(ty) => {
                    Some(ScalarTerm::integer_field_path(parameter.place, vec![], ty))
                }
                _ => None,
            }
        })?;
        super::entry_inputs::canonical(proposition, entry).ok()
    }
}

/// A cleanup after the inspected storage, or an external completion, would
/// require its own account of the returned contents.
fn normal_returns(machine: &TerminalMachine) -> Vec<EdgeId> {
    let mut edges = Vec::new();
    for block in &machine.blocks {
        match &block.terminator {
            Terminator::ReturnUnit {
                edge,
                cleanup_actions,
            }
            | Terminator::Return {
                edge,
                cleanup_actions,
                ..
            }
            | Terminator::ReturnStructural {
                edge,
                cleanup_actions,
                ..
            } => {
                if !cleanup_actions.is_empty() {
                    return Vec::new();
                }
                edges.push(*edge);
            }
            Terminator::ExternalCompletion { .. } => return Vec::new(),
            Terminator::Jump { .. }
            | Terminator::Conditional { .. }
            | Terminator::StructuralCase { .. }
            | Terminator::Crash { .. }
            | Terminator::InvokeNoNormal { .. }
            | Terminator::EliminateEmpty { .. } => {}
        }
    }
    edges
}

pub(super) fn prove(
    machine: &mut PreparedMachine<'_, '_>,
    tables: &MachineTables,
    unreached: &[ValueId],
    activity: &mut ProofSearchActivity,
) -> Checked<Vec<ReturnedScalarFact>> {
    if machine.module.entry != Some(TargetMachineCallable::Machine(machine.id))
        || machine.module.identities.invocation_scope().is_none()
    {
        return Ok(Vec::new());
    }
    let homes: Vec<_> = machine
        .lowered
        .structural_parameters
        .iter()
        .filter_map(|parameter| {
            let StructuralTypeShape::PrimitiveScalar(
                ty @ (ScalarType::Integer(_) | ScalarType::Boolean),
            ) = machine
                .module
                .structural_type(parameter.structural_type)?
                .shape
            else {
                return None;
            };
            (parameter.access == StructuralAccess::MutableBorrow).then_some((parameter, ty))
        })
        .collect();
    if homes.is_empty() {
        return Ok(Vec::new());
    }
    let edges = normal_returns(machine.lowered);
    if edges.is_empty() {
        return Ok(Vec::new());
    }

    let obligation = ObligationId::new(tables.obligations.len() as u64 + 1)
        .expect("one more checking obligation");
    let mut returns = Vec::with_capacity(edges.len());
    let mut candidates = Vec::new();
    let mut bounds = vec![candidates::Bounds::default(); homes.len()];
    for edge in edges {
        let point = ProgramPoint::Edge(edge);
        let mut storage =
            machine
                .storage
                .at_return(machine.references, tables, edge, Construct::Machine)?;
        let mut contents = vec![None; machine.lowered.parameter_order.len()];
        let known = hypotheses(machine.graph, tables, machine.id, obligation, point)?;
        let mut prover = Prover::new(
            machine.context,
            &known.assumptions,
            &known.axioms,
            &tables.values,
            activity,
        );
        for ((parameter, ty), bounds) in homes.iter().zip(&mut bounds) {
            let value = storage
                .field(ContentPlaceVersion::Current, parameter.place, &[], *ty)
                .ok();
            if let ScalarType::Integer(integer) = *ty {
                bounds.include(integer, value.as_ref(), &mut prover);
            }
            let Some(slot) = contents.get_mut(parameter.position as usize) else {
                return Ok(Vec::new());
            };
            *slot = value;
        }
        candidates::comparisons(&mut candidates, &contents, &known, machine.context, tables);
        returns.push((point, contents));
    }
    for ((parameter, ty), limits) in homes.into_iter().zip(bounds) {
        limits.propose(&mut candidates, parameter.position, ty);
    }

    if returns.len() > 1 {
        // A strict comparison or equality at one exit may be non-strict at another.
        let count = candidates.len();
        for i in 0..count {
            let fact = candidates[i];
            if matches!(fact.ty, ScalarType::Integer(_))
                && fact
                    .operands
                    .iter()
                    .all(|op| matches!(op, ReturnedOperand::Parameter(_)))
            {
                if matches!(
                    fact.comparison,
                    ReturnedComparison::Equal | ReturnedComparison::LessThan
                ) {
                    candidates.push(ReturnedScalarFact {
                        comparison: ReturnedComparison::LessOrEqual,
                        ..fact
                    });
                }
                if fact.comparison == ReturnedComparison::Equal {
                    let [a, b] = fact.operands;
                    candidates.push(ReturnedScalarFact {
                        operands: [b, a],
                        comparison: ReturnedComparison::LessOrEqual,
                        ..fact
                    });
                }
            }
        }
    }
    candidates.sort_unstable_by_key(|fact| (fact.comparison, fact.ty, fact.operands));
    candidates.dedup();
    let mut exact = vec![None; machine.lowered.parameter_order.len()];
    candidates.retain(|fact| {
        let literal = match fact.operands {
            [ReturnedOperand::Parameter(p), ReturnedOperand::Integer(n)]
            | [ReturnedOperand::Integer(n), ReturnedOperand::Parameter(p)] => Some((p as usize, n)),
            _ => None,
        };
        if literal.is_some_and(|(p, n)| exact[p] == Some(n)) {
            return false;
        }
        let proved = returns.iter().all(|(point, contents)| {
            fact.proposition(|position| contents.get(position as usize)?.clone())
                .is_some_and(|goal| {
                    decide(
                        machine.context,
                        machine.graph,
                        tables,
                        machine.id,
                        obligation,
                        *point,
                        goal,
                        unreached,
                        activity,
                    )
                    .is_ok()
                })
        });
        if proved
            && fact.comparison == ReturnedComparison::Equal
            && let Some((p, n)) = literal
        {
            exact[p] = Some(n);
        }
        proved
    });
    Ok(candidates)
}
