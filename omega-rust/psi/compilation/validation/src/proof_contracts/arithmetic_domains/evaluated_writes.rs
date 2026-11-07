//! Writes from the subject and patterns that have actually completed. Later
//! patterns and selected bodies cannot invalidate an earlier arm's premises.
//! The ordered set indexes candidates; the call-frame owner decides overlap.

use std::collections::BTreeSet;
use std::ops::Bound::{Included, Unbounded};

#[derive(Default)]
pub(super) struct EvaluatedWritePrefix {
    writes: BTreeSet<String>,
    unknown: bool,
}

impl EvaluatedWritePrefix {
    pub(super) fn extend(&mut self, writes: Option<Vec<String>>) {
        if self.unknown {
            return;
        }
        match writes {
            Some(writes) => self.writes.extend(writes),
            None => {
                self.unknown = true;
                self.writes.clear();
            }
        }
    }

    pub(super) fn is_unknown(&self) -> bool {
        self.unknown
    }

    pub(super) fn is_empty(&self) -> bool {
        !self.unknown && self.writes.is_empty()
    }

    pub(super) fn overlaps(&self, path: &str) -> bool {
        if self.unknown {
            return true;
        }
        if self.writes.is_empty() {
            return false;
        }
        let agrees =
            |candidate: &str| crate::machine_calls::calls::frame_paths_overlap(path, candidate);
        // An ancestor write must be an exact string prefix. Looking up those
        // prefixes avoids scanning all earlier writes for every new arm.
        if path
            .char_indices()
            .map(|(end, _)| &path[..end])
            .chain(std::iter::once(path))
            .any(|prefix| self.writes.get(prefix).is_some_and(|write| agrees(write)))
        {
            return true;
        }
        let mut candidate = String::with_capacity(path.len() + 3);
        // A collection-content frame also covers its indexed descendants,
        // while its descriptor length remains a separate observation.
        for (position, character) in path.char_indices() {
            if character == '[' {
                candidate.clear();
                candidate.push_str(&path[..position]);
                candidate.push_str("[*]");
                if self
                    .writes
                    .get(candidate.as_str())
                    .is_some_and(|write| agrees(write))
                {
                    return true;
                }
            }
        }
        // The first write in either descendant range suffices: all entries
        // with that prefix are contiguous and the canonical relation covers
        // each. A wildcard read additionally covers the collection's indexes.
        let descendants = [
            (Some(path), "."),
            (Some(path), "["),
            (path.strip_suffix("[*]"), "["),
        ];
        for (parent, separator) in descendants {
            let Some(parent) = parent else { continue };
            candidate.clear();
            candidate.push_str(parent);
            candidate.push_str(separator);
            if self
                .writes
                .range::<str, _>((Included(candidate.as_str()), Unbounded))
                .next()
                .is_some_and(|write| write.starts_with(candidate.as_str()) && agrees(write))
            {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::EvaluatedWritePrefix;

    #[test]
    fn accumulated_prefix_matches_the_call_frame_overlap_oracle() {
        let observed = [
            "packet",
            "packet.value",
            "packet.value.child",
            "packet.value.child.nested",
            "packet.value_suffix",
            "packets.value",
            "packet.cells",
            "packet.cells[0]",
            "packet.cells[0].field",
            "packet.cells[1].field",
            "packet.cells[*]",
            "packet.cells.len",
            "packet.side",
        ];
        let mut prefix = EvaluatedWritePrefix::default();
        let mut linear = Vec::new();
        for written in [
            vec![],
            vec!["packet.value.child"],
            vec!["packets.value"],
            vec!["packet.cells[0].field"],
            vec!["packet.cells[*]"],
            vec!["packet"],
        ] {
            prefix.extend(Some(
                written.iter().map(|path| (*path).to_owned()).collect(),
            ));
            linear.extend(written);
            for path in observed {
                assert_eq!(
                    prefix.overlaps(path),
                    linear
                        .iter()
                        .any(|write| crate::machine_calls::calls::frame_paths_overlap(path, write)),
                    "read {path}, evaluated writes {linear:?}",
                );
            }
        }
        prefix.extend(None);
        assert!(prefix.is_unknown());
        assert!(!prefix.is_empty());
        prefix.extend(Some(Vec::new()));
        assert!(prefix.is_unknown());
        for path in observed {
            assert!(prefix.overlaps(path));
        }
    }
}
