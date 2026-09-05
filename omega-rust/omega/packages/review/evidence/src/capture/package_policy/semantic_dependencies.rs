//! Retained semantic edges with package-grouped private implementation consumers.

use crate::capture::semantics::conformances::policy_callable_identity;
use crate::capture::semantics::declarations::{nominal_identity, reviewed_package_owns};
use crate::capture::semantics::facts::exactly_one;
use crate::record::*;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;

pub(super) fn project(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
    callables: &PackagePolicyCallables,
) -> Result<Vec<PackagePolicySemanticDependency>, Vec<Diagnostic>> {
    let derived = psi_typed_trees_to_checked_trees::derive_checked_semantic_dependencies(
        &compilation.typed,
        &compilation.facts,
    );
    if derived != compilation.facts.flow.semantic_dependencies {
        return Err(super::rejected(
            "semantic dependencies differ from exact checked rederivation",
        ));
    }
    let mut rows = Vec::new();
    for fact in &derived.rows {
        let owner = nominal_identity(compilation, fact.consumer_machine)?;
        if !reviewed_package_owns(&owner, package)? {
            continue;
        }
        let machine = exactly_one(
            compilation
                .machines()
                .iter()
                .filter(|machine| machine.symbol == fact.consumer_machine),
            "policy semantic dependency",
            "consumer machine",
        )?;
        let consumer = if machine.is_public
            || machine.supply_mode.is_boundary_declaration()
            || matches!(
                machine.supply_mode,
                psi_language_semantics::MachineSupplyMode::ExternalRealization { .. }
            )
            || compilation.selected_build_machine_symbol() == Some(machine.symbol)
        {
            let identity = policy_callable_identity(compilation, machine.symbol)?;
            if !callables
                .callables()
                .iter()
                .any(|callable| *callable.identity() == identity)
            {
                return Err(super::rejected(
                    "public semantic dependency has no normalized callable owner",
                ));
            }
            PackagePolicySemanticDependencyConsumer::Callable(identity)
        } else {
            if fact.exposure
                != psi_checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation
            {
                return Err(super::rejected(
                    "private semantic consumer has public interface exposure",
                ));
            }
            PackagePolicySemanticDependencyConsumer::PackageImplementation
        };
        let dependency = if fact.kind
            == psi_checked_trees::CheckedSemanticDependencyKind::AutomaticCleanupMachine
        {
            // The checked dependency owner selected this exact attached drop
            // declaration; its overload coordinate is source semantics, not a
            // private intermediate state or generated call-site label.
            policy_callable_identity(compilation, fact.dependency)?
        } else {
            nominal_identity(compilation, fact.dependency)?
        };
        rows.push(PackagePolicySemanticDependency {
            consumer,
            dependency,
            exposure: match fact.exposure {
                psi_checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation => {
                    PackageReviewSemanticDependencyExposure::PrivateImplementation
                }
                psi_checked_trees::CheckedSemanticDependencyExposure::PublicInterface => {
                    PackageReviewSemanticDependencyExposure::PublicInterface
                }
            },
            kind: match fact.kind {
                psi_checked_trees::CheckedSemanticDependencyKind::NominalIdentity => {
                    PackageReviewSemanticDependencyKind::NominalIdentity
                }
                psi_checked_trees::CheckedSemanticDependencyKind::Layout => {
                    PackageReviewSemanticDependencyKind::Layout
                }
                psi_checked_trees::CheckedSemanticDependencyKind::OwnershipBehavior => {
                    PackageReviewSemanticDependencyKind::OwnershipBehavior
                }
                psi_checked_trees::CheckedSemanticDependencyKind::AutomaticCleanup => {
                    PackageReviewSemanticDependencyKind::AutomaticCleanup
                }
                psi_checked_trees::CheckedSemanticDependencyKind::AutomaticCleanupMachine => {
                    PackageReviewSemanticDependencyKind::AutomaticCleanupMachine
                }
            },
        });
    }
    rows.sort();
    rows.dedup();
    Ok(rows)
}
