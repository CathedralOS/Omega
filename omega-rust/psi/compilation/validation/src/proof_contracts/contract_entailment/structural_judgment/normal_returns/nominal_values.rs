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
            // Resolve saved fields before their local bindings disappear.
            // Follow any captured values still used by those fields, while
            // preserving the returned capture's exact identity.
            receipt.observed_value = self.resolve(receipt.observed_value);
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

    /// Follow an owned capture back to its checked value and fields. Rebuilding
    /// an equal constructor does not inherit that value's identity or validity.
    pub(in crate::proof_contracts::contract_entailment::structural_judgment) fn completed_nominal_subject(
        &self,
        value: &StructuralTerm,
    ) -> StructuralTerm {
        let mut observed = value.clone();
        for _ in 0..self.completed_nominal_values.len() {
            let (root, path) = match &observed {
                StructuralTerm::Projection { subject, path } => (&**subject, Some(path)),
                value => (value, None),
            };
            let Some(capture) = self
                .completed_nominal_values
                .iter()
                .rev()
                .find(|capture| &capture.value == root)
            else {
                break;
            };
            let mut source = capture.observed_value.clone();
            if let Some(path) = path {
                // Typed projection consumes authenticated inline observations
                // and masks reference children; a descriptor still cannot
                // authorize reading its referent.
                for field in path.split('.') {
                    let Some(projected) =
                        super::super::super::observed_projections::project_declared_field(
                            self.program,
                            self,
                            source,
                            field,
                        )
                    else {
                        return observed;
                    };
                    source = projected;
                }
            }
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
