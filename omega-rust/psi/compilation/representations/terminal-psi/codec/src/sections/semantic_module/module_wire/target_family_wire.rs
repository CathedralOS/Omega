//! Source-free family membership and supply on the canonical wire.

use super::super::CodecError;
use super::super::wire::{Reader, Writer, decode_counted};
use terminal_psi::{
    TargetMachineCallable, TargetMachineFamily, TargetMachineFamilyMember, TargetMachineSupply,
};

fn encode_callable(writer: &mut Writer, callable: TargetMachineCallable) {
    match callable {
        TargetMachineCallable::Machine(machine) => {
            writer.u8(1);
            writer.id(machine);
        }
        TargetMachineCallable::Boundary(boundary) => {
            writer.u8(2);
            writer.id(boundary);
        }
    }
}

fn decode_callable(reader: &mut Reader<'_>) -> Result<TargetMachineCallable, CodecError> {
    Ok(match reader.u8()? {
        1 => TargetMachineCallable::Machine(reader.id("MachineId")?),
        2 => TargetMachineCallable::Boundary(reader.id("BoundaryMachineId")?),
        tag => return Err(CodecError::InvalidTag("TargetMachineCallable", tag)),
    })
}

pub(super) fn encode_family(
    writer: &mut Writer,
    family: &TargetMachineFamily,
) -> Result<(), CodecError> {
    encode_callable(writer, family.canonical);
    writer.len("target-family members", family.members.len())?;
    for member in &family.members {
        writer.string("target-family deployment tag", &member.target)?;
        encode_callable(writer, member.callable);
        writer.u8(match member.supply {
            TargetMachineSupply::CheckedBody => 1,
            TargetMachineSupply::Requirement => 2,
            TargetMachineSupply::TopLevelRequirement => 3,
            TargetMachineSupply::Boundary => 4,
            TargetMachineSupply::AdmissionClaim => 5,
            TargetMachineSupply::ExternalRealization => 6,
        });
    }
    Ok(())
}

pub(super) fn decode_family(reader: &mut Reader<'_>) -> Result<TargetMachineFamily, CodecError> {
    Ok(TargetMachineFamily {
        canonical: decode_callable(reader)?,
        members: decode_counted(reader, |reader| {
            Ok(TargetMachineFamilyMember {
                target: reader.string("target-family deployment tag")?,
                callable: decode_callable(reader)?,
                supply: match reader.u8()? {
                    1 => TargetMachineSupply::CheckedBody,
                    2 => TargetMachineSupply::Requirement,
                    3 => TargetMachineSupply::TopLevelRequirement,
                    4 => TargetMachineSupply::Boundary,
                    5 => TargetMachineSupply::AdmissionClaim,
                    6 => TargetMachineSupply::ExternalRealization,
                    tag => return Err(CodecError::InvalidTag("TargetMachineSupply", tag)),
                },
            })
        })?,
    })
}
