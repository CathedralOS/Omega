//! Activation-owned, immutable sets of exact reflected service applications.
use evaluation_accounting::{EvaluationInvocationAccount, EvaluationStorageLease};
use semantic_vocabulary::{StructuralTypeId, dependency_reach::DependencyReach};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use terminal_interpreter::{TerminalEffectRejection, TerminalStructuralValue};
use terminal_psi::{TerminalIdentityIndex, TerminalModule};

const SERVICE_BASE: u64 = 1 << 60;
const SET_BASE: u64 = 1 << 61;

/// Only performed, source-designated reflection calls issue references in this activation.
#[derive(Default)]
pub(crate) struct ReflectedServices {
    issued: Mutex<
        BTreeMap<TerminalIdentityIndex, (TerminalStructuralValue, Option<EvaluationStorageLease>)>,
    >,
}
impl ReflectedServices {
    pub(crate) fn issue(
        &self,
        application: TerminalIdentityIndex,
        ty: StructuralTypeId,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<TerminalStructuralValue, TerminalEffectRejection> {
        let mut issued = self
            .issued
            .lock()
            .map_err(|_| TerminalEffectRejection::new("Build service reflection lock poisoned"))?;
        let marker = TerminalStructuralValue::minted(
            SERVICE_BASE + u64::from(application.0),
            ty,
            Vec::new(),
        );
        if let std::collections::btree_map::Entry::Vacant(entry) = issued.entry(application) {
            let lease = account
                .map(|account| account.reserve_site(0, 1, 0))
                .transpose()
                .map_err(|error| {
                    TerminalEffectRejection::new(format!(
                        "Build service reflection storage: {error:?}"
                    ))
                })?;
            entry.insert((marker.clone(), lease));
        }
        Ok(marker)
    }
}
struct Service {
    identity: String,
    parents: Vec<TerminalIdentityIndex>,
}
struct IssuedSet {
    marker: TerminalStructuralValue,
    prior: Option<usize>,
    service: TerminalIdentityIndex,
    _storage: Option<EvaluationStorageLease>,
}

#[derive(Default)]
pub(super) struct ReachSets {
    pub(super) reflections: Arc<ReflectedServices>,
    services: BTreeMap<TerminalIdentityIndex, Service>,
    sets: Mutex<Vec<IssuedSet>>,
}

impl ReachSets {
    pub fn from_module(module: &TerminalModule, reflects: bool) -> Result<Self, String> {
        let mut sets = Self::default();
        if reflects {
            let applications: BTreeMap<_, _> = module
                .interfaces
                .services
                .iter()
                .map(|row| (row.id, row.identity))
                .collect();
            for row in &module.interfaces.services {
                let name = module
                    .identities
                    .text(row.identity)
                    .ok_or("reflected service has no application identity")?;
                let parents = row
                    .parents
                    .iter()
                    .map(|id| {
                        applications
                            .get(id)
                            .copied()
                            .ok_or("reflected service has an unknown parent")
                    })
                    .collect::<Result<_, _>>()?;
                sets.services.insert(
                    row.identity,
                    Service {
                        identity: name.to_owned(),
                        parents,
                    },
                );
            }
        }
        Ok(sets)
    }

    pub fn extend(
        &self,
        prior: &TerminalStructuralValue,
        marker: &TerminalStructuralValue,
        ty: StructuralTypeId,
        account: Option<&EvaluationInvocationAccount>,
    ) -> Result<TerminalStructuralValue, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let service = marker
            .opaque_identity
            .checked_sub(SERVICE_BASE)
            .and_then(|offset| u32::try_from(offset).ok())
            .map(TerminalIdentityIndex)
            .ok_or_else(|| refuse("max_reach requires a service reference this Build issued"))?;
        let issued = self
            .reflections
            .issued
            .lock()
            .map_err(|_| refuse("Build service reflection lock poisoned"))?
            .get(&service)
            .is_some_and(|(issued, _)| issued == marker);
        if !issued || !self.services.contains_key(&service) {
            return Err(refuse(
                "reflected boundary service is absent from this Build",
            ));
        }
        let mut sets = self
            .sets
            .lock()
            .map_err(|_| refuse("Build service sets lock poisoned"))?;
        let previous = Self::issued(&sets, prior);
        let identity = u64::try_from(sets.len())
            .ok()
            .filter(|&index| index < SET_BASE)
            .ok_or_else(|| refuse("Build service set markers exhausted"))?;
        let lease = account
            .map(|account| account.reserve_site(0, 1, 0))
            .transpose()
            .map_err(|error| {
                TerminalEffectRejection::new(format!("Build service set storage: {error:?}"))
            })?;
        let marker = TerminalStructuralValue::minted(SET_BASE + identity, ty, Vec::new());
        sets.try_reserve(1)
            .map_err(|_| refuse("cannot retain Build service set"))?;
        sets.push(IssuedSet {
            marker: marker.clone(),
            prior: previous,
            service,
            _storage: lease,
        });
        Ok(marker)
    }

    /// The authored empty record denotes the empty set. Only a row this activation
    /// issued can contribute a service, and each row points strictly backwards.
    fn issued(sets: &[IssuedSet], marker: &TerminalStructuralValue) -> Option<usize> {
        let index = usize::try_from(marker.opaque_identity.checked_sub(SET_BASE)?).ok()?;
        sets.get(index)
            .filter(|node| &node.marker == marker)
            .map(|_| index)
    }

    pub fn get(
        &self,
        marker: &TerminalStructuralValue,
    ) -> Result<DependencyReach, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let sets = self
            .sets
            .lock()
            .map_err(|_| refuse("Build service sets lock poisoned"))?;
        let mut node = Self::issued(&sets, marker);
        let mut applications = BTreeSet::new();
        while let Some(index) = node {
            let row = &sets[index];
            applications.insert(row.service);
            node = row.prior;
        }
        let mut work: Vec<_> = applications.into_iter().collect();
        let mut services = BTreeSet::new();
        while let Some(id) = work.pop() {
            let row = self
                .services
                .get(&id)
                .ok_or_else(|| refuse("reflected service has an unknown application"))?;
            if services.insert(row.identity.as_str()) {
                work.extend(row.parents.iter().copied());
            }
        }
        Ok(DependencyReach::from_checked_services(
            services.into_iter().map(str::to_owned),
        ))
    }
}
