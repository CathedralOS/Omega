use super::engine::{DescriptorReadDepth, Origin, State, Value};
use super::*;

impl<Dependency> DescriptorProviderOrigin<Dependency> {
    pub(crate) fn map_dependencies<Mapped>(
        &self,
        mut map: impl FnMut(&Dependency) -> Option<Mapped>,
    ) -> Option<DescriptorProviderOrigin<Mapped>> {
        Some(DescriptorProviderOrigin {
            boundary: self.boundary,
            output: self.output.clone(),
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

impl DescriptorAnalysis<'_> {
    /// The permission of a provider-returned descriptor immediately before
    /// this action. Earlier cleanups on the same edge may have replaced it.
    pub fn provider_access_at_invocation(
        &mut self,
        module: &terminal_psi::TerminalModule,
        machine: &terminal_psi::TerminalMachine,
        position: DescriptorPosition,
        action: DescriptorInvocationOwner,
        descriptor: &DescriptorLocation,
    ) -> Result<Option<terminal_psi::StructuralAccess>, &'static str> {
        self.replay
            .prepare_states(machine.id)
            .ok_or("provider descriptor has no checked snapshots")?;
        let body = self
            .replay
            .machine(machine.id)
            .ok_or("provider descriptor machine is absent")?;
        let state = self
            .replay
            .invocation_state(machine.id, position, action)
            .ok_or("provider descriptor action has no reachable prestate")?;
        let Some(origins) = self
            .replay
            .read(body, state, descriptor)
            .and_then(|value| self.replay.symbolic(body, value))
        else {
            return Ok(None);
        };
        crate::references::reference_completion::provider_loan_access(
            module, machine, descriptor, &origins,
        )
    }
}

impl Replay<'_> {
    pub(crate) fn substitute_lifetime_dependencies(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        stored: &[DescriptorLifetimeDebt<DescriptorStoredOrigin>],
        reads: &DescriptorReadDepth<'_>,
    ) -> Option<Vec<DescriptorLifetimeDebt<Origin>>> {
        let mut dependencies = BTreeSet::new();
        for dependency in stored {
            for captured in
                self.substitute_with_depth(machine, state, call, &dependency.origin, reads)?
            {
                match captured {
                    Origin::Provider(previous) => {
                        dependencies.extend(previous.lifetime_dependencies);
                    }
                    Origin::ProducedContents(previous) => {
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
        Some(dependencies.into_iter().collect())
    }

    pub(crate) fn substitute_provider_with_depth(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        provider: &DescriptorProviderOrigin<DescriptorStoredOrigin>,
        reads: &DescriptorReadDepth<'_>,
    ) -> Value {
        let lifetime_dependencies = self.substitute_lifetime_dependencies(
            machine,
            state,
            call,
            &provider.lifetime_dependencies,
            reads,
        )?;
        let mut invocations = vec![DescriptorInvocationSite {
            machine: machine.id,
            owner: call.owner,
        }];
        invocations.extend_from_slice(&provider.invocations);
        Some(BTreeSet::from([Origin::Provider(
            DescriptorProviderOrigin {
                boundary: provider.boundary,
                output: provider.output.clone(),
                invocations,
                projection: provider.projection.clone(),
                lifetime_dependencies,
            },
        )]))
    }

    pub(crate) fn source_origin(origin: &DescriptorSourceOrigin) -> Option<Origin> {
        Some(match origin {
            DescriptorSourceOrigin::Provider(provider) => {
                Origin::Provider(provider.map_dependencies(Self::source_origin)?)
            }
            DescriptorSourceOrigin::Backing(origin) => Origin::Backing(origin.clone()),
            DescriptorSourceOrigin::IncomingSlot(location) => {
                Origin::Incoming(location.clone())
            }
            DescriptorSourceOrigin::IncomingContents { query, projection } => {
                Origin::IncomingContents(query.clone(), projection.clone())
            }
        })
    }
}
