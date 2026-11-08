//! Apply shallow established Id substitutions before unfolding compound values.
//! Each unchanged context is traversed once, with a retry only after a change.

use std::collections::VecDeque;

use super::super::super::structural_terms;
use super::{
    IdentitySubstitutions, StructuralJudge, StructuralJudgment, StructuralTerm,
    spend_component_work,
};

struct ContextStep {
    changed: Option<StructuralTerm>,
    descend: bool,
}

impl StructuralJudge<'_> {
    pub(super) fn rewrite_identity_context(
        &self,
        mut value: StructuralTerm,
        depth: usize,
        remaining: &mut usize,
        substitutions: &mut IdentitySubstitutions<'_>,
    ) -> Result<StructuralTerm, &'static str> {
        if substitutions.rows.is_empty() {
            return Ok(value);
        }
        loop {
            let Some((path, next)) =
                self.identity_context_change(&value, depth, remaining, substitutions)?
            else {
                return Ok(value);
            };
            let next = Self::replace_identity_context(value.clone(), &path, next, remaining)?;
            if next.same_resolution_payload(&value) {
                return Ok(value);
            }
            // A rewrite changes this context, not its structural nesting.
            // Every retry still spends the same finite query allowance.
            value = next;
        }
    }

    fn identity_context_change(
        &self,
        value: &StructuralTerm,
        depth: usize,
        remaining: &mut usize,
        substitutions: &mut IdentitySubstitutions<'_>,
    ) -> Result<Option<(Vec<usize>, StructuralTerm)>, &'static str> {
        let step = self.identity_context_step(value, depth, remaining, substitutions)?;
        if let Some(next) = step.changed {
            return Ok(Some((Vec::new(), next)));
        }
        let mut pending = VecDeque::new();
        if step.descend {
            pending.push_back((value, Vec::new(), depth));
        }
        // Queue parents rather than every unvisited child. A wide value does
        // not allocate a queue beyond the nodes actually admitted for descent.
        while let Some((parent, path, parent_depth)) = pending.pop_front() {
            let mut index = 0;
            while let Some(child) = Self::identity_context_child(parent, index) {
                let step =
                    self.identity_context_step(child, parent_depth + 1, remaining, substitutions)?;
                if step.changed.is_some() || step.descend {
                    let mut child_path = path.clone();
                    child_path.push(index);
                    if let Some(next) = step.changed {
                        return Ok(Some((child_path, next)));
                    }
                    pending.push_back((child, child_path, parent_depth + 1));
                }
                index += 1;
            }
        }
        Ok(None)
    }

    fn identity_context_step(
        &self,
        value: &StructuralTerm,
        depth: usize,
        remaining: &mut usize,
        substitutions: &mut IdentitySubstitutions<'_>,
    ) -> Result<ContextStep, &'static str> {
        if depth >= 32 {
            return Err("the contextual Id substitution exceeds the query depth bound");
        }
        spend_component_work(remaining)?;
        if let Some(observed) = self.identity_content_observation(value) {
            let changed = !observed.same_resolution_payload(value);
            return Ok(ContextStep {
                changed: changed.then_some(observed),
                descend: false,
            });
        }
        let owned = structural_terms::owned_observation_type(self.program, value).is_some();
        if (matches!(
            value,
            StructuralTerm::Application { .. } | StructuralTerm::Constructor { .. }
        ) || owned)
            && self.identity_value_is_available(value)
            && let Some(component) = substitutions.replacement(self, value, depth, remaining)?
        {
            let target = substitutions.normalized[component]
                .as_ref()
                .unwrap_or(substitutions.rows[component].values[1]);
            // Preserve an equivalent owned view rather than alternating it
            // with the raw endpoint retained by the evidence producer.
            if !matches!(
                self.identity_representation_correspondence(
                    value.clone(),
                    target.clone(),
                    depth + 1,
                    remaining,
                ),
                StructuralJudgment::Proven
            ) {
                return Ok(ContextStep {
                    changed: Some(target.clone()),
                    descend: false,
                });
            }
        }
        let prepared = match value {
            StructuralTerm::Application { .. } => {
                return Ok(ContextStep {
                    changed: None,
                    descend: true,
                });
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
                    return Ok(ContextStep {
                        changed: None,
                        descend: false,
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
            StructuralTerm::ObservedValue { .. } if owned => {
                return Ok(ContextStep {
                    changed: None,
                    descend: true,
                });
            }
            // Captures, reference descriptors and canonical content remain
            // leaves under their existing observation and custody owners.
            _ => {
                return Ok(ContextStep {
                    changed: None,
                    descend: false,
                });
            }
        };
        let changed = !prepared.same_resolution_payload(value);
        Ok(ContextStep {
            changed: changed.then_some(prepared),
            descend: !changed,
        })
    }

    fn identity_context_child(value: &StructuralTerm, index: usize) -> Option<&StructuralTerm> {
        match value {
            StructuralTerm::Application { arguments, .. } => arguments.get(index),
            StructuralTerm::Constructor { fields, .. } => fields.get(index).map(|(_, value)| value),
            StructuralTerm::ObservedValue { value, .. } if index == 0 => Some(value),
            _ => None,
        }
    }

    fn replace_identity_context(
        value: StructuralTerm,
        path: &[usize],
        replacement: StructuralTerm,
        remaining: &mut usize,
    ) -> Result<StructuralTerm, &'static str> {
        let Some((&selected, rest)) = path.split_first() else {
            return Ok(replacement);
        };
        spend_component_work(remaining)?;
        let mut replacement = Some(replacement);
        let mut index = 0;
        let result = Self::map_identity_context_children(value, |child| {
            let current = index;
            index += 1;
            if current != selected {
                return Ok(None);
            }
            let next = replacement
                .take()
                .ok_or("the contextual Id path repeats a selected child")?;
            Self::replace_identity_context(child.clone(), rest, next, remaining).map(Some)
        })?;
        if replacement.is_some() {
            return Err("the contextual Id path no longer names its observed child");
        }
        Ok(result)
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
