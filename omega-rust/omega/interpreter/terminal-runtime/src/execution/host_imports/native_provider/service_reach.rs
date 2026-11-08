//! Admit the declared service reach of one native import beyond its own trait.
//!
//! Reach is a static ceiling and grants no capability. The private sponsor
//! performs a foreign call on the host, outside every modeled Terminal supply
//! (filesystem namespace, clock, standard streams, domain exit). Modeled
//! recognizers dispatch before this sponsor, so the sponsor only ever sees
//! imports the host does not model. It may perform an additional reached
//! service only when the host consents explicitly
//! (`TerminalNativeServiceDelegation::MatchingHost`: native execution of the
//! same product runs on this host) and the reach is package-local: the owning
//! trait, every additional service and every member of their canonical parent
//! closures are declared by the package that declares the owning trait.
//!
//! The rule bounds declared service labels, not foreign behavior: a delegated
//! call does whatever its native body does on this host, and Terminal/native
//! agreement rests on the embedder's consent that both legs run here. Another
//! package's or the toolchain's services never pass, so their effects stay
//! with that owner's boundary and modeled supply (std `Console`, `Clock`,
//! `TimeHost`, core filesystem services). Std's own raw seams reach std
//! services; on every hosted target each of those imports is claimed by its
//! model (clock, descriptor I/O, descriptor mode) before this sponsor runs.
//!
//! Ownership is the owner segment of the canonical hermetic service identity
//! Terminal Psi retains: `package:<digest>::path` or `toolchain::path`. A
//! missing row, a missing identity or an import without an owning service
//! refuses.

use crate::execution::TerminalNativeServiceDelegation;
use semantic_vocabulary::ServiceId;
use terminal_psi::TerminalModule;

const PACKAGE_OWNER: &str = "package:";

/// The declaring package segment of a package service identity.
fn package_owner(module: &TerminalModule, service: ServiceId) -> Option<&str> {
    let identity = module.identities.text(module.service(service)?.identity)?;
    let (owner, _) = identity.split_once("::")?;
    owner.starts_with(PACKAGE_OWNER).then_some(owner)
}

/// The owner-free path of a canonical identity, so diagnostics stay stable
/// when a package digest changes.
pub(super) fn identity_path(identity: &str) -> &str {
    identity.split_once("::").map_or(identity, |(_, path)| path)
}

fn describe(module: &TerminalModule, service: ServiceId) -> String {
    module
        .service(service)
        .and_then(|row| module.identities.text(row.identity))
        .map_or_else(
            || format!("{service:?} without an identity"),
            |identity| identity_path(identity).to_owned(),
        )
}

