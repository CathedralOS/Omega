//! Arrival contracts observe the target state's explicit value frontier.
//! Declaration names in domain/proposition/type positions are not value reads.

use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_diagnostics::Diagnostic;
use psi_typed_trees::{
    TypedTrees,
    domain::ProofFact,
    expression::{ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
};

pub(super) fn validate(
    program: &TypedTrees,
    machine: &Machine,
    symbols: &TopLevelSymbols<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if !program
        .machine_states(machine)
        .iter()
        .any(|state| !program.state_contracts(state).is_empty())
    {
        return;
    }
    let machine_symbols = MachineSymbols::build(program, machine, diagnostics);
    for state in program.machine_states(machine) {
        let scope = StateContractScope {
            program,
            machine,
            state,
            machine_symbols: &machine_symbols,
            symbols,
        };
        for contract in program.state_contracts(state) {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                match fact {
                    ProofFact::Expression(expression) => scope.expression(*expression, diagnostics),
                    ProofFact::Membership(membership) => {
                        scope.expression(membership.value, diagnostics)
                    }
                    ProofFact::Proposition(application) => {
                        for argument in program
                            .expression_table
                            .expression_handles(application.arguments)
                        {
                            scope.expression(*argument, diagnostics);
                        }
                    }
                }
            }
        }
    }
}

struct StateContractScope<'program, 'symbols> {
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &'program State,
    machine_symbols: &'symbols MachineSymbols<'program>,
    symbols: &'symbols TopLevelSymbols<'program>,
}

impl StateContractScope<'_, '_> {
    fn expression(&self, expression: ExpressionHandle, diagnostics: &mut Vec<Diagnostic>) {
        if !expression.is_valid() {
            return;
        }
        let table = &self.program.expression_table;
        match table.expression(expression) {
            ExpressionNode::Name(path) => {
                let Some(name) = table.name_path_members(path.members).first() else {
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
                if !crate::locals::state_value_root_is_known(
                    self.program,
                    self.machine,
                    self.state,
                    &[],
                    self.machine_symbols,
                    self.symbols,
                    root,
                    name.as_str(),
                ) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` state `{}` arrival contract value `{}` is not in the state's explicit parameter scope",
                        self.machine.name, self.state.name, name,
                    )));
                }
            }
            ExpressionNode::Binary(binary) => {
                self.expression(binary.left, diagnostics);
                self.expression(binary.right, diagnostics);
            }
            ExpressionNode::Unary(unary) => self.expression(unary.operand, diagnostics),
            ExpressionNode::Borrow(borrow) => self.expression(borrow.target, diagnostics),
            ExpressionNode::Cast(cast) => self.expression(cast.value, diagnostics),
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
            | ExpressionNode::String(_)
            | ExpressionNode::ZeroValue(_) => {}
        }
    }
}

#[cfg(test)]
mod tests;
