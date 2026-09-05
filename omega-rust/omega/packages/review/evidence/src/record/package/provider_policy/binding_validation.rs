//! Closed binding ownership and mechanism associations.

use super::*;
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackageReviewForeignLocator, PackageReviewNominalOwner,
};
use omega_target::TargetProfile;

impl PackagePolicyProviderBinding {
    pub(super) fn validate_canonical_structure(
        &self,
        target: TargetProfile,
        row: &PackagePolicyProviderRow,
    ) -> Result<(), &'static str> {
        if row.compiler_intrinsic_execution.is_some()
            && !matches!(self, Self::CompilerIntrinsic { .. })
        {
            return Err("provider intrinsic execution is attached to another mechanism");
        }
        match self {
            Self::Import {
                target: binding_target,
                producer,
                locator,
            } => {
                if binding_target != target.identity().as_str() {
                    return Err("provider import changes selected target");
                }
                validate_producer(producer)?;
                validate_locator(locator, target)?;
            }
            Self::StringBackedImportBootstrap { library, symbol } => {
                if library.is_empty() || symbol.is_empty() {
                    return Err("provider import has an empty locator");
                }
            }
            Self::Syscall { number, evaluated } => {
                if u32::try_from(*number).is_err() {
                    return Err("provider syscall number does not fit its carrier");
                }
                if let Some(evaluated) = evaluated {
                    if evaluated.target != target.identity().as_str()
                        || !matches!(target, TargetProfile::LinuxArm64 | TargetProfile::LinuxX64)
                        || u32::try_from(*number).is_err()
                    {
                        return Err("evaluated provider syscall has an invalid target or number");
                    }
                    validate_producer(&evaluated.producer)?;
                }
            }
            Self::CompilerIntrinsic { machine } => {
                if machine.is_empty() {
                    return Err("provider intrinsic lacks its overload identity");
                }
            }
            Self::VtableSlot { index } => {
                if *index < 0 {
                    return Err("provider vtable slot is negative");
                }
            }
            Self::VtableField {
                table,
                field,
                table_declaration,
            }
            | Self::TableFunction {
                table,
                field,
                table_declaration,
            } => {
                super::validation::nominal(table_declaration)?;
                if table.is_empty() || field.is_empty() {
                    return Err("provider table binding has an empty field coordinate");
                }
            }
            Self::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => {
                if machine_identity.is_empty()
                    || !matches_owner(row.realization.owner, *machine_package_identity)
                {
                    return Err("provider adapter has inconsistent exact ownership");
                }
            }
        }
        Ok(())
    }
}

// These are the allocation-free physical constraints of omega-target's
// normalize_foreign_locator. Recovery cannot clone a candidate to invoke its
// owned constructor outside the shared reader allocation budget.
fn validate_locator(
    locator: &PackageReviewForeignLocator,
    target: TargetProfile,
) -> Result<(), &'static str> {
    let valid = |bytes: &[u8]| !bytes.is_empty() && !bytes.contains(&0);
    let supported = match locator {
        PackageReviewForeignLocator::PeByName { library, export } => {
            target == TargetProfile::WindowsX64 && valid(library) && valid(export)
        }
        PackageReviewForeignLocator::PeByOrdinal { library, ordinal } => {
            target == TargetProfile::WindowsX64 && valid(library) && *ordinal != 0
        }
        PackageReviewForeignLocator::ElfVersioned {
            object,
            symbol,
            version,
        } => {
            matches!(target, TargetProfile::LinuxArm64 | TargetProfile::LinuxX64)
                && valid(object)
                && valid(symbol)
                && valid(version)
        }
        PackageReviewForeignLocator::MachODylibSymbol {
            install_name,
            symbol,
        } => target == TargetProfile::MacosArm64 && valid(install_name) && valid(symbol),
    };
    if supported {
        Ok(())
    } else {
        Err("provider locator has an invalid target or byte coordinate")
    }
}

fn validate_producer(producer: &PackagePolicyEvaluatedBindingProducer) -> Result<(), &'static str> {
    super::validation::nominal(&producer.declaration)?;
    if producer.callable_identity.is_empty()
        || !matches_owner(producer.declaration.owner, producer.package)
    {
        return Err("evaluated provider binding has inconsistent producer ownership");
    }
    Ok(())
}

pub(super) fn matches_owner(
    owner: PackageReviewNominalOwner,
    package: Option<psi_core::PackageKeyIdentity>,
) -> bool {
    match (owner, package) {
        (PackageReviewNominalOwner::Package(actual), Some(expected)) => actual == expected,
        (PackageReviewNominalOwner::ToolchainSource(_), None) => true,
        _ => false,
    }
}
