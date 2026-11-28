//! Resolve a published callback path within the signature that owns its formals.
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    BoundaryParameterKind, StructuralParameterDeclaration, StructuralPathSegment,
    StructuralTypeDeclaration, TerminalIdentityIndex,
};

pub fn structural_formal(
    order: &[BoundaryParameterKind],
    structures: &[StructuralParameterDeclaration],
    parameter: u32,
) -> Option<StructuralTypeId> {
    let position = usize::try_from(parameter).ok()?;
    (order.get(position)? == &BoundaryParameterKind::Structural).then_some(())?;
    let lane = order[..position]
        .iter()
        .filter(|kind| **kind == BoundaryParameterKind::Structural)
        .count();
    Some(structures.get(lane)?.structural_type)
}

/// A signature commitment must survive renumbering the module's identity table and sum cases.
pub fn callback_path_commitment<'a>(
    types: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    identity: impl Fn(TerminalIdentityIndex) -> Option<&'a str>,
) -> Result<[u8; 32], &'static str> {
    use StructuralPathSegment as Step;
    if path.is_empty() {
        return Err("a callback field needs a nonempty path");
    }
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega.callback-parameter-path.v1\0");
    digest.len(path.len());
    for (position, step) in path.iter().enumerate() {
        match step {
            Step::Field(field) => {
                digest.byte(1);
                digest.string(identity(*field).ok_or("callback field lacks a portable identity")?);
            }
            Step::FixedIndex(index) => {
                digest.byte(2);
                digest.u64(*index);
            }
            Step::Case(case) => {
                let owner =
                    crate::runtime_structural_path_tip(types.iter(), root, &path[..position])
                        .ok_or("callback case has no enclosing sum")?;
                let case = owner
                    .shape
                    .sum_cases()
                    .and_then(|cases| cases.iter().find(|row| row.id == *case))
                    .ok_or("callback case is outside its sum")?;
                digest.byte(3);
                digest.string(
                    identity(case.identity).ok_or("callback case lacks a portable identity")?,
                );
            }
            Step::Referent => digest.byte(4),
            _ => return Err("callback field uses a machine-local or imprecise projection"),
        }
    }
    Ok(digest.finish())
}
