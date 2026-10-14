//! Closed conformance applications on the wire: the telescope, subject and
//! trait identities, realization callables, rows and commitment of one
//! application.

use super::super::CodecError;
use super::super::wire::{Reader, Writer};
use crate::sections::semantic_module::wire::decode_counted;
use terminal_psi::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
    ClosedConformanceCallableResult, ClosedConformanceParameterBinding,
    ClosedConformanceParameterKind, ClosedConformanceRow, TerminalIdentityIndex,
};

pub(super) fn encode_closed_conformance_application(
    writer: &mut Writer,
    application: &ClosedConformanceApplication,
) -> Result<(), CodecError> {
    writer.id(application.owner);
    writer.u32(application.declaration_identity.0);
    writer.len("closed conformance telescope", application.telescope.len())?;
    for binding in &application.telescope {
        writer.string("closed conformance parameter", &binding.parameter)?;
        writer.u8(match binding.kind {
            ClosedConformanceParameterKind::Lifetime => 1,
            ClosedConformanceParameterKind::Type => 2,
            ClosedConformanceParameterKind::Const => 3,
            ClosedConformanceParameterKind::Machine => 4,
        });
        writer.string("closed conformance argument", &binding.argument)?;
    }
    writer.u32(application.trait_identity.0);
    writer.strings(
        "closed conformance trait lifetime arguments",
        &application.trait_lifetime_arguments,
    )?;
    writer.strings(
        "closed conformance trait arguments",
        &application.trait_arguments,
    )?;
    writer.len(
        "closed conformance realization callables",
        application.realization_callables.len(),
    )?;
    for callable in &application.realization_callables {
        writer.u32(callable.source_callable_identity.0);
        writer.id(callable.machine);
        match callable.result {
            ClosedConformanceCallableResult::Unit => writer.u8(1),
            ClosedConformanceCallableResult::Scalar(scalar) => {
                writer.u8(2);
                super::super::scalar_wire::encode_scalar_type(writer, scalar);
            }
            ClosedConformanceCallableResult::Structural(identity) => {
                writer.u8(3);
                writer.id(identity);
            }
        }
    }
    writer.len("closed conformance schemas", application.schemas.len())?;
    for schema in &application.schemas {
        for identity in [
            schema.declaring_trait_identity,
            schema.public_requirement_identity,
            schema.requirement_identity,
            schema.realization_identity,
        ] {
            writer.u32(identity.0);
        }
        writer.len("conformance schema definition", schema.definition.len())?;
        writer.bytes(&schema.definition);
        writer.u8(match schema.kind {
            terminal_psi::ClosedConformanceSchemaKind::Nongeneric => 1,
            terminal_psi::ClosedConformanceSchemaKind::Finite => 2,
            terminal_psi::ClosedConformanceSchemaKind::Open => 3,
        });
        writer.u32(schema.lifetime_parameter_count);
        writer.len(
            "conformance method parameter kinds",
            schema.parameter_kinds.len(),
        )?;
        writer.bytes(&schema.parameter_kinds);
        writer.boolean(schema.finite_tuples.is_some());
        if let Some(tuples) = &schema.finite_tuples {
            writer.len("conformance method finite roster", tuples.len())?;
            for tuple in tuples {
                writer.len("conformance finite tuple", tuple.len())?;
                for argument in tuple {
                    writer.u32(argument.0);
                }
            }
        }
    }
    writer.len(
        "closed conformance static applications",
        application.static_applications.len(),
    )?;
    for applied in &application.static_applications {
        writer.u32(applied.public_requirement_identity.0);
        writer.u32(applied.realization_identity.0);
        writer.u32(applied.source_callable_identity.0);
        writer.id(applied.machine);
        writer.boolean(applied.application_commitment.is_some());
        if let Some(commitment) = applied.application_commitment {
            writer.bytes(&commitment);
        }
        writer.len(
            "conformance method arguments",
            applied.method_arguments.len(),
        )?;
        for argument in &applied.method_arguments {
            super::super::reach_application_wire::encode_argument(writer, argument);
        }
    }
    writer.len("closed conformance rows", application.rows.len())?;
    for row in &application.rows {
        writer.u32(row.declaring_trait_identity.0);
        writer.u32(row.public_requirement_identity.0);
        writer.len(
            "closed conformance row family tuple",
            row.family_tuple.len(),
        )?;
        for member in &row.family_tuple {
            writer.u32(member.0);
        }
        writer.u32(row.requirement_identity.0);
        writer.u32(row.realization_identity.0);
        writer.boolean(row.realization_callable_identity.is_some());
        if let Some(identity) = &row.realization_callable_identity {
            writer.u32(identity.0);
        }
    }
    writer.u64(application.report_fingerprint);
    writer.bytes(&application.commitment.as_bytes());
    Ok(())
}

