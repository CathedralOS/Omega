//! Declaration-only projection: package review does not need an executable root.
//!
//! Source coordinates are local to this projection. Consumers resolve them to
//! nominal text before comparing independent projections or Terminal artifacts.

use checked_trees::CheckedTrees;
use std::sync::Arc;
use symbols::SymbolHandle;
use terminal_psi::{NominalDeclarationIdentity, NumericRealization, TerminalIdentityTable};

mod boundary_review;
mod callbacks;
pub use callbacks::PreparedCallbackDeclarations;
mod prepared;
pub use prepared::PreparedDeclarationCatalog;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationCatalog {
    pub identities: Arc<TerminalIdentityTable>,
    pub conformances: Vec<NumericRealization>,
    pub callbacks: PreparedCallbackDeclarations,
    declarations: Vec<(SymbolHandle, NominalDeclarationIdentity)>,
}

impl DeclarationCatalog {
    /// Exact source-to-nominal correspondence owned by this prepared catalogue.
    pub fn correspondence(&self) -> &[(SymbolHandle, NominalDeclarationIdentity)] {
        &self.declarations
    }

    /// Exact retained numeric schema; absence grants no alternative execution.
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
        self.identities.text(declaration.identity())
    }
}

/// Project supplied external declarations without selecting or lowering a body.
/// Closed generic requirements come from checking; this operation instantiates
/// nothing and requires no ProgramEntry. The executable producer shares the
/// same append operation, retaining only declarations demanded by its module.
pub fn project_declaration_catalog(
    checked: &CheckedTrees,
    sources: &[(source::SourceId, [u8; 32])],
) -> Result<DeclarationCatalog, crate::LoweringError> {
    // Checking-only and same-session programs may have no package owner.
    // These coordinates remain private; executable publication still uses its
    // own portable table and requires exact package/toolchain owners.
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation();
    let mut declarations = Vec::new();
    let conformances = crate::retention::numeric_realizations::project(
        &mut identities,
        checked,
        sources,
        |_, _| true,
        |symbol, identity| declarations.push((symbol, identity)),
    )?;
    declarations.sort_by_key(|(symbol, declaration)| {
        (
            symbol.arena_index(),
            symbol.generation(),
            declaration.identity(),
        )
    });
    declarations.dedup();
    let callbacks = PreparedCallbackDeclarations::prepare(checked, sources, &mut identities);
    Ok(DeclarationCatalog {
        identities: Arc::new(identities.build()),
        conformances,
        callbacks,
        declarations,
    })
}
