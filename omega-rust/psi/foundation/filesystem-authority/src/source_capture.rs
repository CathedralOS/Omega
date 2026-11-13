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

/// A selection failure generated independently of the metadata provider.
/// Typed provider errors pass through unchanged; callers classify these variants
/// without parsing their diagnostic text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceSelectionError {
    RootNotDirectory,
    RowLimit { limit: usize },
    PathByteLimit { limit: usize },
    MissingRequired { path: Vec<u8> },
    MemberDisappeared { path: Vec<u8> },
    NoncanonicalPath { path: Vec<u8> },
    DuplicatePath { path: Vec<u8> },
}

impl std::fmt::Display for SourceSelectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootNotDirectory => {
                formatter.write_str("canonical Source root is not a directory")
            }
            Self::RowLimit { limit } => write!(
                formatter,
                "canonical Source metadata exceeds its {limit}-row ceiling"
            ),
            Self::PathByteLimit { limit } => write!(
                formatter,
                "canonical Source metadata path bytes exceed {limit}"
            ),
            Self::MissingRequired { path } => write!(
                formatter,
                "required source capture entry `{}` is absent",
                String::from_utf8_lossy(path)
            ),
            Self::MemberDisappeared { path } => write!(
                formatter,
                "source capture member `{}` disappeared during capture",
                String::from_utf8_lossy(path)
            ),
            Self::NoncanonicalPath { path } => {
                write!(formatter, "Source path is not canonical metadata: {path:?}")
            }
            Self::DuplicatePath { path } => {
                write!(formatter, "Source selection duplicated a path: {path:?}")
            }
        }
    }
}

impl std::error::Error for SourceSelectionError {}

/// Selects metadata without enumerating siblings of explicitly selected files.
///
/// `children` supplies the direct canonical children of a selected directory;
/// `lookup` supplies metadata without following a non-directory ancestor.
/// `limits` is `(row_limit, path_byte_limit)`. The root counts toward the row
/// budget; every admitted non-root path, including ancestors, counts toward
/// the path-byte budget. `validate_path` preserves the caller's path profile:
/// Build metadata uses portable canonical paths while native acquisition may
/// admit native filename bytes. User request entries remain portable.
pub fn select_source_members<Member, Error>(
    request: &SourceCaptureRequest,
    limits: (usize, usize),
    mut lookup: impl FnMut(&[u8]) -> Result<Option<Member>, Error>,
    is_directory: impl Fn(&Member) -> bool,
    mut children: impl FnMut(&[u8]) -> Result<Vec<Vec<u8>>, Error>,
    validate_path: impl Fn(&[u8]) -> bool,
    selection_error: impl Fn(SourceSelectionError) -> Error,
) -> Result<BTreeMap<Vec<u8>, Member>, Error> {
    let (row_limit, _) = limits;
    if row_limit == 0 {
        return Err(selection_error(SourceSelectionError::RowLimit { limit: 0 }));
    }
    let root = lookup(&[])?
        .filter(&is_directory)
        .ok_or_else(|| selection_error(SourceSelectionError::RootNotDirectory))?;
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
                limits,
                pending.len(),
                &mut aggregate_path_bytes,
                ancestor,
                &validate_path,
            )
            .map_err(&selection_error)?;
            selected.insert(ancestor.to_vec(), member);
        }
        let member = if reachable { lookup(declared)? } else { None };
        match member {
            Some(member) => {
                charge_scoped_member(
                    &selected,
                    limits,
                    pending.len(),
                    &mut aggregate_path_bytes,
                    declared,
                    &validate_path,
                )
                .map_err(&selection_error)?;
                pending.push((declared.to_vec(), member));
            }
            None if obligation == SourceCaptureObligation::Required => {
                return Err(selection_error(SourceSelectionError::MissingRequired {
                    path: declared.to_vec(),
                }));
            }
            None => {}
        }
    }
    while let Some((path, member)) = pending.pop() {
        if is_directory(&member) {
            for child in children(&path)? {
                charge_scoped_member(
                    &selected,
                    limits,
                    pending.len() + 1,
                    &mut aggregate_path_bytes,
                    &child,
                    &validate_path,
                )
                .map_err(&selection_error)?;
                let member = lookup(&child)?.ok_or_else(|| {
                    selection_error(SourceSelectionError::MemberDisappeared {
                        path: child.clone(),
                    })
                })?;
                pending.push((child, member));
            }
        }
        if selected.insert(path.clone(), member).is_some() {
            return Err(selection_error(SourceSelectionError::DuplicatePath {
                path,
            }));
        }
    }
    Ok(selected)
}

