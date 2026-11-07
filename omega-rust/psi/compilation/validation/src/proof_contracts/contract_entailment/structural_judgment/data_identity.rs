//! A forwarded data value keeps its identity even when it contains references.
//! Follow already completed values and their checked projections; comparing
//! their fields as scalar contents would require different observations.

use super::{ExpressionHandle, StructuralJudge, StructuralJudgment, StructuralTerm};

impl StructuralJudge<'_> {
    pub(super) fn declaration_data_identity_terms(
        &self,
        expression: ExpressionHandle,
    ) -> Option<[StructuralTerm; 2]> {
        let (owner, parameters) = self.declaration_scope()?;
        let references = super::super::structural_terms::data_identity_operand_references(
            self.program,
            owner,
            parameters,
            expression,
        )?;
        let binary = self.program.expression_table.binary_node(expression)?;
        let [Some(left), Some(right)] =
            [binary.left, binary.right].map(|operand| self.declaration_term(operand))
        else {
            return None;
        };
        let observed = [(&left, references[0]), (&right, references[1])]
            .into_iter()
            .all(|(value, reference)| {
                super::super::structural_terms::scalar_operand_is_observed(
                    self.program,
                    reference,
                    value,
                )
            });
        observed.then_some([left, right])
    }

    /// The caller supplies an actual contract proposition. An executable
    /// Boolean guard uses ordinary intake and cannot donate data identity.
    pub(in crate::proof_contracts::contract_entailment) fn intake_proposition(
        &mut self,
        program: &dyn crate::CheckingInput,
        fact: ExpressionHandle,
    ) {
        self.intake(program, fact);
        self.retain_proposition_identity(fact);
    }

    pub(super) fn retain_proposition_identity(&mut self, fact: ExpressionHandle) {
        if self.case_facts_only {
            return;
        }
        if self
            .program
            .expression_table
            .call_node(fact)
            .is_some_and(|call| {
                crate::mathematical_applications::is_identity_call(self.program, call)
            })
        {
            if let Some(StructuralTerm::Identity { left, right, .. }) = self.declaration_term(fact)
                && self.data_identity_is_observed(&left)
                && self.data_identity_is_observed(&right)
            {
                std::sync::Arc::make_mut(&mut self.captured_result_equations).push(
                    super::normal_returns::CapturedResultEquation::data_identity((*left, *right)),
                );
            }
            return;
        }
        let Some(binary) = self.program.expression_table.binary_node(fact) else {
            return;
        };
        if binary.operator == super::BinaryOperator::And
            && self.has_boolean_connective_meaning(fact)
        {
            self.retain_proposition_identity(binary.left);
            self.retain_proposition_identity(binary.right);
        } else if binary.operator == super::BinaryOperator::Equal
            && let Some([left, right]) = self.declaration_data_identity_terms(fact)
        {
            std::sync::Arc::make_mut(&mut self.captured_result_equations)
                .push(super::normal_returns::CapturedResultEquation::data_identity((left, right)));
        }
    }

    /// The arm owns these checked value identities at its completed path.
    /// Replay the tagged equations intact; scalar replay deliberately omits
    /// them because it cannot compare reference contents or IEEE fields.
    pub(in crate::proof_contracts::contract_entailment) fn retain_data_identities_from(
        &mut self,
        completed: &Self,
    ) {
        for equation in completed
            .captured_result_equations
            .iter()
            .filter(|equation| equation.is_data_identity())
        {
            if !self.captured_result_equations.iter().any(|known| {
                known.is_data_identity()
                    && known.left.same_resolution_payload(&equation.left)
                    && known.right.same_resolution_payload(&equation.right)
            }) {
                std::sync::Arc::make_mut(&mut self.captured_result_equations)
                    .push(equation.clone());
            }
        }
    }

    pub(super) fn judge_data_identity(
        &self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) -> StructuralJudgment {
        if self.case_facts_only || depth >= 32 {
            return StructuralJudgment::Unknown;
        }
        if left.same_resolution_payload(&right)
            && self.data_identity_is_observed(&left)
            && self.data_identity_is_observed(&right)
        {
            return StructuralJudgment::Proven;
        }
        let mut left_remaining = 64;
        let mut right_remaining = 64;
        let left = self.data_identity_forms(left, depth + 1, &mut left_remaining);
        let right = self.data_identity_forms(right, depth + 1, &mut right_remaining);
        if left.iter().any(|left| {
            self.data_identity_is_observed(left)
                && right.iter().any(|right| {
                    self.data_identity_is_observed(right) && left.same_resolution_payload(right)
                })
        }) {
            StructuralJudgment::Proven
        } else {
            StructuralJudgment::Unknown
        }
    }

    /// Keep each actual identity on the path to its saved fields. In
    /// particular, expanding a capture must not discard that capture: its
    /// identity remains known when a reference field's contents are unknown.
    fn data_identity_forms(
        &self,
        value: StructuralTerm,
        depth: usize,
        remaining: &mut usize,
    ) -> Vec<StructuralTerm> {
        if depth >= 32 || *remaining == 0 {
            return vec![value];
        }
        // Carry the remaining fields with each saved subject. Restarting a
        // recursive walk at every field revisits the same parent projections
        // and can exhaust the allowance before reaching their public identity.
        let copied_source = self.completed_value_identity(value.clone());
        let mut pending = Vec::new();
        if !copied_source.same_resolution_payload(&value) {
            // Keep the capture itself as well as its exact copied source.
            // This also substitutes saved inputs inside selected applications
            // without unfolding a body or adding a scalar equality rule.
            pending.push((copied_source, Vec::<String>::new()));
        }
        pending.push((value, Vec::<String>::new()));
        let mut visited: Vec<(StructuralTerm, Vec<String>)> = Vec::new();
        let mut forms: Vec<StructuralTerm> = Vec::new();
        while let Some((value, fields)) = pending.pop() {
            if *remaining == 0 {
                break;
            }
            if visited
                .iter()
                .any(|(known, path)| path == &fields && known.same_resolution_payload(&value))
            {
                continue;
            }
            *remaining -= 1;
            visited.push((value.clone(), fields.clone()));
            if fields.is_empty()
                && !forms
                    .iter()
                    .any(|known| known.same_resolution_payload(&value))
            {
                forms.push(value.clone());
            }
            let mut follow = |value| pending.push((value, fields.clone()));
            if super::super::structural_terms::owned_observation_type(self.program, &value)
                .or_else(|| {
                    super::super::structural_terms::shared_inline_observation_type(
                        self.program,
                        &value,
                    )
                })
                .is_some()
                && let StructuralTerm::ObservedValue { value, .. } = &value
            {
                follow((**value).clone());
            }
            if let StructuralTerm::Variable(name) = &value
                && let Some((_, actual)) = self
                    .substitutions
                    .iter()
                    .find(|(parameter, _)| parameter == name)
            {
                // These names keep the declaration's original input values.
                // A nested call binds its formals to observed actuals before
                // asking for Id; a callee's spelling introduces no new value.
                follow(actual.clone());
            }
            for (left, right) in self.normal_guarantees.iter() {
                if left.same_resolution_payload(&value) {
                    follow(right.clone());
                }
                if right.same_resolution_payload(&value) {
                    follow(left.clone());
                }
            }
            for equation in self
                .captured_result_equations
                .iter()
                .filter(|equation| equation.is_data_identity())
            {
                if equation.left.same_resolution_payload(&value) {
                    follow(equation.right.clone());
                }
                if equation.right.same_resolution_payload(&value) {
                    follow(equation.left.clone());
                }
            }
            for completed in self.completed_nominal_values.iter() {
                if completed.value.same_resolution_payload(&value) {
                    follow(completed.observed_value.clone());
                }
            }
            if let StructuralTerm::Projection { subject, path } = &value {
                let mut projected_fields = path.split('.').map(str::to_owned).collect::<Vec<_>>();
                projected_fields.extend_from_slice(&fields);
                if depth + projected_fields.len() < 32 {
                    pending.push(((**subject).clone(), projected_fields));
                }
            }
            if let Some((field, suffix)) = fields.split_first()
                && let Some(projected) = super::super::observed_projections::project_declared_field(
                    self.program,
                    self,
                    value,
                    field,
                )
            {
                // A public identity can name this intermediate owned field.
                // Its next projection still uses the declared field and masks
                // references instead of observing their contents.
                pending.push((projected, suffix.to_vec()));
            }
        }
        forms
    }

    /// A copied descriptor keeps its size metadata. The bytes behind it still
    /// need a current storage observation. Return only the measure so this
    /// identity rule cannot replace an ordinary value or element read.
    pub(super) fn collection_measure_identity(
        &self,
        measure: language_semantics::declaration_selection::CollectionMeasure,
        collection: StructuralTerm,
        depth: usize,
    ) -> StructuralTerm {
        if let Some(value) = super::super::structural_terms::completed_collection_measure(
            self.program,
            measure,
            &collection,
        ) {
            return value;
        }
        let mut selected = collection;
        if !self.case_facts_only
            && self.data_identity_is_observed(&selected)
            && self
                .captured_result_equations
                .iter()
                .any(|equation| equation.is_data_identity())
        {
            let mut remaining = 64;
            let mut sequence = Self::term_max_captured_occurrence(&selected);
            for alternative in self.data_identity_forms(selected.clone(), depth + 1, &mut remaining)
            {
                if !self.data_identity_is_observed(&alternative) {
                    continue;
                }
                let descriptor = alternative.into_collection_descriptor();
                let earlier = Self::term_max_captured_occurrence(&descriptor);
                // Use the same direction as completed-call value resolution:
                // a later observation follows its earlier saved input. Equal
                // sequences supply no preferred rewrite and remain unchanged.
                if earlier < sequence {
                    selected = descriptor;
                    sequence = earlier;
                }
            }
        }
        super::super::structural_terms::completed_collection_measure(
            self.program,
            measure,
            &selected,
        )
        .unwrap_or(StructuralTerm::CollectionMeasure {
            measure,
            collection: Box::new(selected),
        })
    }

    pub(super) fn data_identity_is_observed(&self, value: &StructuralTerm) -> bool {
        match value {
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } => {
                if crate::reference_shell(self.program, *observed_type).is_some() {
                    // The same descriptor can identify a field of the same
                    // data value. Its hidden referent supplies no equality.
                    descriptor
                        .as_ref()
                        .is_some_and(|descriptor| self.data_identity_is_observed(descriptor))
                } else {
                    self.data_identity_is_observed(value)
                }
            }
            StructuralTerm::Variable(_)
            | StructuralTerm::BoundValue(_)
            | StructuralTerm::BoundProjection { .. }
            | StructuralTerm::CapturedValue { .. }
            | StructuralTerm::Integer(_)
            | StructuralTerm::Float(_) => true,
            StructuralTerm::Projection { subject, .. } => self.data_identity_is_observed(subject),
            StructuralTerm::Constructor { fields, .. } => fields
                .iter()
                .all(|(_, value)| self.data_identity_is_observed(value)),
            StructuralTerm::RepeatedArray { length, element } => {
                *length == 0
                    || element
                        .as_ref()
                        .is_some_and(|element| self.data_identity_is_observed(element))
            }
            // A display-only unknown cannot establish reflexivity. Nor does
            // an unevaluated call description identify an actual call result.
            _ => false,
        }
    }
}
