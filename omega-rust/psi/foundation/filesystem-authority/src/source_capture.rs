//! Caller-selected source namespace shared by acquisition and Build snapshots.
//!
//! Selection owns canonical paths, nonoverlapping obligations and namespace
//! budgets. Callbacks supply member metadata and direct child names; the
//! selector does not open host files or grant access beyond the request.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceCaptureObligation {
    Required, // Member must exist under the captured root; absence fails capture.
    Optional, // Absence is committed cleanly without failing capture.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCaptureRequest {
    entries: BTreeMap<Vec<u8>, SourceCaptureObligation>, // Canonical relative paths mapped to their capture obligations.
}

impl SourceCaptureRequest {
    /// Selects the root subtree as an explicit acquisition-owned inventory.
    /// User entries still go through `new`, which rejects an empty path.
    pub fn entire_tree() -> Self {
        Self {
            entries: BTreeMap::from([(Vec::new(), SourceCaptureObligation::Required)]),
        }
    }

    pub fn new(
        entries: impl IntoIterator<Item = (Vec<u8>, SourceCaptureObligation)>,
    ) -> Result<Self, String> {
        let mut request = BTreeMap::new();
        for (relative_path, obligation) in entries {
            let drive_qualified = relative_path.split(|byte| *byte == b'/').any(|component| {
                component.first().is_some_and(u8::is_ascii_alphabetic)
                    && component.get(1) == Some(&b':')
            });
            if std::str::from_utf8(&relative_path).is_err()
                || drive_qualified
                || !crate::canonical_filesystem_metadata_path_is_canonical(&relative_path, false)
            {
                return Err(format!(
                    "source capture entry is not a canonical relative path: {relative_path:?}"
                ));
            }
            if request.insert(relative_path.clone(), obligation).is_some() {
                return Err(format!(
                    "source capture entry is declared twice: {relative_path:?}"
                ));
            }
        }
        for relative_path in request.keys() {
            for (end, _) in relative_path
                .iter()
                .enumerate()
                .filter(|(_, byte)| **byte == b'/')
            {
                if request.contains_key(&relative_path[..end]) {
                    return Err(format!(
                        "source capture entry {relative_path:?} nests inside declared entry {:?}",
                        &relative_path[..end]
                    ));
                }
            }
        }
        Ok(Self { entries: request })
    }

    pub fn entries(&self) -> impl Iterator<Item = (&[u8], SourceCaptureObligation)> {
        self.entries
            .iter()
            .map(|(path, obligation)| (path.as_slice(), *obligation))
    }
}

