//! Close service schemas and declaration owners at checked-provider binding.
//!
//! Resolved consumers retain source-free records. Only this invocation's
//! catalog maps them back to source symbols for remaining source-contract work.
//! Unused unsupported/local declarations retain refusals instead of introducing
//! new obligations merely because the inventory was prepared eagerly.

use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use checked_trees::typed_trees::TypedTrees;
use checked_trees::typed_trees::service_declarations::ServiceCatalog;
use diagnostics::Diagnostic;
use effects::provider_plan::ServiceSchema;
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;
use symbols::SymbolHandle;

/// Exact declaration and ordered requirement ownership beside the complete
/// target-side schema. This is not a host binding or execution grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDeclaration {
    pub nominal_identity: String,
    pub schema: ServiceSchema,
    pub requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDeclarations {
    sources: Vec<SourceServiceDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceServiceDeclaration {
    symbol: SymbolHandle,
    package: Option<PackageKeyIdentity>,
    path: String,
    nominal_identity: Result<String, Diagnostic>,
    projection: Result<ServiceProjection, Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServiceProjection {
    declaration: Arc<ServiceDeclaration>,
    requirements: Vec<SymbolHandle>,
}

/// Local correspondence borrowed only by source-dependent consumers. Its
/// symbols belong to the catalog's producer, never an independent candidate.
pub struct SourceService<'a> {
    pub symbol: SymbolHandle,
    pub declaration: &'a Arc<ServiceDeclaration>,
    pub requirements: &'a [SymbolHandle],
}

impl SourceServiceDeclaration {
    fn resolved(&self) -> Result<SourceService<'_>, Diagnostic> {
        let projection = self.projection.as_ref().map_err(Clone::clone)?;
        Ok(SourceService {
            symbol: self.symbol,
            declaration: &projection.declaration,
            requirements: &projection.requirements,
        })
    }
}

impl ServiceDeclarations {
    pub fn prepare(
        catalog: ServiceCatalog<'_>,
        calling_plans: &[BoundaryCallingPlanRealization],
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Self {
        let typed = catalog.context().typed;
        let mut sources = Vec::new();
        for definition in catalog.declared_traits() {
            let symbol = definition.schema.symbol();
            let nominal_identity = exact_identity(typed, symbol, exact_toolchain_sources);
            let projection = (|| {
                let nominal_identity = nominal_identity.as_ref().map_err(Clone::clone)?.clone();
                let projected = definition.surface.as_ref().ok_or_else(|| {
                    Diagnostic::error("service declaration has no supported exact schema")
                })?;
                let schema = crate::service_schema::attach_calling(projected, calling_plans);
                let requirements = projected.exact_requirements()?;
                let identities = requirements
                    .iter()
                    .map(|symbol| exact_identity(typed, *symbol, exact_toolchain_sources))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ServiceProjection {
                    declaration: Arc::new(ServiceDeclaration {
                        nominal_identity,
                        schema,
                        requirements: identities,
                    }),
                    requirements,
                })
            })();
            sources.push(SourceServiceDeclaration {
                symbol,
                package: definition.package,
                path: definition.path.to_owned(),
                nominal_identity,
                projection,
            });
        }
        Self { sources }
    }

    /// Package/path is an authored selector, not authority. The caller still
    /// joins the exact schema and any role-required selected-plan commitment.
    pub fn select(
        &self,
        package: PackageKeyIdentity,
        path: &str,
    ) -> Result<SourceService<'_>, Diagnostic> {
        self.unique(
            self.sources
                .iter()
                .filter(|entry| entry.package == Some(package) && entry.path == path),
        )
    }

    pub fn for_symbol(&self, symbol: SymbolHandle) -> Result<SourceService<'_>, Diagnostic> {
        self.unique(self.sources.iter().filter(|entry| entry.symbol == symbol))
    }

    /// Classify a supplied source declaration only after its nominal owner and
    /// complete projected meaning agree with the retained resolved declaration.
    pub fn identifies(
        &self,
        symbol: SymbolHandle,
        expected: &ServiceDeclaration,
    ) -> Result<bool, Diagnostic> {
        let Some(entry) = self.sources.iter().find(|entry| entry.symbol == symbol) else {
            return Ok(false);
        };
        if !entry
            .nominal_identity
            .as_ref()
            .is_ok_and(|identity| *identity == expected.nominal_identity)
        {
            return Ok(false);
        }
        let actual = self.for_symbol(symbol)?;
        if actual.declaration.as_ref() != expected {
            return Err(Diagnostic::error(
                "service declaration changed its exact schema or requirement ownership",
            ));
        }
        Ok(true)
    }

    /// Independent projections compare meaning, never arena handles or catalog
    /// positions. Source consumers receive only this catalog's local symbols.
    pub fn rejoin(&self, expected: &ServiceDeclaration) -> Result<SourceService<'_>, Diagnostic> {
        let actual = self.unique(self.sources.iter().filter(|entry| {
            entry
                .nominal_identity
                .as_ref()
                .is_ok_and(|identity| *identity == expected.nominal_identity)
        }))?;
        if actual.declaration.as_ref() != expected {
            return Err(Diagnostic::error(
                "service declaration changed its exact schema or requirement ownership",
            ));
        }
        Ok(actual)
    }

    fn unique<'a>(
        &'a self,
        mut matches: impl Iterator<Item = &'a SourceServiceDeclaration>,
    ) -> Result<SourceService<'a>, Diagnostic> {
        let Some(first) = matches.next() else {
            return Err(Diagnostic::error(
                "service declaration has no exact Product occurrence",
            ));
        };
        if matches.next().is_some() {
            return Err(Diagnostic::error(
                "service declaration has ambiguous Product occurrences",
            ));
        }
        first.resolved()
    }
}

pub(crate) fn exact_identity(
    typed: &TypedTrees,
    symbol: SymbolHandle,
    sources: &[(source::SourceId, [u8; 32])],
) -> Result<String, Diagnostic> {
    typed
        .exact_owner_nominal_identity(symbol, sources)
        .and_then(|identity| typed.type_identity_text(identity))
        .ok_or_else(|| {
            Diagnostic::error("service declaration has no exact package or toolchain owner")
        })
}
