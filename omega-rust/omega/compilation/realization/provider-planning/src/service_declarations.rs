//! Close service schemas and declaration owners at checked-provider binding.
//!
//! Resolved consumers retain source-free records. Only this invocation's
//! catalog maps them back to source symbols for remaining source-contract work.
//! Unused unsupported/local declarations retain refusals instead of introducing
//! new obligations merely because the inventory was prepared eagerly.

use crate::ProviderPlanningContext;
use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use checked_trees::typed_trees::TypedTrees;
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
        context: ProviderPlanningContext<'_>,
        calling_plans: &[BoundaryCallingPlanRealization],
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Self {
        let typed = context.typed;
        let mut sources = Vec::new();
        for definition in typed.traits() {
            if !definition.is_boundary || !context.contains(definition.symbol) {
                continue;
            }
            let nominal_identity =
                exact_identity(typed, definition.symbol, exact_toolchain_sources);
            let projection = (|| {
                let nominal_identity = nominal_identity.as_ref().map_err(Clone::clone)?.clone();
                let schema = crate::service_schema::from_typed(context, calling_plans, definition)
                    .ok_or_else(|| {
                        Diagnostic::error("service declaration has no supported exact schema")
                    })?;
                let requirements = requirement_symbols(typed, definition.symbol, &schema)?;
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
                symbol: definition.symbol,
                package: typed.symbols.symbol_package_identity(definition.symbol),
                path: typed.symbols.display_path(definition.symbol, "::"),
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

fn exact_identity(
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

/// Resolve schema rows within the exact inherited declaration closure while
/// source still exists. Both custody and independent review use this producer.
fn requirement_symbols(
    typed: &TypedTrees,
    root: SymbolHandle,
    schema: &ServiceSchema,
) -> Result<Vec<SymbolHandle>, Diagnostic> {
    let mut pending = vec![root];
    let mut visited = Vec::new();
    let mut declarations = Vec::new();
    while let Some(symbol) = pending.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let mut matches = typed
            .traits()
            .iter()
            .filter(|definition| definition.symbol == symbol);
        let Some(definition) = matches.next() else {
            return Err(Diagnostic::error(
                "service inheritance has no exact declaration",
            ));
        };
        if matches.next().is_some() {
            return Err(Diagnostic::error(
                "service inheritance has ambiguous declarations",
            ));
        }
        declarations.push(definition);
        pending.extend(
            typed
                .trait_requirements(definition)
                .iter()
                .map(|parent| parent.symbol),
        );
    }
    schema
        .methods
        .iter()
        .map(|method| {
            let mut matches = Vec::new();
            for owner in &declarations {
                if typed.symbols.symbol_package_identity(owner.symbol)
                    != method.requirement_owner_package_identity
                {
                    continue;
                }
                for requirement in typed.trait_machine_signatures(owner) {
                    if typed.symbols.symbol_package_identity(requirement.symbol)
                        == method.requirement_owner_package_identity
                        && typed.trait_requirement_callable_identity_text(owner, requirement)
                            == schema.identity_text(method.requirement_identity)
                    {
                        matches.push(requirement.symbol);
                    }
                }
            }
            let [requirement] = matches.as_slice() else {
                return Err(Diagnostic::error(
                    "service row has no unique exact inherited requirement",
                ));
            };
            Ok(*requirement)
        })
        .collect()
}
