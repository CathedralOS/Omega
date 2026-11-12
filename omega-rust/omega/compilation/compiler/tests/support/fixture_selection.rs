//! Fixture selection shared by discovery and the execution harness.

use std::collections::BTreeSet;

#[derive(Debug)]
pub enum FixtureSelection {
    All,
    Fragments(Vec<String>),
    Exact(BTreeSet<String>),
}

impl FixtureSelection {
    pub fn fragments(text: &str) -> Self {
        let fragments: Vec<_> = text
            .split(',')
            .map(str::trim)
            .filter(|fragment| !fragment.is_empty())
            .map(str::to_owned)
            .collect();
        if fragments.is_empty() {
            Self::All
        } else {
            Self::Fragments(fragments)
        }
    }

    pub fn exact(paths: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut names = BTreeSet::new();
        for path in paths {
            if path.trim().is_empty() {
                return Err("exact fixture selection contains an empty path".to_owned());
            }
            if !names.insert(path.clone()) {
                return Err(format!("duplicate exact fixture path: {path}"));
            }
        }
        if names.is_empty() {
            return Err("exact fixture selection is empty".to_owned());
        }
        Ok(Self::Exact(names))
    }

    pub fn matches(&self, name: &str) -> bool {
        match self {
            Self::All => true,
            Self::Fragments(fragments) => fragments.iter().any(|part| name.contains(part)),
            Self::Exact(paths) => paths.contains(name),
        }
    }

    pub fn is_unfiltered(&self) -> bool {
        matches!(self, Self::All)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn exact_paths_exclude_prefix_neighbors_and_keep_all_requested_rows() {
        let selection = super::FixtureSelection::exact([
            "pass/build/native_component_reuse".to_owned(),
            "fail/contracts/missing_guarantee".to_owned(),
        ])
        .unwrap();
        assert!(selection.matches("pass/build/native_component_reuse"));
        assert!(selection.matches("fail/contracts/missing_guarantee"));
        assert!(!selection.matches("pass/build/native_component_reuse_foreign"));
        assert!(!selection.matches("pass/contracts/missing_guarantee"));
        assert!(!selection.is_unfiltered());
    }

    #[test]
    fn ordinary_fragment_filters_still_select_prefix_neighbors() {
        let selection =
            super::FixtureSelection::fragments(" build/native_component_reuse,fail/proofs ");
        assert!(selection.matches("pass/build/native_component_reuse_foreign"));
        assert!(selection.matches("fail/proofs/control"));
        assert!(!selection.matches("pass/proofs/control"));
        assert!(super::FixtureSelection::fragments(" , ").is_unfiltered());
    }

    #[test]
    fn empty_or_duplicate_exact_requests_refuse_instead_of_running_unfiltered() {
        assert!(super::FixtureSelection::exact([]).is_err());
        assert!(super::FixtureSelection::exact([String::new()]).is_err());
        assert!(
            super::FixtureSelection::exact(["pass/a/b".to_owned(), "pass/a/b".to_owned()]).is_err()
        );
    }
}
