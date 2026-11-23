//! Build and Product dependency registrations emitted by the Build protocol.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub enum DependencyPurpose {
    Product,
    Build,
}

impl DependencyPurpose {
    pub const fn canonical_tag(self) -> u8 {
        match self {
            Self::Product => 0,
            Self::Build => 1,
        }
    }

    pub const ALL: [Self; 2] = [Self::Product, Self::Build];

    pub const fn is_product(self) -> bool {
        matches!(self, Self::Product)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Product => "product",
            Self::Build => "build",
        }
    }

    /// The `Build` operation that registers a dependency for this purpose.
    pub const fn operation(self) -> DependencyOperation {
        match self {
            Self::Product => DependencyOperation::Dependency,
            Self::Build => DependencyOperation::BuildDependency,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DependencyOperation {
    Dependency,
    BuildDependency,
}

impl DependencyOperation {
    pub const ALL: [Self; 2] = [Self::Dependency, Self::BuildDependency];

    pub fn classify(target: &str) -> Option<Self> {
        Some(match target {
            "dependency" => Self::Dependency,
            "build_dependency" => Self::BuildDependency,
            _ => return None,
        })
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Dependency => "dependency",
            Self::BuildDependency => "build_dependency",
        }
    }

    pub const fn purpose(self) -> DependencyPurpose {
        match self {
            Self::Dependency => DependencyPurpose::Product,
            Self::BuildDependency => DependencyPurpose::Build,
        }
    }
}
