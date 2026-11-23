//! The closed service reach of an applied machine.
use semantic_vocabulary::ServiceId;
use terminal_psi::{
    ClosedReachApplication, ClosedReachArgument, ClosedReachCall, ClosedReachCallApplication,
    ClosedReachCapture, ClosedReachMachineBinding, ClosedReachParameter, ClosedReachSchema,
};

use super::CodecError;

use super::wire::{Reader, Writer};
use crate::fields::{Field, InvocationSite};
use crate::wire::{decode_counted, decode_ids, decode_optional_id, encode_optional_id};
use terminal_psi::TerminalIdentityIndex;

pub fn encode(
    writer: &mut Writer,
    application: Option<&ClosedReachApplication>,
) -> Result<(), CodecError> {
    let Some(application) = application else {
        writer.boolean(false);
        return Ok(());
    };
    writer.boolean(true);
    writer.u32(application.template_identity.0);
    writer.bytes(&application.template_commitment);
    encode_operational(
        writer,
        application.template_base_commitment,
        &application.suspension_dependency,
        &application.blocking_dependency,
    )?;
    writer.bytes(&application.specialization_commitment);
    encode_kinds(writer, &application.parameter_kinds)?;
    encode_telescope(writer, &application.telescope)?;
    writer.boolean(application.capture.is_some());
    if let Some(capture) = &application.capture {
        writer.u32(capture.template_identity.0);
        writer.bytes(&capture.template_commitment);
        encode_operational(
            writer,
            capture.template_base_commitment,
            &capture.suspension_dependency,
            &capture.blocking_dependency,
        )?;
        writer.bytes(&capture.specialization_commitment);
        encode_kinds(writer, &capture.parameter_kinds)?;
        encode_telescope(writer, &capture.telescope)?;
    }
    encode_service_ceiling(writer, &application.fixed)?;
    writer.len("reach dependencies", application.dependencies.len())?;
    for binder in &application.dependencies {
        writer.u32(*binder);
    }
    writer.len("reach calls", application.calls.len())?;
    for call in &application.calls {
        InvocationSite::write(writer, &call.site)?;
        writer.u32(call.binder);
        writer.boolean(call.application.is_some());
        if let Some(application) = &call.application {
            writer.id(application.callee);
            writer.bytes(&application.specialization_commitment);
            writer.len("reach call arguments", application.arguments.len())?;
            for argument in &application.arguments {
                encode_argument(writer, argument);
            }
        }
    }
    Ok(())
}

fn encode_kinds(writer: &mut Writer, kinds: &[u8]) -> Result<(), CodecError> {
    writer.len("reach parameter kinds", kinds.len())?;
    writer.bytes(kinds);
    Ok(())
}

fn decode_kinds(reader: &mut Reader<'_>) -> Result<Vec<u8>, CodecError> {
    decode_counted(reader, |reader| {
        let kind = reader.u8()?;
        if kind > 5 {
            return Err(CodecError::InvalidTag("ReachParameterKind", kind));
        }
        Ok(kind)
    })
}

fn encode_telescope(
    writer: &mut Writer,
    telescope: &[ClosedReachParameter],
) -> Result<(), CodecError> {
    writer.len("reach telescope", telescope.len())?;
    for parameter in telescope {
        match parameter {
            ClosedReachParameter::Type { argument } => {
                writer.u8(0);
                writer.string("reach type argument", argument)?;
            }
            ClosedReachParameter::Const { argument } => {
                writer.u8(1);
                writer.string("reach const argument", &argument.atom())?;
            }
            ClosedReachParameter::Proposition { argument } => {
                writer.u8(2);
                writer.string("reach proposition argument", argument)?;
            }
            ClosedReachParameter::Machine(binding) => {
                writer.u8(if binding.upper_bound.is_some() { 3 } else { 4 });
                writer.boolean(binding.nominal_requirement.is_some());
                if let Some(requirement) = &binding.nominal_requirement {
                    writer.string("reach requirement", requirement)?;
                }
                encode_kinds(writer, &binding.requirement_parameter_kinds)?;
                writer.len(
                    "reach requirement arguments",
                    binding.requirement_arguments.len(),
                )?;
                for argument in &binding.requirement_arguments {
                    writer.u32(argument.0);
                }
                if let Some(upper_bound) = &binding.upper_bound {
                    encode_service_ceiling(writer, upper_bound)?;
                }
                writer.boolean(binding.upper_bound_may_suspend);
                writer.boolean(binding.upper_bound_may_block);
                writer.boolean(binding.upper_bound_crashes_none);
                writer.boolean(binding.selected_may_suspend);
                writer.boolean(binding.selected_may_block);
                writer.boolean(binding.selected_may_crash);
                writer.u32(binding.selected_identity.0);
                writer.bytes(&binding.selected_contract_commitment);
                encode_service_ceiling(writer, &binding.selected_reach)?;
                encode_optional_id(writer, binding.callee);
                writer.boolean(binding.schema.is_some());
                if let Some(schema) = &binding.schema {
                    writer.u32(schema.template_identity.0);
                    writer.bytes(&schema.template_commitment);
                }
            }
        }
    }
    Ok(())
}

