//! Declaration-only projection: package review does not need an executable root.
//!
//! Source coordinates are local to this projection. Consumers resolve them to
//! nominal text before comparing independent projections or Terminal artifacts.

use checked_trees::CheckedTrees;
use std::sync::Arc;
use symbols::SymbolHandle;
use terminal_psi::{
    ExternalProviderConformance, NominalDeclarationIdentity, TerminalIdentityTable,
};

mod boundary_review;
mod callbacks;
pub use callbacks::PreparedCallbackDeclarations;
mod prepared;
pub use prepared::PreparedExternalProviderDeclarations;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalProviderDeclarations {
    pub identities: Arc<TerminalIdentityTable>,
    pub conformances: Vec<ExternalProviderConformance>,
    pub callbacks: PreparedCallbackDeclarations,
    declarations: Vec<(
        SymbolHandle,
        Result<NominalDeclarationIdentity, &'static str>,
    )>,
}

impl ExternalProviderDeclarations {
    /// Borrow this producer's exact source-to-nominal correspondence. Source
    /// consumers may join it to the canonical catalog without reopening typed
    /// declarations or inventing identities from normalized signatures.
    pub fn correspondence(
        &self,
    ) -> &[(
        SymbolHandle,
        Result<NominalDeclarationIdentity, &'static str>,
    )] {
        &self.declarations
    }

    /// Eligible shape is retained independently of supported numeric meaning.
    /// Missing candidate facts cannot change the dispatch category selected by
    /// immutable custody.
    pub fn has_requirement_schema(&self, symbol: SymbolHandle) -> bool {
        let Some(nominal) = self.nominal(symbol) else {
            return false;
        };
        self.conformances.iter().any(|candidate| {
            self.identities.text(candidate.requirement.identity()) == Some(nominal)
                && candidate.requirement_schema.is_some()
        })
    }
    /// Resolve a source coordinate in this exact checked owner's namespace.
    pub fn nominal(&self, symbol: SymbolHandle) -> Option<&str> {
        let (_, declaration) = self
            .declarations
            .iter()
            .find(|(source, _)| *source == symbol)?;
        self.identities.text(declaration.as_ref().ok()?.identity())
    }
}

/// Project supplied Product external declarations without selecting or lowering a body.
/// Closed generic requirements come from checking; this operation instantiates
/// nothing and requires no ProgramEntry. The executable producer shares the
/// same append operation, retaining only declarations demanded by its module.
pub fn project_external_provider_declarations(
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
) -> Result<ExternalProviderDeclarations, crate::LoweringError> {
    // Checking-only and same-session programs may have no package owner.
    // These coordinates remain private; executable publication still uses its
    // own portable table and requires exact package/toolchain owners.
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation();
    let mut declarations = Vec::new();
    let conformances = crate::retention::external_providers::project(
        &mut identities,
        checked,
        sources,
        |_, realization, requirement, _| {
            // Equal source can have separate Build and Product instances. A
            // host declaration neither supplies nor competes for a Product
            // satisfaction. Keep distinct Product candidates intact so exact
            // selection still diagnoses genuine ambiguity.
            [realization, requirement].into_iter().all(|symbol| {
                checked
                    .symbols
                    .symbol_is_in_dependency_scope(symbol, source::DependencyScope::Product)
            })
        },
        |symbol, identity| declarations.push((symbol, Ok(identity))),
    )?;
    // Candidate inclusion and declaration correspondence have different
    // lifetimes. Review visits checked requirements even when their source
    // instance supplied no retained Product candidate (for example a separate
    // Build import of the same toolchain declaration). Publish each exact
    // checked requirement's identity here; this adds no satisfaction or plan.
    // Keep a failed identity deferred until review needs that requirement,
    // just as a missing numeric candidate was deferred before this projection.
    let mut represented = declarations
        .iter()
        .map(|(symbol, _)| (symbol.arena_index(), symbol.generation()))
        .collect::<Vec<_>>();
    represented.sort_unstable();
    represented.dedup();
    let requirements = arena::unique_handles_in_order(
        checked
            .facts
            .operators
            .provider_requirements
            .iter()
            .map(|(_, requirement)| requirement.symbol),
    );
    for requirement in requirements {
        if represented
            .binary_search(&(requirement.arena_index(), requirement.generation()))
            .is_ok()
        {
            continue;
        }
        declarations.push((
            requirement,
            crate::machine_lowering::declarations::nominal(
                &mut identities,
                checked,
                sources,
                requirement,
            ),
        ));
    }
    declarations.sort_by_key(|(symbol, declaration)| {
        (
            symbol.arena_index(),
            symbol.generation(),
            declaration
                .as_ref()
                .map(NominalDeclarationIdentity::identity)
                .map_err(|reason| *reason),
        )
    });
    declarations.dedup();
    let callbacks = PreparedCallbackDeclarations::prepare(checked, sources, &mut identities);
    Ok(ExternalProviderDeclarations {
        identities: Arc::new(identities.build()),
        conformances,
        callbacks,
        declarations,
    })
}
