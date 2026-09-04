/// Canonical authored operand kind and order for one `FilesystemHost`
/// requirement. This is evaluator ABI schema, not provider behavior: even an
/// operand a modeled provider does not use must be prepared exactly once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FilesystemHostOperandKind {
    PathBytes,
    Bytes,
    I32,
    U32,
    I64,
    U64,
    MutableBytes,
    MutableI64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FilesystemHostResultKind {
    I32,
    I64,
}

// The enum, declaration-order transcript tags, canonical names, and exact
// operand/result schemas are generated from the one Omega boundary declaration.
include!(concat!(env!("OUT_DIR"), "/filesystem_host_operations.rs"));

impl FilesystemHostOperation {
    /// Classify operations whose returned bytes may reveal an absolute host
    /// path. A rooted transcript must virtualize or reject such a result.
    #[cfg(test)]
    const fn path_result_exposure(self) -> PathResultExposure {
        match self {
            Self::ReadLink => PathResultExposure::MayBeAbsolute,
            Self::Canonicalize | Self::FinalPathNameByHandle => PathResultExposure::AlwaysAbsolute,
            _ => PathResultExposure::None,
        }
    }
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathResultExposure {
    None,
    MayBeAbsolute,
    AlwaysAbsolute,
}

impl std::fmt::Display for FilesystemHostOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.canonical_name())
    }
}

#[cfg(test)]
mod tests {
    use super::{FilesystemHostOperation, PathResultExposure};
    use std::collections::BTreeSet;

    #[test]
    fn generated_names_and_operation_tags_are_unique_and_round_trip() {
        let mut names = BTreeSet::new();
        let mut tags = BTreeSet::new();
        for operation in FilesystemHostOperation::ALL {
            assert!(names.insert(operation.canonical_name()));
            assert!(tags.insert(operation.operation_tag()));
            assert_eq!(
                FilesystemHostOperation::from_canonical_name(operation.canonical_name()),
                Some(operation)
            );
        }
    }

    #[test]
    fn path_result_exposure_covers_conditional_and_unconditional_cases() {
        let exposed: Vec<_> = FilesystemHostOperation::ALL
            .into_iter()
            .filter_map(|operation| {
                let exposure = operation.path_result_exposure();
                (exposure != PathResultExposure::None)
                    .then_some((operation.canonical_name(), exposure))
            })
            .collect();
        assert_eq!(
            exposed,
            vec![
                ("read_link", PathResultExposure::MayBeAbsolute),
                ("canonicalize", PathResultExposure::AlwaysAbsolute),
                (
                    "final_path_name_by_handle",
                    PathResultExposure::AlwaysAbsolute
                ),
            ]
        );
    }
}
