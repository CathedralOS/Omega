//! Complete store and operand frames preserve exact ranked input paths.

use crate::fact_plan::NormalizedWriteFrame;
use crate::validation::CallFrameResolver;
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::statement::StatementNode;

/// A write frame is preservation evidence only when it is complete -- an
/// opaque frame admits nothing -- and every caller-relative path it may
/// write is disjoint from `path`.
pub(super) fn frame_preserves_path(frame: NormalizedWriteFrame, path: &str) -> bool {
    frame.into_complete_paths().is_some_and(|paths| {
        paths
            .iter()
            .all(|written| !crate::validation::frame_paths_overlap(written, path))
    })
}

pub(super) fn prefix_preserves_path<'program>(
    frames: &CallFrameResolver<'program>,
    machine: &'program Machine,
    statements: &[StatementNode],
    path: &str,
) -> bool {
    statements.iter().all(|statement| {
        let direct_writes_preserve = match statement {
            StatementNode::Assignment(_) => {
                frame_preserves_path(frames.assignment_write_frame(machine, statement), path)
            }
            StatementNode::Call(call) => {
                frame_preserves_path(frames.may_write_frame(machine, call), path)
            }
            _ => true,
        };
        direct_writes_preserve
            && frame_preserves_path(frames.statement_value_write_frame(machine, statement), path)
    })
}
