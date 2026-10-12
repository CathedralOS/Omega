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
            BoundaryApplication::Exact {
                arguments: vec![BoundaryApplicationArgument::type_argument(
                    1,
                    BoundaryTypeIdentity::new(TerminalIdentityIndex(2)),
                )],
                lifetimes: Vec::new(),
            },
        );
        let mut writer = Writer::default();
        encode_demand(&mut writer, &demand).unwrap();
        assert!(decode_demand(&mut Reader::new(&writer.finish())).is_err());
    }

    fn lifetime_parameter(
        declaration: TerminalIdentityIndex,
        parameter_ordinal: u32,
    ) -> BoundaryLifetimeParameter {
        BoundaryLifetimeParameter {
            declaration: NominalDeclarationIdentity::new(declaration),
            parameter_ordinal,
        }
    }

    #[test]
    fn application_wire_preserves_categories_nested_tuples_and_lifetime_subjects() {
        let application = BoundaryApplication::exact_with_lifetimes(
            vec![
                BoundaryApplicationArgument::value_argument(
                    0,
                    BoundaryTypeIdentity::new(TerminalIdentityIndex(2)),
                    "u64".into(),
                    "integer(u64,4)".into(),
                )
                .unwrap(),
                BoundaryApplicationArgument::Declaration {
                    binder_ordinal: 1,
                    kind: BoundaryDeclarationArgumentKind::Machine,
                    declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(21)),
                    application: BoundaryApplication::exact(vec![
                        BoundaryApplicationArgument::type_argument(
                            0,
                            BoundaryTypeIdentity::new(TerminalIdentityIndex(20)),
                        ),
                    ])
                    .unwrap(),
                },
                BoundaryApplicationArgument::Declaration {
                    binder_ordinal: 2,
                    kind: BoundaryDeclarationArgumentKind::Conformance,
                    declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(22)),
                    application: BoundaryApplication::Empty,
                },
                BoundaryApplicationArgument::Declaration {
                    binder_ordinal: 3,
                    kind: BoundaryDeclarationArgumentKind::Proposition,
                    declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(23)),
                    application: BoundaryApplication::Empty,
                },
            ],
            vec![
                BoundaryApplicationLifetimeBinding {
                    binder_ordinal: 0,
                    parameter: lifetime_parameter(TerminalIdentityIndex(24), 0),
                    argument: BoundaryLifetimeArgument::Named(lifetime_parameter(
                        TerminalIdentityIndex(25),
                        2,
                    )),
                },
                BoundaryApplicationLifetimeBinding {
                    binder_ordinal: 1,
                    parameter: lifetime_parameter(TerminalIdentityIndex(26), 0),
                    argument: BoundaryLifetimeArgument::Named(lifetime_parameter(
                        TerminalIdentityIndex(25),
                        2,
                    )),
                },
            ],
        )
        .unwrap();
        assert!(application.validate_operator_application().is_err());
        let mut writer = Writer::default();
        encode_application(&mut writer, &application).unwrap();
        let bytes = writer.finish();
        let mut reader = Reader::new(&bytes);
        assert_eq!(decode_application(&mut reader).unwrap(), application);
        assert_eq!(reader.remaining(), 0);
        for end in 0..bytes.len() {
            assert!(decode_application(&mut Reader::new(&bytes[..end])).is_err());
        }
        let lifetime_only =
            BoundaryApplication::exact_with_lifetimes(Vec::new(), application.lifetimes().to_vec())
                .unwrap();
        let mut writer = Writer::default();
        encode_application(&mut writer, &lifetime_only).unwrap();
        assert_eq!(
            decode_application(&mut Reader::new(&writer.finish())).unwrap(),
            lifetime_only
        );
    }

    #[test]
    fn application_wire_rejects_duplicate_lifetime_formals_and_excess_depth() {
        let binding = BoundaryApplicationLifetimeBinding {
            binder_ordinal: 0,
            parameter: lifetime_parameter(TerminalIdentityIndex(26), 0),
            argument: BoundaryLifetimeArgument::Named(lifetime_parameter(
                TerminalIdentityIndex(25),
                0,
            )),
        };
        let mut repeated = binding.clone();
        repeated.binder_ordinal = 1;
        let malformed = BoundaryApplication::Exact {
            arguments: Vec::new(),
            lifetimes: vec![binding, repeated],
        };
        let mut writer = Writer::default();
        encode_application(&mut writer, &malformed).unwrap();
        assert!(decode_application(&mut Reader::new(&writer.finish())).is_err());

        // Untrusted bytes can bypass constructors and the bounded encoder.
        let mut writer = Writer::default();
        for _ in 0..=MAX_BOUNDARY_APPLICATION_DEPTH {
            writer.len("boundary application arguments", 1).unwrap();
            writer.u8(4);
            writer.u32(0);
            writer.u8(1);
            writer.u32(21);
        }
        writer.len("boundary application arguments", 0).unwrap();
        writer.len("boundary application lifetimes", 0).unwrap();
        for _ in 0..=MAX_BOUNDARY_APPLICATION_DEPTH {
            writer.len("boundary application lifetimes", 0).unwrap();
        }
        assert!(decode_application(&mut Reader::new(&writer.finish())).is_err());
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
    BoundaryApplication, BoundaryApplicationArgument, BoundaryApplicationLifetimeBinding,
    BoundaryDeclarationArgumentKind, BoundaryLifetimeArgument, BoundaryLifetimeParameter,
    BoundaryOperatorRequirement, BoundaryTypeIdentity, MAX_BOUNDARY_APPLICATION_DEPTH,
    NominalDeclarationIdentity, TerminalBoundaryApplicationDemand,
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
    encode_application_at_depth(writer, application, 0)
}

