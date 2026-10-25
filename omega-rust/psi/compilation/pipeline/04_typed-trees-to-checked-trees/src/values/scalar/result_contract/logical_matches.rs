//! Logical capture scoping under the callable's existing formal/result policy.
use super::*;
use checked_trees::CheckedStructuralPredicatePathSegment;

#[derive(Clone)]
pub(super) struct ContractSubject {
    position: u32,
    path: Vec<CheckedStructuralPredicatePathSegment>,
    reference: TypeReferenceHandle,
    result: bool,
}

impl ContractPredicates<'_, '_> {
    fn logical_readable_path(
        &self,
        position: u32,
        path: &[CheckedStructuralPredicatePathSegment],
    ) -> Option<TypeReferenceHandle> {
        let mut remaining = self.remaining.get();
        let result = super::super::structural_fields::readable_entry_parameter_path(
            self.program,
            self.parameters,
            position,
            path,
            &mut remaining,
        );
        self.remaining.set(remaining);
        result
    }

    fn logical_subject(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<ContractSubject> {
        if depth >= 64 {
            return None;
        }
        let program = self.program;
        if let Some(observed) = self.logical_saved_subject(expression) {
            return Some(observed);
        }
        // Subjects must be pure readable places. Calls and selectors cannot be
        // replayed to manufacture a completed subject observation.
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_) => {}
            ExpressionNode::Member(member) => {
                self.logical_subject(member.receiver, depth + 1)?;
            }
            ExpressionNode::SavedSubject(_) => {
                return self.logical_saved_subject(expression);
            }
            _ => return None,
        }
        if self.allow_result
            && let Some(result) = validation::reserved_result_place(program, expression)
        {
            if result.machine_symbol != self.owner
                || validation::reserved_result_owner(program, result.root)
                    != Some((self.owner, self.result_type))
            {
                return None;
            }
            let mut reference = self.result_type;
            for segment in &result.segments {
                if validation::reference_shell(program, reference).is_some() {
                    return None;
                }
                reference = validation::project_type_reference_from_segments(
                    program,
                    reference,
                    std::slice::from_ref(segment),
                )?;
            }
            if validation::reference_shell(program, reference).is_some() {
                return None;
            }
            return Some(ContractSubject {
                position: u32::try_from(self.parameters.len()).ok()?,
                path: super::super::structural_fields::structural_projection_path(
                    program,
                    &result.segments,
                )?,
                reference: validation::field_domains::readable_type_reference(program, reference)?,
                result: true,
            });
        }
        let (position, path, _) = super::super::structural_fields::structural_parameter_place(
            program,
            self.parameters,
            expression,
        )?;
        let formal = self.parameters.get(position as usize)?;
        if formal.is_const || !super::contract_storage_root(program, formal, self.allow_result) {
            return None;
        }
        let reference = self.logical_readable_path(position, &path)?;
        Some(ContractSubject {
            position,
            path,
            reference,
            result: false,
        })
    }

    fn logical_saved_subject(&self, expression: ExpressionHandle) -> Option<ContractSubject> {
        let (mut observed, segments) = validation::logical_saved_subject_projection(
            self.program,
            expression,
            &self.logical_captures,
        )?;
        observed
            .path
            .extend(super::super::structural_fields::structural_projection_path(
                self.program,
                &segments,
            )?);
        if observed.result {
            let mut reference = observed.reference;
            for segment in &segments {
                if validation::reference_shell(self.program, reference).is_some() {
                    return None;
                }
                reference = validation::project_type_reference_from_segments(
                    self.program,
                    reference,
                    std::slice::from_ref(segment),
                )?;
            }
            if validation::reference_shell(self.program, reference).is_some() {
                return None;
            }
            observed.reference =
                validation::field_domains::readable_type_reference(self.program, reference)?;
        } else {
            observed.reference = self.logical_readable_path(observed.position, &observed.path)?;
        }
        Some(observed)
    }

    pub(super) fn logical_saved_observation(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
        let observed = self.logical_saved_subject(expression)?;
        let primitive_type = self.program.primitive_type_reference(observed.reference)?;
        let term = if primitive_type == PrimitiveType::Bool {
            CheckedScalarExpression::Boolean(Box::new(
                CheckedBooleanExpression::StructuralParameterField {
                    parameter_position: observed.position,
                    path: observed.path,
                },
            ))
        } else {
            CheckedScalarExpression::StructuralParameterField {
                parameter_position: observed.position,
                path: observed.path,
                primitive_type,
            }
        };
        Some((term, observed.reference))
    }

    fn logical_binding(
        &self,
        expression: ExpressionHandle,
    ) -> Option<validation::LogicalMatchBinding<ContractSubject>> {
        let ExpressionNode::Match(dispatch) = self.program.expression_table.expression(expression)
        else {
            return None;
        };
        let observed = self.logical_subject(dispatch.subject, 0)?;
        let capture =
            validation::logical_record_match_capture(self.program, expression, observed.reference)?;
        Some(validation::LogicalMatchBinding { capture, observed })
    }

    pub(super) fn logical_match_boolean(
        &mut self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<CheckedBooleanExpression> {
        let binding = self.logical_binding(expression)?;
        let arm = binding.capture.arm_value();
        let previous = self.logical_captures.len();
        self.logical_captures.push(binding);
        let result = self.boolean(arm, depth + 1);
        self.logical_captures.truncate(previous);
        result
    }

    pub(super) fn logical_match_numeric(
        &self,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<(CheckedScalarExpression, TypeReferenceHandle)> {
        let binding = self.logical_binding(expression)?;
        let arm = binding.capture.arm_value();
        let mut bindings = self.logical_captures.clone();
        bindings.push(binding);
        let scoped = ContractPredicates {
            program: self.program,
            operators: self.operators,
            owner: self.owner,
            result_type: self.result_type,
            parameters: self.parameters,
            allow_result: self.allow_result,
            remaining: self.remaining,
            logical_captures: bindings,
        };
        scoped.numeric_term(arm, depth + 1)
    }
}
