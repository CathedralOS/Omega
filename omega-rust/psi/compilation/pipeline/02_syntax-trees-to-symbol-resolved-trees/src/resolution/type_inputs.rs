//! Probe input ownership captured at root production, before symbols exist.
//!
//! Allocation ranges bind a syntax use to this frontend's declarations after
//! indexing. No source spelling, span containment, or another frontend's symbol
//! ordinal participates. The frontier transfers these temporary dependencies to
//! its typed probe consumer; ordinary resolved trees retain no probe metadata.

use super::lowerer::RootWatermarks;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolHandle;
use syntax_trees::types::TypeReferenceHandle;

pub(crate) struct LoweredTypeInputs {
    pub(crate) start: RootWatermarks,
    pub(crate) end: RootWatermarks,
    pub(crate) arguments: Vec<TypeReferenceHandle>,
    pub(crate) owner: SymbolHandle,
}

/// The ordinary type lowerer records actual input uses. Both declaration
/// production and deferred body completion close the same ownership scope.
pub(crate) struct TypeInputScope {
    roots: RootWatermarks,
    argument_frontier: usize,
}

impl TypeInputScope {
    pub(crate) fn new(lowerer: &super::lowerer::Lowerer) -> Self {
        Self {
            roots: RootWatermarks::capture(&lowerer.symbol_resolved_trees),
            argument_frontier: lowerer.encountered_pending_type_arguments.len(),
        }
    }

    pub(crate) fn finish(self, lowerer: &mut super::lowerer::Lowerer, owner: SymbolHandle) {
        let mut arguments =
            lowerer.encountered_pending_type_arguments[self.argument_frontier..].to_vec();
        arguments.sort_unstable_by_key(|argument| (argument.arena_index(), argument.generation()));
        arguments.dedup();
        if !arguments.is_empty() {
            lowerer.pending_type_inputs.push(LoweredTypeInputs {
                start: self.roots,
                end: RootWatermarks::capture(&lowerer.symbol_resolved_trees),
                arguments,
                owner,
            });
        }
    }
}

pub(crate) fn bind(
    trees: &SymbolResolvedTrees,
    inputs: &[LoweredTypeInputs],
) -> Result<Vec<(SymbolHandle, TypeReferenceHandle)>, Vec<diagnostics::Diagnostic>> {
    let mut result = Vec::new();
    for input in inputs {
        let start = result.len();
        if input.owner.is_valid() {
            result.extend(
                input
                    .arguments
                    .iter()
                    .map(|argument| (input.owner, *argument)),
            );
            continue;
        }
        macro_rules! bind_roots {
            ($($field:ident),* $(,)?) => {$({
                for ordinal in input.start.$field..input.end.$field {
                    let symbol = trees.$field[ordinal].symbol;
                    if !symbol.is_valid() {
                        return Err(vec![diagnostics::Diagnostic::error("pending type input has an unbound declaration owner")]);
                    }
                    result.extend(input.arguments.iter().map(|argument| (symbol, *argument)));
                }
            })*};
        }
        bind_roots!(
            const_declarations,
            data_definitions,
            domain_definitions,
            machines,
            operators,
            measures,
            propositions,
            mathematical_definitions,
            traits,
            conformances
        );
        if result.len() == start {
            return Err(vec![diagnostics::Diagnostic::error(
                "pending type input lost its produced declaration owner",
            )]);
        }
    }
    Ok(result)
}
