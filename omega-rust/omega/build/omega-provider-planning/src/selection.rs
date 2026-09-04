use psi_symbols::SymbolHandle;

/// Whether one build-owned provider edge is fused into its consumer or kept
/// as an independently selected component boundary.
///
/// Omission at the source surface is deliberately [`Self::Fused`]. Provider
/// declarations never infer or widen this mode for themselves.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CompositionMode {
    #[default]
    Fused,
    Independent,
}

/// One exact declaration identity participating in a provider selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelectionIdentity {
    pub symbol: SymbolHandle,
    pub package: Option<psi_core::PackageKeyIdentity>,
    pub canonical_path: String,
    pub authored_path: String,
}

/// One exact boundary-operator overload coordinate belonging to an authored
/// family selection. The readable family path is deliberately absent here:
/// the compiler-derived requirement identity is the dispatch coordinate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOperatorFamilyCoordinate {
    pub symbol: SymbolHandle,
    pub requirement_identity: String,
    /// Number of static arguments in this declaration's telescope. Zero is an
    /// ordinary non-generic overload. A nonzero value remains fail-closed for
    /// package review until final specialization reconstructs and rechecks the
    /// exact compiler-derived applications.
    pub static_parameter_count: usize,
}

/// One exact package-qualified boundary-operator family.
///
/// `coordinates` is a canonical complete roster, not a caller-selected subset.
/// Build evaluation derives it from every applicable declaration at the
/// authored family path and rejects duplicate coordinate identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderOperatorFamilySelection {
    pub package: Option<psi_core::PackageKeyIdentity>,
    pub canonical_path: String,
    pub authored_path: String,
    coordinates: Vec<ProviderOperatorFamilyCoordinate>,
}

impl ProviderOperatorFamilySelection {
    pub fn new(
        package: Option<psi_core::PackageKeyIdentity>,
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

/// Exact declaration subject selected by one ordinary build row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionSubject {
    BoundaryTrait(ProviderSelectionIdentity),
    BoundaryRequirement(ProviderSelectionIdentity),
    BoundaryOperatorFamily(ProviderOperatorFamilySelection),
}

impl ProviderSelectionSubject {
    pub fn package(&self) -> Option<psi_core::PackageKeyIdentity> {
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

    pub fn selects_schema(&self, schema_symbol: SymbolHandle, requirement_identity: &str) -> bool {
        match self {
            Self::BoundaryTrait(identity) | Self::BoundaryRequirement(identity) => {
                identity.symbol == schema_symbol
            }
            Self::BoundaryOperatorFamily(family) => family.coordinates.iter().any(|coordinate| {
                coordinate.symbol == schema_symbol
                    && coordinate.requirement_identity == requirement_identity
            }),
        }
    }

    pub fn same_declaration_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BoundaryTrait(left), Self::BoundaryTrait(right)) => left.symbol == right.symbol,
            (Self::BoundaryRequirement(left), Self::BoundaryRequirement(right)) => {
                left.symbol == right.symbol
            }
            (Self::BoundaryOperatorFamily(left), Self::BoundaryOperatorFamily(right)) => {
                left.package == right.package && left.canonical_path == right.canonical_path
            }
            _ => false,
        }
    }
}

/// Build-selected provider realization for one exact boundary trait, one exact
/// top-level boundary requirement, or one atomically complete boundary-operator
/// family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelection {
    pub subject: ProviderSelectionSubject,
    pub provider_type: ProviderSelectionIdentity,
    pub composition_mode: CompositionMode,
    pub selecting_machine: SymbolHandle,
    pub source_span: psi_source::SourceSpan,
}

#[cfg(test)]
impl ProviderSelection {
    pub fn exact_for_test(boundary_trait: &str, provider_type: &str) -> Self {
        Self {
            subject: ProviderSelectionSubject::BoundaryTrait(ProviderSelectionIdentity {
                symbol: SymbolHandle::invalid(),
                package: None,
                canonical_path: boundary_trait.to_owned(),
                authored_path: boundary_trait.to_owned(),
            }),
            provider_type: ProviderSelectionIdentity {
                symbol: SymbolHandle::invalid(),
                package: None,
                canonical_path: provider_type.to_owned(),
                authored_path: provider_type.to_owned(),
            },
            composition_mode: CompositionMode::Fused,
            selecting_machine: SymbolHandle::invalid(),
            source_span: psi_source::SourceSpan::default(),
        }
    }

    pub fn operator_family_for_test(
        family: &str,
        provider_type: &str,
        coordinates: &[&str],
    ) -> Self {
        Self {
            subject: ProviderSelectionSubject::BoundaryOperatorFamily(
                ProviderOperatorFamilySelection::new(
                    None,
                    family.to_owned(),
                    family.to_owned(),
                    coordinates
                        .iter()
                        .map(|coordinate| ProviderOperatorFamilyCoordinate {
                            symbol: SymbolHandle::invalid(),
                            requirement_identity: (*coordinate).to_owned(),
                            static_parameter_count: 0,
                        })
                        .collect(),
                )
                .expect("test family has canonical coordinates"),
            ),
            provider_type: ProviderSelectionIdentity {
                symbol: SymbolHandle::invalid(),
                package: None,
                canonical_path: provider_type.to_owned(),
                authored_path: provider_type.to_owned(),
            },
            composition_mode: CompositionMode::Fused,
            selecting_machine: SymbolHandle::invalid(),
            source_span: psi_source::SourceSpan::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(
        symbol: u32,
        package_byte: u8,
        canonical_path: &str,
        authored_path: &str,
    ) -> ProviderSelectionIdentity {
        ProviderSelectionIdentity {
            symbol: SymbolHandle::from_arena_index(symbol),
            package: psi_core::PackageKeyIdentity::from_digest([package_byte; 32]),
            canonical_path: canonical_path.to_owned(),
            authored_path: authored_path.to_owned(),
        }
    }

    #[test]
    fn boundary_requirement_projects_its_exact_identity_axes() {
        let requirement = identity(
            17,
            3,
            "core::InterruptAcknowledgement::complete",
            "InterruptAcknowledgement::complete",
        );
        let subject = ProviderSelectionSubject::BoundaryRequirement(requirement.clone());

        assert_eq!(subject.package(), requirement.package);
        assert_eq!(subject.canonical_path(), requirement.canonical_path);
        assert_eq!(subject.authored_path(), requirement.authored_path);
        assert!(subject.selects_schema(requirement.symbol, "untrusted display identity"));
        assert!(!subject.selects_schema(
            SymbolHandle::from_arena_index(18),
            &requirement.canonical_path
        ));
    }

    #[test]
    fn boundary_requirement_declaration_equality_is_exactly_nominal() {
        let exact = identity(
            21,
            5,
            "core::InterruptAcknowledgement::complete",
            "InterruptAcknowledgement::complete",
        );
        let renamed = identity(21, 6, "renamed::complete", "Alias::complete");
        let same_spelled_decoy = identity(
            22,
            5,
            "core::InterruptAcknowledgement::complete",
            "InterruptAcknowledgement::complete",
        );

        let subject = ProviderSelectionSubject::BoundaryRequirement(exact.clone());
        assert!(
            subject.same_declaration_as(&ProviderSelectionSubject::BoundaryRequirement(renamed))
        );
        assert!(
            !subject.same_declaration_as(&ProviderSelectionSubject::BoundaryRequirement(
                same_spelled_decoy
            ))
        );
        assert!(!subject.same_declaration_as(&ProviderSelectionSubject::BoundaryTrait(exact)));
    }
}
