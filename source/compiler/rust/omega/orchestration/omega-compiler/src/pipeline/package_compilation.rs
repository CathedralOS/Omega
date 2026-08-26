use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

/// One stable package identity, its canonical declared name, and the canonical
/// source root from which this compilation may load it. The name is validated
/// diagnostic metadata and the path is custody/routing data; neither replaces
/// the opaque identity in semantic comparisons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageSourceBinding {
    identity: PackageKeyIdentity,
    canonical_name: String,
    source_root: PathBuf,
}

impl PackageSourceBinding {
    pub fn new(
        identity: PackageKeyIdentity,
        canonical_name: impl Into<String>,
        source_root: PathBuf,
    ) -> Self {
        Self {
            identity,
            canonical_name: canonical_name.into(),
            source_root,
        }
    }

    pub const fn identity(&self) -> PackageKeyIdentity {
        self.identity
    }

    pub fn canonical_name(&self) -> &str {
        &self.canonical_name
    }

    pub fn source_root(&self) -> &Path {
        &self.source_root
    }
}

/// One requester-local alias selected by the reconciled package graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyBinding {
    requester: PackageKeyIdentity,
    alias: String,
    target: PackageKeyIdentity,
}

impl PackageDependencyBinding {
    pub fn new(
        requester: PackageKeyIdentity,
        alias: impl Into<String>,
        target: PackageKeyIdentity,
    ) -> Self {
        Self {
            requester,
            alias: alias.into(),
            target,
        }
    }

    pub const fn requester(&self) -> PackageKeyIdentity {
        self.requester
    }

    pub fn alias(&self) -> &str {
        &self.alias
    }

    pub const fn target(&self) -> PackageKeyIdentity {
        self.target
    }
}

/// Exact, source-path-free dependency closure consumed by one package-aware
/// compilation. This is a semantic subject coordinate, not source custody or
/// an admission verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependencyClosure {
    root: PackageKeyIdentity,
    packages: Vec<PackageKeyIdentity>,
    dependencies: Vec<PackageDependencyBinding>,
}

impl PackageDependencyClosure {
    pub const fn root(&self) -> PackageKeyIdentity {
        self.root
    }

    pub fn packages(&self) -> &[PackageKeyIdentity] {
        &self.packages
    }

    pub fn dependencies(&self) -> &[PackageDependencyBinding] {
        &self.dependencies
    }
}

/// Closed, requester-scoped package bindings accepted by package-aware
/// compilation. Construction validates the complete graph and canonicalizes
/// all source roots before the compiler can consume it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCompilationInputs {
    root: PackageKeyIdentity,
    packages: BTreeMap<PackageKeyIdentity, PathBuf>,
    /// Canonical declared names retained solely for human-facing package
    /// diagnostics. Security decisions continue to compare exact identities.
    package_names: BTreeMap<PackageKeyIdentity, String>,
    dependencies: BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
}

