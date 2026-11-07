//! Reduce admitted value contexts through established Id substitutions before
//! ordinary definitional resolution. This table contains no scalar equations.

use super::{StructuralJudge, StructuralJudgment, StructuralTerm};

impl StructuralJudge<'_> {
    pub(super) fn normalize_identity_value(
        &self,
        value: StructuralTerm,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<StructuralTerm, &'static str> {
        let result = self.normalize_identity_context(value, depth, remaining);
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

    fn normalize_identity_context(
        &self,
        value: StructuralTerm,
        depth: usize,
        remaining: &mut usize,
    ) -> Result<StructuralTerm, &'static str> {
        if depth >= 32 {
            return Err("the Id value context exceeds the query depth bound");
        }
        if *remaining == 0 {
            return Err("the Id value context exhausted the shared query allowance");
        }
        *remaining -= 1;
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
                    .map(|value| self.normalize_identity_context(value, depth + 1, remaining))
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
                        self.normalize_identity_context(value, depth + 1, remaining)
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
                value: Box::new(self.normalize_identity_context(*value, depth + 1, remaining)?),
                descriptor,
            },
            // These values have independent descriptor, capture or algebra
            // custody. Normalization never opens them to rewrite contents.
            other => return Ok(other),
        };
        let value = self.resolve_at(value, depth + 1);
        if !self.identity_value_is_available(&value) {
            return Err("the normalized Id context is not an admitted stable value");
        }
        for (from, to) in self.rewrites.value_identities() {
            if *remaining == 0 {
                return Err("the Id substitution lookup exhausted the shared query allowance");
            }
            *remaining -= 1;
            if value.same_resolution_payload(to)
                || !self.identity_value_is_available(from)
                || !self.identity_value_is_available(to)
            {
                continue;
            }
            if matches!(
                self.identity_representation_correspondence(
                    value.clone(),
                    from.clone(),
                    depth + 1,
                    remaining
                ),
                StructuralJudgment::Proven
            ) {
                return self.normalize_identity_context(to.clone(), depth + 1, remaining);
            }
        }
        Ok(value)
    }
}
