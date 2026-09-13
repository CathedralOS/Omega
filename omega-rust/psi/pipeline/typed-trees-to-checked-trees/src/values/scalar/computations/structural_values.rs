//! Fresh structural establishment shares the scalar operand evaluation owner.
//! Reference leaves are classified before referent-oriented normalization:
//! their value is borrowed-storage custody, never a scalar pointer snapshot.

use super::*;
use checked_trees::{
    CheckedStructuralDispatchArm, CheckedStructuralValue, CheckedStructuralValueHandle,
    CheckedStructuralValueKind, CheckedStructuralValuePlans,
};
use typed_trees::expression::MatchPattern;

// Classify the destination before inserting scalar operand roots. A scalar or
// array Match must not leave partial structural plans when a later arm fails.
pub(super) fn is_record_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    expected: TypeReferenceHandle,
) -> bool {
    if !matches!(
        program.expression_table.expression(expression),
        ExpressionNode::StructLiteral(_) | ExpressionNode::Match(_)
    ) {
        return false;
    }
    let Some(reference) = validation::unwrapped_type_reference(program, expected) else {
        return false;
    };
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(reference)
    else {
        return false;
    };
    let Some(record) = program
        .data_definitions()
        .iter()
        .find(|record| record.symbol == *symbol)
    else {
        return false;
    };
    if program
        .data_members(record)
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
    {
        return false;
    }
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match program.expression_table.expression(expression) {
            ExpressionNode::StructLiteral(literal)
                if literal.case_symbol.is_none() && literal.type_symbol == *symbol => {}
            ExpressionNode::Name(_)
                if validation::plain_owned_value_source(program, expression, expected)
                    .is_some() => {}
            ExpressionNode::Match(dispatch) => {
                let arms = program.expression_table.match_arms(dispatch.arms);
                if arms.is_empty() {
                    return false;
                }
                pending.extend(arms.iter().map(|arm| arm.value));
            }
            _ => return false,
        }
    }
    true
}

