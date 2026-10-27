//! Produce an explicitly named checked implementation, without native settlement.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;

/// Source-issued selection of a public implementation in one immutable package.
/// Its methods borrow portable code; callers cannot replace the selected declaration.
pub struct ProducedBuildProvider {
    code: Option<terminal_codec::CanonicalTerminalArtifact>,
    path: String,
    package: semantic_vocabulary::PackageKeyIdentity,
    declaration: String,
    requirement: String,
    application: String,
}

impl ProducedBuildProvider {
    pub fn artifact(&self) -> Option<&terminal_codec::CanonicalTerminalArtifact> {
        self.code.as_ref()
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn package(&self) -> semantic_vocabulary::PackageKeyIdentity {
        self.package
    }
    pub fn declaration(&self) -> &str {
        &self.declaration
    }
    pub fn requirement(&self) -> &str {
        &self.requirement
    }
    pub fn application(&self) -> &str {
        &self.application
    }
}

pub struct BuildProviderProductionRequest<'checked> {
    pub checked: &'checked CheckedTrees,
    pub exact_toolchain_sources: &'checked [(source::SourceId, [u8; 32])],
    pub package: semantic_vocabulary::PackageKeyIdentity,
    /// Exact namespace-qualified path within `package`, without its dependency alias.
    pub declaration_path: &'checked str,
}

impl BuildProviderProductionRequest<'_> {
    pub fn produce(self) -> Result<ProducedBuildProvider, Vec<Diagnostic>> {
        let refuse = |reason: &str| vec![Diagnostic::error(reason)];
        let checked = self.checked;
        let mut candidates = checked.conformances().iter().filter(|conformance| {
            conformance.is_public
                && checked
                    .symbols
                    .symbol_product_package_identity(conformance.symbol)
                    == Some(self.package)
                && checked.symbols.display_path(conformance.symbol, "::") == self.declaration_path
        });
        let selected = candidates.next().ok_or_else(|| {
            refuse("Build backing names no public implementation in its exact dependency package")
        })?;
        if candidates.next().is_some() {
            return Err(refuse(
                "Build backing declaration is ambiguous in its dependency package",
            ));
        }
        let scope =
            typed_trees::trait_definition::AppliedTraitScope::from_conformance(checked, selected)
                .ok_or_else(|| {
                refuse("Build backing has no complete checked requirement application")
            })?;
        if !scope.declaration.is_boundary
            || !selected.type_parameters.is_empty()
            || !selected.lifetime_parameters.is_empty()
        {
            return Err(refuse(
                "Build backing must name a closed implementation of a boundary requirement",
            ));
        }
        checked
            .require_receiving_trait_operands(&scope)
            .map_err(refuse)?;
        let nominal = |symbol| {
            checked
                .exact_owner_nominal_identity(symbol, self.exact_toolchain_sources)
                .and_then(|identity| checked.type_identity_text(identity))
                .ok_or_else(|| refuse("Build provider declaration lost its exact portable owner"))
        };
        let declaration = nominal(selected.symbol)?;
        let requirement = nominal(scope.declaration.symbol)?;
        let application = scope
            .portable_application_identity(checked)
            .and_then(|identity| checked.type_identity_text(identity))
            .ok_or_else(|| {
                refuse("Build backing requirement application is not closed and portable")
            })?;
        let machines = selected
            .rows
            .iter()
            .map(|row| row.realization_machine)
            .collect::<Vec<_>>();
        let boundaries = selected
            .rows
            .iter()
            .map(|row| row.requirement)
            .collect::<Vec<_>>();
        let code = match machines.first() {
            None => None,
            Some(entry) => {
                let mut request = crate::TerminalProductionRequest::new(
                    checked,
                    crate::TerminalMachineSelection::Symbol(*entry),
                    self.exact_toolchain_sources,
                );
                request.roots = crate::TerminalProgramRoots {
                    machines: &machines,
                    boundaries: &boundaries,
                };
                let produced = request
                    .produce(Default::default(), &mut crate::UnmeasuredProduction)
                    .map_err(|error| error.diagnostics())?;
                Some(produced.into_artifact().map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "Build provider publication: {error:?}"
                    ))]
                })?)
            }
        };
        Ok(ProducedBuildProvider {
            code,
            path: self.declaration_path.to_owned(),
            package: self.package,
            declaration,
            requirement,
            application,
        })
    }
}
