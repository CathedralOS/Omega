//! Commit an authored equation to stable domain and field names. The terminal row
//! carries its own module coordinates and report fingerprint for replay.
use super::{
    Commitment, DomainDigestEncoder, Hash, Signature, TerminalIdentityTableBuilder, TypedTrees,
};
use crate::body::ProjectedContent;

pub(super) fn commit(
    typed: &TypedTrees,
    sources: &[(source_text::SourceId, [u8; 32])],
    signature: &Signature,
    identities: &mut TerminalIdentityTableBuilder,
    commitment: &mut Commitment,
) -> Result<(), &'static str> {
    let side = |term: &ProjectedContent,
                identities: &mut TerminalIdentityTableBuilder|
     -> Result<Vec<[u8; 32]>, &'static str> {
        let mut pending = vec![term];
        let mut parts = Vec::new();
        while let Some(term) = pending.pop() {
            match term {
                ProjectedContent::Separate(children) => pending.extend(children),
                ProjectedContent::Projection {
                    domain,
                    subject,
                    expression,
                    ..
                } => {
                    let owner = crate::qualifications::domain_identity(
                        typed, sources, *domain, identities,
                    )?;
                    let mut leaf = Commitment(DomainDigestEncoder::new(
                        b"omega.content-guarantee-projection.v1",
                    ));
                    identities
                        .text(owner)
                        .ok_or("content guarantee lost its qualification's nominal identity")?
                        .hash(&mut leaf);
                    expression.hash(&mut leaf);
                    subject.hash(&mut leaf);
                    parts.push(leaf.0.finish());
                }
            }
        }
        parts.sort_unstable();
        Ok(parts)
    };
    let mut equations = Vec::with_capacity(signature.content_ensures.len());
    for equation in &signature.content_ensures {
        let mut sides = [
            side(&equation.left, identities)?,
            side(&equation.right, identities)?,
        ];
        sides.sort_unstable();
        let mut row = Commitment(DomainDigestEncoder::new(
            b"omega.content-guarantee-equation.v1",
        ));
        equation.algebra.hash(&mut row);
        sides.hash(&mut row);
        equations.push(row.0.finish());
    }
    equations.sort_unstable();
    b"normal-content-guarantees.v1".hash(commitment);
    equations.hash(commitment);
    Ok(())
}
