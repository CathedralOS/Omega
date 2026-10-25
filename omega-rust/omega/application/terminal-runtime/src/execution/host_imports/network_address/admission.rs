//! Admit complete native contracts before invoking the trusted inet functions.
//! Nested one-field records are traversed as a public shape graph: accepting
//! any equal-sized object would also accept float, array and shifted carriers.

use calling_conventions::{
    BoundaryCallingApplication, BoundaryValueClass, BoundaryValueGraph, CallSignature, ValueShape,
};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, StructuralTypeId};
use target::TargetProfile;
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::TerminalImportSignature;
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    StructuralMultiplicity,
};

use super::{AddressOperation, TerminalRuntimeError};

pub(super) fn select(
    binding: &NormalizedForeignCallBinding,
) -> Result<Option<AddressOperation>, TerminalRuntimeError> {
    let Some((locator, _)) = binding.import() else {
        return Ok(None);
    };
    let target = locator.target();
    // An exact target's runtime supply cannot be borrowed from another host.
    let host = match target {
        TargetProfile::LinuxX64 => cfg!(all(target_os = "linux", target_arch = "x86_64")),
        TargetProfile::LinuxArm64 => cfg!(all(target_os = "linux", target_arch = "aarch64")),
        TargetProfile::MacosArm64 => cfg!(all(target_os = "macos", target_arch = "aarch64")),
        TargetProfile::MacosX64 => cfg!(all(target_os = "macos", target_arch = "x86_64")),
        _ => false,
    };
    if !host {
        return Ok(None);
    }
    for (symbol, operation) in [
        ("inet_makeaddr", AddressOperation::Make),
        ("inet_lnaof", AddressOperation::Host),
    ] {
        let candidate = match target {
            TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 => {
                target::ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: symbol.as_bytes().to_vec(),
                    version: if target == TargetProfile::LinuxX64 {
                        b"GLIBC_2.2.5".to_vec()
                    } else {
                        b"GLIBC_2.17".to_vec()
                    },
                }
            }
            TargetProfile::MacosArm64 | TargetProfile::MacosX64 => {
                target::ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                    symbol: format!("_{symbol}").into_bytes(),
                }
            }
            _ => return Ok(None),
        };
        let offered = target::normalize_foreign_locator(candidate, target).map_err(|_| {
            TerminalRuntimeError::InvalidProvision("inet implementation has an invalid locator")
        })?;
        if offered == *locator {
            return Ok(Some(operation));
        }
    }
    Ok(None)
}

pub(super) fn validate(
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    application: Option<&BoundaryCallingApplication>,
    signature: &TerminalImportSignature,
    operation: AddressOperation,
) -> Result<(StructuralTypeId, [u8; 32]), TerminalRuntimeError> {
    let invalid = TerminalRuntimeError::InvalidProvision;
    let application = application.ok_or(TerminalRuntimeError::InvalidProvision(
        "inet adapter requires the complete admitted opaque calling application",
    ))?;
    let word = IntegerType::new(IntegerSign::Unsigned, 32)
        .map_err(|_| invalid("inet native u32 type is not admitted"))?;
    let scalar = ScalarType::Integer(word);
    let (structural_type, root) = match operation {
        AddressOperation::Make => {
            let BoundaryMachineResult::Structural(result) = &signature.result else {
                return Err(invalid("inet_makeaddr requires one structural result"));
            };
            if signature.scalar_parameters != [scalar, scalar]
                || !signature.structural_parameters.is_empty()
                || result.multiplicity != StructuralMultiplicity::Affine
                || !result.qualifications.is_empty()
                || !result.projected_qualifications.is_empty()
                || declaration.parameter_order != [BoundaryParameterKind::Scalar; 2]
                || application.signature.parameters.len() != 2
            {
                return Err(invalid(
                    "inet_makeaddr requires two u32 scalars and an unqualified affine result",
                ));
            }
            for root in &application.signature.parameters {
                let shape = application
                    .signature
                    .shapes
                    .get(usize::from(*root))
                    .ok_or_else(|| invalid("inet native signature references an absent shape"))?;
                if shape.class != BoundaryValueClass::Integer
                    || shape.byte_size != 4
                    || shape.alignment != 4
                {
                    return Err(invalid(
                        "inet_makeaddr native inputs must be aligned four-byte integers",
                    ));
                }
            }
            (
                result.structural_type,
                application
                    .signature
                    .result
                    .ok_or_else(|| invalid("inet_makeaddr native result shape is absent"))?,
            )
        }
        AddressOperation::Host => {
            let [parameter] = signature.structural_parameters.as_slice() else {
                return Err(invalid(
                    "inet_lnaof requires exactly one structural argument",
                ));
            };
            if !signature.scalar_parameters.is_empty()
                || signature.result != BoundaryMachineResult::Scalar(scalar.into())
                || parameter.multiplicity != StructuralMultiplicity::Affine
                || parameter.access != StructuralAccess::Owned
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
                || declaration.parameter_order != [BoundaryParameterKind::Structural]
            {
                return Err(invalid(
                    "inet_lnaof requires one unqualified owned affine argument and a u32 result",
                ));
            }
            let [root] = application.signature.parameters.as_slice() else {
                return Err(invalid(
                    "inet_lnaof native signature requires exactly one argument",
                ));
            };
            let result = application
                .signature
                .result
                .and_then(|root| application.signature.shapes.get(usize::from(root)))
                .ok_or_else(|| invalid("inet native signature references an absent shape"))?;
            if result.class != BoundaryValueClass::Integer
                || result.byte_size != 4
                || result.alignment != 4
            {
                return Err(invalid(
                    "inet_lnaof native result must be an aligned four-byte integer",
                ));
            }
            (parameter.structural_type, *root)
        }
    };
    let envelope = super::super::native_word::native_word_envelope(&application.signature, root)
        .map_err(invalid)?;
    if envelope.byte_size != 4 || envelope.alignment != 4 || !envelope.contains_record {
        return Err(invalid(
            "inet carrier must be a four-byte aligned C record ending in u32",
        ));
    }
    if application.opaque_applications.len() != 1
        || application.opaque_applications[0].shape_root != root
        || application.opaque_applications[0].selected_application_commitment == [0; 32]
        || application.opaque_applications[0].conformance_application_commitment == [0; 32]
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "inet selected opaque application differs from its exact signature root",
        ));
    }
    let graph = BoundaryValueGraph {
        shapes: &application.signature.shapes,
        fields: &application.signature.fields,
    };
    let policy = application.plan.plan().call.policy;
    let native_signature = CallSignature {
        parameters: application
            .signature
            .parameters
            .iter()
            .map(|root| graph.classify(*root, policy))
            .collect::<Result<Vec<ValueShape>, _>>()
            .map_err(|_| {
                invalid("inet native shape cannot be classified under the selected calling policy")
            })?,
        result: application
            .signature
            .result
            .map(|root| graph.classify(root, policy))
            .transpose()
            .map_err(|_| {
                invalid("inet native shape cannot be classified under the selected calling policy")
            })?,
    };
    super::super::admit_exact_foreign_call(signature, declaration, binding, &native_signature)?;
    Ok((
        structural_type,
        application.opaque_applications[0].selected_application_commitment,
    ))
}