fn encode_application_at_depth(
    writer: &mut Writer,
    application: &BoundaryApplication,
    depth: usize,
) -> Result<(), CodecError> {
    check_application_depth(depth)?;
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
            }
            | BoundaryApplicationArgument::Value {
                binder_ordinal,
                declared_carrier,
                value_type,
                value_encoding,
            } => {
                writer.u8(
                    if matches!(argument, BoundaryApplicationArgument::Const { .. }) {
                        2
                    } else {
                        3
                    },
                );
                writer.u32(*binder_ordinal);
                writer.u32(declared_carrier.identity().0);
                writer.string("boundary value type", value_type)?;
                writer.string("boundary value encoding", value_encoding)?;
            }
            BoundaryApplicationArgument::Declaration {
                binder_ordinal,
                kind,
                declaration,
                application,
            } => {
                writer.u8(4);
                writer.u32(*binder_ordinal);
                writer.u8(match kind {
                    BoundaryDeclarationArgumentKind::Machine => 1,
                    BoundaryDeclarationArgumentKind::Conformance => 2,
                    BoundaryDeclarationArgumentKind::Proposition => 3,
                });
                writer.u32(declaration.identity().0);
                encode_application_at_depth(writer, application, depth + 1)?;
            }
        }
    }
    writer.len(
        "boundary application lifetimes",
        application.lifetimes().len(),
    )?;
    for binding in application.lifetimes() {
        writer.u32(binding.binder_ordinal);
        encode_lifetime_parameter(writer, &binding.parameter)?;
        match &binding.argument {
            BoundaryLifetimeArgument::Named(argument) => {
                encode_lifetime_parameter(writer, argument)?
            }
            BoundaryLifetimeArgument::AnonymousInvocation => {
                // Zero is the reserved absent declaration coordinate. It tags
                // this explicit operand variant, never a fabricated nominal.
                writer.u32(0);
                writer.u32(0);
            }
        }
    }
    Ok(())
}

fn encode_lifetime_parameter(
    writer: &mut Writer,
    parameter: &BoundaryLifetimeParameter,
) -> Result<(), CodecError> {
    if parameter.declaration.identity().is_absent() {
        return Err(CodecError::MalformedStructuralFoundation(
            "named lifetime parameter has no declaration identity",
        ));
    }
    writer.u32(parameter.declaration.identity().0);
    writer.u32(parameter.parameter_ordinal);
    Ok(())
}

