//! Source-owned provider selection inputs retain the exact authored origin.
//! Native planning receives the closed canonical selections instead.
use declaration_contracts::selection::CompositionMode;
use provider_choices::ProviderApplicationId;
use symbols::SymbolHandle;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelectionIdentity {
    pub symbol: SymbolHandle,
    pub application: ProviderApplicationId,
    pub applied: bool,
    pub application_identity: Option<String>,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub canonical_path: String,
    pub authored_path: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOperatorFamilyCoordinate {
    pub symbol: SymbolHandle,
    pub requirement_identity: String,
    pub static_parameter_count: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOperatorFamilySelection {
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub canonical_path: String,
    pub authored_path: String,
    coordinates: Vec<ProviderOperatorFamilyCoordinate>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionSubject {
    BoundaryTrait(ProviderSelectionIdentity),
    BoundaryRequirement(ProviderSelectionIdentity),
    BoundaryOperatorFamily(ProviderOperatorFamilySelection),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelection {
    pub target: Option<declaration_contracts::selection::ProductTarget>,
    pub subject: ProviderSelectionSubject,
    pub provider_type: ProviderSelectionIdentity,
    pub composition_mode: CompositionMode,
    pub origin: crate::declaration_selection::BuildSelectionOrigin,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetProviderDefaults {
    pub target: Option<String>,
    pub machines: Vec<symbols::SymbolHandle>,
    pub selections: Vec<ProviderSelection>,
}
impl ProviderOperatorFamilySelection {
    pub fn new(
        package: Option<semantic_vocabulary::PackageKeyIdentity>,
        canonical_path: String,
        authored_path: String,
        mut coordinates: Vec<ProviderOperatorFamilyCoordinate>,
    ) -> Result<Self, String> {
        coordinates
            .sort_by(|left, right| left.requirement_identity.cmp(&right.requirement_identity));
        if coordinates.is_empty() {
            return Err(format!(
                "boundary-operator family `{authored_path}` contains no applicable coordinates"
            ));
        }
        for pair in coordinates.windows(2) {
            if pair[0].requirement_identity == pair[1].requirement_identity {
                return Err(format!(
                    "boundary-operator family `{authored_path}` contains ambiguous coordinate `{}`",
                    pair[0].requirement_identity
                ));
            }
        }
        Ok(Self {
            package,
            canonical_path,
            authored_path,
            coordinates,
        })
    }
    pub fn coordinates(&self) -> &[ProviderOperatorFamilyCoordinate] {
        &self.coordinates
    }
}
impl ProviderSelectionSubject {
    pub fn package(&self) -> Option<semantic_vocabulary::PackageKeyIdentity> {
        match self {
            Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => identity.package,
            Self::BoundaryOperatorFamily(family) => family.package,
        }
    }
    pub fn canonical_path(&self) -> &str {
        match self {
            Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                &identity.canonical_path
            }
            Self::BoundaryOperatorFamily(family) => &family.canonical_path,
        }
    }
    pub fn authored_path(&self) -> &str {
        match self {
            Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                &identity.authored_path
            }
            Self::BoundaryOperatorFamily(family) => &family.authored_path,
        }
    }
    pub fn same_declaration_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BoundaryTrait(left), Self::BoundaryTrait(right)) => {
                left.symbol == right.symbol
                    && left.application_identity == right.application_identity
            }
            (Self::BoundaryRequirement(left), Self::BoundaryRequirement(right)) => {
                left.symbol == right.symbol
                    && left.application_identity == right.application_identity
            }
            (Self::BoundaryOperatorFamily(left), Self::BoundaryOperatorFamily(right)) => {
                left.package == right.package && left.canonical_path == right.canonical_path
            }
            _ => false,
        }
    }
}