pub(super) fn decode_closed_conformance_application(
    reader: &mut Reader<'_>,
) -> Result<ClosedConformanceApplication, CodecError> {
    Ok(ClosedConformanceApplication {
        owner: reader.id("MachineId")?,
        declaration_identity: TerminalIdentityIndex(reader.u32()?),
        telescope: decode_counted(reader, |reader| {
            Ok(ClosedConformanceParameterBinding {
                parameter: reader.string("closed conformance parameter")?,
                kind: match reader.u8()? {
                    1 => ClosedConformanceParameterKind::Lifetime,
                    2 => ClosedConformanceParameterKind::Type,
                    3 => ClosedConformanceParameterKind::Const,
                    4 => ClosedConformanceParameterKind::Machine,
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "ClosedConformanceParameterKind",
                            tag,
                        ));
                    }
                },
                argument: reader.string("closed conformance argument")?,
            })
        })?,
        trait_identity: TerminalIdentityIndex(reader.u32()?),
        trait_lifetime_arguments: reader.strings("closed conformance trait lifetime arguments")?,
        trait_arguments: reader.strings("closed conformance trait arguments")?,
        realization_callables: decode_counted(reader, |reader| {
            Ok(terminal_psi::ClosedConformanceRealizationCallable {
                source_callable_identity: TerminalIdentityIndex(reader.u32()?),
                machine: reader.id("MachineId")?,
                result: match reader.u8()? {
                    1 => ClosedConformanceCallableResult::Unit,
                    2 => ClosedConformanceCallableResult::Scalar(
                        super::super::scalar_wire::decode_scalar_type(reader)?,
                    ),
                    3 => {
                        ClosedConformanceCallableResult::Structural(reader.id("StructuralTypeId")?)
                    }
                    tag => {
                        return Err(CodecError::InvalidTag(
                            "ClosedConformanceCallableResult",
                            tag,
                        ));
                    }
                },
            })
        })?,
        schemas: decode_counted(reader, |reader| {
            let declaring_trait_identity = TerminalIdentityIndex(reader.u32()?);
            let public_requirement_identity = TerminalIdentityIndex(reader.u32()?);
            let requirement_identity = TerminalIdentityIndex(reader.u32()?);
            let realization_identity = TerminalIdentityIndex(reader.u32()?);
            let definition = decode_counted(reader, |reader| reader.u8())?;
            let kind = match reader.u8()? {
                1 => terminal_psi::ClosedConformanceSchemaKind::Nongeneric,
                2 => terminal_psi::ClosedConformanceSchemaKind::Finite,
                3 => terminal_psi::ClosedConformanceSchemaKind::Open,
                tag => return Err(CodecError::InvalidTag("ClosedConformanceSchemaKind", tag)),
            };
            let lifetime_parameter_count = reader.u32()?;
            let parameter_kinds = decode_counted(reader, |reader| reader.u8())?;
            let finite_tuples = if reader.boolean()? {
                Some(decode_counted(reader, |reader| {
                    decode_counted(reader, |reader| Ok(TerminalIdentityIndex(reader.u32()?)))
                })?)
            } else {
                None
            };
            Ok(terminal_psi::ClosedConformanceSchema {
                declaring_trait_identity,
                public_requirement_identity,
                requirement_identity,
                realization_identity,
                definition,
                kind,
                lifetime_parameter_count,
                parameter_kinds,
                finite_tuples,
            })
        })?,
        static_applications: decode_counted(reader, |reader| {
            Ok(terminal_psi::ClosedConformanceStaticApplication {
                public_requirement_identity: TerminalIdentityIndex(reader.u32()?),
                realization_identity: TerminalIdentityIndex(reader.u32()?),
                source_callable_identity: TerminalIdentityIndex(reader.u32()?),
                machine: reader.id("MachineId")?,
                application_commitment: if reader.boolean()? {
                    Some(reader.array()?)
                } else {
                    None
                },
                method_arguments: decode_counted(
                    reader,
                    super::super::reach_application_wire::decode_argument,
                )?,
            })
        })?,
        rows: decode_counted(reader, |reader| {
            Ok(ClosedConformanceRow {
                declaring_trait_identity: TerminalIdentityIndex(reader.u32()?), /* closed conformance row declaring trait identity */
                public_requirement_identity: TerminalIdentityIndex(reader.u32()?), /* closed conformance row public requirement identity */
                family_tuple: decode_counted(reader, |reader| {
                    Ok(TerminalIdentityIndex(reader.u32()?))
                })?,
                requirement_identity: TerminalIdentityIndex(reader.u32()?), /* closed conformance row requirement identity */
                realization_identity: TerminalIdentityIndex(reader.u32()?), /* closed conformance row realization identity */
                realization_callable_identity: if reader.boolean()? {
                    Some(TerminalIdentityIndex(reader.u32()?))
                } else {
                    None
                },
            })
        })?,
        report_fingerprint: reader.u64()?,
        commitment: ClosedConformanceApplicationCommitment::from_digest(reader.array()?),
    })
}
