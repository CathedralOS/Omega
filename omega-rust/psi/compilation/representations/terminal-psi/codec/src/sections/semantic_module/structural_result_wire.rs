//! Canonical structural function-result and operation-result wire rows.

use terminal_psi::{
    ResultQualificationEstablishment, StructuralMultiplicity, StructuralOperationResult,
    StructuralResultClaimBinding, StructuralResultDeclaration,
};

use super::CodecError;
use super::structural_signature_wire::{
    decode_projected_qualifications, encode_projected_qualifications,
};
use super::wire::{Reader, Writer};
use crate::sections::semantic_module::structural_place_wire::{
    decode_reference_origin_path, decode_structural_path, encode_reference_origin_path,
    encode_structural_path,
};
use crate::sections::semantic_module::wire::{decode_counted, decode_ids};

pub(crate) fn validate_reference_sources(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
) -> Result<(), CodecError> {
    let Some(result) = machine.result.structural() else {
        return Ok(());
    };
    validate_reference_source_order(result)?;
    for reference in &result.reference_sources {
        let reference_type =
            crate::sections::semantic_module::module_foundation_validation::validate_structural_path(
                module,
                result.structural_type,
                &reference.path,
            )?;
        let Some(terminal_psi::StructuralTypeShape::Reference { referent, access }) =
            module.structural_type(reference_type).map(|row| &row.shape)
        else {
            return crate::codec_error::malformed(
                "reference result source must identify a reference leaf",
            );
        };
        for source in &reference.sources {
            let root = match source {
                terminal_psi::StructuralReferenceOrigin::Parameter(source) => {
                    machine
                        .signature_structural_parameter(source.place)
                        .ok_or(CodecError::MalformedStructuralFoundation(
                            "reference result source must identify a formal ingress",
                        ))?
                        .structural_type
                }
                terminal_psi::StructuralReferenceOrigin::Invocation { carrier_type, .. } => {
                    *carrier_type
                }
            };
            let endpoint =
                crate::sections::semantic_module::module_foundation_validation::validate_reference_origin_family(
                    module, root, source.path(),
                )?;
            if *access != source.access()
                || (source.extent() == semantic_vocabulary::ReferenceOriginExtent::Projected
                    && *referent != endpoint)
            {
                return crate::codec_error::malformed(
                    "reference result source has mismatched referent or access",
                );
            }
        }
    }
    Ok(())
}

fn validate_reference_source_order(result: &StructuralResultDeclaration) -> Result<(), CodecError> {
    if result
        .reference_sources
        .windows(2)
        .any(|pair| pair[0].path >= pair[1].path)
    {
        return crate::codec_error::malformed(
            "reference result sources must have unique ordered paths",
        );
    }
    for reference in &result.reference_sources {
        if reference.sources.is_empty()
            || reference.sources.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return crate::codec_error::malformed(
                "reference result alternatives must be nonempty, unique and ordered",
            );
        }
    }
    Ok(())
}

pub(crate) fn encode_function_result(
    writer: &mut Writer,
    result: &StructuralResultDeclaration,
) -> Result<(), CodecError> {
    writer.id(result.place);
    writer.id(result.structural_type);
    encode_multiplicity(writer, result.multiplicity);
    writer.len(
        "structural result qualifications",
        result.qualifications.len(),
    )?;
    for qualification in &result.qualifications {
        writer.id(*qualification);
    }
    encode_projected_qualifications(writer, &result.projected_qualifications)?;
    writer.len(
        "structural reference result sources",
        result.reference_sources.len(),
    )?;
    for reference in &result.reference_sources {
        encode_structural_path(writer, "reference result path", &reference.path)?;
        writer.len("reference source alternatives", reference.sources.len())?;
        for source in &reference.sources {
            match source {
                terminal_psi::StructuralReferenceOrigin::Parameter(source) => {
                    writer.u8(1);
                    writer.id(source.place);
                }
                terminal_psi::StructuralReferenceOrigin::Invocation { carrier_type, .. } => {
                    writer.u8(2);
                    writer.id(*carrier_type);
                }
            }
            super::structural_signature_wire::encode_structural_access(writer, source.access());
            writer.u8(match source.extent() {
                semantic_vocabulary::ReferenceOriginExtent::Projected => 1,
                semantic_vocabulary::ReferenceOriginExtent::WithinRegion => 2,
            });
            encode_reference_origin_path(writer, "reference source path", source.path())?;
        }
    }
    Ok(())
}

