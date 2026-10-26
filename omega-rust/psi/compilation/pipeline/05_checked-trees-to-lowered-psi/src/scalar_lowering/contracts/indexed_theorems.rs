//! Retain only induction theorems reached through retired logical citations.

use super::*;

pub(crate) fn indexed_theorems(
    checked: &CheckedTrees,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    sources: &[(source::SourceId, [u8; 32])],
    machines: &[(symbols::SymbolHandle, semantic_vocabulary::MachineId)],
) -> Result<Vec<terminal_psi::TerminalIndexedTheorem>, LoweringError> {
    let mut reachable = machines
        .iter()
        .map(|(symbol, _)| *symbol)
        .collect::<Vec<_>>();
    loop {
        let before = reachable.len();
        for citation in &checked.facts.proof.indexed_citations {
            if reachable.contains(&citation.caller) && !reachable.contains(&citation.theorem) {
                reachable.push(citation.theorem);
            }
        }
        if before == reachable.len() {
            break;
        }
    }
    let roots = checked
        .facts
        .proof
        .indexed_theorems
        .iter()
        .filter(|theorem| reachable.contains(&theorem.machine))
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let signature =
        checked
            .facts
            .proof
            .mathematical_signature
            .as_ref()
            .ok_or(LoweringError::Unsupported(
                "cited induction theorem lost its checked mathematical signature",
            ))?;
    let mut retained = Vec::new();
    for theorem in roots {
        // Source checking already decided both typing and complete closure.
        // Transfer the exact roots without replaying kernel conversion here.
        if theorem
            .assumption_closure
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || theorem.assumption_closure.iter().any(|position| {
                !checked
                    .facts
                    .proof
                    .mathematical_safety_assumptions
                    .contains(position)
            })
        {
            return unsupported("cited induction theorem changed its admitted assumption closure");
        }
        let source_contract = crate::machine_lowering::declarations::identity(
            identities,
            checked,
            sources,
            theorem.machine,
        )
        .map_err(LoweringError::InvalidDeclarationIdentity)?;
        let type_identity =
            identities.intern(&checked.type_identity_text(theorem.type_identity).ok_or(
                LoweringError::Unsupported("cited theorem lost its source carrier identity"),
            )?);
        let certificate = terminal_codec::encode_mathematical_certificate(
            &signature.arena,
            &proof_admission::MathematicalCertificate {
                signature: signature.declarations.clone(),
                level_arity: 0,
                context: Vec::new(),
                term: theorem.term,
                expected: theorem.ty,
            },
        )
        .map_err(|_| {
            LoweringError::Unsupported("cited induction theorem is not canonically encodable")
        })?;
        retained.push(terminal_psi::TerminalIndexedTheorem {
            source_contract,
            ensure_ordinal: theorem.ensure_ordinal,
            evidence_source_position: theorem.evidence_source_position,
            type_identity,
            certificate,
            assumption_closure: theorem.assumption_closure.clone(),
        });
    }
    Ok(retained)
}
