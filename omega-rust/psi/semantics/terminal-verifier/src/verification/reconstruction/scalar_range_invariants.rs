//! Inductive range questions use the current header and exact selected edge.
//!
//! Header bounds are hypotheses of the induction step, not trusted producer
//! facts. Reconstruction emits every establishment and preservation obligation
//! together with the ordinary operation-safety questions. The verifier returns
//! authority only if that whole transaction checks. Initial arrival cannot see
//! a header hypothesis; a partial operation cannot see its own result equation
//! when proving safety. Completed operations may then prove the next arrival.
//! Private crash-entry reconstruction does not import these current-value facts.
//!
//! Arrival goals are emitted before successor binding, including scheduling-cut
//! edges. Initial arrival therefore cannot assume the destination's range.
//! Preservation uses current-header hypotheses and completed operation facts;
//! each operation's own safety question was captured before its result facts.

use proof_admission::{Obligation, ObligationClass};
use semantic_vocabulary::{
    BlockId, BoundedIntegerType, EdgeId, MachineId, Proposition, ScalarTerm, ScalarType, ValueId,
};
use terminal_psi::{TerminalMachine, TerminalModule, Terminator};

use super::{ReconstructedOperationObligation, ReconstructedTerminalObligationOwner, path_facts};

fn range_axioms(bounds: BoundedIntegerType, value: ScalarTerm) -> [Proposition; 2] {
    let lower = ScalarTerm::Integer {
        scalar_type: bounds.integer_type(),
        value: bounds.minimum(),
    };
    let upper = ScalarTerm::Integer {
        scalar_type: bounds.integer_type(),
        value: bounds.maximum(),
    };
    [
        Proposition::LessOrEqual(lower, value.clone()),
        Proposition::LessOrEqual(value, upper),
    ]
}

pub(super) fn header_axioms(
    module: &TerminalModule,
    machine: MachineId,
    header: BlockId,
) -> Vec<Proposition> {
    module
        .scalar_range_invariants
        .iter()
        .filter(|invariant| invariant.machine == machine && invariant.header == header)
        .flat_map(|invariant| {
            range_axioms(
                invariant.bounds,
                ScalarTerm::value(
                    invariant.parameter,
                    ScalarType::Integer(invariant.bounds.integer_type()),
                ),
            )
        })
        .collect()
}

pub(super) fn append_arrival_obligations(
    module: &TerminalModule,
    machine: &TerminalMachine,
    terminator: &Terminator,
    value_term: &impl Fn(ValueId) -> ScalarTerm,
    axioms: &[Proposition],
    obligations: &mut Vec<ReconstructedOperationObligation>,
) {
    if !module
        .scalar_range_invariants
        .iter()
        .any(|invariant| invariant.machine == machine.id)
    {
        return;
    }
    let mut append_edge =
        |edge: EdgeId, target: BlockId, arguments: &[ValueId], selected_axioms: &[Proposition]| {
            for invariant in module
                .scalar_range_invariants
                .iter()
                .filter(|invariant| invariant.machine == machine.id && invariant.header == target)
            {
                let header = machine
                    .blocks
                    .iter()
                    .find(|block| block.id == target)
                    .expect("validated invariant header exists");
                let parameter_position = header
                    .parameters
                    .iter()
                    .position(|parameter| parameter.id == invariant.parameter)
                    .expect("validated invariant parameter belongs to its header");
                let arrival = invariant
                    .arrivals
                    .iter()
                    .find(|arrival| arrival.edge == edge)
                    .expect("validated invariant retains every actual arrival");
                let proposition = Proposition::Conjunction(
                    range_axioms(invariant.bounds, value_term(arguments[parameter_position]))
                        .into(),
                );
                obligations.push(ReconstructedOperationObligation {
                    owner: ReconstructedTerminalObligationOwner::ScalarRangeInvariant {
                        machine: machine.id,
                        header: target,
                        parameter: invariant.parameter,
                        edge,
                    },
                    obligation: Obligation {
                        id: arrival.obligation,
                        proposition,
                        class: ObligationClass::Derivable,
                    },
                    semantic_axioms: selected_axioms.to_vec(),
                    canonical_certificate: true,
                });
            }
        };
    match terminator {
        Terminator::Jump {
            edge,
            target,
            arguments,
            ..
        } => append_edge(*edge, *target, arguments, axioms),
        Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            for (successor, positive) in [(when_true, true), (when_false, false)] {
                let mut selected_axioms = axioms.to_vec();
                if let Some(fact) =
                    path_facts::condition_fact(*condition, positive, axioms, value_term)
                    && !selected_axioms.contains(&fact)
                {
                    selected_axioms.push(fact);
                }
                append_edge(
                    successor.edge,
                    successor.target,
                    &successor.arguments,
                    &selected_axioms,
                );
            }
        }
        _ => {}
    }
}
