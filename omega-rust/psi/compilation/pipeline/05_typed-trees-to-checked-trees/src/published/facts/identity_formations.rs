//! Publish the exact checked Id formation plan before source erasure.
//!
//! Id remains a statement, not a witness. This owner resolves source metadata
//! and closes each subject telescope; receiving independently checks the
//! mathematical judgment after lowering encodes this prepared plan.

use checked_trees::{
    CheckedIdentityFormation, CheckedIdentityFormationPlan, CheckedIdentityOwner,
    CheckedIdentityPublication, CheckedMathematicalSignature,
};
use diagnostics::Diagnostic;
use proof_admission::{Declaration, Term};
use typed_trees::TypedTrees;

pub(crate) fn record(
    program: &TypedTrees,
    formations: &[CheckedIdentityFormation],
    signature: Option<&CheckedMathematicalSignature>,
) -> Result<CheckedIdentityFormationPlan, Vec<Diagnostic>> {
    let refuse = |reason: &str| vec![Diagnostic::error(reason)];
    if formations.is_empty() {
        return Ok(CheckedIdentityFormationPlan::default());
    }
    let signature =
        signature.ok_or_else(|| refuse("Id formation lost its checked mathematical signature"))?;
    let mut arena = signature.arena.clone();
    let mut declarations = signature.declarations.clone();
    let mut rows = Vec::with_capacity(formations.len());
    for formation in formations {
        if formation.domains.len() != formation.captures.len() {
            return Err(refuse(
                "Id formation lost its exact source subject telescope",
            ));
        }
        let owner = if program
            .symbols
            .symbol_package_identity(formation.owner)
            .is_none()
            && program.symbols.symbol_source_origin(formation.owner)
                == Some(source::SourceOrigin::User)
        {
            CheckedIdentityOwner::InvocationLocal(format!(
                "invocation-declaration({}:{})",
                formation.owner.arena_index(),
                formation.owner.generation()
            ))
        } else {
            CheckedIdentityOwner::Hermetic(
                program
                    .normalized_hermetic_symbol_identity(formation.owner)
                    .map_err(|reason| refuse(&reason))?,
            )
        };
        let source_carrier = program
            .type_identity_text(formation.type_identity)
            .ok_or_else(|| refuse("Id formation lost its exact source carrier"))?;
        let subject_carriers = formation
            .captures
            .iter()
            .map(|(_, ty)| {
                program
                    .type_identity_text(*ty)
                    .ok_or_else(|| refuse("Id subject lost its exact source carrier"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let subject_origins = formation
            .captures
            .iter()
            .map(|(expression, _)| {
                let span = program.expression_table.source_span(*expression).span;
                Ok((
                    u32::try_from(span.start)
                        .map_err(|_| refuse("Id source subject position exceeds u32"))?,
                    u32::try_from(span.end)
                        .map_err(|_| refuse("Id source subject position exceeds u32"))?,
                ))
            })
            .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
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
        let declaration = u32::try_from(declarations.len())
            .map_err(|_| refuse("Id formation declaration roster exceeds u32"))?;
        declarations.push(Declaration {
            level_arity: 0,
            ty,
            body: Some(body),
        });
        rows.push(CheckedIdentityPublication {
            owner,
            source_carrier,
            subject_carriers,
            subject_origins,
            declaration,
        });
    }
    let term = arena.insert(Term::TwoZero);
    let ty = arena.insert(Term::Two);
    Ok(CheckedIdentityFormationPlan {
        rows,
        arena,
        declarations,
        term,
        ty,
    })
}
