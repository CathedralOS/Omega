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
        let references = super::super::structural_terms::builtin_identity_operand_references(
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
        if let Some(StructuralTerm::Identity { left, right, .. }) = self.declaration_term(fact) {
            self.intake_identity(*left, *right);
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
            self.intake_identity(left, right);
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
                self.intake_identity(equation.left.clone(), equation.right.clone());
            }
        }
    }

    /// Id uses value representation congruence. Executable numeric equality,
    /// including IEEE signed-zero equality, supplies no rule in this reader.
    pub(super) fn judge_explicit_identity(
        &self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) -> StructuralJudgment {
        if depth >= 32 || self.case_facts_only {
            return StructuralJudgment::Unknown;
        }
        let direct = self.judge_data_identity(left.clone(), right.clone(), depth + 1);
        if !matches!(direct, StructuralJudgment::Unknown) {
            return direct;
        }
        let left = self.resolve_at(left, depth + 1);
        let right = self.resolve_at(right, depth + 1);
        if !self.identity_value_is_available(&left) || !self.identity_value_is_available(&right) {
            return StructuralJudgment::Unknown;
        }
        self.exact_identity_congruence(left, right, depth + 1)
    }

    fn exact_identity_congruence(
        &self,
        left: StructuralTerm,
        right: StructuralTerm,
        depth: usize,
    ) -> StructuralJudgment {
        if depth >= 64 {
            return StructuralJudgment::Unknown;
        }
        if left.same_resolution_payload(&right) {
            return StructuralJudgment::Proven;
        }
        // A reference's identity is its descriptor. Owned observation shells
        // retain admission but do not change the value they already observed.
        let unwrap = |term: StructuralTerm| match term {
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } => {
                if crate::reference_shell(self.program, observed_type).is_some() {
                    descriptor.map(|descriptor| *descriptor)
                } else {
                    Some(*value)
                }
            }
            StructuralTerm::IntegerEmbedding { value, .. } => Some(*value),
            other => Some(other),
        };
        let (Some(left), Some(right)) = (unwrap(left), unwrap(right)) else {
            return StructuralJudgment::Unknown;
        };
        if left.same_resolution_payload(&right) {
            return StructuralJudgment::Proven;
        }
        if matches!(
            left,
            StructuralTerm::ObservedValue { .. } | StructuralTerm::IntegerEmbedding { .. }
        ) || matches!(
            right,
            StructuralTerm::ObservedValue { .. } | StructuralTerm::IntegerEmbedding { .. }
        ) {
            return self.exact_identity_congruence(left, right, depth + 1);
        }
        match (&left, &right) {
            (StructuralTerm::Integer(left), StructuralTerm::Integer(right)) => {
                if left == right {
                    StructuralJudgment::Proven
                } else {
                    StructuralJudgment::Refuted
                }
            }
            (StructuralTerm::Float(left), StructuralTerm::Float(right)) => {
                use numerics::literals::FloatFormat;
                let format = match (left.landing(), right.landing()) {
                    (Some(left), Some(right)) if left == right => left,
                    _ => return StructuralJudgment::Unknown,
                };
                let bits = |value: &numerics::literals::FloatLiteral| match value.presented_bits() {
                    Some((actual, bits)) => (actual == format).then_some(bits),
                    None => Some(match format {
                        FloatFormat::F32 => u64::from(value.f32_bits()),
                        FloatFormat::F64 => value.value_f64().to_bits(),
                    }),
                };
                match (bits(left), bits(right)) {
                    (Some(left), Some(right)) if left == right => StructuralJudgment::Proven,
                    (Some(_), Some(_)) => StructuralJudgment::Refuted,
                    _ => StructuralJudgment::Unknown,
                }
            }
            (
                StructuralTerm::Constructor {
                    identity: left_identity,
                    fields: left_fields,
                    ..
                },
                StructuralTerm::Constructor {
                    identity: right_identity,
                    fields: right_fields,
                    ..
                },
            ) => {
                if !left_identity.same_resolution_payload(*right_identity)
                    || left_fields.len() != right_fields.len()
                {
                    return StructuralJudgment::Unknown;
                }
                let right_by_name = right_fields
                    .iter()
                    .map(|(name, value)| (name.as_str(), value))
                    .collect::<std::collections::HashMap<_, _>>();
                if right_by_name.len() != right_fields.len() {
                    return StructuralJudgment::Unknown;
                }
                let Some(declared) = self.identity_constructor_fields(*left_identity) else {
                    return StructuralJudgment::Unknown;
                };
                if declared.len() != left_fields.len() {
                    return StructuralJudgment::Unknown;
                }
                let mut verdict = StructuralJudgment::Proven;
                for (name, left) in left_fields {
                    let Some(right) = right_by_name.get(name.as_str()) else {
                        return StructuralJudgment::Unknown;
                    };
                    let Some(reference) = declared.get(name.as_str()) else {
                        return StructuralJudgment::Unknown;
                    };
                    let observe = |value| {
                        super::super::structural_terms::owned_projection_observation(
                            self.program,
                            *reference,
                            value,
                        )
                    };
                    match self.exact_identity_congruence(
                        observe(left.clone()),
                        observe((*right).clone()),
                        depth + 1,
                    ) {
                        StructuralJudgment::Refuted => return StructuralJudgment::Refuted,
                        StructuralJudgment::Unknown => verdict = StructuralJudgment::Unknown,
                        StructuralJudgment::Proven => {}
                    }
                }
                verdict
            }
            _ => {
                if self.exact_record_eta(&left, &right, depth + 1)
                    || self.exact_record_eta(&right, &left, depth + 1)
                {
                    StructuralJudgment::Proven
                } else {
                    StructuralJudgment::Unknown
                }
            }
        }
    }

    /// Rebuilding every declared field from the same observed record preserves
    /// that record, including IEEE fields. Opaque field spellings grant nothing.
    fn exact_record_eta(
        &self,
        constructor: &StructuralTerm,
        value: &StructuralTerm,
        depth: usize,
    ) -> bool {
        if depth >= 64 {
            return false;
        }
        let StructuralTerm::Constructor {
            identity,
            fields,
            case,
            ..
        } = constructor
        else {
            return false;
        };
        if !case.is_empty() {
            return false;
        }
        let Some(declaration) = identity
            .declaration()
            .and_then(|symbol| self.program.data_definition_by_symbol(symbol))
        else {
            return false;
        };
        let reference = super::super::structural_terms::owned_observation_type(self.program, value)
            .or_else(|| {
                super::super::structural_terms::shared_inline_observation_type(self.program, value)
            });
        let declaration = if let Some(reference) = reference {
            let Some(observed) = super::ConstructorIdentity::at_reference(
                self.program,
                reference,
                declaration,
                None,
            ) else {
                return false;
            };
            if !identity.same_carrier(observed) {
                return false;
            }
            let Some(selected) = self
                .program
                .data_constructor_definition(reference, declaration.symbol)
            else {
                return false;
            };
            selected
        } else {
            declaration
        };
        let members = self.program.data_members(declaration);
        if members
            .iter()
            .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
        {
            return false;
        }
        let declared = self.program.data_member_fields(members).collect::<Vec<_>>();
        if declared.is_empty() || declared.len() != fields.len() {
            return false;
        }
        let actual_by_name = fields
            .iter()
            .map(|(name, value)| (name.as_str(), value))
            .collect::<std::collections::HashMap<_, _>>();
        if actual_by_name.len() != fields.len() {
            return false;
        }
        declared.iter().all(|field| {
            let Some(actual) = actual_by_name.get(field.name.as_str()) else {
                return false;
            };
            let expected = if let Some(reference) = reference {
                super::super::observed_projections::project_observed_value_in_scope(
                    self.program,
                    self,
                    reference,
                    value.clone(),
                    &[facts::PlaceSegment::Field {
                        symbol: field.symbol,
                    }],
                )
            } else {
                super::super::observed_projections::project_field(
                    value.clone(),
                    field.name.as_str(),
                )
            };
            let Some(expected) = expected else {
                return false;
            };
            let expected = self.resolve_at(expected, depth + 1);
            self.data_identity_is_observed(actual)
                && self.data_identity_is_observed(&expected)
                && matches!(
                    self.exact_identity_congruence((*actual).clone(), expected, depth + 1),
                    StructuralJudgment::Proven
                )
        })
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
            && self.identity_value_is_available(&left)
            && self.identity_value_is_available(&right)
        {
            return StructuralJudgment::Proven;
        }
        let mut left_remaining = 64;
        let mut right_remaining = 64;
        let left = self.data_identity_forms(left, depth + 1, &mut left_remaining);
        let right = self.data_identity_forms(right, depth + 1, &mut right_remaining);
        if left.iter().any(|left| {
            self.identity_value_is_available(left)
                && right.iter().any(|right| {
                    self.identity_value_is_available(right) && left.same_resolution_payload(right)
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
            StructuralTerm::IntegerEmbedding { value, .. }
            | StructuralTerm::ContentValue { value, .. } => self.data_identity_is_observed(value),
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
