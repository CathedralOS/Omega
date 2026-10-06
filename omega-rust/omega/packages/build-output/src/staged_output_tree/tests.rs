//! Staged-output tree selection tests.

mod file_selection;

use super::BuildStagedOutputTree;
use crate::{OutputTreeEntry, from_entries};

/// Canonical retained roster: two directories, a nested file pair, and two
/// top-level files, one of them a generated `.omg` source for the
/// include-source join below.
fn baseline_tree() -> BuildStagedOutputTree {
    from_entries(&[
        OutputTreeEntry::directory(b"docs"),
        OutputTreeEntry::directory(b"docs/inner"),
        OutputTreeEntry::regular_file(b"docs/inner/a.txt", b"alpha", false),
        OutputTreeEntry::regular_file(b"docs/inner/gen.omg", b"data Generated {}\n", false),
        OutputTreeEntry::regular_file(b"report.txt", b"report-body", false),
        OutputTreeEntry::regular_file(b"z-last.bin", b"z", false),
    ])
    .expect("baseline retained tree")
}
