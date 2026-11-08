//! Retry a whole value after applying an admitted Id at a child root. The
//! parent can then use its existing bound before unrelated value details unfold.

use super::super::super::structural_terms;
use super::{
    IdentitySubstitutions, StructuralJudge, StructuralJudgment, StructuralTerm,
    spend_component_work,
};

impl StructuralJudge<'_> {
    pub(super) fn rewrite_identity_context(
        &self,
        value: StructuralTerm,
        depth: usize,
        remaining: &mut usize,
        substitutions: &mut IdentitySubstitutions<'_>,
    ) -> Result<StructuralTerm, &'static str> {
        if substitutions.rows.is_empty() {
            return Ok(value);
        }
        let Some(next) = self.identity_context_step(&value, depth, remaining, substitutions)?
        else {
            return Ok(value);
        };
        if next.same_resolution_payload(&value) {
            return Ok(value);
        }
        // Every changed context retries its ancestors before a deeper walk.
        // Rewrites consume the same allowance and logical depth as other Id work.
        self.rewrite_identity_context(next, depth + 1, remaining, substitutions)
    }

    fn identity_context_root_step(
        &self,
        value: &StructuralTerm,
        depth: usize,
        remaining: &mut usize,
        substitutions: &mut IdentitySubstitutions<'_>,
    ) -> Result<Option<StructuralTerm>, &'static str> {
        if depth >= 32 {
            return Err("the contextual Id substitution exceeds the query depth bound");
        }
        spend_component_work(remaining)?;
        if let Some(observed) = self.identity_content_observation(value) {
            return Ok((!observed.same_resolution_payload(value)).then_some(observed));
        }
        let owned = structural_terms::owned_observation_type(self.program, value).is_some();
        if !matches!(
            value,
            StructuralTerm::Application { .. } | StructuralTerm::Constructor { .. }
        ) && !owned
        {
            return Ok(None);
        }
        if !self.identity_value_is_available(value) {
            return Ok(None);
        }
        let Some(component) = substitutions.replacement(self, value, depth, remaining)? else {
            return Ok(None);
        };
        let target = substitutions.normalized[component]
            .as_ref()
            .unwrap_or(substitutions.rows[component].values[1]);
        // Keep an existing owned view of the chosen representation. Returning
        // its raw RHS could otherwise alternate with declared-field wrappers.
        if matches!(
            self.identity_representation_correspondence(
                value.clone(),
                target.clone(),
                depth + 1,
                remaining,
            ),
            StructuralJudgment::Proven
        ) {
            return Ok(None);
        }
        // This is an established endpoint, not a completed normal form. Let
        // the parent retry before resolving it; do not populate the cache.
        Ok(Some(target.clone()))
    }

    fn identity_context_step(
        &self,
        value: &StructuralTerm,
        depth: usize,
        remaining: &mut usize,
        substitutions: &mut IdentitySubstitutions<'_>,
    ) -> Result<Option<StructuralTerm>, &'static str> {
        if let Some(next) =
            self.identity_context_root_step(value, depth, remaining, substitutions)?
        {
            return Ok(Some(next));
        }
        let prepared = match value {
            StructuralTerm::Application { .. } => value.clone(),
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
                    return Ok(None);
                }
                let observed = self
                    .identity_constructor_field_pairs(*identity, fields, *identity, fields)
                    .ok_or("the owned constructor has no complete checked field schema")?;
                StructuralTerm::Constructor {
                    identity: *identity,
                    data: data.clone(),
                    case: case.clone(),
                    fields: fields
                        .iter()
                        .zip(observed)
                        .map(|((name, _), (value, _))| (name.clone(), value))
                        .collect(),
                }
            }
            StructuralTerm::ObservedValue { .. }
                if structural_terms::owned_observation_type(self.program, value).is_some() =>
            {
                value.clone()
            }
            // Captures, reference descriptors and content have their own
            // custody. No contextual walk opens those values.
            _ => return Ok(None),
        };
        if !prepared.same_resolution_payload(value) {
            return Ok(Some(prepared));
        }
        // Try every immediate child root before descending into the first
        // child's details. A later mul(X, one) can expose an enclosing fact
        // without first expanding the already established compound X.
        let direct = Self::map_identity_context_children(prepared, |child| {
            self.identity_context_root_step(child, depth + 1, remaining, substitutions)
        })?;
        if !direct.same_resolution_payload(value) {
            return Ok(Some(direct));
        }
        let mut changed = false;
        let deeper = Self::map_identity_context_children(direct, |child| {
            if changed {
                return Ok(None);
            }
            let next = self.identity_context_step(child, depth + 1, remaining, substitutions)?;
            changed = next.is_some();
            Ok(next)
        })?;
        Ok(changed.then_some(deeper))
    }

    fn map_identity_context_children(
        value: StructuralTerm,
        mut map: impl FnMut(&StructuralTerm) -> Result<Option<StructuralTerm>, &'static str>,
    ) -> Result<StructuralTerm, &'static str> {
        let mut error = None;
        let mut map = |child: StructuralTerm| {
            if error.is_some() {
                return child;
            }
            match map(&child) {
                Ok(Some(next)) => next,
                Ok(None) => child,
                Err(reason) => {
                    error = Some(reason);
                    child
                }
            }
        };
        let value = match value {
            StructuralTerm::ObservedValue {
                observed_type,
                value,
                descriptor,
            } => StructuralTerm::ObservedValue {
                observed_type,
                value: Box::new(map(*value)),
                descriptor,
            },
            other => other.map_children(map),
        };
        match error {
            Some(reason) => Err(reason),
            None => Ok(value),
        }
    }
}
