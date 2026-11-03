//! Keep the checked construction and saved fields of each completed data value.
//! Reading a value from storage still needs a current observation. Matching
//! constructor syntax or a destination type cannot establish that validity.
use super::{ExpressionHandle, StructuralJudge, StructuralTerm, TypeReferenceHandle, ValueScope};

fn traced_nominal_subject(value: &StructuralTerm, depth: usize) -> String {
    if depth >= 3 {
        return "nested".to_owned();
    }
    match value {
        StructuralTerm::ObservedValue {
            observed_type,
            value,
            descriptor,
        } => format!(
            "observed(type={observed_type:?},descriptor={},value={})",
            descriptor.is_some(),
            traced_nominal_subject(value, depth + 1),
        ),
        StructuralTerm::CapturedValue {
            expression,
            statement,
            callable,
            occurrence,
            result_type,
            ..
        } => format!(
            "capture(expression={expression:?},statement={statement:?},callable={callable:?},occurrence={occurrence},type={result_type:?})"
        ),
        StructuralTerm::Projection { subject, path } => format!(
            "projection(fields={},subject={})",
            path.split('.').count(),
            traced_nominal_subject(subject, depth + 1),
        ),
        StructuralTerm::Variable(name) => {
            format!("variable({:?})", name.chars().take(64).collect::<String>())
        }
        StructuralTerm::BoundValue(symbol) => format!("bound({symbol:?})"),
        StructuralTerm::BoundProjection { subject, path } => {
            format!(
                "bound-field(subject={subject:?},fields={})",
                path.split('.').count()
            )
        }
        StructuralTerm::Constructor { fields, .. } => {
            format!("constructor(fields={})", fields.len())
        }
        _ => "other".to_owned(),
    }
}

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
            // Owned field observations keep a typed shell around their saved
            // projection. Read that projection before looking up its producer;
            // reference and shared-borrow shells retain their existing fence.
            let owned = super::super::super::structural_terms::owned_observation_type(
                self.program,
                &observed,
            );
            let subject = match (&observed, owned) {
                (StructuralTerm::ObservedValue { value, .. }, Some(_)) => value.as_ref(),
                _ => &observed,
            };
            let (root, path) = match subject {
                StructuralTerm::Projection { subject, path } => (&**subject, Some(path)),
                value => (value, None),
            };
            let trace = |phase: &str,
                         saved: Option<TypeReferenceHandle>,
                         projected: Option<TypeReferenceHandle>,
                         source: Option<&StructuralTerm>| {
                if super::super::super::struct_trace()
                    && matches!(observed.semantic_value(), StructuralTerm::Projection { .. })
                {
                    eprintln!(
                        "STRUCT nominal-subject owner={:?} state={:?} observed={} root={} owned={owned:?} saved={saved:?} projected={projected:?} fields={} source={} phase={phase}",
                        self.machine_symbol,
                        self.declaration_state,
                        traced_nominal_subject(&observed, 0),
                        traced_nominal_subject(root, 0),
                        path.map_or(0, |path| path.split('.').count()),
                        source.map_or_else(
                            || "none".to_owned(),
                            |source| traced_nominal_subject(source, 0)
                        ),
                    );
                }
            };
            let Some(capture) = self
                .completed_nominal_values
                .iter()
                .rev()
                .find(|capture| capture.value.same_resolution_payload(root))
            else {
                trace("saved-value", None, None, None);
                break;
            };
            let mut projected_type = None;
            if let Some(owned) = owned {
                // The shell must name this producer's actual projected type.
                // A receiving annotation cannot select a different saved field
                // or complete application merely because both are owned data.
                let projected = if let Some(path) = path {
                    let names = path.split('.').collect::<Vec<_>>();
                    let paths =
                        super::super::super::completed_values::owned_term_field_paths_in_scope(
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
                        trace("owned-field-path", Some(capture.reference), None, None);
                        return observed;
                    };
                    crate::project_type_reference_from_segments(
                        self.program,
                        capture.reference,
                        path,
                    )
                } else {
                    Some(capture.reference)
                };
                projected_type = projected;
                if !projected.is_some_and(|projected| {
                    crate::declared_carriers_match(self.program, owned, projected)
                }) {
                    trace("owned-field-type", Some(capture.reference), projected, None);
                    return observed;
                }
            }
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
                        trace(
                            "saved-field-projection",
                            Some(capture.reference),
                            projected_type,
                            None,
                        );
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
                trace(
                    "saved-value-kind",
                    Some(capture.reference),
                    projected_type,
                    Some(&source),
                );
                break;
            }
            trace(
                "followed",
                Some(capture.reference),
                projected_type,
                Some(&source),
            );
            observed = source;
        }
        observed
    }
}
