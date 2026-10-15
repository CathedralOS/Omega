use arena::HandleSpan;
use symbols::{SymbolHandle, SymbolTable};

use super::references::{
    assign_call_symbol, assign_member_symbol, assign_membership_symbol, assign_name_symbol,
    assign_struct_literal_symbols,
};
use crate::symbols::scope::MachineScope;

/// Public entry into a walk: reborrow the shared tables and scope into the
/// visit context, then run the named method on the carried value.
macro_rules! context_wrapped {
    ($name:ident ($parameter:ident : $ty:ty) => $method:ident) => {
        pub(in crate::symbols) fn $name(
            symbols: &SymbolTable,
            machine: &MachineScope<'_>,
            parameters: &[symbol_resolved_trees::signature::StateParameter],
            state_symbol: SymbolHandle,
            expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
            child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
            $parameter: $ty,
        ) {
            ExpressionSymbolContext {
                symbols,
                machine,
                parameters,
                state_symbol,
                expression_table,
                child_type_references,
            }
            .$method($parameter);
        }
    };
}

context_wrapped! {
    assign_static_argument_type_symbols(
        argument: &symbol_resolved_trees::expression::StaticMachineArgument
    ) => assign_static_argument_type_symbols
}

/// Assign static-argument type symbols for every machine argument the call
/// carries.
#[allow(clippy::too_many_arguments)]
pub(in crate::symbols) fn assign_static_arguments_type_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    machine_arguments: &[symbol_resolved_trees::expression::StaticMachineArgument],
) {
    for argument in machine_arguments {
        assign_static_argument_type_symbols(
            symbols,
            machine,
            parameters,
            state_symbol,
            expression_table,
            child_type_references,
            argument,
        );
    }
}

