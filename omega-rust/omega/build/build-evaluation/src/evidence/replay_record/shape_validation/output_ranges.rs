//! Output shape ranges and output tree ranges.

use crate::evidence::replay_record::BuildFilesystemReplayRecordError;
use crate::evidence::replay_record::attempt_codec::{
    AttemptShape, ShapeLogicalInput, ShapeLogicalInputResolution,
};
use crate::evidence::replay_record::hard_links::output_hard_link_paths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OutputShapeRange {
    Directory(usize),
    File { start: usize, end: usize },
    HardLink(usize),
    Symlink(usize),
}

impl OutputShapeRange {
    pub(crate) fn path<'a>(self, shapes: &'a [AttemptShape<'a>]) -> &'a [u8] {
        if let Self::HardLink(index) = self {
            return output_hard_link_paths(&shapes[index])
                .expect("validated Output hard link has exact paths")
                .1;
        }
        let index = match self {
            Self::Directory(index) | Self::File { start: index, .. } | Self::Symlink(index) => {
                index
            }
            Self::HardLink(_) => unreachable!(),
        };
        shapes[index]
            .rooted_paths
            .first()
            .expect("validated Output entry has one rooted path")
            .bytes
    }

    pub(crate) const fn is_directory(self) -> bool {
        matches!(self, Self::Directory(_))
    }
}

fn output_file_end(
    shapes: &[AttemptShape<'_>],
    start: usize,
) -> Result<usize, BuildFilesystemReplayRecordError> {
    if shapes.get(start).is_none_or(|shape| shape.operation != 1) {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay Output file must begin with create",
        ));
    }
    let Some(root_identity) = shapes[start].output.map(|output| output.identity) else {
        return Err(BuildFilesystemReplayRecordError::new(
            "filesystem replay Output create has no descriptor identity",
        ));
    };
    let mut cursor = start + 1;
    loop {
        if cursor == shapes.len() {
            return Err(BuildFilesystemReplayRecordError::new(
                "receipted build output must contain complete create-operation*-close files",
            ));
        }
        if matches!(
            shapes[cursor].operation,
            5 | 7 | 10 | 17 | 41 | 42 | 43 | 44 | 49
        ) {
            cursor += 1;
            continue;
        }
        if shapes[cursor].operation == 45 {
            if cursor + 1 >= shapes.len() || shapes[cursor + 1].operation != 8 {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output duplicate must be immediately retired",
                ));
            }
            cursor += 2;
            continue;
        }
        if shapes[cursor].operation == 46 {
            if cursor + 1 >= shapes.len() || shapes[cursor + 1].operation != 46 {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output lock must be immediately released",
                ));
            }
            cursor += 2;
            continue;
        }
        let closes_root = shapes[cursor].operation == 8
            && matches!(
                shapes[cursor].inputs.as_slice(),
                [ShapeLogicalInput {
                    resolution: ShapeLogicalInputResolution::Resolved(identity),
                    ..
                }] if *identity == root_identity
            );
        if closes_root {
            return Ok(cursor + 1);
        }
        return Err(BuildFilesystemReplayRecordError::new(
            "receipted build output must contain complete create-operation*-close files",
        ));
    }
}

pub(crate) fn output_tree_ranges(
    shapes: &[AttemptShape<'_>],
    output_start: usize,
) -> Result<Vec<OutputShapeRange>, BuildFilesystemReplayRecordError> {
    let mut ranges = Vec::new();
    let mut cursor = output_start;
    while cursor < shapes.len() {
        match shapes[cursor].operation {
            11 => {
                ranges.push(OutputShapeRange::Directory(cursor));
                cursor += 1;
            }
            1 => {
                let end = output_file_end(shapes, cursor)?;
                ranges.push(OutputShapeRange::File { start: cursor, end });
                cursor = end;
            }
            19 | 27 => {
                ranges.push(OutputShapeRange::HardLink(cursor));
                cursor += 1;
            }
            20 => {
                ranges.push(OutputShapeRange::Symlink(cursor));
                cursor += 1;
            }
            _ => {
                return Err(BuildFilesystemReplayRecordError::new(
                    "receipted build output must contain ordered directory, file, hard-link, or symlink entries",
                ));
            }
        }
    }
    Ok(ranges)
}