pub fn decode(reader: &mut Reader<'_>) -> Result<Option<ClosedReachApplication>, CodecError> {
    if !reader.boolean()? {
        return Ok(None);
    }
    let template_identity = TerminalIdentityIndex(reader.u32()?);
    let template_commitment = reader.array()?;
    let (template_base_commitment, suspension_dependency, blocking_dependency) =
        decode_operational(reader)?;
    let specialization_commitment = reader.array()?;
    let parameter_kinds = decode_kinds(reader)?;
    let telescope = decode_telescope(reader)?;
    let capture = if reader.boolean()? {
        let template_identity = TerminalIdentityIndex(reader.u32()?);
        let template_commitment = reader.array()?;
        let (
            template_base_commitment,
            suspension_dependency,
            blocking_dependency,
        ) = decode_operational(reader)?;
        Some(ClosedReachCapture {
            template_identity,
            template_commitment,
            template_base_commitment,
            suspension_dependency,
            blocking_dependency,
            specialization_commitment: reader.array()?,
            parameter_kinds: decode_kinds(reader)?,
            telescope: decode_telescope(reader)?,
        })
    } else {
        None
    };
    Ok(Some(ClosedReachApplication {
        template_identity,
        template_commitment,
        template_base_commitment,
        suspension_dependency,
        blocking_dependency,
        specialization_commitment,
        parameter_kinds,
        telescope,
        capture,
        fixed: decode_ids(reader, "ServiceId")?,
        dependencies: decode_counted(reader, |reader| reader.u32())?,
        calls: decode_counted(reader, |reader| {
            Ok(ClosedReachCall {
                site: InvocationSite::read(reader)?,
                binder: reader.u32()?,
                application: if reader.boolean()? {
                    Some(ClosedReachCallApplication {
                        callee: reader.id("MachineId")?,
                        specialization_commitment: reader.array()?,
                        arguments: decode_counted(reader, decode_argument)?,
                    })
                } else {
                    None
                },
            })
        })?,
    }))
}

pub fn decode_telescope(reader: &mut Reader<'_>) -> Result<Vec<ClosedReachParameter>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(match reader.u8()? {
            0 => ClosedReachParameter::Type {
                argument: reader.string("reach const argument")?,
            },
            1 => ClosedReachParameter::Const {
                argument: application_identity::const_value::CanonicalConstValue::from_atom(
                    &reader.string("reach const argument")?,
                )
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "reach const argument has no canonical value",
                ))?,
            },
            2 => ClosedReachParameter::Proposition {
                argument: reader.string("reach requirement")?,
            },
            tag @ (3 | 4) => {
                let nominal_requirement = if reader.boolean()? {
                    Some(reader.string("reach selected identity")?)
                } else {
                    None
                };
                ClosedReachParameter::Machine(ClosedReachMachineBinding {
                    nominal_requirement,
                    requirement_parameter_kinds: decode_kinds(reader)?,
                    requirement_arguments: decode_counted(reader, |reader| {
                        Ok(TerminalIdentityIndex(reader.u32()?))
                    })?,
                    upper_bound: if tag == 3 {
                        Some(decode_ids(reader, "ServiceId")?)
                    } else {
                        None
                    },
                    upper_bound_may_suspend: reader.boolean()?,
                    upper_bound_may_block: reader.boolean()?,
                    upper_bound_crashes_none: reader.boolean()?,
                    selected_may_suspend: reader.boolean()?,
                    selected_may_block: reader.boolean()?,
                    selected_may_crash: reader.boolean()?,
                    selected_identity: TerminalIdentityIndex(reader.u32()?),
                    selected_contract_commitment: reader.array()?,
                    selected_reach: decode_ids(reader, "ServiceId")?,
                    callee: decode_optional_id(reader, "MachineId")?,
                    schema: if reader.boolean()? {
                        Some(ClosedReachSchema {
                            template_identity: TerminalIdentityIndex(reader.u32()?),
                            template_commitment: reader.array()?,
                        })
                    } else {
                        None
                    },
                })
            }
            tag => return Err(CodecError::InvalidTag("ClosedReachParameter", tag)),
        })
    })
}

