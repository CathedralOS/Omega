use super::{
    Error, PackagePolicyRecoveryLimits,
    callable_policy::callable_conformance,
    identity::{nominal, operator_coordinate, package, type_identity},
    public_api::type_parameter,
    reader::Reader,
    signatures::conformance_bound,
};
use crate::encoding::{EXTERNAL_SUPPLY_POLICY_MAGIC, PACKAGE_EXTERNAL_SUPPLY_POLICY_VERSION};
use crate::record::{
    PackagePolicyEvaluatedBindingProducer, PackagePolicyExternalBinding,
    PackagePolicyExternalCallableSignature, PackagePolicyExternalExecutableSupply,
    PackagePolicyExternalRequirement, PackageReviewExternalCallableParameter,
    PackageReviewForeignLocator,
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
        let supply = policy(&mut reader)?;
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

pub(super) fn policy(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyExternalExecutableSupply, Error> {
    Ok(PackagePolicyExternalExecutableSupply {
        callable: nominal(reader)?,
        signature: external_signature(reader)?,
        requirement: requirement(reader)?,
        binding: binding(reader)?,
    })
}

fn external_signature(
    reader: &mut Reader<'_>,
) -> Result<PackagePolicyExternalCallableSignature, Error> {
    Ok(PackagePolicyExternalCallableSignature {
        lifetime_parameter_count: reader.usize()?,
        static_parameters: reader.sequence(3, type_parameter)?,
        conformance_bounds: reader.sequence(1, conformance_bound)?,
        parameters: reader.sequence(11, |reader| {
            Ok(PackageReviewExternalCallableParameter {
                type_identity: type_identity(reader)?,
                is_const: reader.boolean()?,
                is_mutable: reader.boolean()?,
                is_self: reader.boolean()?,
            })
        })?,
        return_type: reader.option(type_identity)?,
    })
}

fn requirement(reader: &mut Reader<'_>) -> Result<PackagePolicyExternalRequirement, Error> {
    Ok(match reader.byte()? {
        0 => PackagePolicyExternalRequirement::Trait(callable_conformance(reader)?),
        1 => PackagePolicyExternalRequirement::Operator {
            coordinate: operator_coordinate(reader)?,
            alias: reader.option(Reader::string)?,
        },
        2 => PackagePolicyExternalRequirement::TopLevelRequirement {
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
    if !target::TargetProfile::ALL
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