/// Selects metadata without enumerating siblings of explicitly selected files.
///
/// `children` supplies the direct canonical children of a selected directory;
/// `lookup` supplies metadata without following a non-directory ancestor.
/// The root counts toward `row_limit`, and every admitted non-root path counts
/// toward `path_byte_limit`, including ancestors of selected members.
pub fn select_source_members<Member, Error>(
    request: &SourceCaptureRequest,
    row_limit: usize,
    path_byte_limit: usize,
    mut lookup: impl FnMut(&[u8]) -> Result<Option<Member>, Error>,
    is_directory: impl Fn(&Member) -> bool,
    mut children: impl FnMut(&[u8]) -> Result<Vec<Vec<u8>>, Error>,
    selection_error: impl Fn(String) -> Error,
) -> Result<BTreeMap<Vec<u8>, Member>, Error> {
    if row_limit == 0 {
        return Err(selection_error(
            "canonical Source metadata exceeds its 0-row ceiling".to_owned(),
        ));
    }
    let root = lookup(&[])?
        .filter(&is_directory)
        .ok_or_else(|| selection_error("canonical Source root is not a directory".to_owned()))?;
    let mut selected = BTreeMap::from([(Vec::new(), root)]);
    let mut pending = Vec::new();
    let mut aggregate_path_bytes = 0usize;
    for (declared, obligation) in request.entries() {
        if declared.is_empty() {
            // Only `entire_tree` constructs this entry. Move the retained root
            // into the ordinary traversal so it is looked up and retained once.
            let root = selected.remove(declared).expect("root was retained above");
            pending.push((Vec::new(), root));
            continue;
        }
        let mut reachable = true;
        for (end, _) in declared
            .iter()
            .enumerate()
            .filter(|(_, byte)| **byte == b'/')
        {
            let ancestor = &declared[..end];
            if let Some(member) = selected.get(ancestor) {
                if !is_directory(member) {
                    reachable = false;
                    break;
                }
                continue;
            }
            let Some(member) = lookup(ancestor)?.filter(&is_directory) else {
                reachable = false;
                break;
            };
            charge_scoped_member(
                &selected,
                row_limit,
                path_byte_limit,
                pending.len(),
                &mut aggregate_path_bytes,
                ancestor,
            )
            .map_err(&selection_error)?;
            selected.insert(ancestor.to_vec(), member);
        }
        let member = if reachable { lookup(declared)? } else { None };
        match member {
            Some(member) => {
                charge_scoped_member(
                    &selected,
                    row_limit,
                    path_byte_limit,
                    pending.len(),
                    &mut aggregate_path_bytes,
                    declared,
                )
                .map_err(&selection_error)?;
                pending.push((declared.to_vec(), member));
            }
            None if obligation == SourceCaptureObligation::Required => {
                return Err(selection_error(format!(
                    "required source capture entry `{}` is absent",
                    String::from_utf8_lossy(declared)
                )));
            }
            None => {}
        }
    }
    while let Some((path, member)) = pending.pop() {
        if is_directory(&member) {
            for child in children(&path)? {
                charge_scoped_member(
                    &selected,
                    row_limit,
                    path_byte_limit,
                    pending.len() + 1,
                    &mut aggregate_path_bytes,
                    &child,
                )
                .map_err(&selection_error)?;
                let member = lookup(&child)?.ok_or_else(|| {
                    selection_error(format!(
                        "source capture member `{}` disappeared during capture",
                        String::from_utf8_lossy(&child)
                    ))
                })?;
                pending.push((child, member));
            }
        }
        if selected.insert(path.clone(), member).is_some() {
            return Err(selection_error(format!(
                "Source selection duplicated a path: {path:?}"
            )));
        }
    }
    Ok(selected)
}

