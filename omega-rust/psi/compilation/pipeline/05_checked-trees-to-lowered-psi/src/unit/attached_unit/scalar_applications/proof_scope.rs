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
    machines: &[(SymbolHandle, semantic_vocabulary::MachineId)],
    boundaries: &[super::super::operation_frame::BoundaryParameters],
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
        let description = describe(checked, identities, sources, symbol);
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
    let mut callables = arena::Arena::new();
    for &(source, body) in machines {
        callables.insert(terminal_psi::ProofCallableAssociation::Callable {
            declaration: identity(source)?,
            callable: terminal_psi::TerminalCallable::Machine(body),
        });
    }
    for boundary in boundaries {
        callables.insert(terminal_psi::ProofCallableAssociation::Callable {
            declaration: identity(boundary.source)?,
            callable: terminal_psi::TerminalCallable::Boundary(boundary.id),
        });
    }
    Ok(PublishedScope {
        scope: ProofApplicationScope {
            declarations: descriptions,
            callables,
            plan,
            calls,
        },
        coordinates,
    })
}

fn describe(
    checked: &CheckedTrees,
    identities: &mut TerminalIdentityTableBuilder,
    sources: &[(source::SourceId, [u8; 32])],
    symbol: SymbolHandle,
) -> Result<ProofScopeDeclaration, String> {
    if let Some(declaration) =
        declarations::boundary_identity(identities, checked, sources, symbol)?
    {
        return Ok(ProofScopeDeclaration::Callable(declaration));
    }
    if let Some(operator) = checked.typed.operator_definition_by_symbol(symbol)
        && operator.is_boundary
    {
        return declarations::operator_requirement(
            identities,
            checked,
            sources,
            symbol,
            checked
                .typed
                .boundary_operator_requirement_callable_identity(operator),
        )
        .map(ProofScopeDeclaration::Operator)
        .map_err(str::to_owned);
    }
    declarations::semantic_identity(identities, checked, symbol).map(ProofScopeDeclaration::Nominal)
}
