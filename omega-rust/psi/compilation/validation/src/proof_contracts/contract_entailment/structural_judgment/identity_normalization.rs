//! Reduce admitted value contexts through established Id substitutions before
//! ordinary definitional resolution. Compose their checked endpoint views once
//! per question; this table contains no scalar equations or active goals.

use super::{StructuralJudge, StructuralJudgment, StructuralTerm};

struct IdentitySubstitutions<'value> {
    rows: Vec<(&'value StructuralTerm, &'value StructuralTerm)>,
    representatives: Vec<usize>,
    component_ready: Vec<bool>,
    query_depth: usize,
    active: Vec<bool>,
    normalized: Vec<Option<StructuralTerm>>,
}

impl<'value> IdentitySubstitutions<'value> {
    fn prepare(
        judge: &StructuralJudge<'_>,
        rows: impl Iterator<Item = &'value (StructuralTerm, StructuralTerm)>,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Self, &'static str> {
        let mut admitted = Vec::new();
        for (left, right) in rows {
            spend_component_work(remaining)?;
            if judge.identity_value_is_available(left) && judge.identity_value_is_available(right) {
                admitted.push((left, right));
            }
        }
        let count = admitted.len();
        Ok(Self {
            rows: admitted,
            representatives: (0..count).collect(),
            component_ready: vec![false; count],
            query_depth: depth,
            active: vec![false; count],
            normalized: vec![None; count],
        })
    }

    /// An operand must reach an actual endpoint before its component costs
    /// any join/ranking work. Complete only that connected component, retaining
    /// the same smallest existing RHS and original-row-order tie break.
    fn prepare_component(
        &mut self,
        seed: usize,
        judge: &StructuralJudge<'_>,
        remaining: &mut usize,
    ) -> Result<usize, &'static str> {
        if self.component_ready[seed] {
            return Ok(self.representatives[seed]);
        }
        let mut members = vec![seed];
        let mut included = vec![false; self.rows.len()];
        included[seed] = true;
        let mut cursor = 0;
        while cursor < members.len() {
            let row = members[cursor];
            cursor += 1;
            for (candidate, &(other_left, other_right)) in self.rows.iter().enumerate() {
                if included[candidate] {
                    continue;
                }
                let (left, right) = self.rows[row];
                for (left, right) in [
                    (left, other_left),
                    (left, other_right),
                    (right, other_left),
                    (right, other_right),
                ] {
                    spend_component_work(remaining)?;
                    let verdict = judge.identity_representation_correspondence(
                        left.clone(),
                        right.clone(),
                        self.query_depth + 1,
                        remaining,
                    );
                    if matches!(verdict, StructuralJudgment::Proven) {
                        included[candidate] = true;
                        members.push(candidate);
                        break;
                    }
                    if *remaining == 0 {
                        return Err(
                            "the reached Id component exhausted the shared query allowance",
                        );
                    }
                }
            }
        }
        if members
            .iter()
            .any(|&row| self.active[self.representatives[row]])
        {
            return Err("the reached Id component revisits a suspended representative");
        }
        let mut best = None;
        for &row in &members {
            let cost =
                judge.identity_normalization_cost(self.rows[row].1, self.query_depth, remaining)?;
            let candidate = (cost, row);
            if best.is_none_or(|known| candidate < known) {
                best = Some(candidate);
            }
        }
        // The seed is always a member, so there is an existing RHS to retain.
        let (_, representative) = best.ok_or("the reached Id component has no target")?;
        for row in members {
            self.representatives[row] = representative;
            self.component_ready[row] = true;
        }
        Ok(representative)
    }

