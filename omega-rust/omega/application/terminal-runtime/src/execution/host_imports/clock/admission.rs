//! Select exact foreign implementations before validating declaration and ABI.
//! Kernel syscall numbers are platform data (Linux syscall_64.tbl and
//! asm-generic/unistd.h), never derived from requirement or provider names.
//! Scalar carriers and thin loan pointers follow the authored parameter order;
//! storage permission is checked separately from native pointer representation.

use calling_conventions::{CallSignature, ValueShape};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use target::TargetProfile;
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::TerminalImportSignature;
use terminal_psi::{BoundaryMachineDeclaration, BoundaryParameterKind};

use super::{ClockImplementation, ClockNamespace, ClockOperation, TerminalRuntimeError, timespec};

pub(super) fn select(
    binding: &NormalizedForeignCallBinding,
) -> Result<Option<(ClockNamespace, ClockImplementation)>, TerminalRuntimeError> {
    if let Some(system_call) = binding.system_call() {
        let implementation = match (system_call.target, system_call.number) {
            (TargetProfile::LinuxX64, 228) | (TargetProfile::LinuxArm64, 113) => {
                ClockImplementation::ReadTimespec
            }
            (TargetProfile::LinuxX64, 7) => ClockImplementation::Poll,
            (TargetProfile::LinuxX64, 35) | (TargetProfile::LinuxArm64, 101) => {
                ClockImplementation::Nanosleep
            }
            _ => return Ok(None),
        };
        return Ok(Some((ClockNamespace::Linux, implementation)));
    }
    let Some((locator, _)) = binding.import() else {
        return Ok(None);
    };
    let target = locator.target();
    if !matches!(target, TargetProfile::MacosArm64 | TargetProfile::MacosX64) {
        return Ok(None);
    }
    for (symbol, implementation) in [
        (
            b"_clock_gettime_nsec_np".as_slice(),
            ClockImplementation::Read,
        ),
        (
            b"_clock_gettime".as_slice(),
            ClockImplementation::ReadTimespec,
        ),
        (b"_poll".as_slice(), ClockImplementation::Poll),
        (b"_nanosleep".as_slice(), ClockImplementation::Nanosleep),
    ] {
        let offered = target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                symbol: symbol.to_vec(),
            },
            target,
        )
        .map_err(|_| {
            TerminalRuntimeError::InvalidProvision("clock implementation has an invalid locator")
        })?;
        if offered == *locator {
            return Ok(Some((ClockNamespace::Darwin, implementation)));
        }
    }
    Ok(None)
}

pub(super) fn validate(
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    signature: &TerminalImportSignature,
    implementation: ClockImplementation,
    namespace: ClockNamespace,
    storage_layouts: &[NativeBorrowedStorageLayout],
) -> Result<ClockOperation, TerminalRuntimeError> {
    let invalid = || {
        TerminalRuntimeError::InvalidProvision(
            "clock adapter differs from its exact admitted declaration/ABI",
        )
    };
    let signed = IntegerType::new(IntegerSign::Signed, 32).map_err(|_| invalid())?;
    let unsigned = IntegerType::new(IntegerSign::Unsigned, 32).map_err(|_| invalid())?;
    let wide = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
    let signed_wide = IntegerType::new(IntegerSign::Signed, 64).map_err(|_| invalid())?;
    let scalar = ScalarType::Integer;
    // clockid_t occupies one 32-bit native word. Its declared signedness is
    // retained in the logical signature, not mistaken for a different ABI.
    // Both reading operations normalize that exact word in the common owner;
    // wider integers and address carriers are not equivalent declarations.
    let selector_valid = matches!(signature.scalar_parameters.as_slice(),
        [ScalarType::Integer(integer)] if integer.bits() == 32 && !integer.is_address());
    // The kernel returns signed-long status; the actual std declarations either
    // retain i64, retain i32 (poll's C-style status), or discard it as Unit.
    // All modeled statuses fit either signed carrier. Preserve that declaration
    // and its evaluated plan rather than creating a Unit-caller fallback route.
    let status_valid = signature.result.is_unit()
        || signature.result.scalar() == Some(scalar(signed))
        || (matches!(namespace, ClockNamespace::Linux)
            && signature.result.scalar() == Some(scalar(signed_wide)));
    let (shape_valid, parameter_order) = match implementation {
        ClockImplementation::Read => (
            selector_valid
                && signature.result.scalar() == Some(scalar(wide))
                && signature.structural_parameters.is_empty(),
            vec![BoundaryParameterKind::Scalar],
        ),
        ClockImplementation::ReadTimespec => (
            selector_valid && status_valid,
            vec![
                BoundaryParameterKind::Scalar,
                BoundaryParameterKind::Structural,
            ],
        ),
        ClockImplementation::Nanosleep => (
            signature.scalar_parameters == [scalar(wide)] && status_valid,
            vec![
                BoundaryParameterKind::Structural,
                BoundaryParameterKind::Scalar,
            ],
        ),
        ClockImplementation::Poll => (
            matches!(signature.scalar_parameters.as_slice(), [pointer, count, timeout]
                if *pointer == scalar(wide)
                    && (*count == scalar(unsigned) || *count == scalar(wide))
                    && *timeout == scalar(signed))
                && status_valid
                && signature.structural_parameters.is_empty(),
            vec![BoundaryParameterKind::Scalar; 3],
        ),
    };
    if !shape_valid || declaration.parameter_order != parameter_order {
        return Err(invalid());
    }
    let operation = match implementation {
        ClockImplementation::Read => ClockOperation::Read,
        ClockImplementation::Poll => ClockOperation::Poll,
        ClockImplementation::ReadTimespec => ClockOperation::ReadTimespec(timespec::admit(
            signature,
            storage_layouts,
            implementation,
        )?),
        ClockImplementation::Nanosleep => {
            ClockOperation::Nanosleep(timespec::admit(signature, storage_layouts, implementation)?)
        }
    };
    let mut native_signature = CallSignature {
        parameters: Vec::new(),
        result: None,
    };
    let mut scalar_parameters = signature.scalar_parameters.iter();
    for parameter in parameter_order {
        let shape = match parameter {
            BoundaryParameterKind::Scalar => {
                let Some(ScalarType::Integer(integer)) = scalar_parameters.next() else {
                    return Err(invalid());
                };
                let bytes = integer.bits() / 8;
                ValueShape::integer(u32::from(bytes), bytes)
            }
            BoundaryParameterKind::Structural => ValueShape::integer(8, 8),
        };
        native_signature.parameters.push(shape);
    }
    native_signature.result = match signature.result.scalar() {
        Some(ScalarType::Integer(integer)) => {
            let bytes = integer.bits() / 8;
            Some(ValueShape::integer(u32::from(bytes), bytes))
        }
        _ => None,
    };
    super::super::admit_exact_foreign_call(signature, declaration, binding, &native_signature)?;
    Ok(operation)
}