impl PackageCompilationInputs {
    pub fn new(
        root: PackageKeyIdentity,
        packages: Vec<PackageSourceBinding>,
        dependencies: Vec<PackageDependencyBinding>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let mut errors = Vec::new();
        let mut canonical_packages = BTreeMap::new();
        let mut canonical_names = BTreeMap::new();
        let mut roots = BTreeMap::<PathBuf, PackageKeyIdentity>::new();

        for package in packages {
            if !is_kebab_case(&package.canonical_name) {
                errors.push(PackageCompilationInputError::InvalidPackageName {
                    identity: package.identity,
                    name: package.canonical_name.clone(),
                });
            }
            let canonical_root = match canonical_source_root(&package.source_root) {
                Ok(root) => root,
                Err(reason) => {
                    errors.push(PackageCompilationInputError::InvalidSourceRoot {
                        identity: package.identity,
                        path: package.source_root,
                        reason,
                    });
                    continue;
                }
            };

            if canonical_packages
                .insert(package.identity, canonical_root.clone())
                .is_some()
            {
                errors.push(PackageCompilationInputError::DuplicatePackageIdentity {
                    identity: package.identity,
                });
            }
            canonical_names.insert(package.identity, package.canonical_name);
            if let Some(first) = roots.insert(canonical_root.clone(), package.identity) {
                errors.push(PackageCompilationInputError::DuplicateSourceRoot {
                    first,
                    duplicate: package.identity,
                    path: canonical_root,
                });
            }
        }

        let root_rows = roots.iter().collect::<Vec<_>>();
        for (index, (left_root, left_identity)) in root_rows.iter().enumerate() {
            for (right_root, right_identity) in root_rows.iter().skip(index + 1) {
                if left_root.starts_with(right_root) || right_root.starts_with(left_root) {
                    errors.push(PackageCompilationInputError::OverlappingSourceRoots {
                        first: **left_identity,
                        first_root: (*left_root).clone(),
                        second: **right_identity,
                        second_root: (*right_root).clone(),
                    });
                }
            }
        }

        if !canonical_packages.contains_key(&root) {
            errors.push(PackageCompilationInputError::MissingRootPackage { root });
        }

        let mut canonical_dependencies =
            BTreeMap::<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>::new();
        for dependency in dependencies {
            if !is_snake_case(&dependency.alias) {
                errors.push(PackageCompilationInputError::InvalidAlias {
                    requester: dependency.requester,
                    alias: dependency.alias,
                });
                continue;
            }
            if !canonical_packages.contains_key(&dependency.requester) {
                errors.push(PackageCompilationInputError::MissingRequester {
                    requester: dependency.requester,
                });
                continue;
            }
            if !canonical_packages.contains_key(&dependency.target) {
                errors.push(PackageCompilationInputError::MissingTarget {
                    requester: dependency.requester,
                    alias: dependency.alias,
                    target: dependency.target,
                });
                continue;
            }

            let aliases = canonical_dependencies
                .entry(dependency.requester)
                .or_default();
            if aliases
                .insert(dependency.alias.clone(), dependency.target)
                .is_some()
            {
                errors.push(PackageCompilationInputError::DuplicateAlias {
                    requester: dependency.requester,
                    alias: dependency.alias,
                });
            }
        }

        if canonical_packages.contains_key(&root) {
            let reachable = reachable_packages(root, &canonical_dependencies);
            for identity in canonical_packages.keys() {
                if !reachable.contains(identity) {
                    errors.push(PackageCompilationInputError::UnreachablePackage {
                        identity: *identity,
                    });
                }
            }
        }

        if let Some(cycle) = dependency_cycle(&canonical_packages, &canonical_dependencies) {
            errors.push(PackageCompilationInputError::DependencyCycle { cycle });
        }

        if errors.is_empty() {
            Ok(Self {
                root,
                packages: canonical_packages,
                package_names: canonical_names,
                dependencies: canonical_dependencies,
            })
        } else {
            Err(errors)
        }
    }

    pub const fn root(&self) -> PackageKeyIdentity {
        self.root
    }

    pub fn package_root(&self, identity: PackageKeyIdentity) -> Option<&Path> {
        self.packages.get(&identity).map(PathBuf::as_path)
    }

    pub fn package_name(&self, identity: PackageKeyIdentity) -> Option<&str> {
        self.package_names.get(&identity).map(String::as_str)
    }

    pub fn packages(&self) -> impl Iterator<Item = (PackageKeyIdentity, &Path)> {
        self.packages
            .iter()
            .map(|(identity, root)| (*identity, root.as_path()))
    }

    pub fn dependencies(
        &self,
    ) -> impl Iterator<Item = (PackageKeyIdentity, &str, PackageKeyIdentity)> {
        self.dependencies.iter().flat_map(|(requester, aliases)| {
            aliases
                .iter()
                .map(|(alias, target)| (*requester, alias.as_str(), *target))
        })
    }

    /// Project the exact validated graph without source paths, package display
    /// names, immutable source resolutions, or source bytes.
    pub fn dependency_closure(&self) -> PackageDependencyClosure {
        PackageDependencyClosure {
            root: self.root,
            packages: self.packages.keys().copied().collect(),
            dependencies: self
                .dependencies()
                .map(|(requester, alias, target)| {
                    PackageDependencyBinding::new(requester, alias, target)
                })
                .collect(),
        }
    }

    pub(crate) fn dependency_target(
        &self,
        requester: PackageKeyIdentity,
        alias: &str,
    ) -> Option<PackageKeyIdentity> {
        self.dependencies
            .get(&requester)
            .and_then(|aliases| aliases.get(alias))
            .copied()
    }

