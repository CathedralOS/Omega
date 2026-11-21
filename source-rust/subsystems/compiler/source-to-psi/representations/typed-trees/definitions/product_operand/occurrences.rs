use crate::declaration_selection::ProductDeclarationRequest;
use crate::source_state::{
    TypedTrees,
    expression::{BuildSelectionRequest, ExpressionHandle},
    machine::Machine,
    references::{StaticArgumentCall, TypedReferences},
};
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
    let mut occurrences = Vec::new();
    visit_build_selection_occurrences(
        program,
        program.machines().iter(),
        |machine, state, expression, request| {
            if let BuildSelectionRequest::Declaration(request) = request {
                occurrences.push(ProductSelectionOccurrence {
                    machine,
                    state,
                    expression,
                    request: Arc::clone(request),
                });
            }
        },
    );
    occurrences
}

/// Visit the typed Build selections within the requested machines. Each call
/// carries its author and expression so admission can rejoin the executed selection.
pub fn visit_build_selection_occurrences<'a>(
    program: &TypedTrees,
    machines: impl IntoIterator<Item = &'a Machine>,
    visit: impl FnMut(SymbolHandle, SymbolHandle, ExpressionHandle, &BuildSelectionRequest),
) {
    let mut census = Census {
        machine: SymbolHandle::invalid(),
        state: SymbolHandle::invalid(),
        seen: Vec::new(),
        visit,
    };
    for machine in machines {
        census.machine = machine.symbol;
        for state in program.machine_states(machine) {
            census.state = state.symbol;
            census.seen.clear();
            for statement in program.statement_table.statements(state.statement_nodes) {
                for (_, root) in program.tables.expression_roots(statement) {
                    crate::source_state::references::visit_expression(program, &mut census, root);
                }
            }
        }
    }
}

struct Census<Visit> {
    machine: SymbolHandle,
    state: SymbolHandle,
    seen: Vec<Vec<ExpressionHandle>>,
    visit: Visit,
}
impl<Visit> Extend<SymbolHandle> for Census<Visit> {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, _: Symbols) {}
}
impl<Visit> TypedReferences for Census<Visit>
where
    Visit: FnMut(SymbolHandle, SymbolHandle, ExpressionHandle, &BuildSelectionRequest),
{
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
        if let StaticArgumentCall::Expression(expression, call) = call
            && let Some(request) = program
                .expression_table
                .build_selection_request(call.build_selection)
        {
            (self.visit)(self.machine, self.state, expression, request);
        }
        true
    }
}
