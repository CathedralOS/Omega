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
}