    pub(crate) fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        requester == owner
            || self
                .dependencies
                .get(&requester)
                .is_some_and(|aliases| aliases.values().any(|target| *target == owner))
    }

    pub(crate) fn package_label(&self, identity: PackageKeyIdentity) -> String {
        match self.package_name(identity) {
            Some(name) => format!("`{name}` ({})", display_identity(identity)),
            None => display_identity(identity),
        }
    }

    pub(crate) fn package_for_source(&self, source: &Path) -> Option<PackageKeyIdentity> {
        self.packages
            .iter()
            .find_map(|(identity, root)| source.starts_with(root).then_some(*identity))
    }

    pub(crate) fn validate_for_compilation(
        &self,
        root_path: &Path,
        toolchain_root: &Path,
    ) -> Result<(), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        for (identity, expected_root) in &self.packages {
            match canonical_source_root(expected_root) {
                Ok(actual_root) if actual_root == *expected_root => {}
                Ok(actual_root) => diagnostics.push(Diagnostic::error(format!(
                    "package source root {} changed identity before compilation (now {})",
                    expected_root.display(),
                    actual_root.display()
                ))),
                Err(reason) => diagnostics.push(Diagnostic::error(format!(
                    "package source root {} is no longer valid for identity {}: {reason}",
                    expected_root.display(),
                    display_identity(*identity)
                ))),
            }
        }

        match root_path.canonicalize() {
            Ok(root_file) => {
                let expected_root = self
                    .packages
                    .get(&self.root)
                    .expect("validated package graph retains its root");
                if !root_file.starts_with(expected_root) {
                    diagnostics.push(Diagnostic::error(format!(
                        "compilation root {} is outside reconciled root package {}",
                        root_file.display(),
                        expected_root.display()
                    )));
                }
            }
            Err(error) => diagnostics.push(Diagnostic::error(format!(
                "failed to canonicalize compilation root {}: {error}",
                root_path.display()
            ))),
        }

        let canonical_toolchain = toolchain_root
            .canonicalize()
            .unwrap_or_else(|_| toolchain_root.to_path_buf());
        for (identity, root) in &self.packages {
            if root.starts_with(&canonical_toolchain) || canonical_toolchain.starts_with(root) {
                diagnostics.push(Diagnostic::error(format!(
                    "package identity {} source root {} overlaps toolchain root {}",
                    display_identity(*identity),
                    root.display(),
                    canonical_toolchain.display()
                )));
            }
        }

        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }
}

impl psi_build_time_evaluation::BuildTimeSelectionAuthority for PackageCompilationInputs {
    fn allows_declaration_selection(
        &self,
        requester: PackageKeyIdentity,
        owner: PackageKeyIdentity,
    ) -> bool {
        self.allows_declaration_selection(requester, owner)
    }

