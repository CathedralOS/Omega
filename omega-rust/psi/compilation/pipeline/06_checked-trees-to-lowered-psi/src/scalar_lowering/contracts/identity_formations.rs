//! Erase source subject handles while retaining their checked Id formation.
//!
//! Each type-valued lambda abstracts the same ordered source captures. It
//! forms an Id family, not evidence that its endpoints agree. The receiving
//! kernel checks the complete shared signature after canonical decoding.

use crate::LoweringError;
use checked_trees::CheckedTrees;
use proof_admission::{Declaration, MathematicalCertificate, Term};
use terminal_psi::{
    TerminalIdentityFormation, TerminalIdentityFormations, TerminalIdentityTableBuilder,
};

pub(crate) fn identity_formations(
    checked: &CheckedTrees,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<TerminalIdentityFormations, LoweringError> {
    let formations = &checked.facts.proof.identity_formations;
    if formations.is_empty() {
        return Ok(TerminalIdentityFormations::default());
    }
    let signature =
        checked
            .facts
            .proof
            .mathematical_signature
            .as_ref()
            .ok_or(LoweringError::Unsupported(
                "Id formation lost its checked mathematical signature",
            ))?;
    let mut arena = signature.arena.clone();
    let mut declarations = signature.declarations.clone();
    let mut rows = Vec::with_capacity(formations.len());
    for formation in formations {
        if formation.domains.len() != formation.captures.len() {
            return Err(LoweringError::Unsupported(
                "Id formation lost its exact source subject telescope",
            ));
        }
        let source_owner = crate::machine_lowering::declarations::semantic_identity(
            identities,
            checked,
            formation.owner,
        )
        .map_err(LoweringError::InvalidDeclarationIdentity)?;
        let subject_origins = formation
            .captures
            .iter()
            .map(|(expression, _)| {
                let span = checked.expression_table.source_span(*expression).span;
                Ok((
                    u32::try_from(span.start).map_err(|_| {
                        LoweringError::Unsupported("Id subject source position exceeds u32")
                    })?,
                    u32::try_from(span.end).map_err(|_| {
                        LoweringError::Unsupported("Id subject source position exceeds u32")
                    })?,
                ))
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        let source_carrier =
            identities.intern(&checked.type_identity_text(formation.type_identity).ok_or(
                LoweringError::Unsupported("Id formation lost its source carrier identity"),
            )?);
        let subject_carriers = formation
            .captures
            .iter()
            .map(|(_, ty)| {
                checked
                    .type_identity_text(*ty)
                    .map(|name| identities.intern(&name))
                    .ok_or(LoweringError::Unsupported(
                        "Id subject lost its exact source carrier",
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut body = formation.term;
        let mut ty = formation.ty;
        for domain in formation.domains.iter().rev() {
            body = arena.insert(Term::Lambda {
                domain: *domain,
                body,
            });
            ty = arena.insert(Term::Pi {
                domain: *domain,
                codomain: ty,
            });
        }
        let declaration = u32::try_from(declarations.len()).map_err(|_| {
            LoweringError::Unsupported("Id formation declaration roster exceeds u32")
        })?;
        declarations.push(Declaration {
            level_arity: 0,
            ty,
            body: Some(body),
        });
        rows.push(TerminalIdentityFormation {
            source_owner,
            source_carrier,
            subject_carriers,
            subject_origins,
            declaration,
        });
    }
    let term = arena.insert(Term::TwoZero);
    let expected = arena.insert(Term::Two);
    let certificate = terminal_codec::encode_mathematical_certificate(
        &arena,
        &MathematicalCertificate {
            signature: declarations,
            level_arity: 0,
            context: Vec::new(),
            term,
            expected,
        },
    )
    .map_err(|error| {
        LoweringError::InvalidDeclarationIdentity(format!(
            "Id formation cannot be encoded: {error}"
        ))
    })?;
    Ok(TerminalIdentityFormations { rows, certificate })
}