    fn replacement(
        &mut self,
        judge: &StructuralJudge<'_>,
        value: &StructuralTerm,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<Option<usize>, &'static str> {
        for index in 0..self.rows.len() {
            spend_component_work(remaining)?;
            let component = self.representatives[index];
            if self.component_ready[index] {
                let target = self.normalized[component]
                    .as_ref()
                    .unwrap_or(self.rows[component].1);
                if self.active[component] || value.same_resolution_payload(target) {
                    continue;
                }
            }
            let (left, right) = self.rows[index];
            for endpoint in [left, right] {
                let verdict = judge.identity_representation_correspondence(
                    value.clone(),
                    endpoint.clone(),
                    depth + 1,
                    remaining,
                );
                if matches!(verdict, StructuralJudgment::Proven) {
                    let component = self.prepare_component(index, judge, remaining)?;
                    let target = self.normalized[component]
                        .as_ref()
                        .unwrap_or(self.rows[component].1);
                    if !self.active[component] && !value.same_resolution_payload(target) {
                        return Ok(Some(component));
                    }
                    break;
                }
                if *remaining == 0 {
                    return Err("the Id component lookup exhausted the shared query allowance");
                }
            }
        }
        Ok(None)
    }
}

fn spend_component_work(remaining: &mut usize) -> Result<(), &'static str> {
    if *remaining == 0 {
        return Err("the Id component work exhausted the shared query allowance");
    }
    *remaining -= 1;
    Ok(())
}

