//! A completion creates provider provenance, not an input-storage projection.
//! Only lifetime debts are transitive: capturing another provider result takes
//! its flat debt frontier. Repeated replacement therefore joins finite anchors
//! instead of nesting the previous provider history on every loop iteration.
use super::replay::{Origin, State, Value};
use super::*;

impl<Dependency> DescriptorProviderOrigin<Dependency> {
    pub(in crate::descriptor_results) fn map_dependencies<Mapped>(
        &self,
        mut map: impl FnMut(&Dependency) -> Option<Mapped>,
    ) -> Option<DescriptorProviderOrigin<Mapped>> {
        Some(DescriptorProviderOrigin {
            boundary: self.boundary,
            output: self.output,
            invocations: self.invocations.clone(),
            projection: self.projection.clone(),
            lifetime_dependencies: self
                .lifetime_dependencies
                .iter()
                .map(|debt| {
                    Some(DescriptorLifetimeDebt {
                        origin: map(&debt.origin)?,
                        access: debt.access,
                    })
                })
                .collect::<Option<_>>()?,
        })
    }
}

impl Replay<'_> {
    pub(super) fn substitute_provider(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        provider: &DescriptorProviderOrigin<DescriptorStoredOrigin>,
    ) -> Value {
        let mut dependencies = BTreeSet::new();
        for dependency in &provider.lifetime_dependencies {
            for captured in self.substitute(machine, state, call, &dependency.origin)? {
                match captured {
                    Origin::Provider(previous) => {
                        // Do not project these debts into the new referent or
                        // retain the old receiving slot as a spatial parent.
                        dependencies.extend(previous.lifetime_dependencies);
                    }
                    ordinary => {
                        dependencies.insert(DescriptorLifetimeDebt {
                            origin: ordinary,
                            access: dependency.access,
                        });
                    }
                }
            }
        }
        let mut invocations = vec![DescriptorInvocationSite {
            machine: machine.id,
            operation: call.operation,
        }];
        invocations.extend_from_slice(&provider.invocations);
        Some(BTreeSet::from([Origin::Provider(
            DescriptorProviderOrigin {
                boundary: provider.boundary,
                output: provider.output,
                invocations,
                projection: provider.projection.clone(),
                lifetime_dependencies: dependencies.into_iter().collect(),
            },
        )]))
    }

    pub(super) fn source_origin(origin: &DescriptorSourceOrigin) -> Option<Origin> {
        Some(match origin {
            DescriptorSourceOrigin::Provider(provider) => {
                Origin::Provider(provider.map_dependencies(Self::source_origin)?)
            }
            DescriptorSourceOrigin::Backing(origin) => Origin::Backing(origin.clone()),
            DescriptorSourceOrigin::IncomingSlot(location) => Origin::Incoming(location.clone()),
            DescriptorSourceOrigin::IncomingContents(query) => {
                Origin::IncomingContents(query.clone(), Projection::new(&[]))
            }
        })
    }
}
