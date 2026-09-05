//! Reconstruct the exact preheader, guard, decrement, backedge, and exit graph.

use machine_code::RankedU32CountdownMachineCodeRecord;
use semantic_vocabulary::IntegerValue;
use terminal_psi::{
    Operation, OperationKind, OperationResult, StructuralAccess, TerminalMachine,
    TerminalRankedGuard, TerminalRankedSuccessorArgument, Terminator,
};

pub(super) fn replay_ranked_graph_matches(
    machine: &TerminalMachine,
    record: &RankedU32CountdownMachineCodeRecord,
) -> bool {
    let graph = record.custody.graph;
    let Some(ranked) = machine.ranked_scc.as_ref() else {
        return false;
    };
    let [covered] = ranked.covered_cyclic_edges.as_slice() else {
        return false;
    };
    let block = |id| machine.blocks.iter().find(|block| block.id == id);
    let Some(entry) = block(machine.entry) else {
        return false;
    };
    let Terminator::Jump {
        edge: preheader_edge,
        target: preheader_target,
        arguments: preheader_arguments,
        ..
    } = &entry.terminator
    else {
        return false;
    };
    let Some(header) = block(ranked.header) else {
        return false;
    };
    let Some(rank_index) = header
        .parameters
        .iter()
        .position(|parameter| parameter.id == ranked.rank_parameter)
    else {
        return false;
    };
    if *preheader_target != ranked.header || preheader_arguments.len() != header.parameters.len() {
        return false;
    }
    let Some(&initial_value) = preheader_arguments.get(rank_index) else {
        return false;
    };
    let [zero, compare] = header.operations.as_slice() else {
        return false;
    };
    if !matches!(
        zero.kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(0)
        }
    ) {
        return false;
    }
    let Some(zero_value) = scalar_result(zero) else {
        return false;
    };
    if !matches!(
        compare.kind,
        OperationKind::IntegerLessThan { left, right }
            if left == zero_value && right == ranked.rank_parameter
    ) {
        return false;
    }
    let Some(condition) = scalar_result(compare) else {
        return false;
    };
    let Terminator::Conditional {
        condition: terminator_condition,
        when_true,
        when_false,
    } = &header.terminator
    else {
        return false;
    };
    let TerminalRankedGuard::UnsignedParameterPositive {
        block: guard_block,
        edge: guard_edge,
        condition: guard_condition,
        parameter: guard_parameter,
    } = covered.guard;
    if *terminator_condition != condition
        || guard_block != ranked.header
        || guard_edge != when_true.edge
        || when_true.target != covered.source
        || guard_condition != condition
        || guard_parameter != ranked.rank_parameter
    {
        return false;
    }
    let Some(decrement) = block(covered.source) else {
        return false;
    };
    let [one, subtract] = decrement.operations.as_slice() else {
        return false;
    };
    if !matches!(
        one.kind,
        OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(1)
        }
    ) {
        return false;
    }
    let Some(one_value) = scalar_result(one) else {
        return false;
    };
    let OperationKind::ExactIntegerSubtract {
        left,
        right,
        obligation,
    } = subtract.kind
    else {
        return false;
    };
    if left != ranked.rank_parameter || right != one_value {
        return false;
    }
    let Some(subtract_value) = scalar_result(subtract) else {
        return false;
    };
    let TerminalRankedSuccessorArgument::UnsignedParameterMinusOne {
        argument_index,
        argument,
        source_parameter,
        target_parameter,
    } = covered.successor_argument;
    if argument != subtract_value
        || source_parameter != ranked.rank_parameter
        || target_parameter != ranked.rank_parameter
    {
        return false;
    }
    let Terminator::Jump {
        edge: backedge,
        target: backedge_target,
        arguments: backedge_arguments,
        ..
    } = &decrement.terminator
    else {
        return false;
    };
    let Ok(argument_index) = usize::try_from(argument_index) else {
        return false;
    };
    if *backedge != covered.edge
        || *backedge_target != covered.target
        || covered.target != ranked.header
        || backedge_arguments.get(argument_index) != Some(&subtract_value)
    {
        return false;
    }
    let Some(done) = block(when_false.target) else {
        return false;
    };
    let Terminator::ReturnUnit {
        edge: return_edge,
        trivial_affine_discards,
    } = &done.terminator
    else {
        return false;
    };
    let [structural] = machine.structural_parameters.as_slice() else {
        return false;
    };
    let exact_cleanup =
        if structural.is_self && structural.access == StructuralAccess::MutableBorrow {
            trivial_affine_discards.is_empty()
        } else {
            trivial_affine_discards.as_slice() == [structural.place]
        };
    done.operations.is_empty()
        && exact_cleanup
        && graph.entry == machine.entry
        && graph.preheader_edge == *preheader_edge
        && graph.initial_value == initial_value
        && graph.zero_operation == zero.id
        && graph.zero_value == zero_value
        && graph.compare_operation == compare.id
        && graph.false_exit_edge == when_false.edge
        && graph.done_block == done.id
        && graph.one_operation == one.id
        && graph.one_value == one_value
        && graph.subtract_operation == subtract.id
        && graph.subtract_obligation == obligation
        && graph.return_edge == *return_edge
}

fn scalar_result(operation: &Operation) -> Option<semantic_vocabulary::ValueId> {
    let OperationResult::Scalar(result) = operation.result else {
        return None;
    };
    Some(result.id)
}
