//! Visible data candidates for designated Build product queries.
//!
//! Only the query occurrence's retained product dependencies authorize foreign
//! names. Borrowing Build or importing a build-only alias grants no such scope.

use crate::interpreter::evaluator::{Evaluator, SymbolHandle};
use typed_trees::data::DataDefinition;

impl<'program> Evaluator<'program> {
    pub(super) fn product_data_candidates(
        &self,
        name: &str,
        qualified: Option<(&str, &str)>,
        occurrence: source::SourceSpan,
        include_symbol: impl Fn(SymbolHandle) -> bool,
    ) -> Vec<&'program DataDefinition> {
        let authorized = qualified.and_then(|(alias, _)| {
            self.product_declarations
                .symbols
                .product_dependency_target(occurrence, alias)
        });
        match (qualified, authorized) {
            (Some((_, rest)), Some(target)) => self
                .product_declarations
                .data_definitions()
                .iter()
                .filter(|definition| {
                    definition.is_public
                        && include_symbol(definition.symbol)
                        // An alias qualifies a complete path, not a module-relative name.
                        && self.product_declarations.symbols.display_path(definition.symbol, "::") == rest
                        && self
                            .product_declarations
                            .symbols
                            .symbol_product_package_identity(definition.symbol)
                            == Some(target)
                })
                .collect(),
            _ => self
                .product_declarations
                .data_definitions()
                .iter()
                .filter(|definition| {
                    (match qualified {
                        Some(_) => {
                            self.product_declarations
                                .symbols
                                .display_path(definition.symbol, "::")
                                .as_str()
                                == name
                        }
                        None => definition.name.as_str() == name,
                    }) && include_symbol(definition.symbol)
                        && self
                            .product_declarations
                            .symbols
                            .symbol_source_span(definition.symbol)
                            .is_some_and(|span| {
                                self.product_declarations
                                    .symbols
                                    .same_product_package_instance(occurrence, span)
                            })
                })
                .collect(),
        }
    }
}