fn charge_scoped_member<Member>(
    selected: &BTreeMap<Vec<u8>, Member>,
    limits: (usize, usize),
    pending_count: usize,
    aggregate_path_bytes: &mut usize,
    path: &[u8],
    validate_path: &impl Fn(&[u8]) -> bool,
) -> Result<(), SourceSelectionError> {
    let (row_limit, path_byte_limit) = limits;
    if selected
        .len()
        .checked_add(pending_count)
        .is_none_or(|count| count >= row_limit)
    {
        return Err(SourceSelectionError::RowLimit { limit: row_limit });
    }
    if !validate_path(path) {
        return Err(SourceSelectionError::NoncanonicalPath {
            path: path.to_vec(),
        });
    }
    *aggregate_path_bytes = aggregate_path_bytes
        .checked_add(path.len())
        .filter(|bytes| *bytes <= path_byte_limit)
        .ok_or(SourceSelectionError::PathByteLimit {
            limit: path_byte_limit,
        })?;
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
            (rows, path_bytes),
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
            |path| crate::canonical_filesystem_metadata_path_is_canonical(path, false),
            |reason| reason.to_string(),
        )
    }

    #[test]
    fn file_selection_reads_ancestors_without_enumerating_or_reading_siblings() {
        let tree = tree();
        let looked_up = RefCell::new(Vec::new());
        let selected = select_source_members(
            &request(&[(b"src/main.omg", SourceCaptureObligation::Required)]),
            (3, 15),
            |path| {
                looked_up.borrow_mut().push(path.to_vec());
                Ok(tree.get(path).copied())
            },
            |directory| *directory,
            |_| panic!("a file selection must not enumerate its parent directory"),
            |path| crate::canonical_filesystem_metadata_path_is_canonical(path, false),
            |reason| reason.to_string(),
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
                (10, 100),
                |path| {
                    Ok(match path {
                        b"" | b"src" => Some(true),
                        b"src/main.omg" if !missing => Some(false),
                        _ => None,
                    })
                },
                |directory| *directory,
                |_| Ok(children.clone()),
                |path| crate::canonical_filesystem_metadata_path_is_canonical(path, false),
                |reason| reason.to_string(),
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
            Selection(SourceSelectionError),
        }
        let lookup = select_source_members(
            &SourceCaptureRequest::entire_tree(),
            (10, 100),
            |_| Err::<Option<bool>, _>(Failure::LookupChanged),
            |directory| *directory,
            |_| unreachable!(),
            |path| crate::canonical_filesystem_metadata_path_is_canonical(path, false),
            Failure::Selection,
        )
        .unwrap_err();
        assert_eq!(lookup, Failure::LookupChanged);
        let directory = select_source_members(
            &SourceCaptureRequest::entire_tree(),
            (10, 100),
            |_| Ok(Some(true)),
            |directory| *directory,
            |_| Err(Failure::DirectoryChanged),
            |path| crate::canonical_filesystem_metadata_path_is_canonical(path, false),
            Failure::Selection,
        )
        .unwrap_err();
        assert_eq!(directory, Failure::DirectoryChanged);
        let missing = select_source_members(
            &request(&[(b"main.omg", SourceCaptureObligation::Required)]),
            (10, 100),
            |path| Ok(path.is_empty().then_some(true)),
            |directory| *directory,
            |_| unreachable!(),
            |path| crate::canonical_filesystem_metadata_path_is_canonical(path, false),
            Failure::Selection,
        )
        .unwrap_err();
        assert_eq!(
            missing,
            Failure::Selection(SourceSelectionError::MissingRequired {
                path: b"main.omg".to_vec()
            })
        );
    }

    #[test]
    fn callers_classify_resource_failures_without_parsing_diagnostics() {
        #[derive(Debug, PartialEq, Eq)]
        enum Failure {
            TooManyFiles(usize),
            Selection(SourceSelectionError),
        }
        let tree = tree();
        let error = select_source_members(
            &request(&[(b"src/main.omg", SourceCaptureObligation::Required)]),
            (2, 100),
            |path| Ok(tree.get(path).copied()),
            |directory| *directory,
            |_| unreachable!(),
            |path| crate::canonical_filesystem_metadata_path_is_canonical(path, false),
            |reason| match reason {
                SourceSelectionError::RowLimit { limit } => Failure::TooManyFiles(limit),
                other => Failure::Selection(other),
            },
        )
        .unwrap_err();
        assert_eq!(error, Failure::TooManyFiles(2));
    }

    #[test]
    fn native_descendant_names_do_not_broaden_portable_user_requests() {
        let names = vec![b"literal\\name".to_vec(), vec![0xff]];
        let tree = BTreeMap::from([
            (Vec::new(), true),
            (names[0].clone(), false),
            (names[1].clone(), false),
        ]);
        let select = |validate_path: fn(&[u8]) -> bool| {
            select_source_members(
                &SourceCaptureRequest::entire_tree(),
                (3, 100),
                |path| Ok(tree.get(path).copied()),
                |directory| *directory,
                |path| {
                    assert!(path.is_empty());
                    Ok(names.clone())
                },
                validate_path,
                std::convert::identity,
            )
        };
        let native = select(|path| {
            !path.is_empty()
                && path != b"."
                && path != b".."
                && !path.contains(&b'/')
                && !path.contains(&0)
        })
        .unwrap();
        assert_eq!(native, tree);
        let portable =
            select(|path| crate::canonical_filesystem_metadata_path_is_canonical(path, false))
                .unwrap_err();
        assert_eq!(
            portable,
            SourceSelectionError::NoncanonicalPath {
                path: names[0].clone()
            }
        );
        for path in names {
            assert!(
                SourceCaptureRequest::new([(path, SourceCaptureObligation::Required)]).is_err()
            );
        }
    }
}
