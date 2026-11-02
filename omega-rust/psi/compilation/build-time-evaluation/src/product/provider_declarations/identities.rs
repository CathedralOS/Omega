//! Named-provider declaration identity and plan derivation eligibility.
//!
//! Build names a declaration, not an instantiated conformance. Recognizing that
//! declaration must not discard its region telescope; receiving occurrences
//! later supply their checked applications. Concrete candidate admission and
//! retained parent applications remain separate admission obligations.

use checked_trees::typed_trees::{TypedTrees, trait_definition::Conformance};
use symbols::SymbolHandle;

pub fn is_named_provider(typed: &TypedTrees, symbol: SymbolHandle) -> bool {
    typed
        .conformances()
        .iter()
        .any(|definition| definition.symbol == symbol)
}

/// A declaration with no static Type/Const telescope can own a provider plan.
/// Its public regions remain parameters, not caller-region choices made by
/// Build. Executable candidates independently need an exact receiving
/// application and the checked correspondence for every public region slot.
pub fn eligible(definition: &Conformance) -> bool {
    definition.type_parameters.is_empty()
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
