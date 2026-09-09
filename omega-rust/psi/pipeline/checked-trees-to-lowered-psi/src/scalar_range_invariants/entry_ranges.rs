//! Closed entry bounds forwarded directly into a cyclic header are candidates.

use semantic_vocabulary::{BoundedIntegerType, Proposition, ScalarTerm, ScalarType};
use terminal_psi::{ScalarRangeInvariant, TerminalModule, Terminator, ValueDeclaration};

pub(super) fn candidates(module: &TerminalModule) -> Vec<ScalarRangeInvariant> {
    let mut candidates = Vec::new();
    for machine in &module.machines {
        let Some(entry) = machine
            .blocks
            .iter()
            .find(|block| block.id == machine.entry)
        else {
            continue;
        };
        if !entry.operations.is_empty() {
            continue;
        }
        let Terminator::Jump {
            target, arguments, ..
        } = &entry.terminator
        else {
            continue;
        };
        let Ok(components) = terminal_verifier::control_cycle_members(machine) else {
            continue;
        };
        if !components.iter().any(|members| members.contains(target)) {
            continue;
        }
        let Some(header) = machine.blocks.iter().find(|block| block.id == *target) else {
            continue;
        };
        for (parameter, argument) in header.parameters.iter().zip(arguments) {
            let Some(formal) = machine
                .parameters
                .iter()
                .find(|formal| formal.id == *argument)
            else {
                continue;
            };
            if formal.scalar_type != parameter.scalar_type {
                continue;
            }
            if let Some(bounds) = bounds(formal, &machine.contract.requires) {
                candidates.push(ScalarRangeInvariant {
                    machine: machine.id,
                    header: header.id,
                    parameter: parameter.id,
                    bounds,
                    arrivals: Vec::new(),
                });
            }
        }
    }
    candidates.sort_by_key(|candidate| (candidate.machine, candidate.header, candidate.parameter));
    candidates
}

fn bounds(
    parameter: &ValueDeclaration,
    requirements: &[Proposition],
) -> Option<BoundedIntegerType> {
    let ScalarType::Integer(integer) = parameter.scalar_type else {
        return None;
    };
    let subject = ScalarTerm::value(parameter.id, parameter.scalar_type);
    let mut minimum = integer.minimum_value();
    let mut maximum = integer.maximum_value();
    let mut pending = requirements.iter().collect::<Vec<_>>();
    while let Some(proposition) = pending.pop() {
        match proposition {
            Proposition::Conjunction(members) => pending.extend(members),
            Proposition::LessOrEqual(left, right) => {
                if left == &subject
                    && let ScalarTerm::Integer { scalar_type, value } = right
                    && *scalar_type == integer
                    && integer.admits(*value)
                {
                    maximum = maximum.min(*value);
                }
                if right == &subject
                    && let ScalarTerm::Integer { scalar_type, value } = left
                    && *scalar_type == integer
                    && integer.admits(*value)
                {
                    minimum = minimum.max(*value);
                }
            }
            _ => {}
        }
    }
    if minimum == integer.minimum_value() && maximum == integer.maximum_value() {
        return None;
    }
    BoundedIntegerType::new(integer, minimum, maximum).ok()
}
