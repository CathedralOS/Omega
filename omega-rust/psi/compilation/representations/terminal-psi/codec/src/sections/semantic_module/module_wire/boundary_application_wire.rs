//! Closed source-free requirements; selected providers never enter this section.

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::OperationId;

    #[test]
    fn boundary_demand_wire_round_trip_and_truncation() {
        let applications = [
            BoundaryApplication::Empty,
            BoundaryApplication::exact(vec![
                BoundaryApplicationArgument::type_argument(
                    0,
                    BoundaryTypeIdentity::new(TerminalIdentityIndex(1)),
                ),
                BoundaryApplicationArgument::const_argument(
                    1,
                    BoundaryTypeIdentity::new(TerminalIdentityIndex(2)),
                    "u64".into(),
                    "integer(u64,4)".into(),
                )
                .unwrap(),
            ])
            .unwrap(),
        ];
        for application in applications {
            let demand = TerminalBoundaryApplicationDemand::new(
                OperationId::new(1).unwrap(),
                BoundaryOperatorRequirement::new(
                    NominalDeclarationIdentity::new(TerminalIdentityIndex(3)),
                    "operator::compare(Element,Element)->bool".into(),
                    None,
                )
                .unwrap(),
                application,
            );
            let mut writer = Writer::default();
            encode_demand(&mut writer, &demand).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(decode_demand(&mut reader).unwrap(), demand);
            assert_eq!(reader.remaining(), 0);
            for end in 0..bytes.len() {
                assert!(decode_demand(&mut Reader::new(&bytes[..end])).is_err());
            }
        }
    }

    #[test]
    fn boundary_demand_wire_rejects_noncanonical_binder_order() {
        let demand = TerminalBoundaryApplicationDemand::new(
            OperationId::new(1).unwrap(),
            BoundaryOperatorRequirement::new(
                NominalDeclarationIdentity::new(TerminalIdentityIndex(3)),
                "operator::compare".into(),
                None,
            )
            .unwrap(),
            BoundaryApplication::Exact(vec![BoundaryApplicationArgument::type_argument(
                1,
                BoundaryTypeIdentity::new(TerminalIdentityIndex(2)),
            )]),
        );
        let mut writer = Writer::default();
        encode_demand(&mut writer, &demand).unwrap();
        assert!(decode_demand(&mut Reader::new(&writer.finish())).is_err());
    }

    #[test]
    fn boundary_candidate_wire_preserves_owner_and_specialization() {
        let nominal = |name: &str| {
            NominalDeclarationIdentity::new(TerminalIdentityIndex(
                name.bytes()
                    .fold(1u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32)),
            ))
        };
        for specialization in [
            None,
            Some(terminal_psi::CheckedMachineSpecialization {
                template: nominal("owner::template"),
                commitment: [4; 32],
            }),
        ] {
            let candidate = terminal_psi::BoundaryApplicationCandidate {
                requirement: BoundaryOperatorRequirement::new(
                    nominal("owner::requirement"),
                    "operator::compare".into(),
                    semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
                )
                .unwrap(),
                application: BoundaryApplication::Empty,
                selection_identity: TerminalIdentityIndex(9),
                selection_package: semantic_vocabulary::PackageKeyIdentity::from_digest([2; 32]),
                target: Some("windows_x86_64".into()),
                body: terminal_psi::CheckedMachineDeclaration {
                    semantic_declaration: Default::default(),
                    parameter_types: Vec::new(),
                    callable: Default::default(),
                    result_type: Default::default(),
                    machine: nominal("owner::body"),
                    state: nominal("owner::entry"),
                    contract: [3; 32],
                    specialization,
                },
            };
            let mut writer = Writer::default();
            encode_candidate(&mut writer, &candidate).unwrap();
            let bytes = writer.finish();
            let mut reader = Reader::new(&bytes);
            assert_eq!(decode_candidate(&mut reader).unwrap(), candidate);
            assert_eq!(reader.remaining(), 0);
            for end in 0..bytes.len() {
                assert!(decode_candidate(&mut Reader::new(&bytes[..end])).is_err());
            }
            let mut malformed = candidate;
            malformed.body.contract = [0; 32];
            let mut writer = Writer::default();
            assert!(encode_candidate(&mut writer, &malformed).is_err());
        }
    }
}

use super::super::CodecError;
use super::super::wire::{Reader, Writer, decode_counted};
use terminal_psi::TerminalIdentityIndex;
use terminal_psi::{
    BoundaryApplication, BoundaryApplicationArgument, BoundaryOperatorRequirement,
    BoundaryTypeIdentity, NominalDeclarationIdentity, TerminalBoundaryApplicationDemand,
};

pub(super) fn encode_demand(
    writer: &mut Writer,
    demand: &TerminalBoundaryApplicationDemand,
) -> Result<(), CodecError> {
    writer.id(demand.terminal_operation());
    encode_requirement(writer, demand.requirement())?;
    encode_application(writer, demand.application())
}

