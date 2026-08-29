use psi_symbols::SymbolHandle;

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
    /// ordinary non-generic overload. A nonzero value
    /// requires one exact indexed-application coverage row before the family
    /// can enter retained package-review evidence.
    pub static_parameter_count: usize,
}

/// Exact indexed applications retained for one selected operator-family
/// coordinate. The selected plan identity is a report coordinate only; the
/// applications retain their normalized structural identities.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProviderOperatorFamilyExactApplicationCoverage {
    requirement_identity: String,
    provider_plan_report_identity: u64,
    applications: Vec<omega_effects::ConcreteIndexedProviderApplication>,
}

impl ProviderOperatorFamilyExactApplicationCoverage {
    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn provider_plan_report_identity(&self) -> u64 {
        self.provider_plan_report_identity
    }

    pub fn applications(&self) -> &[omega_effects::ConcreteIndexedProviderApplication] {
        &self.applications
    }
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

    /// Rejoin exact indexed-application coverage already attached to the
    /// selected provider closure with this compiler-derived family roster.
    ///
    /// This consumes no authored coverage assertion. Generic coverage remains
    /// fail-closed until the compiler retains proof that the realization is
    /// genuinely generic. Non-generic coordinates must not carry an indexed
    /// coverage row.
    pub fn exact_application_coverage(
        &self,
        selected: &omega_effects::SelectedProviderPlanFacts,
        provider_type: &ProviderSelectionIdentity,
    ) -> Result<Vec<ProviderOperatorFamilyExactApplicationCoverage>, String> {
        let mut retained = Vec::new();
        for coordinate in &self.coordinates {
            let plans = selected
                .plans()
                .iter()
                .filter(|plan| {
                    plan.schema.trait_package_identity == self.package
                        && plan.schema.trait_name == coordinate.requirement_identity
                        && plan.provider_type_package_identity == provider_type.package
                        && plan.provider_type == provider_type.canonical_path
                })
                .collect::<Vec<_>>();
            let [plan] = plans.as_slice() else {
                return Err(format!(
                    "selected boundary-operator coordinate `{}` maps to {} exact selected provider plans for provider `{}`; expected one",
                    coordinate.requirement_identity,
                    plans.len(),
                    provider_type.canonical_path,
                ));
            };
            let coverage = selected
                .indexed_provider_application_coverage()
                .iter()
                .filter(|coverage| {
                    coverage.provider_plan_report_identity() == plan.report_fingerprint()
                        && coverage.schema().trait_package_identity() == self.package
                        && coverage.schema().trait_name() == coordinate.requirement_identity
                })
                .collect::<Vec<_>>();

            if coordinate.static_parameter_count == 0 {
                if !coverage.is_empty() {
                    return Err(format!(
                        "non-generic boundary-operator coordinate `{}` carries indexed-application coverage",
                        coordinate.requirement_identity,
                    ));
                }
                continue;
            }

            let [coverage] = coverage.as_slice() else {
                return Err(format!(
                    "generic boundary-operator coordinate `{}` has {} exact selected coverage rows; expected one",
                    coordinate.requirement_identity,
                    coverage.len(),
                ));
            };
            if coverage.covers_generically() {
                return Err(format!(
                    "generic coverage for boundary-operator coordinate `{}` remains inadmissible until generic realization proof is retained",
                    coordinate.requirement_identity,
                ));
            }
            if coverage.schema().application_arity() != coordinate.static_parameter_count {
                return Err(format!(
                    "boundary-operator coordinate `{}` has {} static parameter(s), but retained coverage has arity {}",
                    coordinate.requirement_identity,
                    coordinate.static_parameter_count,
                    coverage.schema().application_arity(),
                ));
            }
            let applications = coverage
                .exact_applications()
                .expect("generic coverage was rejected above")
                .to_vec();
            if applications.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(format!(
                    "exact application coverage for boundary-operator coordinate `{}` is not canonically ordered or contains a duplicate",
                    coordinate.requirement_identity,
                ));
            }
            retained.push(ProviderOperatorFamilyExactApplicationCoverage {
                requirement_identity: coordinate.requirement_identity.clone(),
                provider_plan_report_identity: plan.report_fingerprint(),
                applications,
            });
        }
        Ok(retained)
    }
}

/// Exact declaration subject selected by one ordinary build row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelectionSubject {
    BoundaryTrait(ProviderSelectionIdentity),
    BoundaryOperatorFamily(ProviderOperatorFamilySelection),
}

impl ProviderSelectionSubject {
    pub fn package(&self) -> Option<psi_core::PackageKeyIdentity> {
        match self {
            Self::BoundaryTrait(identity) => identity.package,
            Self::BoundaryOperatorFamily(family) => family.package,
        }
    }

    pub fn canonical_path(&self) -> &str {
        match self {
            Self::BoundaryTrait(identity) => &identity.canonical_path,
            Self::BoundaryOperatorFamily(family) => &family.canonical_path,
        }
    }

    pub fn authored_path(&self) -> &str {
        match self {
            Self::BoundaryTrait(identity) => &identity.authored_path,
            Self::BoundaryOperatorFamily(family) => &family.authored_path,
        }
    }

    pub fn selects_schema(&self, schema_symbol: SymbolHandle, requirement_identity: &str) -> bool {
        match self {
            Self::BoundaryTrait(identity) => identity.symbol == schema_symbol,
            Self::BoundaryOperatorFamily(family) => family.coordinates.iter().any(|coordinate| {
                coordinate.symbol == schema_symbol
                    && coordinate.requirement_identity == requirement_identity
            }),
        }
    }

    pub fn same_declaration_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::BoundaryTrait(left), Self::BoundaryTrait(right)) => left.symbol == right.symbol,
            (Self::BoundaryOperatorFamily(left), Self::BoundaryOperatorFamily(right)) => {
                left.package == right.package && left.canonical_path == right.canonical_path
            }
            _ => false,
        }
    }
}

/// Build-selected provider realization for one exact boundary trait or one
/// atomically complete boundary-operator family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelection {
    pub subject: ProviderSelectionSubject,
    pub provider_type: ProviderSelectionIdentity,
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
            selecting_machine: SymbolHandle::invalid(),
            source_span: psi_source::SourceSpan::default(),
        }
    }

    pub fn operator_family_with_application_arity_for_test(
        family: &str,
        provider_type: &str,
        coordinates: &[(&str, usize)],
    ) -> Self {
        Self {
            subject: ProviderSelectionSubject::BoundaryOperatorFamily(
                ProviderOperatorFamilySelection::new(
                    None,
                    family.to_owned(),
                    family.to_owned(),
                    coordinates
                        .iter()
                        .map(|(coordinate, static_parameter_count)| {
                            ProviderOperatorFamilyCoordinate {
                                symbol: SymbolHandle::invalid(),
                                requirement_identity: (*coordinate).to_owned(),
                                static_parameter_count: *static_parameter_count,
                            }
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
            selecting_machine: SymbolHandle::invalid(),
            source_span: psi_source::SourceSpan::default(),
        }
    }
}