type Dependency = terminal_psi::ContractDependency<bool, Vec<u32>>;

fn encode_term(writer: &mut Writer, term: &Dependency) -> Result<(), CodecError> {
    writer.boolean(term.concrete);
    writer.len("callback dependency", term.parameters.len())?;
    for parameter in &term.parameters {
        writer.u32(*parameter);
    }
    Ok(())
}

fn decode_term(reader: &mut Reader<'_>) -> Result<Dependency, CodecError> {
    Ok(Dependency {
        concrete: reader.boolean()?,
        parameters: decode_counted(reader, |reader| reader.u32())?,
    })
}

fn encode_operational(
    writer: &mut Writer,
    base: [u8; 32],
    suspension: &Dependency,
    blocking: &Dependency,
) -> Result<(), CodecError> {
    writer.bytes(&base);
    encode_term(writer, suspension)?;
    encode_term(writer, blocking)
}

fn decode_operational(
    reader: &mut Reader<'_>,
) -> Result<([u8; 32], Dependency, Dependency), CodecError> {
    Ok((reader.array()?, decode_term(reader)?, decode_term(reader)?))
}

pub fn encode_argument(writer: &mut Writer, argument: &ClosedReachArgument) {
    match argument {
        ClosedReachArgument::Type(identity) => {
            writer.u8(0);
            writer.u32(identity.0);
        }
        ClosedReachArgument::Const(identity) => {
            writer.u8(1);
            writer.u32(identity.0);
        }
        ClosedReachArgument::Proposition(identity) => {
            writer.u8(2);
            writer.u32(identity.0);
        }
        ClosedReachArgument::Machine {
            identity,
            contract_commitment,
        } => {
            writer.u8(3);
            writer.u32(identity.0);
            writer.bytes(contract_commitment);
        }
    }
}
pub fn decode_argument(reader: &mut Reader<'_>) -> Result<ClosedReachArgument, CodecError> {
    Ok(match reader.u8()? {
        0 => ClosedReachArgument::Type(TerminalIdentityIndex(reader.u32()?)),
        1 => ClosedReachArgument::Const(TerminalIdentityIndex(reader.u32()?)),
        2 => ClosedReachArgument::Proposition(TerminalIdentityIndex(reader.u32()?)),
        3 => ClosedReachArgument::Machine {
            identity: TerminalIdentityIndex(reader.u32()?),
            contract_commitment: reader.array()?,
        },
        tag => return Err(CodecError::InvalidTag("ClosedReachArgument", tag)),
    })
}

pub fn encode_service_ceiling(
    writer: &mut Writer,
    services: &[ServiceId],
) -> Result<(), CodecError> {
    writer.ids("published service ceiling", services)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generic_contract_ends_after_its_blocking_dependency() {
        let mut writer = Writer::default();
        let suspend = Dependency::parameter(3, true);
        let block = true.into();
        encode_operational(&mut writer, [7; 32], &suspend, &block).unwrap();
        let mut expected = vec![7; 32];
        expected.extend_from_slice(&[0, 1, 0, 0, 0, 3, 0, 0, 0, 1, 0, 0, 0, 0]);
        assert_eq!(writer.finish(), expected);
        let mut reader = Reader::new(&expected);
        assert_eq!(
            decode_operational(&mut reader).unwrap(),
            ([7; 32], suspend, block)
        );
        assert_eq!(reader.remaining(), 0);
    }
}