    fn package_label(&self, identity: PackageKeyIdentity) -> String {
        self.package_label(identity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageCompilationInputError {
    InvalidPackageName {
        identity: PackageKeyIdentity,
        name: String,
    },
    InvalidSourceRoot {
        identity: PackageKeyIdentity,
        path: PathBuf,
        reason: String,
    },
    DuplicatePackageIdentity {
        identity: PackageKeyIdentity,
    },
    DuplicateSourceRoot {
        first: PackageKeyIdentity,
        duplicate: PackageKeyIdentity,
        path: PathBuf,
    },
    OverlappingSourceRoots {
        first: PackageKeyIdentity,
        first_root: PathBuf,
        second: PackageKeyIdentity,
        second_root: PathBuf,
    },
    MissingRootPackage {
        root: PackageKeyIdentity,
    },
    InvalidAlias {
        requester: PackageKeyIdentity,
        alias: String,
    },
    MissingRequester {
        requester: PackageKeyIdentity,
    },
    MissingTarget {
        requester: PackageKeyIdentity,
        alias: String,
        target: PackageKeyIdentity,
    },
    DuplicateAlias {
        requester: PackageKeyIdentity,
        alias: String,
    },
    UnreachablePackage {
        identity: PackageKeyIdentity,
    },
    DependencyCycle {
        cycle: Vec<PackageKeyIdentity>,
    },
}

impl fmt::Display for PackageCompilationInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPackageName { identity, name } => write!(
                formatter,
                "package identity {} has invalid canonical name `{name}`; expected lowercase kebab-case",
                display_identity(*identity)
            ),
            Self::InvalidSourceRoot {
                identity,
                path,
                reason,
            } => write!(
                formatter,
                "package identity {} has invalid source root {}: {reason}",
                display_identity(*identity),
                path.display()
            ),
            Self::DuplicatePackageIdentity { identity } => write!(
                formatter,
                "package identity {} has more than one source row",
                display_identity(*identity)
            ),
            Self::DuplicateSourceRoot {
                first,
                duplicate,
                path,
            } => write!(
                formatter,
                "package identities {} and {} share source root {}",
                display_identity(*first),
                display_identity(*duplicate),
                path.display()
            ),
            Self::OverlappingSourceRoots {
                first,
                first_root,
                second,
                second_root,
            } => write!(
                formatter,
                "package identities {} ({}) and {} ({}) have overlapping source roots",
                display_identity(*first),
                first_root.display(),
                display_identity(*second),
                second_root.display()
            ),
            Self::MissingRootPackage { root } => write!(
                formatter,
                "root package identity {} has no source row",
                display_identity(*root)
            ),
            Self::InvalidAlias { requester, alias } => write!(
                formatter,
                "package identity {} has invalid dependency alias `{alias}`",
                display_identity(*requester)
            ),
            Self::MissingRequester { requester } => write!(
                formatter,
                "dependency requester {} has no package source row",
                display_identity(*requester)
            ),
            Self::MissingTarget {
                requester,
                alias,
                target,
            } => write!(
                formatter,
                "dependency `{alias}` from {} targets missing package {}",
                display_identity(*requester),
                display_identity(*target)
            ),
            Self::DuplicateAlias { requester, alias } => write!(
                formatter,
                "package identity {} binds dependency alias `{alias}` more than once",
                display_identity(*requester)
            ),
            Self::UnreachablePackage { identity } => write!(
                formatter,
                "package identity {} is unreachable from the root package",
                display_identity(*identity)
            ),
            Self::DependencyCycle { cycle } => {
                write!(formatter, "package dependency cycle")?;
                for identity in cycle {
                    write!(formatter, " -> {}", display_identity(*identity))?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for PackageCompilationInputError {}

fn canonical_source_root(path: &Path) -> Result<PathBuf, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("cannot inspect source root: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Err("source root must not be a symbolic link".to_owned());
    }
    if !metadata.is_dir() {
        return Err("source root is not a directory".to_owned());
    }
    path.canonicalize()
        .map_err(|error| format!("cannot canonicalize source root: {error}"))
}

fn is_snake_case(value: &str) -> bool {
    value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && !value.ends_with('_')
        && !value.contains("__")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn is_kebab_case(value: &str) -> bool {
    if !value.as_bytes().first().is_some_and(u8::is_ascii_lowercase) || value.ends_with('-') {
        return false;
    }

    let mut previous_separator = false;
    for byte in value.bytes() {
        if byte == b'-' {
            if previous_separator {
                return false;
            }
            previous_separator = true;
            continue;
        }
        previous_separator = false;
        if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() {
            return false;
        }
    }
    true
}

fn reachable_packages(
    root: PackageKeyIdentity,
    dependencies: &BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
) -> BTreeSet<PackageKeyIdentity> {
    let mut reachable = BTreeSet::new();
    let mut pending = vec![root];
    while let Some(identity) = pending.pop() {
        if !reachable.insert(identity) {
            continue;
        }
        if let Some(targets) = dependencies.get(&identity) {
            pending.extend(targets.values().copied());
        }
    }
    reachable
}

fn dependency_cycle(
    packages: &BTreeMap<PackageKeyIdentity, PathBuf>,
    dependencies: &BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
) -> Option<Vec<PackageKeyIdentity>> {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Visit {
        Active,
        Complete,
    }

    fn visit(
        identity: PackageKeyIdentity,
        dependencies: &BTreeMap<PackageKeyIdentity, BTreeMap<String, PackageKeyIdentity>>,
        states: &mut BTreeMap<PackageKeyIdentity, Visit>,
        stack: &mut Vec<PackageKeyIdentity>,
    ) -> Option<Vec<PackageKeyIdentity>> {
        if states.get(&identity) == Some(&Visit::Complete) {
            return None;
        }
        if states.get(&identity) == Some(&Visit::Active) {
            let start = stack.iter().position(|entry| *entry == identity)?;
            let mut cycle = stack[start..].to_vec();
            cycle.push(identity);
            return Some(cycle);
        }

        states.insert(identity, Visit::Active);
        stack.push(identity);
        if let Some(targets) = dependencies.get(&identity) {
            for target in targets.values().copied() {
                if let Some(cycle) = visit(target, dependencies, states, stack) {
                    return Some(cycle);
                }
            }
        }
        stack.pop();
        states.insert(identity, Visit::Complete);
        None
    }

    let mut states = BTreeMap::new();
    let mut stack = Vec::new();
    for identity in packages.keys().copied() {
        if let Some(cycle) = visit(identity, dependencies, &mut states, &mut stack) {
            return Some(cycle);
        }
    }
    None
}

fn display_identity(identity: PackageKeyIdentity) -> String {
    let digest = identity.digest();
    let mut display = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(display, "{byte:02x}");
    }
    display
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "omega-package-compilation-{}-{}",
                std::process::id(),
                NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("create temporary package tree");
            Self(path)
        }

        fn package(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir(&path).expect("create package root");
            path
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn identity(marker: u8) -> PackageKeyIdentity {
        PackageKeyIdentity::from_digest([marker; 32]).expect("nonzero package identity")
    }

    #[test]
    fn requester_local_aliases_may_name_different_targets() {
        let tree = TempTree::new();
        let packages = (1..=4)
            .map(|marker| {
                PackageSourceBinding::new(
                    identity(marker),
                    format!("package-{marker}"),
                    tree.package(&marker.to_string()),
                )
            })
            .collect();
        let inputs = PackageCompilationInputs::new(
            identity(1),
            packages,
            vec![
                PackageDependencyBinding::new(identity(1), "shared", identity(2)),
                PackageDependencyBinding::new(identity(2), "shared", identity(3)),
                PackageDependencyBinding::new(identity(3), "leaf", identity(4)),
            ],
        )
        .expect("requester-local aliases should reconcile");

        assert_eq!(
            inputs.dependency_target(identity(1), "shared"),
            Some(identity(2))
        );
        assert_eq!(
            inputs.dependency_target(identity(2), "shared"),
            Some(identity(3))
        );
        assert_eq!(inputs.package_name(identity(1)), Some("package-1"));
        assert!(inputs.allows_declaration_selection(identity(1), identity(1)));
        assert!(inputs.allows_declaration_selection(identity(1), identity(2)));
        assert!(!inputs.allows_declaration_selection(identity(1), identity(3)));
        assert!(inputs.allows_declaration_selection(identity(2), identity(3)));
        assert!(
            inputs
                .package_label(identity(1))
                .starts_with("`package-1` (")
        );
    }

    #[test]
    fn noncanonical_package_names_reject_at_compiler_handoff() {
        let tree = TempTree::new();
        let errors = PackageCompilationInputs::new(
            identity(1),
            vec![PackageSourceBinding::new(
                identity(1),
                "not_canonical",
                tree.package("root"),
            )],
            Vec::new(),
        )
        .expect_err("compiler inputs must independently reject noncanonical package names");

        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::InvalidPackageName { identity: found, name }
                if *found == identity(1) && name == "not_canonical"
        )));
    }

    #[test]
    fn duplicate_aliases_and_unreachable_rows_reject() {
        let tree = TempTree::new();
        let errors = PackageCompilationInputs::new(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", tree.package("root")),
                PackageSourceBinding::new(identity(2), "first", tree.package("first")),
                PackageSourceBinding::new(identity(3), "second", tree.package("second")),
            ],
            vec![
                PackageDependencyBinding::new(identity(1), "dep", identity(2)),
                PackageDependencyBinding::new(identity(1), "dep", identity(2)),
            ],
        )
        .expect_err("duplicate alias and unreachable package must reject");

        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::DuplicateAlias { alias, .. } if alias == "dep"
        )));
        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::UnreachablePackage { identity: found }
                if *found == identity(3)
        )));
    }

    #[test]
    fn overlapping_roots_and_cycles_reject() {
        let tree = TempTree::new();
        let root = tree.package("root");
        let nested = root.join("nested");
        fs::create_dir(&nested).expect("create nested package");
        let errors = PackageCompilationInputs::new(
            identity(1),
            vec![
                PackageSourceBinding::new(identity(1), "root", root),
                PackageSourceBinding::new(identity(2), "nested", nested),
            ],
            vec![
                PackageDependencyBinding::new(identity(1), "child", identity(2)),
                PackageDependencyBinding::new(identity(2), "parent", identity(1)),
            ],
        )
        .expect_err("overlap and cycle must reject");

        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::OverlappingSourceRoots { .. }
        )));
        assert!(
            errors
                .iter()
                .any(|error| matches!(error, PackageCompilationInputError::DependencyCycle { .. }))
        );
    }

    #[test]
    fn missing_and_symlink_source_roots_reject() {
        let tree = TempTree::new();
        let missing = tree.0.join("missing");
        let errors = PackageCompilationInputs::new(
            identity(1),
            vec![PackageSourceBinding::new(identity(1), "root", missing)],
            Vec::new(),
        )
        .expect_err("missing source root must reject");
        assert!(errors.iter().any(|error| matches!(
            error,
            PackageCompilationInputError::InvalidSourceRoot { .. }
        )));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let actual = tree.package("actual");
            let linked = tree.0.join("linked");
            symlink(actual, &linked).expect("create source-root symlink");
            let errors = PackageCompilationInputs::new(
                identity(1),
                vec![PackageSourceBinding::new(identity(1), "root", linked)],
                Vec::new(),
            )
            .expect_err("symlink source root must reject");
            assert!(errors.iter().any(|error| matches!(
                error,
                PackageCompilationInputError::InvalidSourceRoot { .. }
            )));
        }
    }
}
