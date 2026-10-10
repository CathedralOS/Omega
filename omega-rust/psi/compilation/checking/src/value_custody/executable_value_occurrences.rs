//! Executable value occurrences, before type and proof judgments exist.
//!
//! Preparation and checking need the same expression/origin order. Traverse it
//! once here; checking attaches type and proved range evidence in its callback.
//! Do not deduplicate expression handles: the same authored expression can
//! occur under distinct statement roles. Children retain their parent origin.

mod expression;
mod statement;
mod transition;

use checked_trees::{
    CheckedValueFact, CheckedValueFacts, CheckedValueOrigin, CheckedValueStatementRole,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::references::DeclarationSelection;
use typed_trees::types::PrimitiveType;

/// Visit the selected declaration roots in machine/state order, parent before
/// children. Roots select work, not authority, and retain every distinct origin.
/// The optional primitive is an authored builtin argument destination, not
/// inferred type evidence. The callback owns all produced facts.
pub fn visit_executable_value_occurrences(
    program: &dyn crate::CheckingInput,
    roots: &DeclarationSelection,
    visit: &mut dyn FnMut(ExpressionHandle, CheckedValueOrigin, Option<PrimitiveType>),
) {
    let mut visitor = ValueOccurrenceVisitor { program, visit };
    for machine in program.machines() {
        if !roots.contains(machine.symbol) {
            continue;
        }
        if let Some(subjects) =
            typed_trees::ranking::resolve_machine_witness_subjects(program, machine)
        {
            for (ordinal, expression) in subjects.into_iter().enumerate() {
                visitor.collect_expression(expression,
                    CheckedValueOrigin::MachineDecrease {
                        machine_symbol: machine.symbol,
                        ordinal,
                    },
        None);
            }
        }

        for owned_data in program.machine_owned_data(machine) {
            if owned_data.initial_value.is_valid() {
                visitor.collect_expression(owned_data.initial_value,
                    CheckedValueOrigin::MachineOwnedDataInitializer {
                        machine_symbol: machine.symbol,
                        data_symbol: owned_data.symbol,
                    },
        None);
            }
        }
        for state in program.machine_states(machine) {
            for (statement_index, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                visitor.collect_statement(machine.symbol, state.symbol, statement_index, statement);
            }
        }
    }
}

/// Collect only expression/origin rows for pre-check selection. Their type and
/// range fields remain absent; this does not manufacture checking conclusions.
pub fn collect_executable_value_origins(
    program: &dyn crate::CheckingInput,
    roots: &DeclarationSelection,
) -> CheckedValueFacts {
    let mut facts = CheckedValueFacts::default();
    visit_executable_value_occurrences(program, roots, &mut |expression, origin, _| {
        facts.values.insert(CheckedValueFact {
            expression,
            origin,
            type_reference: typed_trees::types::TypeReferenceHandle::invalid(),
            primitive_type: None,
            integer_range: None,
        });
    });
    facts
}

struct ValueOccurrenceVisitor<'program, 'visitor> {
    program: &'program dyn crate::CheckingInput,
    visit: &'visitor mut dyn FnMut(ExpressionHandle, CheckedValueOrigin, Option<PrimitiveType>),
}

impl ValueOccurrenceVisitor<'_, '_> {
    fn collect_statement_expression(
        &mut self,
        machine_symbol: SymbolHandle,
        state_symbol: SymbolHandle,
        statement_index: usize,
        expression: ExpressionHandle,
        role: CheckedValueStatementRole,
    ) {
        self.collect_expression(expression,
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index,
                role,
            },
        None);
    }


    fn collect_expression(
        &mut self,
        expression: ExpressionHandle,
        origin: CheckedValueOrigin,
        expected_primitive: Option<PrimitiveType>,
    ) {
        if !expression.is_valid() {
            return;
        }
        (self.visit)(expression, origin, expected_primitive);
        self.collect_expression_children(expression);
    }

    fn collect_nested_expression(
        &mut self,
        parent: ExpressionHandle,
        expression: ExpressionHandle,
    ) {
        self.collect_expression(expression, CheckedValueOrigin::NestedExpression { parent },
        None);
    }
}
