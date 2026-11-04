//! Keep the checked construction and saved fields of each completed data value.
//! Reading a value from storage still needs a current observation. Matching
//! constructor syntax or a destination type cannot establish that validity.
use super::{ExpressionHandle, StructuralJudge, StructuralTerm, TypeReferenceHandle, ValueScope};

impl StructuralJudge<'_> {
    /// Keep the returned value's saved field and constructor information
    /// available to its caller. Follow the exact captured values it contains;
    /// unrelated private observations remain in the function.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn returned_nominal_values(
        &self,
        returned: &StructuralTerm,
        caller: &Self,
    ) -> Vec<super::super::CompletedNominalValue> {
        fn retain_captures(
            value: &StructuralTerm,
            receipts: &[super::super::CompletedNominalValue],
            caller_receipts: &[super::super::CompletedNominalValue],
            retained: &mut [bool],
            pending: &mut Vec<usize>,
        ) {
            let mut terms = vec![value];
            while let Some(value) = terms.pop() {
                // The caller already keeps saved information for its exact
                // captured values. Forwarding one keeps that original record.
                if matches!(value.semantic_value(), StructuralTerm::CapturedValue { .. })
                    && !caller_receipts.iter().any(|receipt| {
                        receipt
                            .value
                            .same_resolution_payload(value.semantic_value())
                    })
                    && let Some(index) = receipts.iter().rposition(|receipt| {
                        receipt
                            .value
                            .same_resolution_payload(value.semantic_value())
                    })
                    && !retained[index]
                {
                    retained[index] = true;
                    pending.push(index);
                }
                value.for_each_child(|child| terms.push(child));
            }
        }
        let receipts = self.completed_nominal_values.as_slice();
        let mut retained = vec![false; receipts.len()];
        let mut pending = Vec::new();
        retain_captures(
            returned,
            receipts,
            &caller.completed_nominal_values,
            &mut retained,
            &mut pending,
        );
        let mut exported = Vec::new();
        while let Some(index) = pending.pop() {
            let mut receipt = receipts[index].clone();
            // Callers need the returned field values after local names disappear.
            // Select their original paths before rewriting the whole payload.
            receipt.observed_value = self.detach_returned_nominal_value(
                receipt.reference,
                receipt.observed_value,
                &mut Vec::new(),
            );
            retain_captures(
                &receipt.observed_value,
                receipts,
                &caller.completed_nominal_values,
                &mut retained,
                &mut pending,
            );
            exported.push((index, receipt));
        }
        exported.sort_unstable_by_key(|(index, _)| *index);
        exported.into_iter().map(|(_, receipt)| receipt).collect()
    }

    fn detach_returned_nominal_value(
        &self,
        reference: TypeReferenceHandle,
        value: StructuralTerm,
        active: &mut Vec<TypeReferenceHandle>,
    ) -> StructuralTerm {
        let detached = (|| {
            // Saving a field must not read through a reference. Reference fields
            // keep their descriptors; scalar values keep their original paths.
            if crate::reference_shell(self.program, reference).is_some()
                || self.program.primitive_type_reference(reference).is_some()
                || active.contains(&reference)
                || active.len() >= 32
            {
                return None;
            }
            if let StructuralTerm::ObservedValue { .. } = &value {
                let owned = super::super::super::structural_terms::owned_observation_type(
                    self.program,
                    &value,
                )?;
                if !crate::declared_carriers_match(self.program, reference, owned) {
                    return None;
                }
            }
            if matches!(value.semantic_value(), StructuralTerm::Opaque(_)) {
                return None;
            }
            let definition =
                crate::field_domains::readable_nominal_definition(self.program, reference)?;
            let mut variants = self
                .program
                .data_members(definition)
                .iter()
                .filter_map(typed_trees::data::DataMember::as_variant)
                .peekable();
            let variant = if variants.peek().is_some() {
                let mut selected = variants.filter(|variant| {
                    matches!(
                        self.judge_case(value.clone(), definition.symbol, variant.symbol),
                        super::StructuralJudgment::Proven
                    )
                });
                let selected_variant = selected.next()?;
                if selected.next().is_some() {
                    return None;
                }
                Some(selected_variant)
            } else {
                None
            };
            let observed = super::super::super::structural_terms::owned_projection_observation(
                self.program,
                reference,
                value.clone(),
            );
            active.push(reference);
            let constructed = super::super::super::structural_terms::captured_constructor_term(
                self.program,
                definition,
                variant,
                |field| {
                    let mut path = Vec::new();
                    crate::value_custody::places::push_field_place_segments(
                        self.program,
                        &mut path,
                        field.symbol,
                    );
                    let projected = crate::project_type_reference_from_segments(
                        self.program,
                        reference,
                        &path,
                    )?;
                    let field_value =
                        super::super::super::observed_projections::project_observed_value_in_scope(
                            self.program,
                            self,
                            reference,
                            observed.clone(),
                            &path,
                        )?;
                    Some(self.detach_returned_nominal_value(projected, field_value, active))
                },
            );
            active.pop();
            Some(
                super::super::super::structural_terms::owned_projection_observation(
                    self.program,
                    reference,
                    constructed?,
                ),
            )
        })();
        detached.unwrap_or_else(|| self.resolve(value))
    }

    /// Record the value established by this completed operation. Its unique
    /// capture lets later uses recover the checked fields without treating
    /// another equal-looking construction as already checked.
    pub(super) fn retain_completed_nominal_value(
        &mut self,
        expression: ExpressionHandle,
        reference: TypeReferenceHandle,
        observed: StructuralTerm,
        scope: ValueScope<'_>,
    ) -> Option<StructuralTerm> {
        let value = self.capture_unknown_value(expression, scope)?;
        self.assume_completed_value_equation(reference, value.clone(), observed.clone());
        std::sync::Arc::make_mut(&mut self.completed_nominal_values).push(
            super::super::CompletedNominalValue {
                anchor: expression,
                reference,
                value: value.clone(),
                observed_value: observed,
            },
        );
        // Later field projections need the type of this completed value to
        // follow its saved constructor and keep reference fields masked.
        Some(
            super::super::super::structural_terms::owned_projection_observation(
                self.program,
                reference,
                value,
            ),
        )
    }

    /// Recover the saved fields of this exact completed value. Evaluation
    /// creates this record; rewriting an equality cannot create one. Reading
    /// the source still requires a valid storage observation.
    pub(in crate::proof_contracts::contract_entailment) fn completed_nominal_observation(
        &self,
        value: &StructuralTerm,
    ) -> StructuralTerm {
        let mut observed = value.clone();
        for _ in 0..self.completed_nominal_values.len() {
            let Some(identity) = observed.observation_identity_key() else {
                break;
            };
            let Some(capture) = self.completed_nominal_values.iter().rev().find(|capture| {
                capture.value.observation_identity_key().as_ref() == Some(&identity)
            }) else {
                break;
            };
            observed = capture.observed_value.clone();
        }
        observed
    }

    /// Read a saved owned field before replacing its enclosing capture. The
    /// producer supplies both the complete type and the field information;
    /// an untyped contract projection supplies neither admission nor access.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn completed_owned_field(
        &self,
        value: &StructuralTerm,
    ) -> Option<StructuralTerm> {
        let owned =
            super::super::super::structural_terms::owned_observation_type(self.program, value);
        let subject = match (value, owned) {
            (StructuralTerm::ObservedValue { value, .. }, Some(_)) => value.as_ref(),
            _ => value,
        };
        let StructuralTerm::Projection {
            subject: root,
            path,
        } = subject
        else {
            return None;
        };
        let Some(capture) = self
            .completed_nominal_values
            .iter()
            .rev()
            .find(|capture| capture.value.same_resolution_payload(root))
        else {
            return None;
        };
        let names = path.split('.').collect::<Vec<_>>();
        // Case checks apply to proper prefixes of this requested field path.
        // They cannot assume the field being read or use a reference crossing
        // to reconstruct its contents.
        let paths = super::super::super::completed_values::owned_term_field_paths_in_scope(
            self.program,
            self,
            capture.reference,
            &super::super::super::structural_terms::owned_projection_observation(
                self.program,
                capture.reference,
                capture.value.clone(),
            ),
            &names,
        );
        let [path] = paths.as_slice() else {
            return None;
        };
        let projected =
            crate::project_type_reference_from_segments(self.program, capture.reference, path);
        let Some(_) = projected.filter(|projected| {
            crate::reference_shell(self.program, *projected).is_none()
                && owned.is_none_or(|owned| {
                    crate::declared_carriers_match(self.program, owned, *projected)
                })
        }) else {
            return None;
        };
        super::super::super::observed_projections::project_observed_value_in_scope(
            self.program,
            self,
            capture.reference,
            super::super::super::structural_terms::owned_projection_observation(
                self.program,
                capture.reference,
                capture.observed_value.clone(),
            ),
            path,
        )
    }

    /// Recover this occurrence's constructor and, inside it, the constructor
    /// each owned child construction evaluated. A nested `Outer { inner:
    /// Inner { .. } }` publishes a fresh value per construction, so projecting
    /// `inner.field` needs every receipt on that path, not only the root's.
    /// Each receipt is keyed by its own capture identity; an equal
    /// constructor elsewhere still supplies nothing.
    pub(in crate::proof_contracts::contract_entailment) fn completed_nominal_observation_tree(
        &self,
        value: &StructuralTerm,
    ) -> StructuralTerm {
        match self.completed_nominal_observation(value) {
            StructuralTerm::Constructor { data, case, fields } => StructuralTerm::Constructor {
                data,
                case,
                fields: fields
                    .into_iter()
                    .map(|(name, field)| (name, self.completed_nominal_observation_tree(&field)))
                    .collect(),
            },
            observed => observed,
        }
    }

    /// Follow an owned capture back to its checked value and fields. Rebuilding
    /// an equal constructor does not inherit that value's identity or validity.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn completed_nominal_subject(
        &self,
        value: &StructuralTerm,
    ) -> StructuralTerm {
        let mut observed = value.clone();
        for _ in 0..self.completed_nominal_values.len() {
            let owned = super::super::super::structural_terms::owned_observation_type(
                self.program,
                &observed,
            );
            let subject = match (&observed, owned) {
                (StructuralTerm::ObservedValue { value, .. }, Some(_)) => value.as_ref(),
                _ => &observed,
            };
            let mut source = if matches!(subject, StructuralTerm::Projection { .. }) {
                let Some(source) = self.completed_owned_field(&observed) else {
                    break;
                };
                source
            } else {
                let Some(capture) = self
                    .completed_nominal_values
                    .iter()
                    .rev()
                    .find(|capture| capture.value.same_resolution_payload(subject))
                else {
                    break;
                };
                if owned.is_some_and(|owned| {
                    !crate::declared_carriers_match(self.program, owned, capture.reference)
                }) {
                    break;
                }
                capture.observed_value.clone()
            };
            if super::super::super::structural_terms::owned_observation_type(self.program, &source)
                .is_some()
            {
                // This is the admitted inline value, not a borrowed descriptor.
                // Compare its identity while retaining the constructor fence.
                source = source.into_semantic_value();
            }
            let mut source_root = &source;
            while let StructuralTerm::Projection { subject, .. } = source_root {
                source_root = subject;
            }
            if !matches!(
                source_root,
                StructuralTerm::Variable(_)
                    | StructuralTerm::BoundValue(_)
                    | StructuralTerm::BoundProjection { .. }
                    | StructuralTerm::CapturedValue { .. }
            ) {
                break;
            }
            observed = source;
        }
        observed
    }
}
