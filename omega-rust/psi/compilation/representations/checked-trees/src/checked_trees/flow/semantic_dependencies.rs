use symbols::SymbolHandle;

/// Whether a compiler-derived semantic dependency contributes only to an
/// implementation artifact or is exposed by a published signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckedSemanticDependencyExposure {
    PrivateImplementation,
    PublicInterface,
}

/// The checked semantic fact for which one exact declaration is carried.
///
/// These rows grant no source-name authority. Multiple kinds may name the
/// same declaration because nominal identity, representation, ownership, and
/// automatic cleanup participate in different compatibility decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckedSemanticDependencyKind {
    NominalIdentity,
    Layout,
    OwnershipBehavior,
    AutomaticCleanup,
    AutomaticCleanupMachine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedSemanticDependency {
    pub consumer_machine: SymbolHandle,
    pub dependency: SymbolHandle,
    pub exposure: CheckedSemanticDependencyExposure,
    pub kind: CheckedSemanticDependencyKind,
}

/// Deterministically ordered, package-neutral dependencies derived from
/// successful checking. Exact package owners are joined by Omega orchestration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CheckedSemanticDependencies {
    pub rows: Vec<CheckedSemanticDependency>,
}

impl CheckedSemanticDependencies {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &CheckedSemanticDependency> {
        self.rows.iter()
    }
}
