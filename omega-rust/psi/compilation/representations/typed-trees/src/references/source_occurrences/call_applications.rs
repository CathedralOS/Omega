//! Retain a call's source telescope before logical or executable substitution.
//! A description owns occurrences just as a body does. This records lexical
//! custody only; closing a selection and admitting its operands remain later
//! operations. Derived sidecars cannot create independent authored inputs.

use crate::{TypedTrees, expression::ExpressionNode};
use symbols::SymbolHandle;

impl TypedTrees {
    /// Keep source scopes before signature copies, folding or specialization
    /// consume their original binders. Even an empty invocation needs a record
    /// when its qualifier supplies an independent implementation selection.
    pub fn retain_static_argument_contexts(&mut self) {
        use crate::expression::ExpressionHandle;
        use crate::references::{TypedReferences, visit_program};
        use crate::statement::StatementNode;

        #[derive(Default)]
        struct SourceCalls {
            owner: SymbolHandle,
            outer: Vec<SymbolHandle>,
            visited: std::collections::HashSet<(SymbolHandle, ExpressionHandle)>,
            expressions: Vec<(ExpressionHandle, SymbolHandle)>,
        }
        impl Extend<SymbolHandle> for SourceCalls {
            fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
        }
        impl TypedReferences for SourceCalls {
            fn derived_declaration_references(&self) -> bool {
                false
            }
            fn declaration(&mut self, owner: SymbolHandle) -> bool {
                self.owner = owner;
                self.outer.clear();
                true
            }
            fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
                self.outer.push(self.owner);
                if owner.is_valid() {
                    self.owner = owner;
                }
            }
            fn leave_lexical_scope(&mut self) {
                if let Some(owner) = self.outer.pop() {
                    self.owner = owner;
                }
            }
            fn expression(&mut self, expression: ExpressionHandle) -> bool {
                if !self.visited.insert((self.owner, expression)) {
                    return false;
                }
                self.expressions.push((expression, self.owner));
                true
            }
        }
        let mut roots = SourceCalls::default();
        visit_program(self, &mut roots);
        for (expression, owner) in roots.expressions {
            let Some(call) = self.expression_table.call_node(expression) else {
                continue;
            };
            if call.authored_machine_application.is_valid() {
                let application = call.authored_machine_application;
                let retained = self
                    .expression_table
                    .static_machine_application_mut(application);
                if !retained.lexical_owner.is_valid() {
                    retained.lexical_owner = owner;
                }
                continue;
            }
            let namespace = self
                .expression_table
                .name_path_node(call.receiver)
                .map_or_else(SymbolHandle::invalid, |path| path.symbol);
            if call.machine_arguments.is_empty()
                && !has_region_telescope(self, call.target_symbol)
                && !implementation_namespace(self, namespace)
            {
                continue;
            }
            let call = call.clone();
            let application = self.expression_table.retain_static_machine_application(
                call.target_symbol,
                call.target,
                &call.machine_arguments,
            );
            self.expression_table
                .static_machine_application_mut(application)
                .lexical_owner = owner;
            let ExpressionNode::Call(call) = self.expression_table.expression_mut(expression)
            else {
                unreachable!("retaining a source application does not replace its call");
            };
            call.authored_machine_application = application;
        }
        let statement_calls = self
            .machines()
            .iter()
            .flat_map(|machine| {
                self.machine_states(machine).iter().flat_map(|state| {
                    self.statement_table
                        .iter_statements(state.statement_nodes)
                        .filter_map(|(handle, statement)| {
                            let call = statement.as_call()?;
                            ((!call.machine_arguments.is_empty()
                                || has_region_telescope(self, call.target_symbol)
                                || implementation_namespace(self, call.receiver_symbol))
                                && !call.authored_machine_application.is_valid())
                            .then(|| (handle, machine.symbol, call.clone()))
                        })
                })
            })
            .collect::<Vec<_>>();
        for (handle, owner, call) in statement_calls {
            let application = self.expression_table.retain_static_machine_application(
                call.target_symbol,
                call.target,
                &call.machine_arguments,
            );
            self.expression_table
                .static_machine_application_mut(application)
                .lexical_owner = owner;
            let StatementNode::Call(call) = self.statement_table.statement_mut(handle) else {
                unreachable!("retaining a source application does not replace its call");
            };
            call.authored_machine_application = application;
        }
    }
}

fn implementation_namespace(program: &TypedTrees, symbol: SymbolHandle) -> bool {
    symbol.is_valid()
        && (program.conformance_by_symbol(symbol).is_some()
            || program.symbols.get(symbol).kind == symbols::SymbolKind::ConformanceParameter)
}

fn has_region_telescope(program: &TypedTrees, target: SymbolHandle) -> bool {
    program
        .machine_by_symbol(target)
        .or_else(|| program.machine_holding_state(target))
        .is_some_and(|machine| !machine.lifetime_parameters.is_empty())
}
