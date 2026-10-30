//! Designated requests reached from actual authored body edges. An uncalled
//! body still has formation obligations; discarded arena calls are not roots.
use crate::{
    TypedTrees,
    expression::{BuildSelectionRequest, ExpressionHandle},
    references::{StaticArgumentCall, TypedReferences},
};
use language_semantics::declaration_selection::ProductDeclarationRequest;
use std::sync::Arc;
use symbols::SymbolHandle;

#[derive(Clone)]
pub struct ProductSelectionOccurrence {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub expression: ExpressionHandle,
    pub request: Arc<ProductDeclarationRequest>,
}

pub fn collect_product_selection_occurrences(
    program: &TypedTrees,
) -> Vec<ProductSelectionOccurrence> {
    let mut census = Census {
        machine: SymbolHandle::invalid(),
        state: SymbolHandle::invalid(),
        seen: Vec::new(),
        occurrences: Vec::new(),
    };
    for machine in program.machines() {
        census.machine = machine.symbol;
        for state in program.machine_states(machine) {
            census.state = state.symbol;
            census.seen.clear();
            for statement in program.statement_table.statements(state.statement_nodes) {
                for (_, root) in program.tables.expression_roots(statement) {
                    crate::references::visit_expression(program, &mut census, root);
                }
            }
        }
    }
    census.occurrences
}

struct Census {
    machine: SymbolHandle,
    state: SymbolHandle,
    seen: Vec<Vec<ExpressionHandle>>,
    occurrences: Vec<ProductSelectionOccurrence>,
}
impl Extend<SymbolHandle> for Census {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, _: Symbols) {}
}
impl TypedReferences for Census {
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        let index = expression.arena_index() as usize;
        if self.seen.len() <= index {
            self.seen.resize_with(index + 1, Vec::new);
        }
        if self.seen[index].contains(&expression) {
            return false;
        }
        self.seen[index].push(expression);
        true
    }
    fn call_arguments(&mut self, program: &TypedTrees, call: StaticArgumentCall<'_>) -> bool {
        if let StaticArgumentCall::Expression(expression, call) = call {
            if let Some(BuildSelectionRequest::Declaration(request)) = program
                .expression_table
                .build_selection_request(call.build_selection)
            {
                self.occurrences.push(ProductSelectionOccurrence {
                    machine: self.machine,
                    state: self.state,
                    expression,
                    request: Arc::clone(request),
                });
            }
        }
        true
    }
}
