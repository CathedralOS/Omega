//! Historical fold compatibility on the canonical semantic wire.
use super::super::{
    CodecError,
    wire::{Reader, Writer, decode_counted},
};
use terminal_psi::{
    FoldedConstantCandidate, FoldedConstantClosure, FoldedConstantDependency,
    NominalDeclarationIdentity, TerminalIdentityIndex,
};

pub(super) fn encode(
    writer: &mut Writer,
    closure: &FoldedConstantClosure,
) -> Result<(), CodecError> {
    writer.boolean(closure.target.is_some());
    if let Some(target) = &closure.target {
        writer.string("folded constant target", target)?;
    }
    writer.len("folded constant dependencies", closure.dependencies.len())?;
    for dependency in &closure.dependencies {
        writer.string("folded realization target", &dependency.target)?;
        writer.u32(dependency.requirement.0);
        writer.bytes(&dependency.plan);
        writer.boolean(dependency.candidate.is_some());
        if let Some(candidate) = &dependency.candidate {
            writer.id(candidate.boundary);
            writer.id(candidate.machine);
            writer.u32(candidate.declaration.identity().0);
        }
    }
    Ok(())
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<FoldedConstantClosure, CodecError> {
    let target = if reader.boolean()? {
        Some(reader.string("folded constant target")?)
    } else {
        None
    };
    let dependencies = decode_counted(reader, |reader| {
        let target = reader.string("folded realization target")?;
        let requirement = TerminalIdentityIndex(reader.u32()?);
        let plan = reader.array()?;
        let candidate = if reader.boolean()? {
            Some(FoldedConstantCandidate {
                boundary: reader.id("BoundaryMachineId")?,
                machine: reader.id("MachineId")?,
                declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?)),
            })
        } else {
            None
        };
        Ok(FoldedConstantDependency {
            target,
            requirement,
            plan,
            candidate,
        })
    })?;
    Ok(FoldedConstantClosure {
        target,
        dependencies,
    })
}
