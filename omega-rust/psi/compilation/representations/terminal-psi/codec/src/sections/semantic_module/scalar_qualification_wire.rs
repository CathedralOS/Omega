//! Canonical scalar membership catalogs. Graph custody is checked by the verifier.
use super::{
    CodecError, decode_counted,
    scalar_wire::{
        decode_ieee_float_value, decode_integer_type, decode_integer_value, decode_scalar_type,
        encode_ieee_float_value, encode_integer_type, encode_integer_value, encode_scalar_type,
    },
    wire::{Reader, Writer, decode_ids},
};
use semantic_vocabulary::ScalarQualificationSetId;
use terminal_psi::{
    ScalarDomainDeclaration, ScalarDomainPredicate, ScalarFloatRange, ScalarIntegerRange,
    ScalarQualificationCatalog, ScalarQualificationCoercion, ScalarQualificationSet,
    ScalarReturnOrigin, ScalarReturnQualification, ScalarReturnSite, TerminalIdentityIndex,
};

pub(crate) fn encode(
    writer: &mut Writer,
    catalog: &ScalarQualificationCatalog,
) -> Result<(), CodecError> {
    writer.len("scalar domains", catalog.domains.len())?;
    for domain in &catalog.domains {
        writer.id(domain.id);
        writer.id(domain.semantic_domain);
        super::module_wire::declaration_wire::encode_required_memberships(
            writer,
            &domain.required_memberships,
        )?;
        writer.u32(domain.identity.0);
        encode_scalar_type(writer, domain.carrier);
        writer.boolean(domain.predicate.is_some());
        if let Some(predicate) = &domain.predicate {
            writer.id(predicate.subject);
            super::encode_proposition(writer, &predicate.proposition, 0)?;
        }
        super::module_wire::declaration_wire::encode_establishment_routes(
            writer,
            &domain.establishment_routes,
        )?;
    }
    writer.len("scalar return qualifications", catalog.returns.len())?;
    for returned in &catalog.returns {
        writer.id(returned.machine);
        writer.id(returned.edge);
        writer.id(returned.source);
        writer.id(returned.result);
        match returned.site {
            ScalarReturnSite::NormalExit => writer.u8(0),
            ScalarReturnSite::Completion { argument_ordinal } => {
                writer.u8(1);
                writer.u32(argument_ordinal);
            }
        }
        writer.id(returned.domain);
        writer.len("scalar return origins", returned.origins.len())?;
        for origin in &returned.origins {
            match origin {
                ScalarReturnOrigin::Forwarded => writer.u8(0),
                ScalarReturnOrigin::Vacuous => writer.u8(2),
                ScalarReturnOrigin::Established { route } => {
                    writer.u8(1);
                    writer.u32(*route);
                }
            }
        }
    }
    writer.len("scalar qualification sets", catalog.sets.len())?;
    for set in &catalog.sets {
        writer.u64(set.id.get());
        writer.len("scalar qualification members", set.domains.len())?;
        for domain in &set.domains {
            writer.id(*domain);
        }
    }
    writer.len("scalar field qualifications", catalog.fields.len())?;
    for field in &catalog.fields {
        writer.id(field.field);
        encode_scalar_type(writer, field.value_type.scalar_type);
        writer.u64(field.value_type.qualifications.get());
    }
    writer.len("scalar qualification coercions", catalog.coercions.len())?;
    for coercion in &catalog.coercions {
        writer.id(coercion.machine);
        writer.id(coercion.edge);
        writer.u32(coercion.argument_ordinal);
        writer.id(coercion.source);
        writer.id(coercion.destination);
    }
    writer.len(
        "scalar float entry ranges",
        catalog.float_entry_ranges.len(),
    )?;
    for range in &catalog.float_entry_ranges {
        writer.id(range.machine);
        writer.id(range.parameter);
        encode_ieee_float_value(writer, range.minimum);
        encode_ieee_float_value(writer, range.maximum);
        writer.boolean(range.maximum_inclusive);
    }
    writer.len(
        "scalar integer entry ranges",
        catalog.integer_entry_ranges.len(),
    )?;
    for range in &catalog.integer_entry_ranges {
        writer.id(range.machine);
        writer.id(range.parameter);
        encode_integer_type(writer, range.integer_type);
        encode_integer_value(writer, range.minimum);
        encode_integer_value(writer, range.maximum);
    }
    Ok(())
}

