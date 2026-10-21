//! Identify the sealed resident-owner contract independently of view syntax.
//! These declaration coordinates bound possible effects of a signature. They
//! do not establish a grant, a resident value, backing, or a provider receipt.

use symbols::SymbolHandle;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub struct ResidentOwnerQualifications {
    pub extent: SymbolHandle,
    pub granted_symbol: SymbolHandle,
    pub resident_symbol: SymbolHandle,
    pub granted: language_semantics::SemanticDomainId,
    pub resident: language_semantics::SemanticDomainId,
    pub policy: TypeReferenceHandle,
    pub schema: TypeReferenceHandle,
}

pub fn resident_owner_qualifications(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<ResidentOwnerQualifications> {
    let carrier = program
        .type_reference_table
        .peeled_type_reference(reference);
    let TypeReferenceNode::Named { symbol: extent, .. } =
        program.type_reference_table.type_reference(carrier)
    else {
        return None;
    };
    if !is_extent_declaration(program, *extent) || program.symbols.name(*extent) != "Extent" {
        return None;
    }
    let domains = super::field_domains::declared_domain_constraint_records(program, reference);
    let canonical_domain = |name| {
        let mut matching = domains.iter().copied().filter(|domain| {
            is_extent_declaration(program, domain.symbol)
                && program.domain_definition(domain.symbol).is_some_and(|definition| {
                    definition.name.as_str().rsplit("::").next() == Some(name)
                        && matches!(
                            program.type_reference_table.type_reference(
                                program.type_reference_table.peeled_type_reference(definition.target_type)
                            ),
                            TypeReferenceNode::Named { symbol, .. } if symbol == extent
                        )
                })
        });
        let domain = matching.next()?;
        (matching.next().is_none() && domain.semantic_id.is_valid()).then_some(domain)
    };
    let granted = canonical_domain("Granted")?;
    let resident = canonical_domain("Resident")?;
    if !granted.arguments.is_empty() {
        return None;
    }
    let [policy, schema] = resident.arguments.as_slice() else {
        return None;
    };
    Some(ResidentOwnerQualifications {
        extent: *extent,
        granted_symbol: granted.symbol,
        resident_symbol: resident.symbol,
        granted: granted.semantic_id,
        resident: resident.semantic_id,
        policy: *policy,
        schema: *schema,
    })
}

pub fn is_extent_declaration(program: &dyn crate::CheckingInput, symbol: SymbolHandle) -> bool {
    program
        .symbols
        .symbol_source_span(symbol)
        .and_then(|span| program.symbols.source_file(span))
        .is_some_and(|file| {
            file.origin == source::SourceOrigin::Toolchain
                && file.path.strip_prefix(&file.package_root).ok()
                    == Some(std::path::Path::new("extent.omg"))
        })
}
