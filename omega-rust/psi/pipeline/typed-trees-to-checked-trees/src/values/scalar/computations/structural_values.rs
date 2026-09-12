//! Fresh structural establishment shares the scalar operand evaluation owner.

use super::*;
use checked_trees::{
    CheckedStructuralDispatchArm, CheckedStructuralValue, CheckedStructuralValueHandle,
    CheckedStructuralValueKind, CheckedStructuralValuePlans,
};
use typed_trees::expression::MatchPattern;

impl Builder<'_, '_> {
    pub(super) fn structural_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
    ) -> Option<CheckedStructuralValueHandle> {
        let kind = if let Some(constructor) = self.case_construction(expression) {
            if self
                .program
                .normalized_type_identity(constructor.type_reference)
                != self.program.normalized_type_identity(expected)
            {
                return None;
            }
            for (field_ordinal, field) in self
                .plans
                .case_fields
                .span(constructor.fields)?
                .iter()
                .enumerate()
            {
                self.plans.roots.append(CheckedScalarComputationRoot {
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal: u32::try_from(self.statement_index).ok()?,
                    role: CheckedScalarExpressionRole::StructuralValueField {
                        expression,
                        field_ordinal: u32::try_from(field_ordinal).ok()?,
                    },
                    root: field.value,
                });
            }
            CheckedStructuralValueKind::Case(constructor)
        } else if let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression).clone()
        {
            let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
                self.program.type_reference_table.type_reference(expected)
            else {
                return None;
            };
            if *symbol != literal.type_symbol
                || literal.case_symbol.is_some()
                || !validation::has_plain_owned_contents_with_numeric_constraints(
                    self.program,
                    expected,
                )
                || !matches!(
                    self.program.type_multiplicity(expected),
                    language_semantics::Multiplicity::Affine
                        | language_semantics::Multiplicity::Unrestricted
                )
            {
                return None;
            }
            let data_symbol = *symbol;
            let record = self
                .program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == data_symbol)?;
            let declarations = self.program.data_members(record);
            let authored = self
                .program
                .expression_table
                .struct_fields(literal.fields)
                .to_vec();
            if declarations.len() != authored.len() {
                return None;
            }
            let mut fields = Vec::new();
            for (ordinal, field) in authored.iter().enumerate() {
                if fields
                    .iter()
                    .any(|retained: &checked_trees::CheckedStructuralRecordField| {
                        retained.field == field.field_symbol
                    })
                {
                    return None;
                }
                let declaration = declarations.iter().find_map(|member| match member {
                    typed_trees::data::DataMember::Field(declaration)
                        if declaration.symbol == field.field_symbol
                            && !declaration.relevance.is_erased() =>
                    {
                        Some(declaration)
                    }
                    _ => None,
                })?;
                let primitive = self
                    .program
                    .primitive_type_reference(declaration.type_reference)?;
                let value = self.expression(field.value, primitive)?;
                self.plans.nodes.get_mut(value).authored_root = field.value;
                self.plans.roots.append(CheckedScalarComputationRoot {
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal: u32::try_from(self.statement_index).ok()?,
                    role: CheckedScalarExpressionRole::RecordField {
                        expression,
                        field_ordinal: u32::try_from(ordinal).ok()?,
                    },
                    root: value,
                });
                fields.push(checked_trees::CheckedStructuralRecordField {
                    field: field.field_symbol,
                    value,
                });
            }
            CheckedStructuralValueKind::Record {
                data_symbol,
                fields: values.record_fields.insert_many(fields),
            }
        } else {
            let ExpressionNode::Match(dispatch) =
                self.program.expression_table.expression(expression).clone()
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
            let subject_type = validation::expression_result_type_reference(
                self.program,
                machine,
                state,
                dispatch.subject,
            )
            .and_then(|reference| validation::unwrapped_type_reference(self.program, reference))
            .and_then(|reference| self.program.primitive_type_reference(reference))
            .or_else(|| validation::match_subject_primitive_type(self.program, &dispatch))?;
            let subject = self.expression(dispatch.subject, subject_type)?;
            self.plans.nodes.get_mut(subject).authored_root = dispatch.subject;
            self.plans.roots.append(CheckedScalarComputationRoot {
                machine: self.machine,
                state: self.state,
                statement_ordinal: u32::try_from(self.statement_index).ok()?,
                role: CheckedScalarExpressionRole::StructuralValueSubject { expression },
                root: subject,
            });
            let authored = self
                .program
                .expression_table
                .match_arms(dispatch.arms)
                .to_vec();
            let mut arms = Vec::new();
            let mut covered = false;
            let mut boolean_coverage = [false; 2];
            for (ordinal, arm) in authored.iter().enumerate() {
                let source_arm = arena::Handle::from_parts(
                    dispatch
                        .arms
                        .start()
                        .arena_index()
                        .checked_add(u32::try_from(ordinal).ok()?)?,
                    dispatch.arms.start().generation(),
                );
                let equality_use = if matches!(arm.pattern, MatchPattern::Value(_))
                    && matches!(subject_type, PrimitiveType::F32 | PrimitiveType::F64)
                {
                    self.comparison_use(
                        expression,
                        checked_trees::CheckedOperatorOccurrence::MatchEquality { source_arm },
                    )?
                } else {
                    arena::Handle::invalid()
                };
                let pattern = match arm.pattern {
                    MatchPattern::Wildcard => {
                        covered = true;
                        checked_trees::CheckedScalarDispatchPattern::Wildcard
                    }
                    MatchPattern::Value(pattern) => {
                        if subject_type == PrimitiveType::Bool
                            && let ExpressionNode::Boolean(value) =
                                self.program.expression_table.expression(pattern)
                        {
                            boolean_coverage[usize::from(*value)] = true;
                            covered = boolean_coverage.iter().all(|value| *value);
                        }
                        let computation = self.expression(pattern, subject_type)?;
                        self.plans.nodes.get_mut(computation).authored_root = pattern;
                        self.plans.roots.append(CheckedScalarComputationRoot {
                            machine: self.machine,
                            state: self.state,
                            statement_ordinal: u32::try_from(self.statement_index).ok()?,
                            role: CheckedScalarExpressionRole::StructuralValuePattern {
                                source_arm,
                            },
                            root: computation,
                        });
                        checked_trees::CheckedScalarDispatchPattern::Value(computation)
                    }
                };
                let value = self.structural_value(arm.value, expected, values)?;
                arms.push(CheckedStructuralDispatchArm {
                    source_arm,
                    equality_use,
                    pattern,
                    value,
                });
                if covered {
                    break;
                }
            }
            if !covered {
                return None;
            }
            CheckedStructuralValueKind::Dispatch {
                subject,
                arms: values.dispatch_arms.insert_many(arms),
            }
        };
        Some(
            values
                .nodes
                .append(CheckedStructuralValue { expression, kind }),
        )
    }
}