pub(crate) fn decode(reader: &mut Reader<'_>) -> Result<ScalarQualificationCatalog, CodecError> {
    Ok(ScalarQualificationCatalog {
        domains: decode_counted(reader, |reader| {
            Ok(ScalarDomainDeclaration {
                id: reader.id("ScalarDomainId")?,
                semantic_domain: reader.id("DomainSemanticId")?,
                required_memberships:
                    super::module_wire::declaration_wire::decode_required_memberships(reader)?,
                identity: TerminalIdentityIndex(reader.u32()?),
                carrier: decode_scalar_type(reader)?,
                predicate: if reader.boolean()? {
                    Some(ScalarDomainPredicate {
                        subject: reader.id("ValueId")?,
                        proposition: super::decode_proposition(reader, 0)?,
                    })
                } else {
                    None
                },
                establishment_routes:
                    super::module_wire::declaration_wire::decode_establishment_routes(reader)?,
            })
        })?,
        returns: decode_counted(reader, |reader| {
            Ok(ScalarReturnQualification {
                machine: reader.id("MachineId")?,
                edge: reader.id("EdgeId")?,
                source: reader.id("ValueId")?,
                result: reader.id("ValueId")?,
                site: match reader.u8()? {
                    0 => ScalarReturnSite::NormalExit,
                    1 => ScalarReturnSite::Completion {
                        argument_ordinal: reader.u32()?,
                    },
                    tag => return Err(CodecError::InvalidTag("ScalarReturnSite", tag)),
                },
                domain: reader.id("ScalarDomainId")?,
                origins: decode_counted(reader, |reader| match reader.u8()? {
                    0 => Ok(ScalarReturnOrigin::Forwarded),
                    2 => Ok(ScalarReturnOrigin::Vacuous),
                    1 => Ok(ScalarReturnOrigin::Established {
                        route: reader.u32()?,
                    }),
                    tag => Err(CodecError::InvalidTag("ScalarReturnOrigin", tag)),
                })?,
            })
        })?,
        sets: decode_counted(reader, |reader| {
            Ok(ScalarQualificationSet {
                id: ScalarQualificationSetId::new(reader.u64()?),
                domains: decode_ids(reader, "ScalarDomainId")?,
            })
        })?,
        fields: decode_counted(reader, |reader| {
            Ok(terminal_psi::ScalarFieldQualification {
                field: reader.id("StructuralFieldId")?,
                value_type: semantic_vocabulary::QualifiedScalarType {
                    scalar_type: decode_scalar_type(reader)?,
                    qualifications: ScalarQualificationSetId::new(reader.u64()?),
                },
            })
        })?,
        coercions: decode_counted(reader, |reader| {
            Ok(ScalarQualificationCoercion {
                machine: reader.id("MachineId")?,
                edge: reader.id("EdgeId")?,
                argument_ordinal: reader.u32()?,
                source: reader.id("ValueId")?,
                destination: reader.id("ValueId")?,
            })
        })?,
        float_entry_ranges: decode_counted(reader, |reader| {
            Ok(ScalarFloatRange {
                machine: reader.id("MachineId")?,
                parameter: reader.id("ValueId")?,
                minimum: decode_ieee_float_value(reader)?,
                maximum: decode_ieee_float_value(reader)?,
                maximum_inclusive: reader.boolean()?,
            })
        })?,
        integer_entry_ranges: decode_counted(reader, |reader| {
            Ok(ScalarIntegerRange {
                machine: reader.id("MachineId")?,
                parameter: reader.id("ValueId")?,
                integer_type: decode_integer_type(reader)?,
                minimum: decode_integer_value(reader)?,
                maximum: decode_integer_value(reader)?,
            })
        })?,
    })
}

pub(crate) fn validate(catalog: &ScalarQualificationCatalog) -> Result<(), CodecError> {
    if catalog
        .fields
        .windows(2)
        .any(|pair| pair[0].field >= pair[1].field)
    {
        return Err(CodecError::NonCanonicalOrder("scalar field qualifications"));
    }
    if catalog.returns.windows(2).any(|pair| {
        let key = |row: &ScalarReturnQualification| (row.machine, row.edge, row.domain);
        key(&pair[0]) >= key(&pair[1])
    }) {
        return Err(CodecError::NonCanonicalOrder(
            "scalar return qualifications",
        ));
    }
    for returned in &catalog.returns {
        if returned.origins.is_empty() || returned.origins.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(CodecError::NonCanonicalOrder("scalar return origins"));
        }
    }
    if catalog
        .domains
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(CodecError::NonCanonicalOrder("scalar domains"));
    }
    for domain in &catalog.domains {
        if domain
            .establishment_routes
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(CodecError::NonCanonicalOrder(
                "scalar domain establishment routes",
            ));
        }
        if domain
            .establishment_routes
            .iter()
            .any(|route| route.identity().is_absent())
        {
            return Err(CodecError::NonCanonicalEncoding);
        }
    }
    if catalog.sets.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(CodecError::NonCanonicalOrder("scalar qualification sets"));
    }
    for (index, set) in catalog.sets.iter().enumerate() {
        if set.id.is_empty() || set.domains.is_empty() {
            return Err(CodecError::NonCanonicalEncoding);
        }
        if set.domains.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(CodecError::NonCanonicalOrder(
                "scalar qualification members",
            ));
        }
        if catalog.sets[..index]
            .iter()
            .any(|previous| previous.domains == set.domains)
        {
            return Err(CodecError::NonCanonicalEncoding);
        }
    }
    if catalog.coercions.windows(2).any(|pair| {
        let key = |coercion: &ScalarQualificationCoercion| {
            (coercion.machine, coercion.edge, coercion.argument_ordinal)
        };
        key(&pair[0]) >= key(&pair[1])
    }) {
        return Err(CodecError::NonCanonicalOrder(
            "scalar qualification coercions",
        ));
    }
    if catalog
        .float_entry_ranges
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].parameter) >= (pair[1].machine, pair[1].parameter))
    {
        return Err(CodecError::NonCanonicalOrder("scalar float entry ranges"));
    }
    if catalog
        .float_entry_ranges
        .iter()
        .any(|range| range.minimum.format() != range.maximum.format())
    {
        return Err(CodecError::NonCanonicalEncoding);
    }
    if catalog
        .integer_entry_ranges
        .windows(2)
        .any(|pair| (pair[0].machine, pair[0].parameter) >= (pair[1].machine, pair[1].parameter))
    {
        return Err(CodecError::NonCanonicalOrder("scalar integer entry ranges"));
    }
    // An integer entry range is canonical only on a fixed-width carrier with
    // both endpoints admitted and ordered; an address carrier, a
    // sign-mismatched endpoint, or a reversed interval is malformed wire.
    if catalog
        .integer_entry_ranges
        .iter()
        .any(|range| !range.ordered())
    {
        return Err(CodecError::NonCanonicalEncoding);
    }
    Ok(())
}
