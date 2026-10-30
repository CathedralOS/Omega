//! Canonical pending scalar applications, never target-selected answers.

use super::super::CodecError;
use super::super::scalar_wire::{decode_scalar_type, encode_scalar_type};
use super::super::wire::{Reader, Writer};
use super::callable_wire::{decode_callable, encode_callable};
use terminal_psi::{LayoutMeasurement, ScalarApplication, ScalarApplications};

pub(super) fn encode(
    writer: &mut Writer,
    applications: &ScalarApplications,
) -> Result<(), CodecError> {
    writer.len("scalar applications", applications.declarations().len())?;
    for application in applications.declarations() {
        match application {
            ScalarApplication::Missing => {
                return Err(CodecError::MalformedStructuralFoundation(
                    "scalar application catalog contains the absent sentinel",
                ));
            }
            ScalarApplication::LayoutObservation {
                subject,
                measurement,
                semantics_version,
                scalar_type,
            } => {
                writer.u8(1);
                writer.id(*subject);
                writer.u8(match measurement {
                    LayoutMeasurement::Size => 1,
                    LayoutMeasurement::Alignment => 2,
                });
                writer.u32(*semantics_version);
                encode_scalar_type(writer, *scalar_type);
            }
            ScalarApplication::Evaluate { entry, scalar_type } => {
                writer.u8(2);
                encode_callable(writer, *entry);
                encode_scalar_type(writer, *scalar_type);
            }
        }
    }
    super::scalar_condition_wire::encode(writer, &applications.conditions)?;
    super::proof_scope_wire::encode(writer, &applications.proof_scope)
}

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<ScalarApplications, CodecError> {
    let count = reader.count()?;
    let mut applications = ScalarApplications::default();
    for _ in 0..count {
        let declaration = match reader.u8()? {
            1 => ScalarApplication::LayoutObservation {
                subject: reader.id("StructuralTypeId")?,
                measurement: match reader.u8()? {
                    1 => LayoutMeasurement::Size,
                    2 => LayoutMeasurement::Alignment,
                    tag => return Err(CodecError::InvalidTag("LayoutMeasurement", tag)),
                },
                semantics_version: reader.u32()?,
                scalar_type: decode_scalar_type(reader)?,
            },
            2 => ScalarApplication::Evaluate {
                entry: decode_callable(reader)?,
                scalar_type: decode_scalar_type(reader)?,
            },
            tag => return Err(CodecError::InvalidTag("ScalarApplication", tag)),
        };
        applications
            .insert(declaration)
            .map_err(CodecError::MalformedStructuralFoundation)?;
    }
    applications.conditions = super::scalar_condition_wire::decode(reader)?;
    applications.proof_scope = super::proof_scope_wire::decode(reader)?;
    Ok(applications)
}
