use super::{
    Error, PackagePolicyRecoveryLimits,
    identity::{nominal, operator_coordinate, package, type_identity},
    reader::Reader,
    signatures::external_signature,
};
use crate::encoding::{EXTERNAL_SUPPLY_POLICY_MAGIC, PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION};
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalExecutableSupply, PackageReviewCallableConformance,
    PackageReviewExternalRequirement, PackageReviewForeignLocator,
};

impl PackagePolicyExternalExecutableSupply {
    /// Recover the complete inert component, including recursive contracts.
    /// This validates the bounded record vocabulary, not the truth of authored
    /// policy, an old compiler execution, or permission to accept a candidate.
    pub fn recover_canonical(
        bytes: &[u8],
        limits: PackagePolicyRecoveryLimits,
    ) -> Result<Self, Error> {
        let mut reader = Reader::new(bytes, limits)?;
        reader.literal(EXTERNAL_SUPPLY_POLICY_MAGIC)?;
        if reader.u16()? != PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION {
            return Err(Error::UnsupportedVersion);
        }
        let supply = Self {
            callable: nominal(&mut reader)?,
            signature: external_signature(&mut reader)?,
            requirement: requirement(&mut reader)?,
            binding: binding(&mut reader)?,
        };
        reader.finish()?;
        reader.canonical_scratch(bytes.len())?;
        // The ordinary encoder also checks cross-field representation rules,
        // such as complete selected conformance applications. This is a format
        // check, never a reconstruction of compiler or acceptance evidence.
        if supply
            .canonical_bytes()
            .map_err(|_| Error::NonCanonicalEncoding)?
            != bytes
        {
            return Err(Error::NonCanonicalEncoding);
        }
        Ok(supply)
    }
}

fn requirement(reader: &mut Reader<'_>) -> Result<PackageReviewExternalRequirement, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewExternalRequirement::Trait(PackageReviewCallableConformance {
            trait_identity: nominal(reader)?,
            requirement_identity: nominal(reader)?,
            requirement_lifetime_partition: reader.sequence(4, Reader::u32)?,
            arguments: reader.sequence(8, type_identity)?,
            alias: reader.option(Reader::string)?,
        }),
        1 => PackageReviewExternalRequirement::Operator {
            coordinate: operator_coordinate(reader)?,
            alias: reader.option(Reader::string)?,
        },
        2 => PackageReviewExternalRequirement::TopLevelRequirement {
            identity: nominal(reader)?,
            signature: external_signature(reader)?,
            alias: reader.option(Reader::string)?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

fn binding(reader: &mut Reader<'_>) -> Result<PackagePolicyExternalBinding, Error> {
    Ok(match reader.byte()? {
        0 => PackagePolicyExternalBinding::Import {
            library: reader.string()?,
            symbol: reader.string()?,
        },
        1 => PackagePolicyExternalBinding::Syscall {
            number: reader.i64()?,
        },
        2 => PackagePolicyExternalBinding::CompilerIntrinsic,
        3 => PackagePolicyExternalBinding::VtableSlot {
            index: reader.i64()?,
        },
        4 => PackagePolicyExternalBinding::VtableField {
            field: reader.string()?,
        },
        5 => PackagePolicyExternalBinding::TableFunction {
            field: reader.string()?,
        },
        6 => PackagePolicyExternalBinding::NormalizedImport {
            target: target(reader)?,
            locator: locator(reader)?,
            producer: producer(reader)?,
        },
        7 => PackagePolicyExternalBinding::NormalizedSyscall {
            target: target(reader)?,
            number: reader.i64()?,
            producer: producer(reader)?,
        },
        _ => return Err(Error::InvalidTag),
    })
}

fn target(reader: &mut Reader<'_>) -> Result<String, Error> {
    let name = reader.string()?;
    if !omega_target::TargetProfile::ALL
        .iter()
        .any(|profile| profile.identity().as_str() == name)
    {
        return Err(Error::InvalidValue);
    }
    Ok(name)
}

pub(super) fn producer(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyEvaluatedBindingProducer, Error> {
    Ok(PackagePolicyEvaluatedBindingProducer {
        declaration: nominal(reader)?,
        package: reader.option(package)?,
        callable_identity: reader.string()?,
    })
}

pub(super) fn locator(reader: &mut Reader<'_>) -> Result<PackageReviewForeignLocator, Error> {
    Ok(match reader.byte()? {
        0 => PackageReviewForeignLocator::PeByName {
            library: reader.bytes()?,
            export: reader.bytes()?,
        },
        1 => PackageReviewForeignLocator::PeByOrdinal {
            library: reader.bytes()?,
            ordinal: reader.u16()?,
        },
        2 => PackageReviewForeignLocator::ElfVersioned {
            object: reader.bytes()?,
            symbol: reader.bytes()?,
            version: reader.bytes()?,
        },
        3 => PackageReviewForeignLocator::MachODylibSymbol {
            install_name: reader.bytes()?,
            symbol: reader.bytes()?,
        },
        _ => return Err(Error::InvalidTag),
    })
}
