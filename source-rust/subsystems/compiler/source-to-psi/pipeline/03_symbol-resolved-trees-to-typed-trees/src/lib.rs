/*
We attach type and signature meaning to symbol-resolved Omega trees
(`lower_symbol_resolved_trees`), and extend retained typed bases across
generated-source continuations (`lower_seeded_extension` and
`lower_symbol_resolved_trees_to_seeded_base`).
*/

/*
Our typing route runs through two shared cohort operations in `lowerer.rs`
(`type_resolved_cohort` and `close_typed_cohort`). Before erasing resolved
membership nodes, `patterns/equality_and_exhaustiveness.rs` validates bare case
equality and match exhaustiveness. After typed data applications acquire their
substituted fields, `expressions/equality_preparation.rs` selects exact core
comparisons. `declarations/items.rs` then lowers every declaration family through
`declarations/contracts_and_signatures.rs`, `types/type_reference.rs`,
`expressions/expression_lowering.rs`, `statements/statement_lowering.rs`, and
`expressions/call_and_product_selections.rs`.
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
#[path = "../tests/unit/data_application_tests.rs"]
mod data_application_tests;

// An incomplete typing check refuses at the point where its required
// declaration, type, or occurrence cannot be established.
fn unavailable_judgment(construct: &str) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic::error(format!("typing could not complete the check ({construct})"))
}

mod declarations {
    pub(crate) mod constant_initializers;
    pub(crate) mod constructor_applications;
    pub(crate) mod contracts_and_signatures;
    pub(crate) mod items;
}

mod patterns {
    pub(crate) mod data_patterns;
    pub(crate) mod equality_and_exhaustiveness;
}

mod statements {
    pub(crate) mod statement_lowering;
}

mod types {
    pub(crate) mod access_borrow_factory;
    pub(crate) mod domain_cast_accounting;
    pub(crate) mod type_reference;
    pub(crate) mod typing_queries;
    pub(crate) mod undeclared_type_names;
}

mod lowerer;

#[cfg(test)]
#[path = "../tests/unit/ranking_defaults_tests.rs"]
mod ranking_defaults_tests;

#[cfg(test)]
#[path = "../tests/unit/qualified_operator_home_tests.rs"]
mod qualified_operator_home_tests;

#[cfg(test)]
#[path = "../tests/unit/operand_owner_tests.rs"]
mod operand_owner_tests;

#[cfg(test)]
#[path = "../tests/unit/undeclared_type_name_tests.rs"]
mod undeclared_type_name_tests;

#[cfg(test)]
#[path = "../tests/unit/installed_interrupt_tests.rs"]
mod installed_interrupt_tests;

#[cfg(test)]
#[path = "../tests/unit/bodyless_headers_tests.rs"]
mod bodyless_headers_tests;

#[cfg(test)]
#[path = "../tests/unit/applied_service_tests.rs"]
mod applied_service_tests;

#[cfg(test)]
#[path = "../tests/unit/relation_family_tests.rs"]
mod relation_family_tests;

#[cfg(test)]
#[path = "../tests/unit/cohort_equivalence_tests.rs"]
mod cohort_equivalence_tests;

pub(crate) use declarations::contracts_and_signatures::{contracts, signatures};

pub(crate) mod expressions {
    pub(crate) mod atomic_calls;
    pub(crate) mod call_and_product_selections;
    pub(crate) mod equality_preparation;
    pub(crate) mod expression_lowering;
    pub(crate) mod expression_meanings;
    pub(crate) mod operand_owners;
    pub(crate) mod operator_calls;
    pub(crate) mod runtime_callables;

    pub(crate) use crate::expressions::call_and_product_selections::{
        build_selections, call_results, capture_product_operands, construction_destinations,
        installed_interrupts, product_queries, product_selections, qualification_casts,
    };
    pub(crate) use crate::expressions::expression_lowering::expression;
    pub(crate) use crate::patterns::equality_and_exhaustiveness::{
        equality, exhaustiveness, proposition,
    };
    pub(crate) use crate::statements::statement_lowering::statement;
}

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
pub use lowerer::lower_symbol_resolved_trees;
pub use lowerer::seeded_continuation::{
    SeededTypingBase, lower_symbol_resolved_trees_to_seeded_base,
};
