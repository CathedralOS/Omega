//! Output namespace creation order and exact per-file event membership.

use crate::evidence::replay_record::BuildFilesystemReplayRecordError;
use crate::evidence::replay_record::attempt_codec::{
    AttemptShape, ShapeLogicalInput, ShapeLogicalInputResolution,
};
use crate::evidence::replay_record::hard_links::output_hard_link_paths;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub(crate) enum OutputEntryAttempts {
    Directory(usize),
    File { attempts: Vec<usize> },
    HardLink(usize),
    Symlink(usize),
}

impl OutputEntryAttempts {
    pub(crate) fn path<'a>(&self, shapes: &'a [AttemptShape<'a>]) -> &'a [u8] {
        if let Self::HardLink(position) = self {
            return output_hard_link_paths(&shapes[*position])
                .expect("validated hard-link paths")
                .1;
        }
        shapes[self.attempt_indices()[0]]
            .rooted_paths
            .first()
            .expect("validated Output path")
            .bytes
    }

    pub(crate) fn attempt_indices(&self) -> &[usize] {
        match self {
            Self::File { attempts } => attempts,
            Self::Directory(position) | Self::HardLink(position) | Self::Symlink(position) => {
                std::slice::from_ref(position)
            }
        }
    }

    pub(crate) const fn is_directory(&self) -> bool {
        matches!(self, Self::Directory(_))
    }
}

struct DescriptorLifetime {
    file: usize,
    closed: bool,
}

pub(crate) fn output_tree_membership(
    shapes: &[AttemptShape<'_>],
    output_start: usize,
) -> Result<Vec<OutputEntryAttempts>, BuildFilesystemReplayRecordError> {
    let failure = |message| BuildFilesystemReplayRecordError::new(message);
    let mut entries = Vec::new();
    // Wire identities are sparse, untrusted u64 values. Retain retired keys to
    // reject lifetime reuse without allocating by an externally chosen index.
    let mut descriptors = BTreeMap::<u64, DescriptorLifetime>::new();
    for (position, shape) in shapes.iter().enumerate().skip(output_start) {
        if matches!(shape.operation, 1 | 11 | 19 | 20 | 27) {
            if entries.len() == checked_interpreter::MAX_FILESYSTEM_REPLAY_OUTPUT_DIRECTORIES {
                return Err(failure(
                    "receipted build output exceeds the Output-tree entry ceiling",
                ));
            }
            // Membership readers access namespace paths before the detailed
            // per-operation validator. Establish those lanes first.
            if matches!(shape.operation, 19 | 27) {
                output_hard_link_paths(shape)?;
            } else if shape.rooted_paths.len() != 1 {
                return Err(failure("Output namespace event has no unique rooted path"));
            }
        }
        match shape.operation {
            11 => entries.push(OutputEntryAttempts::Directory(position)),
            19 | 27 => entries.push(OutputEntryAttempts::HardLink(position)),
            20 => entries.push(OutputEntryAttempts::Symlink(position)),
            1 => {
                let output = shape
                    .output
                    .ok_or_else(|| failure("Output create has no descriptor identity"))?;
                if descriptors.contains_key(&output.identity) {
                    return Err(failure("Output descriptor identity is reused"));
                }
                descriptors.insert(
                    output.identity,
                    DescriptorLifetime {
                        file: entries.len(),
                        closed: false,
                    },
                );
                entries.push(OutputEntryAttempts::File {
                    attempts: vec![position],
                });
            }
            5 | 7 | 8 | 10 | 17 | 41 | 42 | 43 | 44 | 45 | 46 | 49 => {
                let [
                    ShapeLogicalInput {
                        ordinal: 0,
                        kind: 0,
                        resolution: ShapeLogicalInputResolution::Resolved(identity),
                    },
                ] = shape.inputs.as_slice()
                else {
                    return Err(failure("Output operation has no exact descriptor input"));
                };
                let descriptor = descriptors
                    .get_mut(identity)
                    .ok_or_else(|| failure("Output operation uses an unknown descriptor"))?;
                if descriptor.closed {
                    return Err(failure("Output operation uses a retired descriptor"));
                }
                let file = descriptor.file;
                let OutputEntryAttempts::File { attempts } = &mut entries[file] else {
                    return Err(failure("Output descriptor has no file owner"));
                };
                attempts.push(position);
                if shape.operation == 8 {
                    descriptor.closed = true;
                } else if shape.operation == 45 {
                    let output = shape
                        .output
                        .ok_or_else(|| failure("Output duplicate has no descriptor identity"))?;
                    if descriptors.contains_key(&output.identity) {
                        return Err(failure("Output descriptor identity is reused"));
                    }
                    descriptors.insert(
                        output.identity,
                        DescriptorLifetime {
                            file,
                            closed: false,
                        },
                    );
                }
            }
            _ => {
                return Err(failure(
                    "Output event is not a supported namespace or descriptor operation",
                ));
            }
        }
    }
    if descriptors.values().any(|descriptor| !descriptor.closed) {
        return Err(failure("Output descriptor lifetime is not closed"));
    }
    Ok(entries)
}
