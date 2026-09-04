//! Ranked facts that must rejoin the retained terminal machine-code carrier.

use omega_machine_code::{
    MachineCodeFunction, MachineCodePlan, RankedU32CountdownMachineCodeRecord,
};
use omega_target_operations::TerminalPsiProvenance;
use psi_core::{IntegerSign, IntegerType, IntegerValue};
use psi_terminal::{TerminalRankedGuard, TerminalRankedSuccessorArgument};

use super::graph;

pub(in crate::ranked_u32_countdown::contract) fn validate(
    plan: &MachineCodePlan,
    function: &MachineCodeFunction,
    record: &RankedU32CountdownMachineCodeRecord,
) -> Option<()> {
    let graph = record.custody.graph;
    let component = &record.custody.ranked_scc;
    let replay = &record.custody.semantic_replay;
    let [replay_machine] = replay.machines.as_slice() else {
        return None;
    };
    let [covered] = component.covered_cyclic_edges.as_slice() else {
        return None;
    };
    let TerminalRankedGuard::UnsignedParameterPositive {
        block: guard_block,
        edge: guard_edge,
        parameter: guard_parameter,
        ..
    } = covered.guard;
    let TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
        argument_index,
        source_parameter,
        target_parameter,
        ..
    } = covered.successor_argument;
    let expected_provenance = TerminalPsiProvenance {
        operations: vec![
            graph.zero_operation,
            graph.compare_operation,
            graph.one_operation,
            graph.subtract_operation,
        ],
        edges: vec![
            graph.preheader_edge,
            guard_edge,
            graph.false_exit_edge,
            covered.edge,
            graph.return_edge,
        ],
    };
    (component.rank_type == IntegerType::new(IntegerSign::Unsigned, 32).expect("u32 is valid")
        && component.lower_bound == IntegerValue::Unsigned(0)
        && component.upper_bound == IntegerValue::Unsigned(u128::from(u32::MAX))
        && covered.target == component.header
        && guard_block == component.header
        && guard_parameter == component.rank_parameter
        && source_parameter == component.rank_parameter
        && target_parameter == component.rank_parameter
        && argument_index == 0
        && graph.entry != component.header
        && graph.done_block != component.header
        && function.provenance == expected_provenance
        && psi_terminal_codec::terminal_psi_identity(replay).ok() == Some(plan.psi)
        && replay.entry == function.machine
        && replay_machine.id == function.machine
        && replay_machine.attachment == function.attachment
        && replay_machine.ranked_scc.as_ref() == Some(component)
        && replay.structural_types == record.structural_types
        && graph::replay_ranked_graph_matches(replay_machine, record))
    .then_some(())
}
