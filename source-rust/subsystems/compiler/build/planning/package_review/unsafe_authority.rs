//! Keep the checked native operations behind the service names in a fresh
//! package audit, including operations in private machines.
mod admission;
use super::input::PolicySource;
pub use admission::{ReviewedUnsafeAdmission, ReviewedUnsafePath};
use diagnostics::Diagnostic;
use language_core::core_vocabulary::UnsafeAtom;
use review_evidence::record::PackageReviewNominalIdentity;
use semantic_vocabulary::{OperationId, PackageKeyIdentity, ServiceId, TargetMachineCallable as Callable};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedUnsafeAuthority {
    pub enabled: Vec<UnsafeAtom>,
    pub uses: Vec<ReviewedUnsafeUse>,
    pub dependencies: Vec<ReviewedUnsafeDependency>,
}

/// The selected Product edge has its own authority, including when two
/// registrations select the same package with different ceilings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedUnsafeDependency {
    pub alias: String,
    pub target: PackageKeyIdentity,
    pub admits: Vec<UnsafeAtom>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewedUnsafeUse {
    pub callable: PackageReviewNominalIdentity,
    pub origins: Vec<ReviewedUnsafeOrigin>,
    pub paths: Vec<ReviewedUnsafePath>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewedUnsafeOrigin {
    NativeOperation {
        id: OperationId,
        name: &'static str,
        atoms: &'static [UnsafeAtom],
    },
    BoundaryContract {
        atoms: Vec<UnsafeAtom>,
    },
}

pub(super) struct UnsafeInventory {
    pub report: ReviewedUnsafeAuthority,
    /// Concrete services, including parents, accounted for by checked operations.
    pub services: Vec<ServiceId>,
}

impl PolicySource<'_> {
    pub(super) fn unsafe_authority(
        &self,
        inputs: &package_compilation::PackageCompilationInputs,
        selected: &compiler::SelectedProviderResult,
    ) -> Result<UnsafeInventory, Vec<Diagnostic>> {
        let package = inputs.root();
        let accepted = self.accepted();
        let module = accepted.module();
        let elaboration = accepted.closure().elaboration();
        let admissions = admission::paths(self, selected)?;
        let mut uses = Vec::new();
        let mut native_atoms = BTreeSet::new();
        let mut cited_services = Vec::new();
        for (machine, symbol) in elaboration.machines.iter() {
            let body = module
                .machine(machine)
                .ok_or_else(|| vec![Diagnostic::error("package review lost a checked machine")])?;
            let mut origins = Vec::new();
            for operation in body.operations() {
                let Some((name, atoms)) =
                    terminal_semantics::storage::native_borrows::required_authority(
                        module, operation,
                    )
                    .map_err(|detail| vec![Diagnostic::error(detail)])?
                else {
                    continue;
                };
                native_atoms.extend(atoms.iter().copied());
                origins.push(ReviewedUnsafeOrigin::NativeOperation {
                    id: operation.id,
                    name,
                    atoms,
                });
            }
            if !origins.is_empty() {
                let paths = admission::for_callable(&admissions, Callable::Machine(machine));
                if !paths.is_empty() || self.owns(*symbol, package)? {
                    uses.push(ReviewedUnsafeUse {
                        callable: self.nominal(*symbol)?,
                        origins,
                        paths,
                    });
                }
            }
        }
        let services = accepted.unsafe_services()?;
        for (boundary, symbol) in elaboration.boundaries.iter() {
            let declaration = module
                .boundary_machine(boundary)
                .ok_or_else(|| vec![Diagnostic::error("package review lost a checked boundary")])?;
            let atoms: Vec<_> = declaration
                .published_service_ceiling
                .iter()
                .filter_map(|&service| {
                    let atom = services.get(service).copied().flatten()?;
                    cited_services.push(service);
                    Some(atom)
                })
                .collect();
            if !atoms.is_empty() {
                let paths = admission::for_callable(&admissions, Callable::Boundary(boundary));
                if !paths.is_empty() || self.owns(*symbol, package)? {
                    uses.push(ReviewedUnsafeUse {
                        callable: self.nominal(*symbol)?,
                        origins: vec![ReviewedUnsafeOrigin::BoundaryContract { atoms }],
                        paths,
                    });
                }
            }
        }
        for atom in native_atoms {
            let service = module
                .core_unsafe_service(atom)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            cited_services.push(service.id);
        }
        cited_services.sort_unstable();
        cited_services.dedup();
        for index in 0..cited_services.len() {
            let service = module.service(cited_services[index]).ok_or_else(|| {
                vec![Diagnostic::error(
                    "package review lost a checked Unsafe service",
                )]
            })?;
            cited_services.extend_from_slice(&service.parents);
        }
        cited_services.sort_unstable();
        cited_services.dedup();
        let purpose = build_declarations::DependencyPurpose::Product;
        let mut dependencies = Vec::new();
        for (alias, target) in inputs.direct_dependencies(purpose) {
            let ceiling = inputs
                .dependency_max_reach(package, purpose, alias)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package audit lost an authored dependency ceiling",
                    )]
                })?;
            let mut admits = Vec::new();
            for (service, atom) in services.iter() {
                let Some(atom) = atom else { continue };
                let canonical = module
                    .service(service)
                    .and_then(|decl| module.identities.text(decl.identity))
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "package audit lost an authenticated Unsafe service",
                        )]
                    })?;
                if ceiling.admits(canonical) {
                    admits.push(*atom);
                }
            }
            admits.sort_unstable();
            admits.dedup();
            dependencies.push(ReviewedUnsafeDependency {
                alias: alias.to_owned(),
                target,
                admits,
            });
        }
        if cited_services.is_empty()
            && self.build().enabled_unsafe_atoms.is_empty()
            && dependencies.iter().all(|edge| edge.admits.is_empty())
        {
            dependencies.clear();
        }
        Ok(UnsafeInventory {
            services: cited_services,
            report: ReviewedUnsafeAuthority {
                enabled: self
                    .evaluation
                    .configuration
                    .enabled_unsafe_atoms
                    .iter()
                    .copied()
                    .collect(),
                uses,
                dependencies,
            },
        })
    }
}