impl Builder<'_, '_> {
    pub(super) fn structural_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueHandle> {
        let kind = if validation::reference_result_custody::parts(self.program, expected).is_some()
        {
            let state = self
                .program
                .machines()
                .iter()
                .find(|machine| machine.symbol == self.machine)
                .and_then(|machine| {
                    self.program
                        .machine_states(machine)
                        .iter()
                        .find(|state| state.symbol == self.state)
                })?;
            CheckedStructuralValueKind::Reference {
                source: validation::reference_result_custody::initializer_source(
                    self.program,
                    state,
                    expression,
                    expected,
                )?,
            }
        } else if matches!(
            self.program.expression_table.expression(expression),
            ExpressionNode::Name(_)
        ) && validation::scalar_case_constructor(self.program, expression).is_none()
            && let Some(argument) = self.owned_record_place(expression, expected)
        {
            CheckedStructuralValueKind::Place(argument)
        } else if let ExpressionNode::Call(call) =
            self.program.expression_table.expression(expression)
        {
            let returned = crate::flow::call_target_return_type(self.program, call.target_symbol)?;
            if self.program.normalized_type_identity(returned)
                != self.program.normalized_type_identity(expected)
                || !(validation::has_plain_owned_contents_with_numeric_constraints(
                    self.program,
                    returned,
                ) || validation::reference_result_custody::is_reference_record(
                    self.program,
                    returned,
                ))
            {
                return None;
            }
            let (source_call, call_ordinal) = self.call_ordinal(expression, call.target_symbol)?;
            self.record_call_arguments(
                pure,
                u32::try_from(self.statement_index).ok()?,
                call_ordinal,
                call.target_symbol,
                self.program
                    .expression_table
                    .expression_handles(call.arguments),
            );
            CheckedStructuralValueKind::Call { source_call }
        } else if let Some(constructor) = self.case_construction(expression) {
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
            self.program.expression_table.expression(expression)
            && literal.case_symbol.is_none()
        {
            self.record_value(expression, expected, values, pure)?
        } else if let Some(symbol) =
            validation::scalar_case_value_source(self.program, expression, expected)
        {
            CheckedStructuralValueKind::Place(checked_trees::CheckedUnitStructuralArgumentPlan {
                source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol,
                },
                path: Vec::new(),
                type_identity: self
                    .program
                    .normalized_type_identity(expected)
                    .into_string(),
                access: checked_trees::CheckedStructuralAccess::Owned,
            })
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
                // First-match semantics make a repeated literal Boolean arm
                // unreachable. It cannot transfer an owner or evaluate fields.
                if subject_type == PrimitiveType::Bool
                    && let MatchPattern::Value(pattern) = arm.pattern
                    && let ExpressionNode::Boolean(value) =
                        self.program.expression_table.expression(pattern)
                    && boolean_coverage[usize::from(*value)]
                {
                    continue;
                }
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
                let value = self.structural_value(arm.value, expected, values, pure)?;
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
    fn owned_record_place(
        &self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
    ) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
        let ExpressionNode::Name(name) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        if !name.symbol.is_valid()
            || name.head_symbol != name.symbol
            || name.members.count() != 1
            || !validation::has_plain_owned_contents_with_numeric_constraints(
                self.program,
                expected,
            )
        {
            return None;
        }
        let TypeReferenceNode::Named { symbol, .. } =
            self.program.type_reference_table.type_reference(expected)
        else {
            return None;
        };
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)?;
        if self
            .program
            .data_members(data)
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return None;
        }
        let owner = self
            .program
            .machines()
            .iter()
            .find(|machine| machine.symbol == self.machine)?;
        let state = self
            .program
            .machine_states(owner)
            .iter()
            .find(|state| state.symbol == self.state)?;
        let (source, reference) = if let Some((index, parameter)) = self
            .authored_parameters
            .iter()
            .filter(|parameter| {
                !parameter.is_const
                    && self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
            })
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == name.symbol)
        {
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                    parameter_index: u32::try_from(index).ok()?,
                },
                parameter.type_reference,
            )
        } else {
            let local = self
                .program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .take(self.statement_index)
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local)
                        if local.symbol == name.symbol && local.initial_value.is_valid() =>
                    {
                        Some(local)
                    }
                    _ => None,
                })?;
            (
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
                    symbol: local.symbol,
                },
                local.type_reference,
            )
        };
        if self.program.normalized_type_identity(reference)
            != self.program.normalized_type_identity(expected)
            || !validation::has_plain_owned_contents_with_numeric_constraints(
                self.program,
                reference,
            )
            || !matches!(
                self.program.type_reference_table.type_reference(reference),
                typed_trees::types::TypeReferenceNode::Named { .. }
            )
        {
            return None;
        }
        Some(checked_trees::CheckedUnitStructuralArgumentPlan {
            source,
            path: Vec::new(),
            type_identity: self
                .program
                .normalized_type_identity(reference)
                .into_string(),
            access: checked_trees::CheckedStructuralAccess::Owned,
        })
    }

    fn record_value(
        &mut self,
        expression: ExpressionHandle,
        expected: TypeReferenceHandle,
        values: &mut CheckedStructuralValuePlans,
        pure: &CheckedScalarExpressionPlans,
    ) -> Option<CheckedStructuralValueKind> {
        let ExpressionNode::StructLiteral(literal) =
            self.program.expression_table.expression(expression).clone()
        else {
            return None;
        };
        if !validation::has_plain_owned_contents_with_numeric_constraints(self.program, expected)
            && !validation::reference_result_custody::is_reference_record(self.program, expected)
        {
            return None;
        }
        let reference = validation::unwrapped_type_reference(self.program, expected)?;
        let typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
            self.program.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        if literal.case_name.is_some() || literal.type_symbol != *symbol {
            return None;
        }
        let data = self
            .program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == *symbol)?;
        let declared = self.program.data_members(data);
        if declared
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return None;
        }
        let authored = self
            .program
            .expression_table
            .struct_fields(literal.fields)
            .to_vec();
        let relevant = declared
            .iter()
            .filter_map(|member| match member {
                typed_trees::data::DataMember::Field(field) if !field.relevance.is_erased() => {
                    Some(field)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if authored.len() != relevant.len() {
            return None;
        }
        let mut fields = Vec::new();
        for (ordinal, initializer) in authored.into_iter().enumerate() {
            let field = relevant
                .iter()
                .find(|field| field.symbol == initializer.field_symbol)?;
            if fields
                .iter()
                .any(|prior: &checked_trees::CheckedStructuralRecordField| {
                    prior.field == field.symbol
                })
            {
                return None;
            }
            let reference =
                validation::unwrapped_type_reference(self.program, field.type_reference)?;
            let value =
                if validation::reference_result_custody::parts(self.program, field.type_reference)
                    .is_some()
                {
                    checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        self.structural_value(
                            initializer.value,
                            field.type_reference,
                            values,
                            pure,
                        )?,
                    )
                } else if let Some(primitive) = self.program.primitive_type_reference(reference) {
                    let root = self.expression(initializer.value, primitive)?;
                    self.plans.nodes.get_mut(root).authored_root = initializer.value;
                    self.plans.roots.append(CheckedScalarComputationRoot {
                        machine: self.machine,
                        state: self.state,
                        statement_ordinal: u32::try_from(self.statement_index).ok()?,
                        role: CheckedScalarExpressionRole::RecordField {
                            expression,
                            field_ordinal: u32::try_from(ordinal).ok()?,
                        },
                        root,
                    });
                    checked_trees::CheckedStructuralRecordFieldValue::Scalar(root)
                } else {
                    checked_trees::CheckedStructuralRecordFieldValue::Structural(
                        self.structural_value(
                            initializer.value,
                            field.type_reference,
                            values,
                            pure,
                        )?,
                    )
                };
            fields.push(checked_trees::CheckedStructuralRecordField {
                field: field.symbol,
                expression: initializer.value,
                type_reference: field.type_reference,
                value,
            });
        }
        Some(CheckedStructuralValueKind::Record {
            data_symbol: literal.type_symbol,
            fields: values.record_fields.insert_many(fields),
        })
    }
}
