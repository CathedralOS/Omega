//! Close application coordinates while Psi still owns the symbol table.
//!
//! Publication assigns fresh artifact-local ids. Distinct overloads, binders
//! and specializations cannot alias just because their nominal names coincide.
use crate::lowering_error::LoweringError;
use crate::machine_lowering::declarations;
use checked_trees::CheckedTrees;
use semantic_vocabulary::proof_applications::ProofCallGraph;
use symbols::SymbolHandle;
use terminal_psi::{
    ProofApplicationScope, ProofDeclarationId, ProofScopeDeclaration, TerminalIdentityTableBuilder,
};

pub(super) struct PublishedScope {
    pub scope: ProofApplicationScope,
    pub coordinates: Vec<(SymbolHandle, ProofDeclarationId)>,
}

pub(super) fn publish(
    checked: &CheckedTrees,
    identities: &mut TerminalIdentityTableBuilder,
    sources: &[(source::SourceId, [u8; 32])],
) -> Result<PublishedScope, LoweringError> {
    let source = &checked.facts.proof_applications;
    let mut coordinates = Vec::new();
    let mut descriptions = arena::Arena::default();
    let mut identity = |symbol: SymbolHandle| -> Result<ProofDeclarationId, LoweringError> {
        if !symbol.is_valid() {
            return Ok(ProofDeclarationId::default());
        }
        if let Some((_, identity)) = coordinates.iter().find(|(owner, _)| *owner == symbol) {
            return Ok(*identity);
        }
        let ordinal = u32::try_from(coordinates.len())
            .ok()
            .and_then(|position| position.checked_add(1))
            .ok_or(LoweringError::Unsupported(
                "proof declaration namespace is exhausted",
            ))?;
        let identity = ProofDeclarationId(ordinal);
        let description = if checked
            .facts
            .declaration_descriptions
            .description(symbol)
            .is_some()
        {
            declarations::identity(identities, checked, sources, symbol)
                .map(ProofScopeDeclaration::Callable)
        } else {
            declarations::semantic_identity(identities, checked, symbol)
                .map(ProofScopeDeclaration::Nominal)
        };
        descriptions.insert(description.unwrap_or_else(ProofScopeDeclaration::Unavailable));
        coordinates.push((symbol, identity));
        Ok(identity)
    };
    let plan = source.try_map(&mut identity)?;
    let calls = ProofCallGraph::capture(
        &checked.facts.operational,
        source.machines.iter().map(|(_, row)| row.machine),
        &mut identity,
    )?;
    Ok(PublishedScope {
        scope: ProofApplicationScope {
            declarations: descriptions,
            plan,
            calls,
        },
        coordinates,
    })
}