fn encode_requirement(
    writer: &mut Writer,
    requirement: &BoundaryOperatorRequirement,
) -> Result<(), CodecError> {
    writer.u32(requirement.declaration().identity().0);
    writer.string("boundary requirement overload", requirement.overload())?;
    encode_package(writer, requirement.package());
    Ok(())
}

fn encode_application(
    writer: &mut Writer,
    application: &BoundaryApplication,
) -> Result<(), CodecError> {
    writer.len(
        "boundary application arguments",
        application.arguments().len(),
    )?;
    for argument in application.arguments() {
        match argument {
            BoundaryApplicationArgument::Type {
                binder_ordinal,
                type_identity,
            } => {
                writer.u8(1);
                writer.u32(*binder_ordinal);
                writer.u32(type_identity.identity().0);
            }
            BoundaryApplicationArgument::Const {
                binder_ordinal,
                declared_carrier,
                value_type,
                value_encoding,
            } => {
                writer.u8(2);
                writer.u32(*binder_ordinal);
                writer.u32(declared_carrier.identity().0);
                writer.string("boundary const value type", value_type)?;
                writer.string("boundary const value", value_encoding)?;
            }
        }
    }
    Ok(())
}

pub(super) fn decode_demand(
    reader: &mut Reader<'_>,
) -> Result<TerminalBoundaryApplicationDemand, CodecError> {
    let operation = reader.id("OperationId")?;
    let requirement = decode_requirement(reader)?;
    let application = decode_application(reader)?;
    Ok(TerminalBoundaryApplicationDemand::new(
        operation,
        requirement,
        application,
    ))
}

fn decode_requirement(reader: &mut Reader<'_>) -> Result<BoundaryOperatorRequirement, CodecError> {
    let declaration = NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
    BoundaryOperatorRequirement::new(
        declaration,
        reader.string("boundary requirement overload")?,
        decode_package(reader)?,
    )
    .map_err(CodecError::MalformedStructuralFoundation)
}

fn decode_application(reader: &mut Reader<'_>) -> Result<BoundaryApplication, CodecError> {
    let arguments = decode_counted(reader, |reader| {
        let tag = reader.u8()?;
        let ordinal = reader.u32()?;
        match tag {
            1 => Ok(BoundaryApplicationArgument::type_argument(
                ordinal,
                BoundaryTypeIdentity::new(TerminalIdentityIndex(reader.u32()?)),
            )),
            2 => BoundaryApplicationArgument::const_argument(
                ordinal,
                BoundaryTypeIdentity::new(TerminalIdentityIndex(reader.u32()?)),
                reader.string("boundary const value type")?,
                reader.string("boundary const value")?,
            )
            .map_err(CodecError::MalformedStructuralFoundation),
            tag => Err(CodecError::InvalidTag("BoundaryApplicationArgument", tag)),
        }
    })?;
    let application = if arguments.is_empty() {
        BoundaryApplication::Empty
    } else {
        BoundaryApplication::exact(arguments).map_err(CodecError::MalformedStructuralFoundation)?
    };
    Ok(application)
}

fn encode_package(writer: &mut Writer, package: Option<semantic_vocabulary::PackageKeyIdentity>) {
    writer.boolean(package.is_some());
    if let Some(package) = package {
        writer.bytes(&package.digest());
    }
}

fn decode_package(
    reader: &mut Reader<'_>,
) -> Result<Option<semantic_vocabulary::PackageKeyIdentity>, CodecError> {
    if !reader.boolean()? {
        return Ok(None);
    }
    semantic_vocabulary::PackageKeyIdentity::from_digest(reader.array()?)
        .map(Some)
        .ok_or(CodecError::MalformedStructuralFoundation(
            "empty boundary package identity",
        ))
}

pub(super) fn encode_candidate(
    writer: &mut Writer,
    candidate: &terminal_psi::BoundaryApplicationCandidate,
) -> Result<(), CodecError> {
    encode_requirement(writer, &candidate.requirement)?;
    encode_application(writer, &candidate.application)?;
    writer.u32(candidate.selection_identity.0);
    encode_package(writer, candidate.selection_package);
    writer.boolean(candidate.target.is_some());
    if let Some(target) = &candidate.target {
        writer.string("boundary candidate target", target)?;
    }
    super::super::machine_declaration_wire::encode(writer, &candidate.body)
}

pub(super) fn decode_candidate(
    reader: &mut Reader<'_>,
) -> Result<terminal_psi::BoundaryApplicationCandidate, CodecError> {
    let requirement = decode_requirement(reader)?;
    let application = decode_application(reader)?;
    let selection_identity = TerminalIdentityIndex(reader.u32()?);
    let selection_package = decode_package(reader)?;
    let target = if reader.boolean()? {
        Some(reader.string("boundary candidate target")?)
    } else {
        None
    };
    let body = super::super::machine_declaration_wire::decode(reader)?;
    if selection_identity.is_absent() || target.as_ref().is_some_and(|target| target.is_empty()) {
        return Err(CodecError::MalformedStructuralFoundation(
            "empty boundary candidate identity or commitment",
        ));
    }
    Ok(terminal_psi::BoundaryApplicationCandidate {
        requirement,
        application,
        selection_identity,
        selection_package,
        target,
        body,
    })
}
