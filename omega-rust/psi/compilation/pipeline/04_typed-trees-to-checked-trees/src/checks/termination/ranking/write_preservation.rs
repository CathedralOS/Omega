//! Complete store and operand frames preserve exact ranked input paths.

use facts::NormalizedWriteFrame;
use typed_trees::machine::Machine;
use typed_trees::statement::StatementNode;
use validation::{
    CallFrameResolver, RankPremiseObservation, RankProtectedPath, frame_preserves_rank_premises,
};

/// A write frame is preservation evidence only when it is complete -- an
/// opaque frame admits nothing -- and every caller-relative path it may
/// write leaves the carrier as it was.
fn frame_preserves_premise(frame: NormalizedWriteFrame, carrier: &RankProtectedPath) -> bool {
    frame_preserves_rank_premises(frame, std::slice::from_ref(carrier))
}

pub(super) fn prefix_preserves_premise<'program>(
    frames: &CallFrameResolver<'program>,
    machine: &'program Machine,
    statements: &[StatementNode],
    carrier: &RankProtectedPath,
) -> bool {
    statements.iter().all(|statement| {
        let direct_writes_preserve = match statement {
            StatementNode::Assignment(_) => {
                frame_preserves_premise(frames.assignment_write_frame(machine, statement), carrier)
            }
            StatementNode::Call(call) => {
                frame_preserves_premise(frames.may_write_frame(machine, call), carrier)
            }
            _ => true,
        };
        direct_writes_preserve
            && frame_preserves_premise(
                frames.statement_value_write_frame(machine, statement),
                carrier,
            )
    })
}

/// A whole-value carrier: every overlapping write invalidates it.
pub(super) fn prefix_preserves_path<'program>(
    frames: &CallFrameResolver<'program>,
    machine: &'program Machine,
    statements: &[StatementNode],
    path: &str,
) -> bool {
    prefix_preserves_premise(
        frames,
        machine,
        statements,
        &RankProtectedPath {
            path: path.to_owned(),
            observation: RankPremiseObservation::Value,
        },
    )
}
