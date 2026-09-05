//! Value operand visibility shared by proof assertions and arrival contracts.
//! This traversal checks scope only; calls are not executed or granted facts.

use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
};

pub(crate) struct StateValueScope<'program, 'symbols> {
    pub(crate) program: &'program TypedTrees,
    pub(crate) machine: &'program Machine,
    pub(crate) state: &'program State,
    pub(crate) machine_symbols: &'symbols MachineSymbols<'program>,
    pub(crate) symbols: &'symbols TopLevelSymbols<'program>,
    pub(crate) prior_statements: &'symbols [psi_typed_trees::statement::StatementNode],
    pub(crate) context: &'static str,
}

impl StateValueScope<'_, '_> {
    pub(crate) fn expression(
        &self,
        expression: ExpressionHandle,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        if !expression.is_valid() {
            return;
        }
        let table = &self.program.expression_table;
        match table.expression(expression) {
            ExpressionNode::Name(path) => {
                let members = table.name_path_members(path.members);
                let Some(name) = members.first() else {
                    return;
                };
                let root = if path.head_symbol.is_valid() {
                    path.head_symbol
                } else {
                    table
                        .name_path_member_symbols(path.member_symbols)
                        .first()
                        .copied()
                        .filter(|symbol| symbol.is_valid())
                        .unwrap_or(path.symbol)
                };
                let bare_identity_matches = members.len() != 1
                    || !path.head_symbol.is_valid()
                    || path.head_symbol == path.symbol;
                if !bare_identity_matches
                    || !crate::locals::state_value_root_is_known(
                        self.program,
                        self.machine,
                        self.state,
                        self.prior_statements,
                        self.machine_symbols,
                        self.symbols,
                        root,
                        name.as_str(),
                    )
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` {} value `{}` is not in the state's explicit parameter scope or prior local declarations",
                        self.machine.name, self.state.name, self.context, name,
                    )));
                }
            }
            ExpressionNode::Binary(binary) => {
                self.expression(binary.left, diagnostics);
                self.expression(binary.right, diagnostics);
            }
            ExpressionNode::Unary(unary) => self.expression(unary.operand, diagnostics),
            ExpressionNode::Borrow(borrow) => self.expression(borrow.target, diagnostics),
            ExpressionNode::Cast(cast) => {
                self.expression(cast.value, diagnostics);
                self.type_reference(cast.target_type, diagnostics);
            }
            ExpressionNode::Member(member) => self.expression(member.receiver, diagnostics),
            ExpressionNode::Indexed(indexed) => {
                self.expression(indexed.collection, diagnostics);
                self.expression(indexed.index, diagnostics);
            }
            ExpressionNode::Range(range) => {
                self.expression(range.start, diagnostics);
                self.expression(range.end, diagnostics);
            }
            ExpressionNode::Atomic(atomic) => {
                self.expression(atomic.value, diagnostics);
                self.expression(atomic.result, diagnostics);
            }
            ExpressionNode::Call(call) => {
                if !crate::locals::expression_call_has_operator_namespace(self.program, call) {
                    self.expression(call.receiver, diagnostics);
                }
                for argument in table.expression_handles(call.arguments) {
                    self.expression(*argument, diagnostics);
                }
            }
            ExpressionNode::ArrayLiteral(elements) => {
                for element in table.expression_handles(*elements) {
                    self.expression(*element, diagnostics);
                }
            }
            ExpressionNode::StructLiteral(literal) => {
                for field in table.struct_fields(literal.fields) {
                    self.expression(field.value, diagnostics);
                }
            }
            ExpressionNode::Boolean(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::String(_) => {}
            ExpressionNode::ZeroValue(reference) => self.type_reference(*reference, diagnostics),
        }
    }
}
