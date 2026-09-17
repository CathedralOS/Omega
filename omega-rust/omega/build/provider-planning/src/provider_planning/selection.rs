use symbols::SymbolHandle;
use typed_trees::TypedTrees;

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
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
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
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub canonical_path: String,
    pub authored_path: String,
    coordinates: Vec<ProviderOperatorFamilyCoordinate>,
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

    /// Derive the complete canonical family roster from one exact resolved
    /// operator symbol. The family is every boundary overload declared at
    /// the representative's package-qualified path, whether top-level or
    /// domain-owned; declaration order and the spelling the build used are
    /// not part of the result.
    pub fn derive(
        typed: &TypedTrees,
        operator_symbol: SymbolHandle,
        authored_path: String,
    ) -> Result<Self, String> {
        let Some(representative) =
            typed_trees::operator::declaration_by_symbol(typed, operator_symbol)
        else {
            return Err(format!(
                "provider selection subject `{authored_path}` has no exact retained operator declaration"
            ));
        };
        let canonical_path = operator_canonical_path(typed, representative);
        let package = typed.symbols.symbol_package_identity(representative.symbol);
        let coordinates = family_coordinates(typed, package, &canonical_path);
        Self::new(package, canonical_path, authored_path, coordinates)
    }

    pub fn coordinates(&self) -> &[ProviderOperatorFamilyCoordinate] {
        &self.coordinates
    }

    /// Replay this retained roster against the typed program that must have
    /// produced it. Every current member of the family must be present with
    /// its exact symbol and static telescope, and nothing else may be
    /// present: a missing member is a partial row, a member that no longer
    /// resolves is stale, a member resolving outside the family is padding,
    /// and a same-spelled member with another symbol or telescope is a
    /// substitution. Each rejection names the coordinate.
    pub fn replay_against_typed(&self, typed: &TypedTrees) -> Vec<String> {
        let expected = family_coordinates(typed, self.package, &self.canonical_path);
        let mut reasons = Vec::new();
        for coordinate in &self.coordinates {
            let Some(current) = expected
                .iter()
                .find(|current| current.requirement_identity == coordinate.requirement_identity)
            else {
                let reason =
                    match typed_trees::operator::declaration_by_symbol(typed, coordinate.symbol) {
                        None => "no longer resolves to a boundary operator declaration",
                        Some(_) => "resolves to a declaration outside the family",
                    };
                reasons.push(format!(
                    "boundary-operator family `{}` selection retains coordinate `{}`, which {reason}",
                    self.authored_path, coordinate.requirement_identity,
                ));
                continue;
            };
            if current.symbol != coordinate.symbol {
                reasons.push(format!(
                    "boundary-operator family `{}` selection retains coordinate `{}` under a substituted declaration symbol",
                    self.authored_path, coordinate.requirement_identity,
                ));
            }
            if current.static_parameter_count != coordinate.static_parameter_count {
                reasons.push(format!(
                    "boundary-operator family `{}` selection retains coordinate `{}` with static-telescope arity {}, but its exact declaration has arity {}",
                    self.authored_path,
                    coordinate.requirement_identity,
                    coordinate.static_parameter_count,
                    current.static_parameter_count,
                ));
            }
        }
        for current in &expected {
            if !self
                .coordinates
                .iter()
                .any(|coordinate| coordinate.requirement_identity == current.requirement_identity)
            {
                reasons.push(format!(
                    "boundary-operator family `{}` selection omits coordinate `{}`; a family override covers every canonical overload or none",
                    self.authored_path, current.requirement_identity,
                ));
            }
        }
        reasons
    }
}

fn operator_canonical_path(
    typed: &TypedTrees,
    operator: &typed_trees::operator::OperatorDefinition,
) -> String {
    typed
        .operator_path_members(operator.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

/// Every boundary overload declared at one package-qualified operator path,
/// in declaration order; [`ProviderOperatorFamilySelection::new`] canonicalizes
/// and deduplicates the result.
fn family_coordinates(
    typed: &TypedTrees,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
    canonical_path: &str,
) -> Vec<ProviderOperatorFamilyCoordinate> {
    typed
        .operators()
        .iter()
        .chain(
            typed
                .domain_definitions()
                .iter()
                .flat_map(|domain| typed.domain_operators(domain)),
        )
        .filter(|operator| {
            operator.is_boundary
                && typed.symbols.symbol_package_identity(operator.symbol) == package
                && operator_canonical_path(typed, operator) == canonical_path
        })
        .map(|operator| ProviderOperatorFamilyCoordinate {
            symbol: operator.symbol,
            requirement_identity: typed_trees::operator::boundary_operator_requirement_identity(
                typed, operator,
            ),
            static_parameter_count: operator.lifetime_parameters.len()
                + typed.operator_type_parameters(operator).len(),
        })
        .collect()
}

/// Exact declaration subject selected by one ordinary build row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionSubject {
    BoundaryTrait(ProviderSelectionIdentity),
    BoundaryRequirement(ProviderSelectionIdentity),
    BoundaryOperatorFamily(ProviderOperatorFamilySelection),
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
    pub source_span: source::SourceSpan,
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
            source_span: source::SourceSpan::default(),
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
            source_span: source::SourceSpan::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProviderSelectionIdentity, ProviderSelectionSubject, SymbolHandle};

    fn identity(
        symbol: u32,
        package_byte: u8,
        canonical_path: &str,
        authored_path: &str,
    ) -> ProviderSelectionIdentity {
        ProviderSelectionIdentity {
            symbol: SymbolHandle::from_arena_index(symbol),
            package: semantic_vocabulary::PackageKeyIdentity::from_digest([package_byte; 32]),
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
