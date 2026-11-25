/*
We attach type and signature meaning to symbol-resolved Omega trees
(`lower_symbol_resolved_trees`), and extend retained typed bases across
generated-source continuations (`lower_seeded_extension` and
`lower_symbol_resolved_trees_to_seeded_base`).
*/

/*
Our typing route runs through two shared cohort operations in `lowerer.rs`
(`type_resolved_cohort` and `close_typed_cohort`). Before erasing resolved
membership nodes, `equality_and_exhaustiveness.rs` validates bare case
equality and match exhaustiveness. After typed data applications acquire their
substituted fields, `equality_preparation.rs` selects exact core comparisons.
`declarations.rs` then lowers every declaration family through
`contracts_and_signatures.rs`, `type_reference.rs`, `expression_lowering.rs`,
`statement_lowering.rs`, and `call_and_product_selections.rs`.
*/

/*
Stage 03 (`symbol-resolved-trees-to-typed-trees`) consumes `SymbolResolvedTrees`
from `syntax-trees-to-symbol-resolved-trees` (Stage 02) and produces
`TypedTrees` for `typed-trees-to-prepared-typed-trees` (Stage 04).
*/

#![forbid(unsafe_code)]

// Crate-wide aliases for sibling stages and the shared test fixtures.
#[cfg(test)]
use source_admission::LoadedSource;
use source_declarations::{declaration_selection, source_selections};
#[cfg(test)]
use source_lexer as lexing;
#[cfg(test)]
use source_parser as parsing;
use source_symbol_table::{source_inputs, source_symbols, source_tables};
use symbol_resolved_trees::source_resolved;
#[cfg(test)]
use syntax_trees_to_symbol_resolved_trees::resolution;
use typed_trees::source_state;
#[cfg(test)]
#[path = "../../../../../../../tests/support/frontend_operations.rs"]
mod frontend_fixture;

#[cfg(test)]
fn resolve_fixture(text: &str) -> source_resolved::SymbolResolvedTrees {
    try_resolve_fixture(text).expect("fixture declarations and bodies resolve")
}

#[cfg(test)]
fn try_resolve_fixture(
    text: &str,
) -> Result<source_resolved::SymbolResolvedTrees, Vec<diagnostics::Diagnostic>> {
    let mut files = source_inputs::SourceMap::default();
    let source = files.add("test/main.omg".into(), text.to_owned()).source_id;
    let tokens = frontend_fixture::Lexer::new(text).tokenize().unwrap();
    let syntax = frontend_fixture::parse_syntax_trees_with_id(source, &tokens).unwrap();
    resolution::resolve(resolution::ResolutionRequest {
        syntax,
        sources: Some(std::sync::Arc::new(files)),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
}

#[cfg(test)]
mod data_application_tests;

// An incomplete typing check refuses at the point where its required
// declaration, type, or occurrence cannot be established.
fn unavailable_judgment(construct: &str) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic::error(format!("typing could not complete the check ({construct})"))
}

mod atomic_calls;
mod call_and_product_selections;
mod constant_initializers;
mod constructor_applications;
mod contracts_and_signatures;
mod data_patterns;
mod declarations;
mod domain_cast_accounting;
mod equality_and_exhaustiveness;
mod equality_preparation;
mod expression_lowering;
mod expression_meanings;
mod lowerer;
mod operator_calls;
mod runtime_callables;
mod statement_lowering;
mod type_reference;
mod typing_queries;
mod undeclared_type_names;

#[cfg(test)]
mod ranking_defaults_tests;

#[cfg(test)]
mod qualified_operator_home_tests;

#[cfg(test)]
mod operand_owner_tests;

#[cfg(test)]
mod undeclared_type_name_tests;

#[cfg(test)]
mod bodyless_headers_tests;

#[cfg(test)]
mod applied_service_tests;

#[cfg(test)]
mod relation_family_tests;

pub(crate) use contracts_and_signatures::{contracts, signatures};

pub(crate) mod expressions {
    pub(crate) use crate::call_and_product_selections::{
        build_selections, call_results, capture_product_operands, construction_destinations,
        installed_interrupts, product_queries, product_selections, qualification_casts,
    };
    pub(crate) use crate::equality_and_exhaustiveness::{
        equality, exhaustiveness, proposition,
    };
    pub(crate) use crate::expression_lowering::expression;
    pub(crate) use crate::statement_lowering::statement;
}

mod access_borrow_factory;

pub(crate) mod tables {
    use crate::source_resolved::SymbolResolvedTrees;
    use crate::source_resolved::expression::ExpressionTable as ResolvedExpressions;
    use crate::source_resolved::statement::StatementTable as ResolvedStatements;
    use crate::source_resolved::types::{TypeConstraint, TypeReferenceTable};
    use crate::source_state::TypedTreeTables;
    use crate::source_state::expression::ExpressionTable;
    use crate::source_state::statement::StatementTable;
    use arena::Arena;

    pub(crate) fn expression_table(tables: &TypedTreeTables) -> &ExpressionTable {
        &tables.expression_table
    }

    pub(crate) fn expression_table_mut(tables: &mut TypedTreeTables) -> &mut ExpressionTable {
        &mut tables.expression_table
    }

    pub(crate) fn statement_table_mut(tables: &mut TypedTreeTables) -> &mut StatementTable {
        &mut tables.statement_table
    }

    pub(crate) fn resolved_expressions(source: &SymbolResolvedTrees) -> &ResolvedExpressions {
        &source.tables.bodies.expressions
    }

    pub(crate) fn resolved_statements(source: &SymbolResolvedTrees) -> &ResolvedStatements {
        &source.tables.bodies.statements
    }

    pub(crate) fn type_constraints(source: &SymbolResolvedTrees) -> &Arena<TypeConstraint> {
        &source.tables.types.constraints
    }

    pub(crate) fn type_references(source: &SymbolResolvedTrees) -> &TypeReferenceTable {
        &source.tables.types.references
    }
}

#[cfg(any(test, feature = "test-support"))]
pub use access_borrow_factory::{access_borrow_factory, access_borrow_factory_of};
#[cfg(any(test, feature = "test-support"))]
pub use lowerer::lower_symbol_resolved_trees;
#[cfg(any(test, feature = "test-support"))]
pub use lowerer::seeded_continuation::{
    SeededContinuationError, SeededTypingBase, lower_seeded_extension,
    lower_symbol_resolved_trees_to_seeded_base,
};
#[cfg(not(any(test, feature = "test-support")))]
pub use lowerer::seeded_continuation::{
    SeededTypingBase, lower_symbol_resolved_trees_to_seeded_base,
};