fn check_application_depth(depth: usize) -> Result<(), CodecError> {
    if depth > MAX_BOUNDARY_APPLICATION_DEPTH {
        return Err(CodecError::MalformedStructuralFoundation(
            "boundary application nesting exceeds the supported depth",
        ));
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
    let application = decode_application_at_depth(reader, 0)?;
    application
        .validate()
        .map_err(CodecError::MalformedStructuralFoundation)?;
    Ok(application)
}

fn decode_application_at_depth(
    reader: &mut Reader<'_>,
    depth: usize,
) -> Result<BoundaryApplication, CodecError> {
    check_application_depth(depth)?;
    let arguments = decode_counted(reader, |reader| {
        let tag = reader.u8()?;
        let ordinal = reader.u32()?;
        match tag {
            1 => Ok(BoundaryApplicationArgument::type_argument(
                ordinal,
                BoundaryTypeIdentity::new(TerminalIdentityIndex(reader.u32()?)),
            )),
            2 | 3 => {
                let declared_carrier =
                    BoundaryTypeIdentity::new(TerminalIdentityIndex(reader.u32()?));
                let value_type = reader.string("boundary value type")?;
                let value_encoding = reader.string("boundary value encoding")?;
                if tag == 2 {
                    BoundaryApplicationArgument::const_argument(
                        ordinal,
                        declared_carrier,
                        value_type,
                        value_encoding,
                    )
                } else {
                    BoundaryApplicationArgument::value_argument(
                        ordinal,
                        declared_carrier,
                        value_type,
                        value_encoding,
                    )
                }
                .map_err(CodecError::MalformedStructuralFoundation)
            }
            4 => {
                let kind = match reader.u8()? {
                    1 => BoundaryDeclarationArgumentKind::Machine,
                    2 => BoundaryDeclarationArgumentKind::Conformance,
                    3 => BoundaryDeclarationArgumentKind::Proposition,
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "BoundaryDeclarationArgumentKind",
                            tag,
                        ));
                    }
                };
                let declaration =
                    NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?));
                let application = decode_application_at_depth(reader, depth + 1)?;
                Ok(BoundaryApplicationArgument::Declaration {
                    binder_ordinal: ordinal,
                    kind,
                    declaration,
                    application,
                })
            }
            tag => Err(CodecError::InvalidTag("BoundaryApplicationArgument", tag)),
        }
    })?;
    let lifetimes = decode_counted(reader, |reader| {
        Ok(BoundaryApplicationLifetimeBinding {
            binder_ordinal: reader.u32()?,
            parameter: decode_lifetime_parameter(reader)?,
            argument: {
                let declaration = reader.u32()?;
                let ordinal = reader.u32()?;
                if declaration == 0 {
                    if ordinal != 0 {
                        return Err(CodecError::MalformedStructuralFoundation(
                            "anonymous lifetime operand has an invalid ordinal tag",
                        ));
                    }
                    BoundaryLifetimeArgument::AnonymousInvocation
                } else {
                    BoundaryLifetimeArgument::Named(BoundaryLifetimeParameter {
                        declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(
                            declaration,
                        )),
                        parameter_ordinal: ordinal,
                    })
                }
            },
        })
    })?;
    if arguments.is_empty() && lifetimes.is_empty() {
        Ok(BoundaryApplication::Empty)
    } else {
        Ok(BoundaryApplication::Exact {
            arguments,
            lifetimes,
        })
    }
}

fn decode_lifetime_parameter(
    reader: &mut Reader<'_>,
) -> Result<BoundaryLifetimeParameter, CodecError> {
    Ok(BoundaryLifetimeParameter {
        declaration: NominalDeclarationIdentity::new(TerminalIdentityIndex(reader.u32()?)),
        parameter_ordinal: reader.u32()?,
    })
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
