//! Exact, bounded custody of the expression/type graph a source edit replaces.

mod bindings;
mod building;
mod expressions;
mod static_arguments;
#[cfg(test)]
mod tests;
mod types;

use diagnostics::Diagnostic;
use std::collections::BTreeSet;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, StaticMachineArgument};
use typed_trees::types::TypeReferenceHandle;

const MAX_NODES: usize = 65_536;
pub(super) use static_arguments::{validate_call_static_arguments, validate_static_arguments};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GraphGuard {
    expression_roots: Vec<ExpressionHandle>,
    type_roots: Vec<TypeReferenceHandle>,
    symbol_roots: Vec<SymbolHandle>,
    static_roots: Vec<StaticMachineArgument>,
    expressions: Vec<expressions::Snapshot>,
    types: Vec<types::Snapshot>,
    bindings: Vec<bindings::Snapshot>,
}

impl GraphGuard {
    pub(super) fn capture(
        program: &TypedTrees,
        expressions: &[ExpressionHandle],
        types: &[TypeReferenceHandle],
        symbols: &[SymbolHandle],
        static_roots: &[StaticMachineArgument],
    ) -> Result<Self, Vec<Diagnostic>> {
        if expressions
            .len()
            .saturating_add(types.len())
            .saturating_add(symbols.len())
            .saturating_add(static_roots.len())
            > MAX_NODES
        {
            return Err(rejected(
                "source-edit root set exceeds its finite node budget",
            ));
        }
        let mut builder = Builder {
            program,
            pending: Vec::new(),
            seen_expressions: BTreeSet::new(),
            seen_types: BTreeSet::new(),
            seen_symbols: BTreeSet::new(),
            elements: 0,
            result: Self {
                expression_roots: expressions.to_vec(),
                type_roots: types.to_vec(),
                symbol_roots: symbols.to_vec(),
                static_roots: Vec::new(),
                expressions: Vec::new(),
                types: Vec::new(),
                bindings: Vec::new(),
            },
        };
        static_arguments::capture(&mut builder, static_roots)?;
        builder.result.static_roots = static_roots.to_vec();
        for root in expressions {
            builder.expression(*root)?;
        }
        for root in types {
            builder.type_reference(*root)?;
        }
        for root in symbols {
            builder.symbol(*root)?;
        }
        while let Some(pending) = builder.pending.pop() {
            match pending {
                Pending::Expression(handle) => expressions::capture(&mut builder, handle)?,
                Pending::Type(handle) => types::capture(&mut builder, handle)?,
                Pending::Symbol(handle) => bindings::capture(&mut builder, handle)?,
            }
        }
        Ok(builder.result)
    }

    pub(super) fn validate(&self, program: &TypedTrees) -> Result<(), Vec<Diagnostic>> {
        let current = Self::capture(
            program,
            &self.expression_roots,
            &self.type_roots,
            &self.symbol_roots,
            &self.static_roots,
        )?;
        if self == &current {
            Ok(())
        } else {
            Err(rejected(
                "settled expression, operand binding, or signature type changed",
            ))
        }
    }
}

enum Pending {
    Expression(ExpressionHandle),
    Type(TypeReferenceHandle),
    Symbol(SymbolHandle),
}

struct Builder<'program> {
    program: &'program TypedTrees,
    pending: Vec<Pending>,
    // Graph reachability is sparse in the program arenas; ordered sets bound
    // repeated/cyclic visits without cloning or indexing the whole program.
    seen_expressions: BTreeSet<(u32, u32)>,
    seen_types: BTreeSet<(u32, u32)>,
    seen_symbols: BTreeSet<(u32, u32)>,
    elements: usize,
    result: GraphGuard,
}

fn rejected(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "selected dispatch source custody rejects {message}"
    ))]
}
