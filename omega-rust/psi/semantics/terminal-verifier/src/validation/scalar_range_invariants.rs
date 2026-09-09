//! Scalar induction claims name exact headers and every actual arrival.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{MachineId, ScalarType};
use terminal_psi::{TerminalMachine, TerminalModule, Terminator};

use super::{IdRegistry, ModuleError, insert_unique};

pub(super) fn validate(
    module: &TerminalModule,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    registry: &mut IdRegistry,
) -> Result<(), ModuleError> {
    let mut previous = None;
    for invariant in &module.scalar_range_invariants {
        let identity = (invariant.machine, invariant.header, invariant.parameter);
        if previous.is_some_and(|previous| previous >= identity) {
            return Err(ModuleError::NonCanonicalScalarRangeInvariants);
        }
        previous = Some(identity);
        let invalid = || ModuleError::InvalidScalarRangeInvariant {
            machine: invariant.machine,
            header: invariant.header,
            parameter: invariant.parameter,
        };
        let invalid_arrivals = || ModuleError::InvalidScalarRangeInvariantArrivals {
            machine: invariant.machine,
            header: invariant.header,
            parameter: invariant.parameter,
        };
        let machine = machines.get(&invariant.machine).ok_or_else(invalid)?;
        let header = machine
            .blocks
            .iter()
            .find(|block| block.id == invariant.header)
            .ok_or_else(invalid)?;
        if !header.parameters.iter().any(|parameter| {
            parameter.id == invariant.parameter
                && parameter.scalar_type == ScalarType::Integer(invariant.bounds.integer_type())
        }) || !crate::control_graph::feedback_edges(machine)
            .values()
            .any(|target| *target == invariant.header)
        {
            return Err(invalid());
        }
        let mut arrivals = BTreeSet::new();
        for block in &machine.blocks {
            match &block.terminator {
                Terminator::Jump { edge, target, .. } if *target == invariant.header => {
                    arrivals.insert(*edge);
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    for successor in [when_true, when_false] {
                        if successor.target == invariant.header {
                            arrivals.insert(successor.edge);
                        }
                    }
                }
                Terminator::StructuralCase { cases, .. }
                    if cases
                        .iter()
                        .any(|successor| successor.target == invariant.header) =>
                {
                    return Err(invalid_arrivals());
                }
                _ => {}
            }
        }
        if !invariant
            .arrivals
            .iter()
            .map(|arrival| arrival.edge)
            .eq(arrivals)
        {
            return Err(invalid_arrivals());
        }
        for arrival in &invariant.arrivals {
            insert_unique(
                &mut registry.obligations,
                arrival.obligation,
                ModuleError::DuplicateObligation,
            )?;
        }
    }
    Ok(())
}
