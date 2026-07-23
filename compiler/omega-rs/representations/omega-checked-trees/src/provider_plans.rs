use omega_effects::provider_plan::ProviderPlan;
use std::collections::BTreeSet;

/// The exact provider plans selected by the compiler for one checked program.
///
/// Candidates remain ordinary policy values. This carrier retains only the
/// fully covering candidates selected for the concrete target, in canonical
/// name order, so later provider execution and generated-machine lowering do
/// not have to rediscover selection from source declarations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderPlanFacts {
    plans: Vec<ProviderPlan>,
    normalized_identity: u64,
}

impl Default for SelectedProviderPlanFacts {
    fn default() -> Self {
        Self {
            plans: Vec::new(),
            normalized_identity: fingerprint_selected_plans(&[]),
        }
    }
}

impl SelectedProviderPlanFacts {
    pub fn from_selection(
        candidates: &[ProviderPlan],
        selected_names: &[String],
    ) -> Result<Self, String> {
        let mut names = BTreeSet::new();
        for name in selected_names {
            if !names.insert(name.as_str()) {
                return Err(format!(
                    "selected provider plan `{name}` appears more than once"
                ));
            }
        }

        let mut plans = Vec::with_capacity(names.len());
        let mut identities = BTreeSet::new();
        for name in names {
            let matches = candidates
                .iter()
                .filter(|candidate| candidate.name == name)
                .collect::<Vec<_>>();
            let [plan] = matches.as_slice() else {
                return Err(match matches.len() {
                    0 => format!(
                        "selected provider plan `{name}` is absent from the validated candidate set"
                    ),
                    count => format!(
                        "selected provider plan `{name}` matches {count} candidates; selection must identify exactly one plan"
                    ),
                });
            };
            let errors = plan.validate_against_schema();
            if !errors.is_empty() {
                return Err(format!(
                    "selected provider plan `{name}` is not fully covering: {}",
                    errors.join("; ")
                ));
            }
            let identity = plan.identity_fingerprint();
            if identity == 0 {
                return Err(format!(
                    "selected provider plan `{name}` produced the reserved zero identity"
                ));
            }
            if !identities.insert(identity) {
                return Err(format!(
                    "selected provider plan `{name}` collides with another selected plan at identity {identity:#018x}"
                ));
            }
            plans.push((*plan).clone());
        }

        let normalized_identity = fingerprint_selected_plans(&plans);
        Ok(Self {
            plans,
            normalized_identity,
        })
    }

    pub fn plans(&self) -> &[ProviderPlan] {
        &self.plans
    }

    pub fn plan_by_name(&self, name: &str) -> Option<&ProviderPlan> {
        self.plans.iter().find(|plan| plan.name == name)
    }

    pub fn plan_by_identity(&self, identity: u64) -> Option<&ProviderPlan> {
        self.plans
            .iter()
            .find(|plan| plan.identity_fingerprint() == identity)
    }

    pub const fn normalized_identity(&self) -> u64 {
        self.normalized_identity
    }

    pub fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }
}

fn fingerprint_selected_plans(plans: &[ProviderPlan]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in (plans.len() as u64).to_le_bytes().into_iter().chain(
        plans
            .iter()
            .flat_map(|plan| plan.identity_fingerprint().to_le_bytes()),
    ) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_effects::{
        EffectSet,
        provider_plan::{ProviderBinding, ProviderPlanRow, ServiceMethod, ServiceSchema},
    };

    fn candidate(name: &str, method: &str) -> ProviderPlan {
        ProviderPlan {
            name: name.into(),
            provider_type: format!("{name}Provider"),
            target: "x86_64-unknown-none".into(),
            schema: ServiceSchema {
                trait_name: format!("{name}Service"),
                methods: vec![ServiceMethod {
                    name: method.into(),
                    parameter_count: 0,
                    has_result: false,
                    effects: Vec::new(),
                    calling_plan_fingerprint: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: method.into(),
                binding: ProviderBinding::CheckedAdapter {
                    machine: format!("{name}Provider::{method}"),
                },
            }],
            effect_set: EffectSet::empty(),
            origin_package: "test".into(),
        }
    }

    #[test]
    fn selected_plans_are_retained_in_canonical_order() {
        let alpha = candidate("Alpha", "read");
        let beta = candidate("Beta", "write");
        let candidates = vec![beta.clone(), alpha.clone()];

        let first = SelectedProviderPlanFacts::from_selection(
            &candidates,
            &["Beta".into(), "Alpha".into()],
        )
        .expect("valid selection");
        let second = SelectedProviderPlanFacts::from_selection(
            &candidates,
            &["Alpha".into(), "Beta".into()],
        )
        .expect("valid selection");

        assert_eq!(first, second);
        assert_eq!(
            first
                .plans()
                .iter()
                .map(|plan| plan.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Alpha", "Beta"]
        );
        assert_eq!(
            first
                .plan_by_identity(alpha.identity_fingerprint())
                .map(|plan| plan.name.as_str()),
            Some("Alpha")
        );
    }

    #[test]
    fn absent_duplicate_and_partial_selections_reject() {
        let complete = candidate("Complete", "run");
        assert!(
            SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&complete),
                &["Missing".into()]
            )
            .expect_err("missing candidate must reject")
            .contains("absent")
        );
        assert!(
            SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&complete),
                &["Complete".into(), "Complete".into()]
            )
            .expect_err("duplicate selection must reject")
            .contains("more than once")
        );

        let mut partial = candidate("Partial", "run");
        partial.rows.clear();
        assert!(
            SelectedProviderPlanFacts::from_selection(&[partial], &["Partial".into()])
                .expect_err("partial selected plan must reject")
                .contains("not fully covering")
        );
    }
}
