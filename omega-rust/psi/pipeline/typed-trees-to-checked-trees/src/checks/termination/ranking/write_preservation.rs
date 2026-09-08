//! Complete store and operand frames preserve exact ranked input paths.

use facts::NormalizedWriteFrame;
use typed_trees::machine::Machine;
use typed_trees::statement::StatementNode;
use validation::CallFrameResolver;

pub(super) fn prefix_preserves_path<'program>(
    frames: &CallFrameResolver<'program>,
    machine: &'program Machine,
    statements: &[StatementNode],
    path: &str,
) -> bool {
    let disjoint = |frame: NormalizedWriteFrame| {
        frame.into_complete_paths().is_some_and(|paths| {
            paths
                .iter()
                .all(|written| !validation::frame_paths_overlap(written, path))
        })
    };
    statements.iter().all(|statement| {
        let direct_writes_preserve = match statement {
            StatementNode::Assignment(_) => {
                disjoint(frames.assignment_write_frame(machine, statement))
            }
            StatementNode::Call(call) => disjoint(frames.may_write_frame(machine, call)),
            _ => true,
        };
        direct_writes_preserve && disjoint(frames.statement_value_write_frame(machine, statement))
    })
}
