//! Historical fold compatibility on the canonical semantic wire.
use super::super::{
    CodecError,
    wire::{Reader, Writer, decode_counted},
};
use terminal_psi::{
    FoldedConstantCandidate, FoldedConstantClosure, FoldedConstantDependency,
    FoldedLayoutAlternative, FoldedLayoutObservation, FoldedLayoutOwner, FoldedLayoutRequirement,
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
    writer.len(
        "folded layout requirements",
        closure.layout_requirements.len(),
    )?;
    for requirement in &closure.layout_requirements {
        match requirement.owner {
            FoldedLayoutOwner::Machine(machine) => {
                writer.u8(1);
                writer.id(machine);
            }
            FoldedLayoutOwner::Carrier(carrier) => {
                writer.u8(2);
                writer.id(carrier);
            }
        }
        writer.len("folded layout alternatives", requirement.alternatives.len())?;
        for alternative in &requirement.alternatives {
            writer.string("folded layout target", &alternative.target)?;
            writer.len("folded layout observations", alternative.observations.len())?;
            for observation in &alternative.observations {
                super::super::layout_observation_wire::encode_subject(
                    writer,
                    observation.kind,
                    observation.carrier,
                );
                writer.u64(observation.value);
            }
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
    let layout_requirements = decode_counted(reader, |reader| {
        let owner = match reader.u8()? {
            1 => FoldedLayoutOwner::Machine(reader.id("MachineId")?),
            2 => FoldedLayoutOwner::Carrier(reader.id("StructuralTypeId")?),
            tag => return Err(CodecError::InvalidTag("FoldedLayoutOwner", tag)),
        };
        let alternatives = decode_counted(reader, |reader| {
            let target = reader.string("folded layout target")?;
            let observations = decode_counted(reader, |reader| {
                let (kind, carrier) =
                    super::super::layout_observation_wire::decode_subject(reader)?;
                Ok(FoldedLayoutObservation {
                    kind,
                    carrier,
                    value: reader.u64()?,
                })
            })?;
            Ok(FoldedLayoutAlternative {
                target,
                observations,
            })
        })?;
        Ok(FoldedLayoutRequirement {
            owner,
            alternatives,
        })
    })?;
    Ok(FoldedConstantClosure {
        target,
        dependencies,
        layout_requirements,
    })
}
