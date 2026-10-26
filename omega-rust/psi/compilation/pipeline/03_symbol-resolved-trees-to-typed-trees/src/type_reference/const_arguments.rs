//! Publish structured const atoms after their actual carrier is resolved.

use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceNode;

/// Authored names and pre-normalized arguments retain the selected Const
/// symbol through resolution. Their declaration supplies the actual carrier;
/// the receiving formal and encoded schema labels supply no nominal authority.
/// Retained cohorts are immutable, so only this run's new rows are rewritten.
pub(crate) fn normalize_const_argument_carriers(
    program: &mut TypedTrees,
    frontier: usize,
) -> Result<(), Diagnostic> {
    let mut replacements = Vec::new();
    for (site, symbol, spelling) in program.type_reference_table.named_references() {
        if usize::try_from(site.arena_index()).is_ok_and(|index| index <= frontier) {
            continue;
        }
        let Some(declaration) = program.const_declaration_by_symbol(symbol) else {
            continue;
        };
        let Some(_) = declaration
            .evaluated_value
            .as_ref()
            .and_then(|value| value.encoding())
        else {
            // Pending initializer evaluation remains an ordinary const name.
            continue;
        };
        let Some(value) =
            typed_trees::typed_trees::canonical_const_declaration_value(program, symbol)
        else {
            return Err(Diagnostic::error(
                "selected constant has no canonical static carrier value",
            ));
        };
        let decoded = value.decode_encoding().ok_or_else(|| {
            Diagnostic::error("selected constant has malformed canonical value encoding")
        })?;
        if !matches!(
            decoded,
            DecodedCanonicalConstValue::Array { .. }
                | DecodedCanonicalConstValue::Record { .. }
                | DecodedCanonicalConstValue::Variant { .. }
        ) {
            continue;
        }
        if let Some(retained) = CanonicalConstValue::from_atom(spelling)
            && retained.encoding != value.encoding
        {
            return Err(Diagnostic::error(
                "structured constant argument differs from its selected declaration value",
            ));
        }
        replacements.push((site, value.atom()));
    }
    for (site, spelling) in replacements {
        program.type_reference_table.substitute_node(
            site,
            TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: Identifier::generated(spelling),
            },
        );
    }
    Ok(())
}
