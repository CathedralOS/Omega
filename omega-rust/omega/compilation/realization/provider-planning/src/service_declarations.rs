// Target Calling evidence attachment onto shared, Source-closed service
// declarations. The source-owned census already supplies nominal owners and
// ordered requirement identities (`projected.exact_requirements()?`), so this
// target join never reopens checked trees, while unused unsupported or local
// declarations retain deferred refusals until demanded.

use diagnostics::Diagnostic;
use effects::provider_plan::ServiceSchema;
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;
use symbols::SymbolHandle;

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
