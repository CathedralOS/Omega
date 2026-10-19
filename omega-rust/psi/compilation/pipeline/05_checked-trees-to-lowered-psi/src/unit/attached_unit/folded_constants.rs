//! Project already admitted historical folds through existing exact candidates.
use super::{
    CheckedTrees, CheckedUnitProviderCandidate, LoweringError, MachineId, lookup_machine_id,
};
use terminal_psi::{
    FoldedConstantCandidate, FoldedConstantClosure, FoldedConstantDependency,
    ProviderCandidateConformance, TerminalIdentityTableBuilder,
};

pub(super) fn lower(
    checked: &CheckedTrees,
    identities: &mut TerminalIdentityTableBuilder,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    sources: &[CheckedUnitProviderCandidate],
    candidates: &[ProviderCandidateConformance],
    machines: &[(symbols::SymbolHandle, MachineId)],
) -> Result<FoldedConstantClosure, LoweringError> {
    let typed = &checked.typed;
    let closure = &typed.constant_recipe_closure;
    if closure.receipts.is_empty() {
        return Ok(Default::default());
    }
    if closure.target_scope.is_empty() {
        return Err(LoweringError::Unsupported(
            "folded constants lost their primary target closure",
        ));
    }
    let mut dependencies = Vec::new();
    for receipt in &closure.receipts {
        let (requirement, candidate) = if receipt.provider_machine.is_valid() {
            let mut matching = sources.iter().filter(|source| {
                source.boundary == receipt.requirement
                    && source.provider == receipt.provider_declaration
                    && source.candidate == receipt.provider_machine
            });
            let source = matching.next().ok_or(LoweringError::Unsupported(
                "folded constant lost its exact selected provider candidate",
            ))?;
            if matching.next().is_some() {
                return Err(LoweringError::Unsupported(
                    "folded constant provider candidate is ambiguous",
                ));
            }
            let machine = lookup_machine_id(machines, receipt.provider_machine)?;
            let declaration = crate::machine_lowering::declarations::nominal(
                identities,
                checked,
                exact_toolchain_sources,
                receipt.provider_declaration,
            )
            .map_err(LoweringError::Unsupported)?;
            let identity = typed.callable_identity_text(source.requirement_identity);
            let mut matching = candidates.iter().filter(|candidate| {
                candidate.candidate == machine
                    && candidate.provider_declaration == declaration
                    && identities.text(candidate.requirement_identity) == Some(identity.as_str())
            });
            let row = matching.next().ok_or(LoweringError::Unsupported(
                "folded constant has no retained canonical provider association",
            ))?;
            if matching.next().is_some() {
                return Err(LoweringError::Unsupported(
                    "folded constant canonical association is ambiguous",
                ));
            }
            (
                row.source_requirement_identity,
                Some(FoldedConstantCandidate {
                    boundary: row.boundary,
                    machine: row.candidate,
                    declaration: row.provider_declaration.clone(),
                }),
            )
        } else {
            if receipt.provider_declaration.is_valid() {
                return Err(LoweringError::Unsupported(
                    "intrinsic fold carries a checked provider declaration without its body",
                ));
            }
            let identity = if let Some((owner, signature)) =
                typed.trait_machine_signature(receipt.requirement)
            {
                typed.trait_requirement_callable_identity(owner, signature)
            } else if let Some(operator) = typed.operator_definition_by_symbol(receipt.requirement)
            {
                typed.boundary_operator_requirement_callable_identity(operator)
            } else if let Some(machine) = typed.machine_by_symbol(receipt.requirement) {
                typed.machine_callable_identity(machine)
            } else {
                return Err(LoweringError::Unsupported(
                    "intrinsic fold lost its exact requirement declaration",
                ));
            };
            (
                identities.intern(&typed.callable_identity_text(identity)),
                None,
            )
        };
        dependencies.push(FoldedConstantDependency {
            target: receipt.target_scope.clone(),
            requirement,
            plan: receipt.plan_identity,
            candidate,
        });
    }
    dependencies.sort();
    dependencies.dedup();
    Ok(FoldedConstantClosure {
        target: Some(closure.target_scope.clone()),
        dependencies,
    })
}
