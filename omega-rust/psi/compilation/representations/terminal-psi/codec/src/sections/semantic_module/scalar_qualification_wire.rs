//! Canonical scalar membership catalogs. Graph custody is checked by the verifier.
use super::{
    CodecError, decode_counted,
    scalar_wire::{
        decode_ieee_float_value, decode_integer_type, decode_integer_value, decode_scalar_type,
        encode_ieee_float_value, encode_integer_type, encode_integer_value, encode_scalar_type,
    },
    wire::{Reader, Writer},
};
use semantic_vocabulary::ScalarQualificationSetId;
use terminal_psi::{
    ScalarDomainDeclaration, ScalarDomainEstablishmentRoute, ScalarFloatRange, ScalarIntegerRange,
    ScalarQualificationCatalog, ScalarQualificationCoercion, ScalarQualificationSet,
};

const ROUTE_CHECKED_REQUIREMENT: u8 = 0;
const ROUTE_BOUNDARY_REQUIREMENT: u8 = 1;
const ROUTE_EXACT_MACHINE: u8 = 2;

pub(crate) fn encode(
    writer: &mut Writer,
    catalog: &ScalarQualificationCatalog,
) -> Result<(), CodecError> {
    writer.len("scalar domains", catalog.domains.len())?;
    for domain in &catalog.domains {
        writer.id(domain.id);
        writer.id(domain.semantic_domain);
        writer.string("scalar domain identity", &domain.identity)?;
        encode_scalar_type(writer, domain.carrier);
        writer.len(
            "scalar domain establishment routes",
            domain.establishment_routes.len(),
        )?;
        for route in &domain.establishment_routes {
            match route {
                ScalarDomainEstablishmentRoute::CheckedRequirement {
                    requirement_identity,
                } => {
                    writer.u8(ROUTE_CHECKED_REQUIREMENT);
                    writer.string("establishment requirement", requirement_identity)?;
                }
                ScalarDomainEstablishmentRoute::BoundaryRequirement {
                    requirement_identity,
                } => {
                    writer.u8(ROUTE_BOUNDARY_REQUIREMENT);
                    writer.string("establishment requirement", requirement_identity)?;
                }
                ScalarDomainEstablishmentRoute::ExactMachine { machine_identity } => {
                    writer.u8(ROUTE_EXACT_MACHINE);
                    writer.string("establishment machine", machine_identity)?;
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

#[cfg(test)]
mod tests;

pub(crate) fn decode(reader: &mut Reader<'_>) -> Result<ScalarQualificationCatalog, CodecError> {
    Ok(ScalarQualificationCatalog {
        domains: decode_counted(reader, |reader| {
            Ok(ScalarDomainDeclaration {
                id: reader.id("ScalarDomainId")?,
                semantic_domain: reader.id("DomainSemanticId")?,
                identity: reader.string("scalar domain identity")?,
                carrier: decode_scalar_type(reader)?,
                establishment_routes: decode_counted(reader, |reader| match reader.u8()? {
                    ROUTE_CHECKED_REQUIREMENT => {
                        Ok(ScalarDomainEstablishmentRoute::CheckedRequirement {
                            requirement_identity: reader.string("establishment requirement")?,
                        })
                    }
                    ROUTE_BOUNDARY_REQUIREMENT => {
                        Ok(ScalarDomainEstablishmentRoute::BoundaryRequirement {
                            requirement_identity: reader.string("establishment requirement")?,
                        })
                    }
                    ROUTE_EXACT_MACHINE => Ok(ScalarDomainEstablishmentRoute::ExactMachine {
                        machine_identity: reader.string("establishment machine")?,
                    }),
                    tag => Err(CodecError::InvalidTag(
                        "ScalarDomainEstablishmentRoute",
                        tag,
                    )),
                })?,
            })
        })?,
        sets: decode_counted(reader, |reader| {
            Ok(ScalarQualificationSet {
                id: ScalarQualificationSetId::new(reader.u64()?),
                domains: decode_counted(reader, |reader| reader.id("ScalarDomainId"))?,
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
            .any(|route| route.identity().is_empty())
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
