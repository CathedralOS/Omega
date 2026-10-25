//! Exact declaration operands for the common arithmetic walk. Executable
//! scopes may add flow evidence; declaration scopes never synthesize a frame.

use super::ExpressionWalk;
use crate::proof_contracts::arithmetic_domains::{
    ArithmeticDomain, Diagnostic, ExpressionHandle, Interval, PrimitiveType,
};
use typed_trees::types::TypeReferenceHandle;

impl ExpressionWalk<'_> {
    pub(super) fn result_type(&self, expression: ExpressionHandle) -> Option<TypeReferenceHandle> {
        if let Some((machine, state)) = self.machine.zip(self.state) {
            return crate::expression_result_type_reference(
                self.program,
                machine,
                state,
                expression,
            );
        }
        let parameters = self
            .machine
            .and_then(|machine| self.program.machine_states(machine).first())
            .map_or(self.parameters, |entry| {
                self.program.state_parameters(entry)
            });
        crate::parameter_expression_result_type_reference(
            self.program,
            self.lexical_owner,
            parameters,
            expression,
        )
    }

    pub(super) fn declared_type(
        &self,
        expression: ExpressionHandle,
    ) -> Option<TypeReferenceHandle> {
        if let Some(machine) = self.machine {
            return crate::declared_place_type_raw(self.program, machine, self.state, expression);
        }
        self.result_type(expression)
    }

    pub(super) fn builtin_binary(&self, expression: ExpressionHandle) -> bool {
        if let Some(machine) = self.machine {
            return crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning(
                self.program, machine, self.state, expression,
            );
        }
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return false;
        };
        let Some(spelling) = typed_trees::operator::binary_operator_spelling(binary.operator)
        else {
            return false;
        };
        typed_trees::operator::has_builtin_spelled_expression_meaning(
            self.program,
            self.lexical_owner,
            expression,
            spelling,
            &[
                self.result_type(binary.left),
                self.result_type(binary.right),
            ],
        )
    }

    pub(super) fn indexed_builtin(
        &self,
        expression: ExpressionHandle,
        collection: ExpressionHandle,
    ) -> bool {
        if let Some(machine) = self.machine {
            return crate::value_custody::places::indexed_operation_has_builtin_meaning(
                self.program,
                machine,
                self.state,
                expression,
                self.result_type(collection),
            );
        }
        let Some(indexed) = self.program.expression_table.indexed_node(expression) else {
            return false;
        };
        typed_trees::operator::has_builtin_spelled_expression_meaning(
            self.program,
            self.lexical_owner,
            expression,
            language_core::OperatorSpelling::Index,
            &[
                self.result_type(collection),
                self.result_type(indexed.index),
            ],
        )
    }

    pub(super) fn division_formation(
        &self,
        binary: &typed_trees::expression::TableBinaryExpression,
        domain: ArithmeticDomain,
        primitive: PrimitiveType,
        left: Interval,
        right: Interval,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> bool {
        if let Some(machine) = self.machine {
            return super::super::division_definedness::validate_operands(
                self.program,
                machine,
                self.state,
                self.environment,
                binary,
                domain,
                primitive,
                left,
                right,
                self.owner,
                diagnostics,
            );
        }
        super::super::division_definedness::validate_interval_operands(
            binary.operator,
            domain,
            primitive,
            left,
            right,
            self.owner,
            diagnostics,
        )
    }
}
