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
equality, synthesized `Equatable` conformances, and match exhaustiveness.
`declarations.rs` then lowers every declaration family through
`contracts_and_signatures.rs`, `type_reference.rs`, `expression_lowering.rs`,
`statement_lowering.rs`, and `call_and_product_selections.rs`.
*/

/*
Stage 03 (`symbol-resolved-trees-to-typed-trees`) consumes `SymbolResolvedTrees`
from `syntax-trees-to-symbol-resolved-trees` (Stage 02) and produces
`TypedTrees` for `typed-trees-to-typed-trees` (Stage 04).
*/

#![forbid(unsafe_code)]

// Typing retains declarations and their exact identities. Any operation that
// still needs a retired checker judgment refuses at the requested construct;
// it does not imply that lowering or a later consumer has proved it.
fn unavailable_judgment(construct: &str) -> diagnostics::Diagnostic {
    diagnostics::Diagnostic::error(format!(
        "no product route yet: {construct} requires a lowered-form judgment"
    ))
}

mod call_and_product_selections;
mod constant_initializers;
mod constructor_applications;
mod contracts_and_signatures;
mod declarations;
mod domain_cast_accounting;
mod equality_and_exhaustiveness;
mod expression_lowering;
mod lowerer;
mod statement_lowering;
mod type_reference;
mod typing_queries;

#[cfg(test)]
mod ranking_defaults_tests;

pub(crate) use contracts_and_signatures::{contracts, signatures};

pub(crate) mod expressions {
    pub(crate) use crate::call_and_product_selections::{
        build_selections, call_results, capture_product_operands, construction_destinations,
        installed_interrupts, product_queries, product_selections, qualification_casts,
    };
    pub(crate) use crate::equality_and_exhaustiveness::{
        equality, equatable, exhaustiveness, proposition,
    };
    pub(crate) use crate::expression_lowering::expression;
    pub(crate) use crate::statement_lowering::statement;
}

mod access_borrow_factory;

pub(crate) mod tables {
    use arena::Arena;
    use symbol_resolved_trees::SymbolResolvedTrees;
    use symbol_resolved_trees::expression::ExpressionTable as ResolvedExpressions;
    use symbol_resolved_trees::statement::StatementTable as ResolvedStatements;
    use symbol_resolved_trees::types::{TypeConstraint, TypeReferenceTable};
    use typed_trees::TypedTreeTables;
    use typed_trees::expression::ExpressionTable;
    use typed_trees::statement::StatementTable;

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

pub use access_borrow_factory::{access_borrow_factory, access_borrow_factory_of};
pub use lowerer::lower_symbol_resolved_trees;
pub use lowerer::seeded_continuation::{
    SeededContinuationError, SeededTypingBase, lower_seeded_extension,
    lower_symbol_resolved_trees_to_seeded_base,
};
