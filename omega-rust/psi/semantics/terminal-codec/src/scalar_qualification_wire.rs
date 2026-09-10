//! Canonical scalar membership catalogs. Graph custody is checked by the verifier.
use super::{
    CodecError, decode_counted,
    scalar_wire::{decode_scalar_type, encode_scalar_type},
    wire::{Reader, Writer},
};
use semantic_vocabulary::ScalarQualificationSetId;
use terminal_psi::{
    ScalarDomainDeclaration, ScalarQualificationCatalog, ScalarQualificationCoercion,
    ScalarQualificationSet,
};

pub(super) fn encode(
    writer: &mut Writer,
    catalog: &ScalarQualificationCatalog,
) -> Result<(), CodecError> {
    writer.len("scalar domains", catalog.domains.len())?;
    for domain in &catalog.domains {
        writer.id(domain.id);
        writer.id(domain.semantic_domain);
        writer.string("scalar domain identity", &domain.identity)?;
        encode_scalar_type(writer, domain.carrier);
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
    Ok(())
}

#[cfg(test)]
#[path = "scalar_qualification_wire_tests.rs"]
mod tests;

pub(super) fn decode(reader: &mut Reader<'_>) -> Result<ScalarQualificationCatalog, CodecError> {
    Ok(ScalarQualificationCatalog {
        domains: decode_counted(reader, |reader| {
            Ok(ScalarDomainDeclaration {
                id: reader.id("ScalarDomainId")?,
                semantic_domain: reader.id("DomainSemanticId")?,
                identity: reader.string("scalar domain identity")?,
                carrier: decode_scalar_type(reader)?,
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
    })
}

pub(super) fn validate(catalog: &ScalarQualificationCatalog) -> Result<(), CodecError> {
    if catalog
        .domains
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(CodecError::NonCanonicalOrder("scalar domains"));
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
    Ok(())
}
