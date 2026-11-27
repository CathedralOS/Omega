//! Verify Build's restrictions against a selected native composition. Semantic
//! operations and physical mechanism receipts must satisfy their own checks.
#![forbid(unsafe_code)]

pub mod closure;
mod physical;
use build_model::behavior_exclusions::{AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind};
use diagnostics::Diagnostic;
use semantic_vocabulary::{CrashCause, ServiceId, TerminalAuthorityClass};
use terminal_psi::TerminalModule;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BehaviorExclusion {
    CrashCause(CrashCause),
    Service(ServiceId),
    PhysicalAuthorityClass(TerminalAuthorityClass),
}

impl BehaviorExclusion {
    pub fn describe(self, module: &terminal_psi::TerminalModule) -> String {
        match self {
            Self::CrashCause(cause) => {
                format!("crash cause {cause:?}")
            }
            Self::Service(service) => match module.service(service) {
                Some(declaration) => format!(
                    "service `{}`",
                    module
                        .identities
                        .text(declaration.identity)
                        .unwrap_or_default()
                ),
                None => format!("service {service}"),
            },
            Self::PhysicalAuthorityClass(class) => {
                format!("physical authority class {class:?}")
            }
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BehaviorExclusions {
    crash_causes: Vec<CrashCause>,
    services: Vec<ServiceId>,
    physical_authority_classes: Vec<TerminalAuthorityClass>,
}

impl BehaviorExclusions {
    pub(crate) fn from_selections(selections: impl IntoIterator<Item = BehaviorExclusion>) -> Self {
        let mut exclusions = Self::default();
        for selection in selections {
            match selection {
                BehaviorExclusion::CrashCause(cause) => exclusions.crash_causes.push(cause),
                BehaviorExclusion::Service(service) => exclusions.services.push(service),
                BehaviorExclusion::PhysicalAuthorityClass(class) => {
                    exclusions.physical_authority_classes.push(class)
                }
            }
        }
        exclusions.sort_and_deduplicate();
        exclusions
    }

    pub fn union(&mut self, other: &Self) {
        self.crash_causes.extend_from_slice(&other.crash_causes);
        self.services.extend_from_slice(&other.services);
        self.physical_authority_classes
            .extend_from_slice(&other.physical_authority_classes);
        self.sort_and_deduplicate();
    }

    pub fn is_empty(&self) -> bool {
        self.crash_causes.is_empty()
            && self.services.is_empty()
            && self.physical_authority_classes.is_empty()
    }

    pub(crate) fn excludes_crash_cause(&self, cause: CrashCause) -> bool {
        self.crash_causes.contains(&cause)
    }

    pub(crate) fn excludes_service(&self, service: ServiceId) -> bool {
        self.services.contains(&service)
    }

    pub(crate) fn excludes_physical_authority_class(&self, class: TerminalAuthorityClass) -> bool {
        self.physical_authority_classes.contains(&class)
    }

    pub fn crash_causes(&self) -> &[CrashCause] {
        &self.crash_causes
    }

    pub fn services(&self) -> &[ServiceId] {
        &self.services
    }

    pub(crate) fn physical_authority_classes(&self) -> &[TerminalAuthorityClass] {
        &self.physical_authority_classes
    }

    fn sort_and_deduplicate(&mut self) {
        self.crash_causes.sort_unstable();
        self.crash_causes.dedup();
        self.services.sort_unstable();
        self.services.dedup();
        self.physical_authority_classes.sort_unstable();
        self.physical_authority_classes.dedup();
    }
}

pub fn authored_behavior_exclusion_set_in(
    requests: &[AuthoredBehaviorExclusion],
    module: &TerminalModule,
) -> Result<BehaviorExclusions, Vec<Diagnostic>> {
    let mut resolved = Vec::with_capacity(requests.len());
    for row in requests {
        let exclusion = match &row.kind {
            AuthoredBehaviorExclusionKind::CrashCause { cause, .. } => {
                BehaviorExclusion::CrashCause(*cause)
            }
            AuthoredBehaviorExclusionKind::PhysicalAuthorityClass { class, .. } => {
                BehaviorExclusion::PhysicalAuthorityClass(*class)
            }
            AuthoredBehaviorExclusionKind::Service { identity, .. } => {
                let id = service_for_exclusion(module, identity).map_err(|reason| {
                    vec![
                        Diagnostic::error(format!("excluded service `{identity}` {reason}"))
                            .with_source_span(row.source_span),
                    ]
                })?;
                BehaviorExclusion::Service(id)
            }
        };
        resolved.push(exclusion);
    }
    Ok(BehaviorExclusions::from_selections(resolved))
}

fn service_for_exclusion(
    module: &TerminalModule,
    identity: &str,
) -> Result<ServiceId, &'static str> {
    let mut matches = module
        .interfaces.services
        .iter()
        .filter(|service| module.identities.text(service.identity) == Some(identity));
    let service = matches
        .next()
        .ok_or("has no declaration in the selected Psi artifact")?;
    if matches.next().is_some() {
        return Err("has ambiguous declarations in the selected Psi artifact");
    }
    Ok(service.id)
}