pub(in crate::symbols) fn assign_statement_expression_symbols(
    symbols: &SymbolTable,
    machine: &MachineScope<'_>,
    parameters: &[symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &mut symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
    expression: symbol_resolved_trees::expression::ExpressionHandle,
) {
    if !expression.is_valid() {
        return;
    }

    assign_expression_table_symbols(
        symbols,
        machine,
        parameters,
        state_symbol,
        expression_table,
        child_type_references,
        expression,
    );
}

context_wrapped! {
    assign_expression_span_symbols(
        expressions: HandleSpan<symbol_resolved_trees::expression::ExpressionHandle>
    ) => assign_expression_span_symbols
}

context_wrapped! {
    assign_expression_table_symbols(
        expression: symbol_resolved_trees::expression::ExpressionHandle
    ) => assign_expression_table_symbols
}

// Each recursive visit reborrows the same tables and lexical scope. Keep the
// indexed span reads and independent type-reference checks at their visit sites:
// earlier recursive visits may mutate the tables that later visits read.
struct ExpressionSymbolContext<'visit, 'scope> {
    symbols: &'visit SymbolTable,
    machine: &'visit MachineScope<'scope>,
    parameters: &'visit [symbol_resolved_trees::signature::StateParameter],
    state_symbol: SymbolHandle,
    expression_table: &'visit mut symbol_resolved_trees::expression::ExpressionTable,
    child_type_references: &'visit mut arena::Arena<symbol_resolved_trees::types::TypeReference>,
}

impl ExpressionSymbolContext<'_, '_> {
    /// Reassigns one type reference with this context's symbol table, machine
    /// scope, parameters, state symbol, and child arenas — every walk's call.
    fn reassign_type_reference(
        &mut self,
        type_reference: arena::Handle<symbol_resolved_trees::types::TypeReference>,
    ) {
        crate::symbols::type_references::reassign_type_reference_symbols_and_values(
            self.symbols,
            self.machine,
            self.parameters,
            self.state_symbol,
            self.expression_table,
            self.child_type_references,
            type_reference,
        );
    }

    fn assign_static_argument_type_symbols(
        &mut self,
        argument: &symbol_resolved_trees::expression::StaticMachineArgument,
    ) {
        if argument.type_reference.is_valid() {
            self.reassign_type_reference(argument.type_reference);
        }
        if let Some(application) = &argument.application {
            for nested in &application.arguments {
                self.assign_static_argument_type_symbols(nested);
            }
        }
    }

    fn assign_expression_span_symbols(
        &mut self,
        expressions: HandleSpan<symbol_resolved_trees::expression::ExpressionHandle>,
    ) {
        let count = expressions.count();
        for offset in 0..count {
            let expression = self.expression_table.expression_handles(expressions)[offset as usize];
            self.assign_expression_table_symbols(expression);
        }
    }

    fn assign_expression_table_symbols(
        &mut self,
        expression: symbol_resolved_trees::expression::ExpressionHandle,
    ) {
        if !expression.is_valid() {
            return;
        }

        match self.expression_table.expression(expression).clone() {
            symbol_resolved_trees::expression::ExpressionNode::Match(dispatch) => {
                self.assign_expression_table_symbols(dispatch.subject);
                for offset in 0..dispatch.arms.count() {
                    let arm = self.expression_table.match_arms(dispatch.arms)[offset as usize];
                    if let symbol_resolved_trees::expression::MatchPattern::Value(pattern) =
                        arm.pattern
                    {
                        self.assign_expression_table_symbols(pattern);
                    }
                    self.assign_expression_table_symbols(arm.value);
                }
            }
            symbol_resolved_trees::expression::ExpressionNode::Atomic(atomic) => {
                // The instruction-observed prior has its own destination expression,
                // distinct from references inside the arithmetic-shaped carrier.
                // Resolve it in the same state scope so downstream consumers bind
                // the declared local by identity, never by retained spelling.
                self.assign_expression_table_symbols(atomic.result);
                self.assign_expression_table_symbols(atomic.value);
            }
            symbol_resolved_trees::expression::ExpressionNode::ArrayLiteral(values) => {
                self.assign_expression_span_symbols(values);
            }
            symbol_resolved_trees::expression::ExpressionNode::Binary(binary) => {
                self.assign_expression_table_symbols(binary.left);
                self.assign_expression_table_symbols(binary.right);
            }
            symbol_resolved_trees::expression::ExpressionNode::Boolean(_)
            | symbol_resolved_trees::expression::ExpressionNode::Float(_)
            | symbol_resolved_trees::expression::ExpressionNode::UndeterminedNaN(_)
            | symbol_resolved_trees::expression::ExpressionNode::Integer(_)
            | symbol_resolved_trees::expression::ExpressionNode::String(_) => {}
            symbol_resolved_trees::expression::ExpressionNode::Cast(cast) => {
                self.assign_expression_table_symbols(cast.value);
                self.reassign_type_reference(cast.target_type);
                crate::symbols::type_references::each_type_reference_handle(
                    cast.semantic_domain_arguments,
                    |handle| {
                        self.reassign_type_reference(handle);
                    },
                );
            }
            symbol_resolved_trees::expression::ExpressionNode::Call(call) => {
                for argument in &call.machine_arguments {
                    self.assign_static_argument_type_symbols(argument);
                }
                if call.requirement_application.is_valid() {
                    self.reassign_type_reference(call.requirement_application);
                }
                if call.receiver.is_valid() {
                    self.assign_expression_table_symbols(call.receiver);
                }
                self.assign_expression_span_symbols(call.arguments);
                assign_call_symbol(
                    self.symbols,
                    self.machine,
                    self.parameters,
                    self.state_symbol,
                    self.expression_table,
                    self.child_type_references,
                    call.receiver,
                    &call,
                    expression,
                );
            }
            symbol_resolved_trees::expression::ExpressionNode::Indexed(indexed) => {
                self.assign_expression_table_symbols(indexed.collection);
                self.assign_expression_table_symbols(indexed.index);
            }
            symbol_resolved_trees::expression::ExpressionNode::Range(range) => {
                if range.start.is_valid() {
                    self.assign_expression_table_symbols(range.start);
                }
                if range.end.is_valid() {
                    self.assign_expression_table_symbols(range.end);
                }
            }
            symbol_resolved_trees::expression::ExpressionNode::Member(member) => {
                let receiver = member.receiver;
                let member_name = member.member.clone();
                self.assign_expression_table_symbols(receiver);
                assign_member_symbol(
                    self.symbols,
                    self.machine,
                    self.state_symbol,
                    self.expression_table,
                    receiver,
                    &member_name,
                    expression,
                );
            }
            symbol_resolved_trees::expression::ExpressionNode::Membership(membership) => {
                self.assign_expression_table_symbols(membership.value);
                assign_membership_symbol(
                    self.symbols,
                    self.expression_table,
                    membership.domain,
                    expression,
                );
            }
            symbol_resolved_trees::expression::ExpressionNode::Borrow(inner) => {
                self.assign_expression_table_symbols(inner.target);
            }
            symbol_resolved_trees::expression::ExpressionNode::Unary(unary) => {
                self.assign_expression_table_symbols(unary.operand);
            }
            symbol_resolved_trees::expression::ExpressionNode::Name(path) => {
                assign_name_symbol(
                    self.symbols,
                    self.machine,
                    self.parameters,
                    self.state_symbol,
                    self.expression_table,
                    &path,
                    expression,
                );
            }
            symbol_resolved_trees::expression::ExpressionNode::StructLiteral(struct_literal) => {
                let count = struct_literal.fields.count();
                for offset in 0..count {
                    let field = &self.expression_table.struct_fields(struct_literal.fields)
                        [offset as usize];
                    self.assign_expression_table_symbols(field.value);
                }
                assign_struct_literal_symbols(self.symbols, self.expression_table, expression);
            }
            symbol_resolved_trees::expression::ExpressionNode::ZeroValue(type_reference) => {
                self.reassign_type_reference(type_reference);
            }
        }
    }
}