impl StructuralJudge<'_> {
    pub(super) fn normalize_identity_values(
        &self,
        values: [StructuralTerm; 2],
        depth: usize,
        remaining: &mut usize,
    ) -> Result<[StructuralTerm; 2], &'static str> {
        let result = (|| {
            let mut substitutions = IdentitySubstitutions::prepare(
                self,
                self.rewrites.value_identities(),
                depth,
                remaining,
            )?;
            let [left, right] = values;
            Ok([
                self.normalize_identity_context(left, depth, remaining, &mut substitutions)?,
                self.normalize_identity_context(right, depth, remaining, &mut substitutions)?,
            ])
        })();
        if let Err(reason) = result.as_ref()
            && std::env::var_os("OMEGA_STRUCT_TRACE").is_some()
        {
            eprintln!(
                "identity-normalization owner={:?}: {reason}",
                self.machine_symbol
            );
        }
        result
    }

    fn identity_content_type(
        &self,
        value: &StructuralTerm,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let own_type = match value {
            StructuralTerm::Application { result_type, .. }
            | StructuralTerm::ObservedValue {
                observed_type: result_type,
                ..
            } => *result_type,
            StructuralTerm::Constructor {
                identity: super::ConstructorIdentity::Nominal { reference, .. },
                ..
            } => *reference,
            _ => return None,
        };
        (own_type.is_valid() && crate::content_algebra_for_type(self.program, own_type).is_some())
            .then_some(own_type)
    }

    fn identity_normalization_cost(
        &self,
        value: &StructuralTerm,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<usize, &'static str> {
        if depth >= 32 {
            return Err("the Id component target exceeds the query depth bound");
        }
        spend_component_work(remaining)?;
        if self.identity_content_type(value).is_some() {
            return Ok(1);
        }
        let mut cost = 1;
        match value {
            StructuralTerm::Application { arguments, .. } => {
                for argument in arguments {
                    cost += self.identity_normalization_cost(argument, depth + 1, remaining)?;
                }
            }
            StructuralTerm::Constructor { fields, .. } => {
                for (_, value) in fields {
                    cost += self.identity_normalization_cost(value, depth + 1, remaining)?;
                }
            }
            StructuralTerm::ObservedValue { value: inner, .. }
                if super::super::structural_terms::owned_observation_type(self.program, value)
                    .is_some() =>
            {
                cost += self.identity_normalization_cost(inner, depth + 1, remaining)?;
            }
            _ => {}
        }
        Ok(cost)
    }

    fn normalize_identity_context(
        &self,
        value: StructuralTerm,
        depth: usize,
        remaining: &mut usize,
        substitutions: &mut IdentitySubstitutions<'_>,
    ) -> Result<StructuralTerm, &'static str> {
        if depth >= 32 {
            return Err("the Id value context exceeds the query depth bound");
        }
        if *remaining == 0 {
            return Err("the Id value context exhausted the shared query allowance");
        }
        *remaining -= 1;
        // Ordinary call producers can retain raw actuals. Recover only this
        // value's own recorded type before entering its structural children;
        // canonical content formers are not records of observable endpoints.
        if let Some(reference) = self.identity_content_type(&value) {
            return Ok(
                super::super::structural_terms::owned_projection_observation(
                    self.program,
                    reference,
                    value,
                ),
            );
        }
        // An established whole value can replace this application without
        // decomposing a subtree its proof has already accounted for. Matching
        // remains representation-only and does not solve a child question.
        let original_application = if matches!(&value, StructuralTerm::Application { .. })
            && self.identity_value_is_available(&value)
        {
            if let Some(component) = substitutions.replacement(self, &value, depth, remaining)? {
                return self.normalize_identity_representative(
                    component,
                    depth,
                    remaining,
                    substitutions,
                );
            }
            Some(value.clone())
        } else {
            None
        };
        let owned_observation =
            super::super::structural_terms::owned_observation_type(self.program, &value).is_some();
        let value = match value {
            StructuralTerm::Application {
                result_type,
                target,
                selections,
                machine,
                arguments,
            } => {
                let arguments = arguments
                    .into_iter()
                    .map(|value| {
                        self.normalize_identity_context(value, depth + 1, remaining, substitutions)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                StructuralTerm::Application {
                    result_type,
                    target,
                    selections,
                    machine,
                    arguments,
                }
            }
            StructuralTerm::Constructor {
                identity,
                data,
                case,
                fields,
            } => {
                if identity.boolean().is_some() {
                    if !fields.is_empty() {
                        return Err("a Boolean constructor has unexpected value fields");
                    }
                    return Ok(StructuralTerm::Constructor {
                        identity,
                        data,
                        case,
                        fields,
                    });
                }
                // Each field keeps its own checked schema; reference and
                // canonical-content values remain whole leaves below.
                let observed = self
                    .identity_constructor_field_pairs(identity, &fields, identity, &fields)
                    .ok_or("the owned constructor has no complete checked field schema")?;
                let fields = fields
                    .into_iter()
                    // The schema owner returns observations in left-field
                    // order. Rebuild from those typed values, preserving its
                    // reference descriptors and opaque content boundaries.
                    .zip(observed)
                    .map(|((name, _), (value, _))| {
                        self.normalize_identity_context(value, depth + 1, remaining, substitutions)
                            .map(|value| (name, value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                StructuralTerm::Constructor {
                    identity,
                    data,
                    case,
                    fields,
                }
            }
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } if owned_observation => StructuralTerm::ObservedValue {
                observed_type,
                value: Box::new(self.normalize_identity_context(
                    *value,
                    depth + 1,
                    remaining,
                    substitutions,
                )?),
                descriptor,
            },
            // These values have independent descriptor, capture or algebra
            // custody. Normalization never opens them to rewrite contents.
            other => return Ok(other),
        };
        let resolved = self.resolve_at(value.clone(), depth + 1);
        if !self.identity_value_is_available(&resolved) {
            return Err("the normalized Id context is not an admitted stable value");
        }
        // Reduction can expose new value children after the original children
        // were normalized. Revisit only a changed resolvent, through the same
        // own-type boundary and shared finite query allowance.
        if !resolved.same_resolution_payload(&value) {
            return self.normalize_identity_context(resolved, depth + 1, remaining, substitutions);
        }
        let value = resolved;
        if original_application
            .as_ref()
            .is_some_and(|original| original.same_resolution_payload(&value))
        {
            return Ok(value);
        }
        if let Some(component) = substitutions.replacement(self, &value, depth, remaining)? {
            return self.normalize_identity_representative(
                component,
                depth,
                remaining,
                substitutions,
            );
        }
        Ok(value)
    }

    fn normalize_identity_representative(
        &self,
        component: usize,
        depth: usize,
        remaining: &mut usize,
        substitutions: &mut IdentitySubstitutions<'_>,
    ) -> Result<StructuralTerm, &'static str> {
        if let Some(value) = &substitutions.normalized[component] {
            return Ok(value.clone());
        }
        // A representative may itself compute further. Normalize it once
        // with its component suspended, then retain that result for both
        // operands. Revisit cannot send it back through the same edge.
        substitutions.active[component] = true;
        let target = substitutions.rows[component].1.clone();
        let result = self.normalize_identity_context(target, depth + 1, remaining, substitutions);
        substitutions.active[component] = false;
        if let Ok(value) = &result {
            substitutions.normalized[component] = Some(value.clone());
        }
        result
    }
}
