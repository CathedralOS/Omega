//! Exact named-provider declaration eligibility and retained parent applications.

use checked_trees::typed_trees::{TypedTrees, trait_definition::Conformance};
use symbols::SymbolHandle;

pub fn is_named_provider(typed: &TypedTrees, symbol: SymbolHandle) -> bool {
    typed
        .conformances()
        .iter()
        .any(|definition| definition.symbol == symbol && eligible(definition))
}

/// A closed nongeneric named implementation can be selected as a provider;
/// a generic one needs an explicit application that provider selection does
/// not carry.
pub fn eligible(definition: &Conformance) -> bool {
    definition.type_parameters.is_empty() && definition.lifetime_parameters.is_empty()
}

/// Retain only a declaring-trait lifetime application the map actually owns.
/// Parent lifetime composition is not retained on closed rows. An empty parent
/// telescope needs no substitution; a nonempty one needs that missing evidence.
pub fn named_requirement_lifetime_arguments<'a>(
    typed: &'a TypedTrees,
    implementation: &'a Conformance,
    declaring_trait: SymbolHandle,
) -> Option<&'a [u32]> {
    let root = typed.trait_definition_by_symbol(implementation.trait_symbol)?;
    let arguments = implementation.trait_lifetime_arguments.as_slice();
    if arguments.len() != root.lifetime_parameters.len()
        || arguments.iter().any(|ordinal| {
            usize::try_from(*ordinal).map_or(true, |ordinal| {
                ordinal >= implementation.lifetime_parameters.len()
            })
        })
    {
        return None;
    }
    if root.symbol == declaring_trait {
        return Some(arguments);
    }
    let declaring = typed.trait_definition_by_symbol(declaring_trait)?;
    declaring.lifetime_parameters.is_empty().then_some(&[])
}
