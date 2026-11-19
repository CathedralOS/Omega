// Closed binding ownership, foreign locator physical validation, and mechanism associations.
use crate::record::PackagePolicyProviderBinding;
use crate::record::PackagePolicyProviderRow;
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackageReviewForeignLocator, PackageReviewNominalOwner,
};
use target::TargetProfile;

impl PackagePolicyProviderBinding {
    pub(crate) fn validate_native(&self, target: TargetProfile) -> Result<(), &'static str> {
        match self {
            Self::Import {
                target: selected,
                producer,
                locator,
            } if selected == target.identity().as_str() => {
                validate_producer(producer)?;
                validate_locator(locator, target)
            }
            Self::Syscall {
                number,
                evaluated: Some(evaluated),
            } if evaluated.target == target.identity().as_str()
                && matches!(target, TargetProfile::LinuxArm64 | TargetProfile::LinuxX64)
                && u32::try_from(*number).is_ok() =>
            {
                validate_producer(&evaluated.producer)
            }
            _ => Err("native inventory has an invalid target, coordinate or producer"),
        }
    }
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
                storage,
            }
            | Self::TableFunction {
                table,
                field,
                table_declaration,
                storage,
            } => {
                table_declaration.validate_resolved()?;
                if table.is_empty() || field.is_empty() {
                    return Err("provider table binding has an empty field coordinate");
                }
                match storage {
                    Ok(storage)
                        if storage.table_application.is_empty()
                            || storage.field_declaration.is_empty() =>
                    {
                        return Err(
                            "provider table binding lost its checked storage correspondence",
                        );
                    }
                    Err(reason) if reason.is_empty() => {
                        return Err("provider table binding lost its deferred refusal");
                    }
                    _ => {}
                }
            }
            Self::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => {
                if machine_identity.is_empty()
                    || !row.realization.as_ref().is_some_and(|realization| {
                        matches_owner(realization.owner, *machine_package_identity)
                    })
                {
                    return Err("provider adapter has inconsistent exact ownership");
                }
            }
        }
        Ok(())
    }
}

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
        } => {
            matches!(target, TargetProfile::MacosArm64 | TargetProfile::MacosX64)
                && valid(install_name)
                && valid(symbol)
        }
    };
    if supported {
        Ok(())
    } else {
        Err("provider locator has an invalid target or byte coordinate")
    }
}

fn validate_producer(producer: &PackagePolicyEvaluatedBindingProducer) -> Result<(), &'static str> {
    producer.declaration.validate_resolved()?;
    if producer.callable_identity.is_empty()
        || !matches_owner(producer.declaration.owner, producer.package)
    {
        return Err("evaluated provider binding has inconsistent producer ownership");
    }
    Ok(())
}

pub(super) fn matches_owner(
    owner: PackageReviewNominalOwner,
    package: Option<semantic_vocabulary::PackageKeyIdentity>,
) -> bool {
    match (owner, package) {
        (PackageReviewNominalOwner::Package(actual), Some(expected)) => actual == expected,
        (PackageReviewNominalOwner::ToolchainSource(_), None) => true,
        _ => false,
    }
}
