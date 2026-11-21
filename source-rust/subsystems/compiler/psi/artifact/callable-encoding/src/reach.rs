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
        &application.conditional_carry,
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
            &capture.conditional_carry,
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
    let (template_base_commitment, suspension_dependency, blocking_dependency, conditional_carry) =
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
            conditional_carry,
        ) = decode_operational(reader)?;
        Some(ClosedReachCapture {
            template_identity,
            template_commitment,
            template_base_commitment,
            suspension_dependency,
            blocking_dependency,
            conditional_carry,
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
        conditional_carry,
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
type Carry = terminal_psi::ConditionalCarryRequirement;

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

fn encode_coordinate(writer: &mut Writer, coordinate: usize) -> Result<(), CodecError> {
    writer.u64(
        u64::try_from(coordinate).map_err(|_| CodecError::CollectionTooLong("carry coordinate"))?,
    );
    Ok(())
}

fn decode_coordinate(reader: &mut Reader<'_>) -> Result<usize, CodecError> {
    usize::try_from(reader.u64()?).map_err(|_| CodecError::CollectionTooLong("carry coordinate"))
}

fn encode_acquisition(
    writer: &mut Writer,
    machine: &str,
    state: &str,
    source: &psi_effect_contracts::ScopedPermissionEventSource<String>,
) -> Result<(), CodecError> {
    use psi_effect_contracts::ScopedPermissionEventSource as Source;
    writer.string("carry acquisition machine", machine)?;
    writer.string("carry acquisition state", state)?;
    match source {
        Source::StateEntry => writer.u8(0),
        Source::Statement { statement_index } => {
            writer.u8(1);
            encode_coordinate(writer, *statement_index)?;
        }
        Source::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => {
            writer.u8(2);
            encode_coordinate(writer, *statement_index)?;
            encode_coordinate(writer, *call_ordinal)?;
            writer.string("carry acquisition target", target_symbol)?;
        }
        Source::InstructionDisposition { statement_index } => {
            writer.u8(3);
            encode_coordinate(writer, *statement_index)?;
        }
        Source::InstructionExitDisposition { statement_index } => {
            writer.u8(4);
            encode_coordinate(writer, *statement_index)?;
        }
        Source::StateExit => writer.u8(5),
    }
    Ok(())
}
fn decode_acquisition(
    reader: &mut Reader<'_>,
) -> Result<
    (
        String,
        String,
        psi_effect_contracts::ScopedPermissionEventSource<String>,
    ),
    CodecError,
> {
    use psi_effect_contracts::ScopedPermissionEventSource as Source;
    let machine = reader.string("carry acquisition machine")?;
    let state = reader.string("carry acquisition state")?;
    let source = match reader.u8()? {
        0 => Source::StateEntry,
        1 => Source::Statement {
            statement_index: decode_coordinate(reader)?,
        },
        2 => Source::Call {
            statement_index: decode_coordinate(reader)?,
            call_ordinal: decode_coordinate(reader)?,
            target_symbol: reader.string("carry acquisition target")?,
        },
        3 => Source::InstructionDisposition {
            statement_index: decode_coordinate(reader)?,
        },
        4 => Source::InstructionExitDisposition {
            statement_index: decode_coordinate(reader)?,
        },
        5 => Source::StateExit,
        tag => return Err(CodecError::InvalidTag("carry acquisition", tag)),
    };
    Ok((machine, state, source))
}

fn encode_operational(
    writer: &mut Writer,
    base: [u8; 32],
    suspension: &Dependency,
    blocking: &Dependency,
    carry: &[Carry],
) -> Result<(), CodecError> {
    use psi_effect_contracts::SuspensionValueOrigin;
    writer.bytes(&base);
    encode_term(writer, suspension)?;
    encode_term(writer, blocking)?;
    writer.len("conditional carry", carry.len())?;
    for row in carry {
        writer.u32(row.state);
        writer.u64(row.statement_index);
        writer.u64(row.call_ordinal);
        match &row.subject {
            SuspensionValueOrigin::Parameter { symbol, position } => {
                writer.u8(0);
                writer.string("carry subject", symbol)?;
                encode_coordinate(writer, *position)?;
            }
            SuspensionValueOrigin::Local {
                symbol,
                statement_index,
                environment_position,
            } => {
                writer.u8(1);
                writer.string("carry subject", symbol)?;
                encode_coordinate(writer, *statement_index)?;
                encode_coordinate(writer, *environment_position)?;
            }
            SuspensionValueOrigin::CallArgument { position } => {
                writer.u8(2);
                encode_coordinate(writer, *position)?;
            }
        }
        writer.string("carry type", &row.value_type)?;
        writer.len("carry acquisitions", row.claims.len())?;
        for arrival in &row.claims {
            let psi_effect_contracts::ScopedPermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source,
                ordinal,
            } = &arrival.claim_identity
            else {
                return Err(CodecError::MalformedStructuralFoundation(
                    "conditional carry receiving claim is unknown",
                ));
            };
            encode_acquisition(writer, machine_symbol, state_symbol, source)?;
            writer.u32(*ordinal);
            match &arrival.provenance {
                psi_effect_contracts::ScopedPermissionProvenance::Unknown => {
                    return Err(CodecError::MalformedStructuralFoundation(
                        "conditional carry acquisition join is unknown",
                    ));
                }
                psi_effect_contracts::ScopedPermissionProvenance::Established {
                    machine_symbol,
                    state_symbol,
                    source,
                } => {
                    writer.u8(1);
                    encode_acquisition(writer, machine_symbol, state_symbol, source)?;
                }
                psi_effect_contracts::ScopedPermissionProvenance::Joined {
                    machine_symbol,
                    state_symbol,
                    source,
                    ordinal,
                } => {
                    writer.u8(2);
                    encode_acquisition(writer, machine_symbol, state_symbol, source)?;
                    writer.u32(*ordinal);
                }
            }
        }
        encode_term(writer, &row.condition)?;
    }
    Ok(())
}

fn decode_operational(
    reader: &mut Reader<'_>,
) -> Result<([u8; 32], Dependency, Dependency, Vec<Carry>), CodecError> {
    use psi_effect_contracts::SuspensionValueOrigin;
    let base = reader.array()?;
    let suspension = decode_term(reader)?;
    let blocking = decode_term(reader)?;
    let carry = decode_counted(reader, |reader| {
        let state = reader.u32()?;
        let statement_index = reader.u64()?;
        let call_ordinal = reader.u64()?;
        let subject = match reader.u8()? {
            0 => SuspensionValueOrigin::Parameter {
                symbol: reader.string("carry subject")?,
                position: decode_coordinate(reader)?,
            },
            1 => SuspensionValueOrigin::Local {
                symbol: reader.string("carry subject")?,
                statement_index: decode_coordinate(reader)?,
                environment_position: decode_coordinate(reader)?,
            },
            2 => SuspensionValueOrigin::CallArgument {
                position: decode_coordinate(reader)?,
            },
            tag => return Err(CodecError::InvalidTag("carry subject", tag)),
        };
        let value_type = reader.string("carry type")?;
        let claims = decode_counted(reader, |reader| {
            let (machine_symbol, state_symbol, source) = decode_acquisition(reader)?;
            let claim_identity = psi_effect_contracts::ScopedPermissionClaimIdentity::Established {
                machine_symbol,
                state_symbol,
                source,
                ordinal: reader.u32()?,
            };
            let tag = reader.u8()?;
            let (machine_symbol, state_symbol, source) = decode_acquisition(reader)?;
            let provenance = match tag {
                1 => psi_effect_contracts::ScopedPermissionProvenance::Established {
                    machine_symbol,
                    state_symbol,
                    source,
                },
                2 => psi_effect_contracts::ScopedPermissionProvenance::Joined {
                    machine_symbol,
                    state_symbol,
                    source,
                    ordinal: reader.u32()?,
                },
                tag => return Err(CodecError::InvalidTag("carry acquisition join", tag)),
            };
            Ok(psi_effect_contracts::PermissionArrival {
                claim_identity,
                provenance,
            })
        })?;
        Ok(Carry {
            state,
            statement_index,
            call_ordinal,
            subject,
            value_type,
            claims,
            condition: decode_term(reader)?,
        })
    })?;
    Ok((base, suspension, blocking, carry))
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
