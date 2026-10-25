//! Locators select known C operations; exact declarations and complete ABI
//! applications determine whether their values can cross this particular slot.

use super::{AllocationOperation, TerminalRuntimeError};
use calling_conventions::{BoundaryCallingApplication, CallSignature, ValueShape};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerSign, IntegerType, ScalarType, StructuralTypeId,
};
use target::TargetProfile;
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{TerminalImportSignature, TerminalProgram};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    StructuralFieldType, StructuralTypeShape,
};

pub(super) fn select(
    binding: &NormalizedForeignCallBinding,
) -> Result<Option<AllocationOperation>, TerminalRuntimeError> {
    let Some((locator, _)) = binding.import() else {
        return Ok(None);
    };
    let target = locator.target();
    let host_matches = match target {
        TargetProfile::LinuxX64 => cfg!(all(target_os = "linux", target_arch = "x86_64")),
        TargetProfile::LinuxArm64 => cfg!(all(target_os = "linux", target_arch = "aarch64")),
        TargetProfile::MacosArm64 => cfg!(all(target_os = "macos", target_arch = "aarch64")),
        TargetProfile::MacosX64 => cfg!(all(target_os = "macos", target_arch = "x86_64")),
        _ => false,
    };
    if !host_matches {
        return Ok(None);
    }
    for (symbol, operation) in [
        ("calloc", AllocationOperation::Allocate),
        ("free", AllocationOperation::Release),
    ] {
        let candidates = match target {
            TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 => {
                vec![target::ForeignLocatorCandidate::ElfVersioned {
                    object: b"libc.so.6".to_vec(),
                    symbol: symbol.as_bytes().to_vec(),
                    version: if target == TargetProfile::LinuxX64 {
                        b"GLIBC_2.2.5".to_vec()
                    } else {
                        b"GLIBC_2.17".to_vec()
                    },
                }]
            }
            TargetProfile::MacosArm64 | TargetProfile::MacosX64 => [
                b"/usr/lib/libSystem.B.dylib".as_slice(),
                b"libSystem.B.dylib".as_slice(),
            ]
            .into_iter()
            .map(
                |library| target::ForeignLocatorCandidate::MachODylibSymbol {
                    install_name: library.to_vec(),
                    symbol: format!("_{symbol}").into_bytes(),
                },
            )
            .collect(),
            _ => return Ok(None),
        };
        for candidate in candidates {
            let offered = target::normalize_foreign_locator(candidate, target).map_err(|_| {
                TerminalRuntimeError::InvalidProvision(
                    "allocation implementation has an invalid native locator",
                )
            })?;
            if offered == *locator {
                return Ok(Some(operation));
            }
        }
    }
    Ok(None)
}

pub(super) fn validate(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    application: Option<&BoundaryCallingApplication>,
    signature: &TerminalImportSignature,
    operation: AllocationOperation,
) -> Result<Option<Vec<Path>>, TerminalRuntimeError> {
    let invalid = TerminalRuntimeError::InvalidProvision;
    let address = ScalarType::Integer(
        IntegerType::address(64)
            .map_err(|_| invalid("allocation requires a supported address carrier"))?,
    );
    let unsigned = ScalarType::Integer(
        IntegerType::new(IntegerSign::Unsigned, 64)
            .map_err(|_| invalid("allocation requires a supported size carrier"))?,
    );
    let mut payload = None;
    let native = match operation {
        AllocationOperation::Allocate => {
            if signature.scalar_parameters != [unsigned, unsigned]
                || !signature.structural_parameters.is_empty()
                || signature.result != BoundaryMachineResult::Scalar(address.into())
                || declaration.parameter_order != [BoundaryParameterKind::Scalar; 2]
            {
                return Err(invalid(
                    "calloc requires two u64 sizes and a bare address result",
                ));
            }
            CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 2],
                result: Some(ValueShape::integer(8, 8)),
            }
        }
        AllocationOperation::Release => {
            if !signature.result.is_unit() {
                return Err(invalid("free must return Unit"));
            }
            if signature.scalar_parameters == [address]
                && signature.structural_parameters.is_empty()
                && declaration.parameter_order == [BoundaryParameterKind::Scalar]
            {
                // An ordinary raw address has no structural payload lane.
            } else {
                let [parameter] = signature.structural_parameters.as_slice() else {
                    return Err(invalid(
                        "free requires exactly one address-bearing owned value",
                    ));
                };
                if !signature.scalar_parameters.is_empty()
                    || declaration.parameter_order != [BoundaryParameterKind::Structural]
                    || parameter.access != StructuralAccess::Owned
                {
                    return Err(invalid(
                        "free cannot consume a borrowed or multi-argument value",
                    ));
                }
                let mut path = Vec::new();
                address_payload(
                    program,
                    parameter.structural_type,
                    address,
                    &mut Vec::new(),
                    &mut path,
                )?;
                let application = application.ok_or(invalid(
                    "owned free requires the complete admitted calling application",
                ))?;
                if application.signature.parameters.len() != 1
                    || application.signature.result.is_some()
                {
                    return Err(invalid(
                        "owned free calling application has a different argument/result roster",
                    ));
                }
                let envelope = super::super::native_word::native_word_envelope(
                    &application.signature,
                    application.signature.parameters[0],
                )
                .map_err(invalid)?;
                if envelope.byte_size != 8 || envelope.alignment != 8 {
                    return Err(invalid(
                        "free native argument must occupy one aligned pointer word",
                    ));
                }
                payload = Some(path);
            }
            CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            }
        }
    };
    super::super::admit_exact_foreign_call(signature, declaration, binding, &native)?;
    if let Some(application) = application {
        if application.plan.plan() != &binding.boundary_entry_plan
            || application.commitment == [0; 32]
            || !application.opaque_applications.is_empty()
        {
            return Err(invalid(
                "allocation calling application is unbound or contains opaque/callback transport",
            ));
        }
    }
    Ok(payload)
}

fn address_payload(
    program: &TerminalProgram,
    root: StructuralTypeId,
    address: ScalarType,
    visiting: &mut Vec<StructuralTypeId>,
    path: &mut Vec<Path>,
) -> Result<(), TerminalRuntimeError> {
    let invalid = TerminalRuntimeError::InvalidProvision;
    if visiting.contains(&root) {
        return Err(invalid("free address payload contains a recursive value"));
    }
    visiting.push(root);
    let declaration = program
        .structural_type(root)
        .ok_or(invalid("free address payload type is absent"))?;
    match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(scalar) if *scalar == address => {}
        StructuralTypeShape::Record { fields, .. } => {
            let relevant: Vec<_> = fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .collect();
            let [field] = relevant.as_slice() else {
                return Err(invalid(
                    "free owned value must contain exactly one address payload",
                ));
            };
            if field.storage.is_some() || matches!(field.field_type, (StructuralFieldType::Atomic(_) | StructuralFieldType::AtomicBoundedInteger(_)))
            {
                return Err(invalid(
                    "free address payload cannot use atomic or encoded storage",
                ));
            }
            path.push(Path::Field(field.id));
            if let Some(child) = field.field_type.as_structural() {
                address_payload(program, child, address, visiting, path)?;
            } else if field.field_type.scalar_type() != Some(address) {
                return Err(invalid("free owned payload is not an address"));
            }
        }
        _ => return Err(invalid("free owned value is not a public address payload")),
    }
    visiting.pop();
    Ok(())
}
