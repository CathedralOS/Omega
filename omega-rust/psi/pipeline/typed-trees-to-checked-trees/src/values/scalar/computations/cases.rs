//! Constructor payloads use the ordinary operand graph before tag observation.
//! The same dynamic field computations retain their calls, traps and selected
//! operators; knowing the constructed case does not erase their evaluation.

use super::*;
use checked_trees::{
    CheckedScalarCaseComputationField, CheckedScalarCaseConstruction,
    CheckedScalarComputationStructuralArgument,
};

impl Builder<'_, '_> {
    pub(super) fn case_membership(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        let ExpressionNode::Binary(binary) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let machine = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == self.machine)?;
        let state = self
            .program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == self.state)?;
        if !validation::has_exact_case_membership_meaning(
            self.program,
            machine,
            Some(state),
            expression,
            binary,
        ) {
            return None;
        }
        let ExpressionNode::Name(selected) = self.program.expression_table.expression(binary.right)
        else {
            return None;
        };
        let case = selected.symbol;
        if let ExpressionNode::Name(name) = self.program.expression_table.expression(binary.left)
            && let Some(local) = self
                .program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(self.statement_index)
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.symbol == name.symbol => Some(local),
                    _ => None,
                })
        {
            if local.is_mutable
                || name.head_symbol != name.symbol
                || self
                    .program
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
                || !validation::has_plain_owned_contents_with_numeric_constraints(
                    self.program,
                    local.type_reference,
                )
            {
                return None;
            }
            return Some(self.insert(PrimitiveType::Bool, CheckedScalarComputationKind::CaseMembership {
                source_expression: expression,
                subject: CheckedScalarComputationStructuralArgument::Place(checked_trees::CheckedUnitStructuralArgumentPlan {
                    source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol: local.symbol },
                    path: Vec::new(),
                    type_identity: self.program.normalized_type_identity(local.type_reference).as_str().to_owned(),
                    access: checked_trees::CheckedStructuralAccess::SharedBorrow,
                }),
                case,
            }));
        }
        let constructor = self.case_construction(binary.left)?;
        Some(self.insert(
            PrimitiveType::Bool,
            CheckedScalarComputationKind::CaseMembership {
                source_expression: expression,
                subject: CheckedScalarComputationStructuralArgument::Case(constructor),
                case,
            },
        ))
    }

    /// Membership and stored values share the same authored field computations.
    pub(super) fn case_construction(
        &mut self,
        expression: ExpressionHandle,
    ) -> Option<CheckedScalarCaseConstruction> {
        let constructor = validation::scalar_case_constructor(self.program, expression)?;
        let mut fields = Vec::with_capacity(constructor.fields.len());
        for (symbol, source, primitive) in constructor.fields {
            let value = self.expression(source, primitive)?;
            self.plans.nodes.get_mut(value).authored_root = source;
            fields.push(CheckedScalarCaseComputationField { symbol, value });
        }
        let fields = self.plans.case_fields.insert_many(fields);
        Some(CheckedScalarCaseConstruction {
            expression,
            type_reference: constructor.type_reference,
            case: constructor.case,
            fields,
        })
    }
}
