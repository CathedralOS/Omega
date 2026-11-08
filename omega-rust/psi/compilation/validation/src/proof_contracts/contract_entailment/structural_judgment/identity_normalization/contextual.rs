//! Retry a whole value after applying an admitted Id at a child root. The
//! parent can then use its existing bound before unrelated value details unfold.

use super::super::super::structural_terms;
use super::{
    IdentitySubstitutions, StructuralJudge, StructuralJudgment, StructuralTerm,
    spend_component_work,
};

struct ContextStep {
    changed: Option<StructuralTerm>,
    reached: bool,
}

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
        if depth >= 32 {
            return Err("the contextual Id substitution exceeds the query depth bound");
        }
        // Visit the same shallow frontier across all siblings before opening
        // a deeper compound value. A change always retries the outer context.
        for frontier in 0..(32 - depth) {
            let step =
                self.identity_context_step(&value, depth, frontier, remaining, substitutions)?;
            if let Some(next) = step.changed {
                if next.same_resolution_payload(&value) {
                    return Ok(value);
                }
                return self.rewrite_identity_context(next, depth + 1, remaining, substitutions);
            }
            if !step.reached {
                break;
            }
        }
        Ok(value)
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
        frontier: usize,
        remaining: &mut usize,
        substitutions: &mut IdentitySubstitutions<'_>,
    ) -> Result<ContextStep, &'static str> {
        if frontier == 0 {
            return self
                .identity_context_root_step(value, depth, remaining, substitutions)
                .map(|changed| ContextStep {
                    changed,
                    reached: true,
                });
        }
        if depth >= 32 {
            return Err("the contextual Id substitution exceeds the query depth bound");
        }
        spend_component_work(remaining)?;
        if let Some(observed) = self.identity_content_observation(value) {
            return Ok(ContextStep {
                changed: (!observed.same_resolution_payload(value)).then_some(observed),
                reached: false,
            });
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
                    return Ok(ContextStep {
                        changed: None,
                        reached: false,
                    });
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
            _ => {
                return Ok(ContextStep {
                    changed: None,
                    reached: false,
                });
            }
        };
        if !prepared.same_resolution_payload(value) {
            return Ok(ContextStep {
                changed: Some(prepared),
                reached: true,
            });
        }
        let mut changed = false;
        let mut reached = false;
        let next = Self::map_identity_context_children(prepared, |child| {
            if changed {
                return Ok(None);
            }
            let step = self.identity_context_step(
                child,
                depth + 1,
                frontier - 1,
                remaining,
                substitutions,
            )?;
            reached |= step.reached;
            changed = step.changed.is_some();
            Ok(step.changed)
        })?;
        Ok(ContextStep {
            changed: changed.then_some(next),
            reached,
        })
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
