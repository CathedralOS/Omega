//! Bounds on the final contents of a mutable integer argument. The ordinary
//! return binding follows writes and loans. Candidate bounds become available
//! to the caller only after the kernel verifies them on every normal return.
use super::{Bindings, PreparedMachine, ProofSearchActivity, decide, hypotheses, prove::Prover};
use semantic_vocabulary::{
    ContentPlaceVersion, EdgeId, IntegerType, IntegerValue, ObligationId, Proposition, ScalarTerm,
    ScalarType, TargetMachineCallable, ValueId,
};
use terminal_check_tables::{Checked, Construct, MachineTables, ProgramPoint};
use terminal_psi::{
    StructuralAccess, StructuralParameterDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalModule, Terminator,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerBound {
    Equal(IntegerValue),
    AtLeast(IntegerValue),
    AtMost(IntegerValue),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReturnedIntegerFact {
    position: u32,
    integer: IntegerType,
    bound: IntegerBound,
}

impl ReturnedIntegerFact {
    pub fn position(&self) -> u32 {
        self.position
    }
    pub fn bound(&self) -> IntegerBound {
        self.bound
    }

    fn over(&self, value: ScalarTerm) -> Proposition {
        let literal = |value| ScalarTerm::Integer {
            scalar_type: self.integer,
            value,
        };
        match self.bound {
            IntegerBound::Equal(n) => Proposition::Equal(value, literal(n)),
            IntegerBound::AtLeast(n) => Proposition::LessOrEqual(literal(n), value),
            IntegerBound::AtMost(n) => Proposition::LessOrEqual(value, literal(n)),
        }
    }

    /// Restate this fact on the receiving declaration after its caller has
    /// matched the storage homes. Place IDs belong to a single module.
    pub fn on_parameter(
        &self,
        module: &TerminalModule,
        parameter: &StructuralParameterDeclaration,
    ) -> Option<Proposition> {
        if parameter.position != self.position
            || parameter.access != StructuralAccess::MutableBorrow
            || module.structural_type(parameter.structural_type)?.shape
                != StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(self.integer))
        {
            return None;
        }
        let Some(TargetMachineCallable::Machine(entry)) = module.entry else {
            return None;
        };
        let proposition = self.over(ScalarTerm::IntegerField {
            version: ContentPlaceVersion::Current,
            root: parameter.place,
            path: Vec::new(),
            scalar_type: self.integer,
        });
        super::entry_inputs::canonical(proposition, entry).ok()
    }
}

/// Return storage is read at the edge. A subsequent cleanup or an external
/// completion would require its own account of the resulting storage.
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
) -> Checked<Vec<ReturnedIntegerFact>> {
    let mut facts = Vec::new();
    if machine.module.entry != Some(TargetMachineCallable::Machine(machine.id))
        || machine.module.identities.invocation_scope().is_none()
    {
        return Ok(facts);
    }
    let edges = normal_returns(machine.lowered);
    if edges.is_empty() {
        return Ok(facts);
    }
    let obligation = ObligationId::new(tables.obligations.len() as u64 + 1)
        .expect("one more checking obligation");
    'parameter: for param in &machine.lowered.structural_parameters {
        if param.access != StructuralAccess::MutableBorrow {
            continue;
        }
        let Some(StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))) = machine
            .module
            .structural_type(param.structural_type)
            .map(|ty| &ty.shape)
        else {
            continue;
        };
        let (Some(minimum), Some(maximum)) = (integer.minimum_value(), integer.maximum_value())
        else {
            continue;
        };
        let mut returns = Vec::new();
        let mut lowest = Some(None);
        let mut highest = Some(None);
        for &edge in &edges {
            let point = ProgramPoint::Edge(edge);
            let value = machine
                .storage
                .at_return(machine.references, tables, edge, Construct::Machine)?
                .field(
                    ContentPlaceVersion::Current,
                    param.place,
                    &[],
                    ScalarType::Integer(*integer),
                );
            let Ok(value) = value else {
                continue 'parameter;
            };
            let (low, high) = match &value {
                ScalarTerm::Integer { scalar_type, value } if scalar_type == integer => {
                    (Some(*value), Some(*value))
                }
                ScalarTerm::Value { id, .. } => {
                    let known = hypotheses(machine.graph, tables, machine.id, obligation, point)?;
                    let mut prover = Prover::new(
                        machine.context,
                        &known.assumptions,
                        &known.axioms,
                        &tables.values,
                        activity,
                    );
                    prover.literal_bounds(*id)
                }
                _ => (None, None),
            };
            lowest = super::result_summaries::widest(
                lowest,
                Some(low.unwrap_or(minimum)),
                *integer,
                std::cmp::Ordering::Less,
            );
            highest = super::result_summaries::widest(
                highest,
                Some(high.unwrap_or(maximum)),
                *integer,
                std::cmp::Ordering::Greater,
            );
            returns.push((point, value));
        }
        let low = lowest.flatten();
        let high = highest.flatten();
        let mut establish = |bound| {
            let fact = ReturnedIntegerFact {
                position: param.position,
                integer: *integer,
                bound,
            };
            let proved = returns.iter().all(|(point, value)| {
                decide(
                    machine.context,
                    machine.graph,
                    tables,
                    machine.id,
                    obligation,
                    *point,
                    fact.over(value.clone()),
                    unreached,
                    activity,
                )
                .is_ok()
            });
            if proved {
                facts.push(fact);
            }
            proved
        };
        if let Some(value) = low
            && high == Some(value)
            && establish(IntegerBound::Equal(value))
        {
            continue;
        }
        if let Some(value) = low.filter(|n| *n != minimum) {
            establish(IntegerBound::AtLeast(value));
        }
        if let Some(value) = high.filter(|n| *n != maximum) {
            establish(IntegerBound::AtMost(value));
        }
    }
    Ok(facts)
}
