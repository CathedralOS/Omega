//! Close application coordinates while Psi still owns the symbol table.
//!
//! Publication assigns fresh artifact-local ids. Distinct overloads, binders
//! and specializations cannot alias just because their nominal names coincide.
use crate::machine_lowering::declarations;
use crate::shared::lowering_error::LoweringError;
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
            callable: terminal_psi::TargetMachineCallable::Machine(body),
        });
    }
    for boundary in boundaries {
        callables.insert(terminal_psi::ProofCallableAssociation::Callable {
            declaration: identity(boundary.source)?,
            callable: terminal_psi::TargetMachineCallable::Boundary(boundary.id),
        });
    }
    // A selected generic template may have no emitted body or full signature.
    // Keep its independent header on the same declaration roster nonetheless.
    let mut headers = checked
        .facts
        .declaration_descriptions
        .headers
        .iter()
        .map(|(_, header)| Ok((identity(header.machine)?, header)))
        .collect::<Result<Vec<_>, LoweringError>>()?;
    drop(identity);
    headers.sort_by_key(|(declaration, _)| *declaration);
    let mut callable_headers = arena::Arena::new();
    for (declaration, header) in headers {
        let callable = if header.callable.is_valid() {
            Ok(identities.intern(&checked.callable_identity_text(header.callable)))
        } else {
            Err("checked machine has no callable signature identity".to_owned())
        };
        let synchronous_invocations = header
            .synchronous_invocations
            .as_ref()
            .map_err(Clone::clone)
            .and_then(|span| {
                let targets = checked
                    .facts
                    .declaration_descriptions
                    .invocation_targets
                    .span(*span)
                    .ok_or_else(|| {
                        "checked declaration lost its retained invocation targets".to_owned()
                    })?;
                Ok(targets
                    .iter()
                    .map(|target| terminal_psi::DeclaredSynchronousBoundaryTarget {
                        declaration: identities.intern(&target.declaration),
                        package: target.package,
                        application: target
                            .application
                            .as_ref()
                            .map(|application| identities.intern(application)),
                    })
                    .collect())
            });
        callable_headers.insert(terminal_psi::ProofCallableHeader {
            declaration,
            callable,
            package: header.package,
            source_target: header.source_target.clone(),
            product_eligible: header.product_eligible,
            supplies_checked_body: header.supplies_checked_body,
            synchronous_invocations,
        });
    }
    Ok(PublishedScope {
        scope: ProofApplicationScope {
            declarations: descriptions,
            callable_headers,
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