fn charge_scoped_member<Member>(
    selected: &BTreeMap<Vec<u8>, Member>,
    row_limit: usize,
    path_byte_limit: usize,
    pending_count: usize,
    aggregate_path_bytes: &mut usize,
    path: &[u8],
) -> Result<(), String> {
    if selected
        .len()
        .checked_add(pending_count)
        .is_none_or(|count| count >= row_limit)
    {
        return Err(format!(
            "canonical Source metadata exceeds its {row_limit}-row ceiling"
        ));
    }
    if !crate::canonical_filesystem_metadata_path_is_canonical(path, false) {
        return Err(format!("Source path is not canonical metadata: {path:?}"));
    }
    *aggregate_path_bytes = aggregate_path_bytes
        .checked_add(path.len())
        .filter(|bytes| *bytes <= path_byte_limit)
        .ok_or_else(|| format!("canonical Source metadata path bytes exceed {path_byte_limit}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn request(entries: &[(&[u8], SourceCaptureObligation)]) -> SourceCaptureRequest {
        SourceCaptureRequest::new(
            entries
                .iter()
                .map(|(path, obligation)| (path.to_vec(), *obligation)),
        )
        .unwrap()
    }

    fn tree() -> BTreeMap<Vec<u8>, bool> {
        [
            (b"".as_slice(), true),
            (b"src".as_slice(), true),
            (b"src/main.omg".as_slice(), false),
            (b"src/private".as_slice(), true),
            (b"src/private/key".as_slice(), false),
            (b"other.omg".as_slice(), false),
        ]
        .into_iter()
        .map(|(path, directory)| (path.to_vec(), directory))
        .collect()
    }

    fn select(
        request: &SourceCaptureRequest,
        tree: &BTreeMap<Vec<u8>, bool>,
        rows: usize,
        path_bytes: usize,
    ) -> Result<BTreeMap<Vec<u8>, bool>, String> {
        select_source_members(
            request,
            rows,
            path_bytes,
            |path| Ok(tree.get(path).copied()),
            |directory| *directory,
            |path| {
                let mut prefix = path.to_vec();
                if !prefix.is_empty() {
                    prefix.push(b'/');
                }
                Ok(tree
                    .keys()
                    .filter(|child| {
                        child.starts_with(&prefix)
                            && child.len() > prefix.len()
                            && !child[prefix.len()..].contains(&b'/')
                    })
                    .cloned()
                    .collect())
            },
            std::convert::identity,
        )
    }

    #[test]
    fn file_selection_reads_ancestors_without_enumerating_or_reading_siblings() {
        let tree = tree();
        let looked_up = RefCell::new(Vec::new());
        let selected = select_source_members(
            &request(&[(b"src/main.omg", SourceCaptureObligation::Required)]),
            3,
            15,
            |path| {
                looked_up.borrow_mut().push(path.to_vec());
                Ok(tree.get(path).copied())
            },
            |directory| *directory,
            |_| panic!("a file selection must not enumerate its parent directory"),
            std::convert::identity,
        )
        .unwrap();
        let expected = [b"".to_vec(), b"src".to_vec(), b"src/main.omg".to_vec()];
        assert_eq!(selected.into_keys().collect::<Vec<_>>(), expected);
        assert_eq!(*looked_up.borrow(), expected);
    }

    #[test]
    fn a_selected_subtree_includes_descendants_without_selecting_other_roots() {
        let tree = tree();
        let selected = select(
            &request(&[(b"src", SourceCaptureObligation::Required)]),
            &tree,
            5,
            100,
        )
        .unwrap();
        assert!(selected.contains_key(b"src/private/key".as_slice()));
        assert!(!selected.contains_key(b"other.omg".as_slice()));
        assert_eq!(selected.len(), 5);
    }

    #[test]
    fn explicit_entire_tree_retains_root_once_and_obeys_root_row_budget() {
        let tree = tree();
        let selected = select(&SourceCaptureRequest::entire_tree(), &tree, 6, 100).unwrap();
        assert_eq!(selected, tree);
        let root_only = BTreeMap::from([(Vec::new(), true)]);
        assert_eq!(
            select(&SourceCaptureRequest::entire_tree(), &root_only, 1, 0).unwrap(),
            root_only
        );
        assert_eq!(
            select(&SourceCaptureRequest::entire_tree(), &root_only, 0, 0).unwrap_err(),
            "canonical Source metadata exceeds its 0-row ceiling"
        );
        assert_eq!(
            select(&SourceCaptureRequest::new([]).unwrap(), &tree, 1, 0).unwrap(),
            root_only
        );
    }

    #[test]
    fn missing_or_nondirectory_ancestors_respect_required_and_optional_obligations() {
        let tree = tree();
        for path in [b"absent/member".as_slice(), b"other.omg/member".as_slice()] {
            let selected = select(
                &request(&[(path, SourceCaptureObligation::Optional)]),
                &tree,
                2,
                100,
            )
            .unwrap();
            assert_eq!(selected, BTreeMap::from([(Vec::new(), true)]));
            let error = select(
                &request(&[(path, SourceCaptureObligation::Required)]),
                &tree,
                2,
                100,
            )
            .unwrap_err();
            assert_eq!(
                error,
                format!(
                    "required source capture entry `{}` is absent",
                    String::from_utf8_lossy(path)
                )
            );
        }
    }

    #[test]
    fn caller_budgets_count_ancestor_paths_and_all_selected_rows() {
        let tree = tree();
        let request = request(&[(b"src/main.omg", SourceCaptureObligation::Required)]);
        assert!(select(&request, &tree, 3, 15).is_ok());
        assert_eq!(
            select(&request, &tree, 2, 15).unwrap_err(),
            "canonical Source metadata exceeds its 2-row ceiling"
        );
        assert_eq!(
            select(&request, &tree, 3, 14).unwrap_err(),
            "canonical Source metadata path bytes exceed 14"
        );
    }

    #[test]
    fn malformed_duplicate_and_nested_user_entries_keep_their_refusals() {
        for path in [
            b"".as_slice(),
            b".".as_slice(),
            b"../a".as_slice(),
            b"a//b".as_slice(),
            b"C:/a".as_slice(),
            b"a/Z:entry".as_slice(),
            &[0xff],
        ] {
            let error =
                SourceCaptureRequest::new([(path.to_vec(), SourceCaptureObligation::Required)])
                    .unwrap_err();
            assert!(error.starts_with("source capture entry is not a canonical relative path:"));
        }
        let duplicate = SourceCaptureRequest::new([
            (b"main.omg".to_vec(), SourceCaptureObligation::Required),
            (b"main.omg".to_vec(), SourceCaptureObligation::Optional),
        ])
        .unwrap_err();
        assert!(duplicate.starts_with("source capture entry is declared twice:"));
        let nested = SourceCaptureRequest::new([
            (b"src".to_vec(), SourceCaptureObligation::Optional),
            (b"src/main.omg".to_vec(), SourceCaptureObligation::Required),
        ])
        .unwrap_err();
        assert!(nested.contains("nests inside declared entry"));
    }

    #[test]
    fn inconsistent_directory_observations_cannot_produce_a_successful_selection() {
        let request = request(&[(b"src", SourceCaptureObligation::Required)]);
        let select_children = |children: Vec<Vec<u8>>, missing: bool| {
            select_source_members(
                &request,
                10,
                100,
                |path| {
                    Ok(match path {
                        b"" | b"src" => Some(true),
                        b"src/main.omg" if !missing => Some(false),
                        _ => None,
                    })
                },
                |directory| *directory,
                |_| Ok(children.clone()),
                std::convert::identity,
            )
        };
        assert_eq!(
            select_children(vec![b"src/main.omg".to_vec()], true).unwrap_err(),
            "source capture member `src/main.omg` disappeared during capture"
        );
        assert!(
            select_children(
                vec![b"src/main.omg".to_vec(), b"src/main.omg".to_vec()],
                false
            )
            .unwrap_err()
            .starts_with("Source selection duplicated a path:")
        );
    }

    #[test]
    fn typed_callback_failures_survive_without_becoming_selection_errors() {
        #[derive(Debug, PartialEq, Eq)]
        enum Failure {
            LookupChanged,
            DirectoryChanged,
            Selection(String),
        }
        let lookup = select_source_members(
            &SourceCaptureRequest::entire_tree(),
            10,
            100,
            |_| Err::<Option<bool>, _>(Failure::LookupChanged),
            |directory| *directory,
            |_| unreachable!(),
            Failure::Selection,
        )
        .unwrap_err();
        assert_eq!(lookup, Failure::LookupChanged);
        let directory = select_source_members(
            &SourceCaptureRequest::entire_tree(),
            10,
            100,
            |_| Ok(Some(true)),
            |directory| *directory,
            |_| Err(Failure::DirectoryChanged),
            Failure::Selection,
        )
        .unwrap_err();
        assert_eq!(directory, Failure::DirectoryChanged);
        let missing = select_source_members(
            &request(&[(b"main.omg", SourceCaptureObligation::Required)]),
            10,
            100,
            |path| Ok(path.is_empty().then_some(true)),
            |directory| *directory,
            |_| unreachable!(),
            Failure::Selection,
        )
        .unwrap_err();
        assert_eq!(
            missing,
            Failure::Selection("required source capture entry `main.omg` is absent".to_owned())
        );
    }
}
