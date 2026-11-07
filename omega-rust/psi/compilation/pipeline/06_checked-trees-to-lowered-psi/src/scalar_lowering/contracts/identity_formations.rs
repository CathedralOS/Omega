//! Encode the Id formation plan already completed by checking.

use crate::LoweringError;
use checked_trees::{CheckedIdentityOwner, CheckedTrees};
use proof_admission::MathematicalCertificate;
use terminal_psi::{
    TerminalIdentityFormation, TerminalIdentityFormations, TerminalIdentityTableBuilder,
};

pub(crate) fn identity_formations(
    checked: &CheckedTrees,
    identities: &mut TerminalIdentityTableBuilder,
) -> Result<TerminalIdentityFormations, LoweringError> {
    let plan = &checked.facts.proof.identity_formation_plan;
    if plan.rows.is_empty() {
        return Ok(TerminalIdentityFormations::default());
    }
    let rows = plan
        .rows
        .iter()
        .map(|row| {
            let source_owner = match &row.owner {
                CheckedIdentityOwner::Hermetic(identity) => identities.intern(identity),
                CheckedIdentityOwner::InvocationLocal(coordinate) => identities
                    .intern_invocation_local(coordinate)
                    .map_err(|reason| {
                        LoweringError::InvalidDeclarationIdentity(reason.to_owned())
                    })?,
            };
            Ok(TerminalIdentityFormation {
                source_owner,
                source_carrier: identities.intern(&row.source_carrier),
                subject_carriers: row
                    .subject_carriers
                    .iter()
                    .map(|identity| identities.intern(identity))
                    .collect(),
                subject_origins: row.subject_origins.clone(),
                declaration: row.declaration,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let certificate = terminal_codec::encode_mathematical_certificate(
        &plan.arena,
        &MathematicalCertificate {
            signature: plan.declarations.clone(),
            level_arity: 0,
            context: Vec::new(),
            term: plan.term,
            expected: plan.ty,
        },
    )
    .map_err(|error| {
        LoweringError::InvalidDeclarationIdentity(format!(
            "Id formation cannot be encoded: {error}"
        ))
    })?;
    Ok(TerminalIdentityFormations { rows, certificate })
}
