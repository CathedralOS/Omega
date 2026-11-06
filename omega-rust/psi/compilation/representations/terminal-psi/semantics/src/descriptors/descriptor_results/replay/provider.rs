//! A completion keeps its provider identity and lifetime debts. A separately
//! checked forwarding request can also retain captured input reference values.
//! Flatten both inventories when replacing a provider output, so a loop keeps
//! finite source choices instead of nesting its earlier execution history.
use super::engine::{Origin, State, Value};
use super::*;

impl<Dependency> DescriptorProviderOrigin<Dependency> {
    pub(in crate::descriptors::descriptor_results) fn map_dependencies<Mapped>(
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
            forwarded_sources: match &self.forwarded_sources {
                Some(sources) => Some(
                    sources
                        .iter()
                        .map(|source| {
                            Some(match source {
                                DescriptorForwardingSource::Value(value) => {
                                    DescriptorForwardingSource::Value(map(value)?)
                                }
                                DescriptorForwardingSource::Address(value) => {
                                    DescriptorForwardingSource::Address(map(value)?)
                                }
                                DescriptorForwardingSource::ReferenceArgument { position } => {
                                    DescriptorForwardingSource::ReferenceArgument {
                                        position: *position,
                                    }
                                }
                            })
                        })
                        .collect::<Option<Vec<_>>>()?,
                ),
                None => None,
            },
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
        // Forwarded pointer values keep their captured geometry. Flatten only
        // independently forwarded providers, so a loop cannot grow a nested
        // provider history. An opaque or projected source stays unavailable.
        let forwarded_sources = provider.forwarded_sources.as_ref().and_then(|sources| {
            let mut captured_sources = BTreeSet::new();
            for source in sources {
                let (captured, is_address) = match source {
                    DescriptorForwardingSource::Value(value) => {
                        (self.substitute(machine, state, call, value)?, false)
                    }
                    DescriptorForwardingSource::Address(value) => (
                        self.substitute_forwarded_address(machine, state, call, value)?,
                        true,
                    ),
                    DescriptorForwardingSource::ReferenceArgument { position } => {
                        // The erased outer reference selects the actual's
                        // storage, not the pointer contents stored in it.
                        let query = call.argument_contents.get(*position)?.as_ref()?;
                        (
                            self.unchanged_payload_targets(machine, state, query)?
                                .into_iter()
                                .map(Origin::Storage)
                                .collect(),
                            true,
                        )
                    }
                };
                for captured in captured {
                    match captured {
                        Origin::Provider(previous) => {
                            if previous.projection != Projection::new(&[]) {
                                return None;
                            }
                            for source in previous.forwarded_sources? {
                                let value = match &source {
                                    DescriptorForwardingSource::Value(value)
                                    | DescriptorForwardingSource::Address(value) => value,
                                    DescriptorForwardingSource::ReferenceArgument { .. } => {
                                        return None;
                                    }
                                };
                                if matches!(value, Origin::Provider(_)) {
                                    return None;
                                }
                                captured_sources.insert(source);
                            }
                        }
                        ordinary => {
                            captured_sources.insert(if is_address {
                                DescriptorForwardingSource::Address(ordinary)
                            } else {
                                DescriptorForwardingSource::Value(ordinary)
                            });
                        }
                    }
                }
            }
            (!captured_sources.is_empty()).then(|| captured_sources.into_iter().collect())
        });
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
                forwarded_sources,
            },
        )]))
    }

    fn substitute_forwarded_address(
        &self,
        machine: &DescriptorMachine,
        state: &State,
        call: &DescriptorInvocation,
        source: &DescriptorStoredOrigin,
    ) -> Value {
        let DescriptorStoredOrigin::Parameter {
            position,
            projection,
        } = source
        else {
            return None;
        };
        if !projection.exact
            || projection.indirect
            || projection.window.is_some()
            || projection.captured_at.is_some()
        {
            return None;
        }
        let callee = self.machine(call.stored_effects_callee()?)?;
        let formal = exactly_one(callee.places.iter(), |row| {
            matches!(row.definition, DescriptorDefinition::Parameter { position: candidate, .. }
                if candidate == *position)
        })?;
        let path = crate::descriptors::descriptor_lifetimes::descriptor_location(
            self.types,
            formal.place,
            *callee.place_types.get(&formal.place)?,
            &projection.path,
            None,
        )?;
        let mut query = call.argument_contents.get(*position)?.as_ref()?.clone();
        query
            .steps
            .extend(path.path.into_iter().map(DescriptorContentsStep::Address));
        Some(
            self.unchanged_payload_targets(machine, state, &query)?
                .into_iter()
                .map(Origin::Storage)
                .collect(),
        )
    }

    pub(super) fn source_origin(origin: &DescriptorSourceOrigin) -> Option<Origin> {
        Some(match origin {
            DescriptorSourceOrigin::Provider(provider) => {
                Origin::Provider(provider.map_dependencies(Self::source_origin)?)
            }
            DescriptorSourceOrigin::Backing(origin) => Origin::Backing(origin.clone()),
            DescriptorSourceOrigin::IncomingSlot(location) => Origin::Incoming(location.clone()),
            DescriptorSourceOrigin::IncomingContents { query, projection } => {
                Origin::IncomingContents(query.clone(), projection.clone())
            }
        })
    }
}
