//! Recognize the exact toolchain-owned opaque-representation relationship.
//! Build activation and package availability review share this check; recognition
//! does not select a conformance or authorize a representation.

use source::SourceOrigin;
use std::path::Path;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::TypeParameterKind;

const REPRESENTATION_TRAIT_NAME: &str = "OpaqueRepresentation";
const REPRESENTATION_TRAIT_SOURCE: &str = "core/representation.omg";

/// Whether `symbol` is the exact compiler-owned, capability-free
/// `OpaqueRepresentation<Opaque>` relationship. Package review uses the same
/// check when publishing producer availability; source spelling alone never
/// establishes this role.
pub fn is_compiler_owned_opaque_representation_trait(
    typed: &TypedTrees,
    symbol: SymbolHandle,
) -> bool {
    let Some(definition) = typed.traits().iter().find(|trait_| trait_.symbol == symbol) else {
        return false;
    };
    let parameters = typed.trait_type_parameters(definition);
    if definition.name.as_str() != REPRESENTATION_TRAIT_NAME
        || definition.is_boundary
        || !definition.lifetime_parameters.is_empty()
        || parameters.len() != 1
        || !matches!(parameters[0].kind, TypeParameterKind::Type)
        || !definition.conformance_bounds.is_empty()
        || !typed.trait_requirements(definition).is_empty()
        || !typed.trait_machine_signatures(definition).is_empty()
    {
        return false;
    }
    let Some(span) = typed.symbols.symbol_source_span(symbol) else {
        return false;
    };
    let Some(source) = typed.symbols.source_file(span) else {
        return false;
    };
    source.origin == SourceOrigin::Toolchain
        && source
            .path
            .ends_with(Path::new(REPRESENTATION_TRAIT_SOURCE))
}