pub(crate) fn decode_function_result(
    reader: &mut Reader<'_>,
) -> Result<StructuralResultDeclaration, CodecError> {
    let result = StructuralResultDeclaration {
        place: reader.id("PlaceId")?,
        structural_type: reader.id("StructuralTypeId")?,
        multiplicity: decode_multiplicity(reader)?,
        qualifications: decode_ids(reader, "StructuralDomainId")?,
        projected_qualifications: decode_projected_qualifications(reader)?,
        reference_sources: decode_counted(reader, |reader| {
            Ok(terminal_psi::StructuralReferenceResultSource {
                path: decode_structural_path(reader)?,
                sources: decode_counted(reader, |reader| {
                    let kind = reader.u8()?;
                    let (parameter, carrier) = match kind {
                        1 => (Some(reader.id("PlaceId")?), None),
                        2 => (None, Some(reader.id("StructuralTypeId")?)),
                        tag => {
                            return Err(CodecError::InvalidTag("StructuralReferenceOrigin", tag));
                        }
                    };
                    let access =
                        super::structural_signature_wire::decode_structural_access(reader)?;
                    let extent = match reader.u8()? {
                        1 => semantic_vocabulary::ReferenceOriginExtent::Projected,
                        2 => semantic_vocabulary::ReferenceOriginExtent::WithinRegion,
                        tag => return Err(CodecError::InvalidTag("ReferenceOriginExtent", tag)),
                    };
                    let path = decode_reference_origin_path(reader)?;
                    match kind {
                        1 => Ok(terminal_psi::StructuralReferenceOrigin::Parameter(
                            terminal_psi::ReferenceOriginSource {
                                place: parameter.ok_or(CodecError::NonCanonicalEncoding)?,
                                access,
                                extent,
                                path,
                            },
                        )),
                        2 => Ok(terminal_psi::StructuralReferenceOrigin::Invocation {
                            carrier_type: carrier.ok_or(CodecError::NonCanonicalEncoding)?,
                            access,
                            extent,
                            path,
                        }),
                        tag => Err(CodecError::InvalidTag("StructuralReferenceOrigin", tag)),
                    }
                })?,
            })
        })?,
    };
    validate_reference_source_order(&result)?;
    Ok(result)
}

pub(crate) fn encode_operation_result(
    writer: &mut Writer,
    result: &StructuralOperationResult,
) -> Result<(), CodecError> {
    writer.id(result.place);
    writer.id(result.structural_type);
    encode_multiplicity(writer, result.multiplicity);
    writer.len(
        "structural operation result qualifications",
        result.qualifications.len(),
    )?;
    for qualification in &result.qualifications {
        writer.id(*qualification);
    }
    encode_projected_qualifications(writer, &result.projected_qualifications)?;
    writer.len(
        "operation result qualification establishments",
        result.qualification_establishments.len(),
    )?;
    for establishment in &result.qualification_establishments {
        writer.id(establishment.domain);
        writer.u64(u64::from(establishment.route));
    }
    encode_claim_bindings(writer, &result.claims)
}

/// Operation results and returning edges carry the same path-based claim join.
/// Keep one row codec so neither envelope can lose the Case guard or rebind a
/// claim by machine-local identity order instead of its typed source path.
pub(crate) fn encode_claim_bindings(
    writer: &mut Writer,
    bindings: &[StructuralResultClaimBinding],
) -> Result<(), CodecError> {
    writer.len("structural claim bindings", bindings.len())?;
    for claim in bindings {
        writer.id(claim.claim);
        encode_structural_path(writer, "structural claim binding path", &claim.path)?;
    }
    Ok(())
}

pub(crate) fn decode_operation_result(
    reader: &mut Reader<'_>,
) -> Result<StructuralOperationResult, CodecError> {
    Ok(StructuralOperationResult {
        place: reader.id("PlaceId")?,
        structural_type: reader.id("StructuralTypeId")?,
        multiplicity: decode_multiplicity(reader)?,
        qualifications: decode_ids(reader, "StructuralDomainId")?,
        projected_qualifications: decode_projected_qualifications(reader)?,
        qualification_establishments: decode_counted(reader, |reader| {
            Ok(ResultQualificationEstablishment {
                domain: reader.id("StructuralDomainId")?,
                route: u32::try_from(reader.u64()?).map_err(|_| {
                    CodecError::MalformedStructuralFoundation("route index out of range")
                })?,
            })
        })?,
        claims: decode_claim_bindings(reader)?,
    })
}

pub(crate) fn decode_claim_bindings(
    reader: &mut Reader<'_>,
) -> Result<Vec<StructuralResultClaimBinding>, CodecError> {
    decode_counted(reader, |reader| {
        Ok(StructuralResultClaimBinding {
            claim: reader.id("ClaimId")?,
            path: decode_structural_path(reader)?,
        })
    })
}

pub(crate) fn encode_multiplicity(writer: &mut Writer, multiplicity: StructuralMultiplicity) {
    writer.u8(match multiplicity {
        StructuralMultiplicity::Unrestricted => 1,
        StructuralMultiplicity::Affine => 2,
        StructuralMultiplicity::Linear => 3,
    });
}

pub(crate) fn decode_multiplicity(
    reader: &mut Reader<'_>,
) -> Result<StructuralMultiplicity, CodecError> {
    match reader.u8()? {
        1 => Ok(StructuralMultiplicity::Unrestricted),
        2 => Ok(StructuralMultiplicity::Affine),
        3 => Ok(StructuralMultiplicity::Linear),
        tag => Err(CodecError::InvalidTag("StructuralMultiplicity", tag)),
    }
}
