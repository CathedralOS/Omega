//! Checked declaration coordinates used to select an already checked body.
//! These export identities carry no provider choice or execution permission.

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct BoundaryBodySelection {
    pub requirement: String,
    pub requirement_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub callable: String,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub provider: String,
    pub provider_package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub provider_application: Option<String>,
    pub requirement_lifetime_partition: Vec<u32>,
    /// Includes inherited source target context, not just a local prefix.
    pub target: Option<String>,
    /// Visibility of the complete machine/provider/satisfaction relationship.
    pub is_product: bool,
}

/// A checked declaration or one closed body's eligibility. The payload belongs
/// to the consumer; selection never derives evidence from that payload.
#[derive(Clone, Copy)]
pub struct CheckedBodyCandidate<'candidate, Body> {
    pub body: Body,
    pub declaration: &'candidate BoundaryBodySelection,
    pub target: Option<&'candidate str>,
    pub is_product: bool,
}