pub(super) fn admit_additional_services(
    module: &TerminalModule,
    owning_service: Option<ServiceId>,
    reach: impl IntoIterator<Item = ServiceId>,
    delegation: TerminalNativeServiceDelegation,
) -> Result<(), String> {
    let additional: Vec<ServiceId> = reach
        .into_iter()
        .filter(|service| Some(*service) != owning_service)
        .collect();
    if additional.is_empty() {
        return Ok(());
    }
    if delegation != TerminalNativeServiceDelegation::MatchingHost {
        let refused: Vec<String> = additional
            .iter()
            .map(|service| describe(module, *service))
            .collect();
        return Err(format!(
            "private native sponsor has no native-service delegation for additional service reach {refused:?}"
        ));
    }
    let Some((owning_service, owner)) =
        owning_service.and_then(|service| Some((service, package_owner(module, service)?)))
    else {
        return Err(format!(
            "native-service delegation needs a package-owned import trait for additional reach {:?}",
            additional
                .iter()
                .map(|service| describe(module, *service))
                .collect::<Vec<_>>()
        ));
    };
    // The delegated call performs the owning trait's closure as well as the
    // additional services, so every member of both closures stays local.
    let mut refused = Vec::new();
    for service in std::iter::once(owning_service).chain(additional) {
        let Some(row) = module.service(service) else {
            refused.push(describe(module, service));
            continue;
        };
        for member in std::iter::once(service).chain(row.parents.iter().copied()) {
            if package_owner(module, member) != Some(owner) {
                refused.push(describe(module, member));
            }
        }
    }
    if refused.is_empty() {
        return Ok(());
    }
    refused.sort();
    refused.dedup();
    Err(format!(
        "native-service delegation performs only reach declared by the import's own package; \
         {refused:?} belongs to another owner and needs that owner's modeled supply"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use terminal_psi::{ServiceDeclaration, TerminalIdentityTableBuilder};

    fn service(id: u64) -> ServiceId {
        ServiceId::new(id).unwrap()
    }

    const LOCAL: &str = "package:94865ea4";
    const STD: &str = "package:322a3e06";

    /// 1 owning trait and 2 memberless service of the import's package; 3 the
    /// std `Console` trait; 4 a toolchain filesystem service; 5 a local service
    /// whose parent closure is std `Console`; 6 a local service whose parent
    /// is local; 7 has no row.
    fn module() -> TerminalModule {
        let mut identities = TerminalIdentityTableBuilder::default();
        let rows = [
            (1, format!("{LOCAL}::CharacterCase"), vec![]),
            (2, format!("{LOCAL}::Lettering"), vec![]),
            (3, format!("{STD}::Console"), vec![]),
            (4, "toolchain::FilesystemContentRead".to_owned(), vec![]),
            (5, format!("{LOCAL}::Shouting"), vec![service(3)]),
            (6, format!("{LOCAL}::Capitals"), vec![service(2)]),
        ]
        .map(|(id, text, parents)| ServiceDeclaration {
            id: service(id),
            identity: identities.intern(&text),
            parents,
        });
        let mut module = TerminalModule::declarations();
        module.services = rows.to_vec();
        module.identities = identities.build();
        module
    }

    fn admit(
        owning: Option<u64>,
        reach: &[u64],
        delegation: TerminalNativeServiceDelegation,
    ) -> Result<(), String> {
        admit_additional_services(
            &module(),
            owning.map(service),
            reach.iter().copied().map(service),
            delegation,
        )
    }

    const MATCHING: TerminalNativeServiceDelegation = TerminalNativeServiceDelegation::MatchingHost;

    #[test]
    fn owning_service_alone_needs_no_delegation() {
        for delegation in [TerminalNativeServiceDelegation::Unprovided, MATCHING] {
            assert_eq!(admit(Some(1), &[1], delegation), Ok(()));
            assert_eq!(admit(None, &[], delegation), Ok(()));
        }
    }

    #[test]
    fn package_local_reach_needs_explicit_matching_host_consent() {
        let refused = admit(
            Some(1),
            &[1, 2],
            TerminalNativeServiceDelegation::Unprovided,
        )
        .unwrap_err();
        assert!(refused.contains("Lettering"), "{refused}");
        assert_eq!(admit(Some(1), &[1, 2], MATCHING), Ok(()));
        assert_eq!(admit(Some(1), &[1, 6], MATCHING), Ok(()));
    }

    #[test]
    fn matching_host_never_performs_another_owners_service() {
        for (extra, shown) in [
            (3, "Console"),
            (4, "FilesystemContentRead"),
            (7, "without an identity"),
        ] {
            let refused = admit(Some(1), &[2, extra], MATCHING).unwrap_err();
            assert!(!refused.contains("Lettering"), "{refused}");
            assert!(refused.contains(shown), "{refused}");
            assert!(!refused.contains("322a3e06"), "{refused}");
        }
    }

    #[test]
    fn parent_closure_stays_inside_the_owning_package() {
        let refused = admit(Some(1), &[5], MATCHING).unwrap_err();
        assert!(refused.contains("Console"), "{refused}");
        assert!(!refused.contains("Shouting"), "{refused}");
    }

    #[test]
    fn delegated_reach_keeps_the_owning_trait_closure_local() {
        // The owning trait's own std parent is performed by the same call.
        let refused = admit(Some(5), &[2], MATCHING).unwrap_err();
        assert!(refused.contains("Console"), "{refused}");
        assert!(!refused.contains("Lettering"), "{refused}");
        assert_eq!(admit(Some(6), &[1], MATCHING), Ok(()));
    }

    #[test]
    fn foreign_owned_or_unowned_imports_perform_no_additional_reach() {
        // A std import reaching a local service is still cross-package.
        let refused = admit(Some(3), &[2], MATCHING).unwrap_err();
        assert!(refused.contains("Lettering"), "{refused}");
        for owning in [None, Some(4), Some(7)] {
            assert!(admit(owning, &[2], MATCHING).is_err(), "{owning:?}");
        }
    }
}
