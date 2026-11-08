/*
We admit source-level opaque representation relationships
(`form_opaque_representation_request`, `collect_opaque_representation_selections`,
and `rederive_opaque_representation_selections`) and recognize the
toolchain-owned `OpaqueRepresentation` trait
(`is_compiler_owned_opaque_representation_trait`).
*/

/*
Our admission service harvests `Build::select_representation<Opaque, Conformance>`
selections from the authoritative build machine, closes the named conformance
application, verifies that the complete concrete carrier graph is inert (and
structurally copyable when `[copy]` is requested), and replays retained
selections against the final typed graph without accepting physical ABI numbers
from source.
*/

/*
`representation-planning` sits beside `validation` and produces validated
`OpaqueRepresentationSelection` records for build evaluation and final Psi
checking.
*/

#![forbid(unsafe_code)]

mod representation_selection;

pub(crate) mod representation_trait {
    use source::SourceOrigin;
    use std::path::Path;
    use symbols::SymbolHandle;
    use typed_trees::TypedTrees;
    use typed_trees::data::TypeParameterKind;

    const REPRESENTATION_TRAIT_NAME: &str = "OpaqueRepresentation";
    const REPRESENTATION_TRAIT_SOURCE: &str = "core/representation.omg";

    pub fn is_compiler_owned_opaque_representation_trait(
        typed: &TypedTrees,
        symbol: SymbolHandle,
    ) -> bool {
        let Some(definition) = typed.trait_definition_by_symbol(symbol) else {
            return false;
        };
        let parameters = typed.trait_type_parameters(definition);
        if definition.name.as_str() != REPRESENTATION_TRAIT_NAME
            || definition.is_boundary
            || !definition.lifetime_parameters.is_empty()
            || parameters.len() != 2
            || !parameters
                .iter()
                .all(|parameter| matches!(parameter.kind, TypeParameterKind::Type))
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
}

pub use representation_selection::{
    FormedOpaqueRepresentation, collect_opaque_representation_selections,
    form_opaque_representation_request, rederive_opaque_representation_selections,
};
pub use representation_selections::{
    OPAQUE_REPRESENTATION_APPLICATION_SCHEMA_VERSION, OpaqueRepresentationApplicationOrigin,
    OpaqueRepresentationCopyDisposition, OpaqueRepresentationLifecycleDisposition,
    OpaqueRepresentationSelection, selected_application_commitment, selection_for_opaque,
};
pub use representation_trait::is_compiler_owned_opaque_representation_trait;
