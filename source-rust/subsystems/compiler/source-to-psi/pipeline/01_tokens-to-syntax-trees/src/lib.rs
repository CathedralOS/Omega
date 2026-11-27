//! Parse token streams into unresolved syntax trees, preserving source order and spans.
//! `parse_into` and `parse_file` parse one file into a caller-owned syntax page;
//! `driver/parse_source_batch` owns multi-file page scheduling.
//! Grammar helpers and partial tree construction are private to this stage.

#![forbid(unsafe_code)]

pub use errors::parse_error::ParseError;

use crate::declarations::items::{ParsedDeclaration, parse_item};
use crate::token_input::cursor::Input;
use source_text::{SourceId, SourcePosition};
use syntax_trees::{ParsedRootItems, ParsedSource, SyntaxTrees, item::Item};
use tokens::TokenStream;

pub fn parse_into(
    syntax_trees: &mut SyntaxTrees,
    source_id: SourceId,
    tokens: &TokenStream<'_>,
) -> Result<ParsedRootItems, ParseError> {
    let qualifiers = token_input::file_qualifiers::FileQualifiers::default();
    let mut input = Input::new(source_id, tokens, syntax_trees);
    input.file_qualifiers = Some(&qualifiers);
    let mut roots = ParsedRootItems::default();
    while !input.tokens.is_empty() {
        let (declaration, rest) = parse_item(syntax_trees, input)?;
        match declaration {
            ParsedDeclaration::Item(item) => {
                if matches!(item, Item::Use(_)) {
                    if roots.import_range.is_empty() {
                        roots.import_range.start = roots.items.len();
                    }
                    roots.import_range.end = roots.items.len() + 1;
                }
                roots.items.push(syntax_trees.push_root_item(item));
            }
            ParsedDeclaration::DataFamily(items) => {
                for item in items {
                    roots.items.push(syntax_trees.push_root_item(item));
                }
            }
            ParsedDeclaration::Mathematical(definition) => {
                syntax_trees.push_root_mathematical_definition(definition);
            }
        }
        input = rest;
    }

    crate::declarations::imports::reject_packed_contract_imports(syntax_trees, &roots)?;
    if let Some(site) = qualifiers.packed_instruction() {
        crate::declarations::imports::import_packed_contracts(syntax_trees, &mut roots, site);
    }
    crate::declarations::imports::check_file_aliases(syntax_trees, &roots, tokens.source_text())?;
    crate::declarations::imports::mark_product_operand_imports(syntax_trees, &roots, &qualifiers);
    Ok(roots)
}

/// Transfer the streamed file's path and roots to its syntax record. The
/// caller can recycle its tokens as soon as this call returns.
pub fn parse_file(
    syntax_trees: &mut SyntaxTrees,
    lexed_source: &mut tokens::LexedSource,
) -> Result<ParsedSource, ParseError> {
    let path = std::mem::take(&mut lexed_source.path);
    let root_items = parse_into(syntax_trees, lexed_source.source_id, &lexed_source.tokens)
        .map_err(|mut error| {
            let position = SourcePosition::of(
                lexed_source.tokens.source_text(),
                error.source_span.span.start,
            );
            error.location = Some((path.clone(), position));
            error
        })?;
    Ok(ParsedSource {
        source_id: lexed_source.source_id,
        path,
        source: lexed_source.tokens.shared_source(),
        origin: lexed_source.origin,
        root_items: root_items.items,
        import_root_range: root_items.import_range,
        scope: lexed_source.scope,
    })
}

mod statements {
    pub(crate) mod destructuring;
    pub(crate) mod expressions;
    pub(crate) mod local_data;
    pub(crate) mod parse;
    pub(crate) mod sequence;
}
mod transitions {
    pub(crate) mod blocks;
    pub(crate) mod guards;
    pub(crate) mod targets;
}
mod contracts {
    pub(crate) mod bindings;
    pub(crate) mod clauses;
    pub(crate) mod conformance;
    pub(crate) mod effects;
    pub(crate) mod facts;
    pub(crate) mod signatures;
}
mod parameters {
    pub(crate) mod callable;
    #[cfg(test)]
    #[path = "../../tests/unit/generic_parameters.rs"]
    mod generic_parameters_tests;
    pub(crate) mod generics;
    pub(crate) mod relevance;
    pub(crate) mod requirements;
}
mod declarations {
    pub(crate) mod capabilities;
    pub(crate) mod conformances;
    pub(crate) mod constants;
    pub(crate) mod data;
    pub(crate) mod domains;
    pub(crate) mod imports;
    pub(crate) mod items;
    pub(crate) mod machine_bodies;
    pub(crate) mod machines;
    pub(crate) mod mathematical_bindings;
    pub(crate) mod measures;
    pub(crate) mod namespaces;
    pub(crate) mod operators;
    pub(crate) mod propositions;
    pub(crate) mod traits;
}
mod expressions {
    pub(crate) mod context;
    pub(crate) mod membership;
    pub(crate) mod operators;
    pub(crate) mod parse;
    pub(crate) mod postfix;
    pub(crate) mod primary;
    pub(crate) mod proof_facts;
    pub(crate) mod static_arguments;
    #[cfg(test)]
    #[path = "../../tests/unit/static_targets.rs"]
    mod static_targets_tests;
}
mod assembly {
    pub(crate) mod parse;
}
mod errors {
    pub(crate) mod expected;
    pub(crate) mod parse_error;
}
mod token_input {
    pub(crate) mod file_qualifiers;
    pub(crate) mod cursor;
    pub(crate) mod delimiters;
}
mod literals {
    pub(crate) mod numbers;
}
mod patterns {
    pub(crate) mod bindings;
    pub(crate) mod fields;
    pub(crate) mod positional;
}
mod types {
    pub(crate) mod properties;
    #[cfg(test)]
    #[path = "../../tests/unit/qualified_types.rs"]
    mod qualified_names;
    pub(crate) mod references;
    #[cfg(test)]
    #[path = "../../tests/unit/type_remainders.rs"]
    mod remainder_tests;
    #[cfg(test)]
    #[path = "../../tests/unit/static_argument_delimiters.rs"]
    mod static_argument_delimiters_tests;
}

#[cfg(test)]
#[path = "../tests/unit/mod.rs"]
mod tests;

#[cfg(test)]
#[path = "../tests/support/internal.rs"]
mod test_support;
#[cfg(test)]
use test_support::{parse_syntax_trees, parse_syntax_trees_with_id};

#[cfg(test)]
use crate as parsing;
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_files::LoadedSource;
#[cfg(test)]
#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;
