//! An exact admitted C abs call over one signed int word. Owned record
//! envelopes retain their typed payload and ordinary boundary claim transfer.

use super::super::TerminalRuntimeError;
use calling_conventions::{BoundaryCallingApplication, CallSignature, ValueShape};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Path, IntegerSign, IntegerType, IntegerValue, ScalarType,
    StructuralTypeId,
};
use target::{ForeignLocatorCandidate, TargetProfile};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalProgram, TerminalScalarValue, TerminalStorageContents,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
};

pub(super) struct IntegerAbsoluteImport {
    signature: TerminalImportSignature,
    payload: Option<(StructuralTypeId, Vec<Path>)>,
}

impl IntegerAbsoluteImport {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        application: Option<&BoundaryCallingApplication>,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        if !matches_locator(binding)? {
            return Ok(None);
        }
        let invalid = TerminalRuntimeError::InvalidProvision;
        let integer = IntegerType::new(IntegerSign::Signed, 32)
            .map_err(|_| invalid("abs requires the exact signed C int type"))?;
        let scalar = ScalarType::Integer(integer);
        let signature = TerminalImportSignature::from(declaration);
        let result = match signature.result {
            BoundaryMachineResult::Unit => None,
            BoundaryMachineResult::Scalar(ref result)
                if result.scalar_type == scalar
                    && result.qualifications
                        == semantic_vocabulary::ScalarQualificationSetId::ZERO =>
            {
                Some(ValueShape::integer(4, 4))
            }
            _ => {
                return Err(invalid(
                    "abs requires a plain i32 or discarded native result",
                ));
            }
        };
        let application = application.ok_or(invalid(
            "abs requires its complete evaluated calling application",
        ))?;
        let [root] = application.signature.parameters.as_slice() else {
            return Err(invalid("abs requires exactly one native argument"));
        };
        let payload = if signature.scalar_parameters == [scalar]
            && signature.structural_parameters.is_empty()
            && declaration.parameter_order == [BoundaryParameterKind::Scalar]
        {
            None
        } else {
            let [parameter] = signature.structural_parameters.as_slice() else {
                return Err(invalid("abs requires exactly one signed int carrier"));
            };
            if !signature.scalar_parameters.is_empty()
                || declaration.parameter_order != [BoundaryParameterKind::Structural]
                || parameter.access != StructuralAccess::Owned
                || !parameter.qualifications.is_empty()
                || !parameter.projected_qualifications.is_empty()
            {
                return Err(invalid(
                    "abs cannot borrow or supply membership for its owned int carrier",
                ));
            }
            Some((
                parameter.structural_type,
                super::native_word::scalar_payload_path(
                    program,
                    parameter.structural_type,
                    scalar,
                )?,
            ))
        };
        let envelope = super::native_word::native_word_envelope(&application.signature, *root)
            .map_err(invalid)?;
        if envelope.byte_size != 4 || envelope.alignment != 4 {
            return Err(invalid("abs native argument is not one aligned C int word"));
        }
        if application.plan.plan() != &binding.boundary_entry_plan
            || application.commitment == [0; 32]
            || !application.opaque_applications.is_empty()
            || binding
                .import()
                .is_none_or(|(locator, _)| application.target != locator.target().native_target())
        {
            return Err(invalid(
                "abs calling application differs from its admitted target and plan",
            ));
        }
        let native = CallSignature {
            parameters: vec![ValueShape::integer(4, 4)],
            result,
        };
        super::admit_exact_foreign_call(&signature, declaration, binding, &native)?;
        Ok(Some(Self { signature, payload }))
    }
}

fn matches_locator(binding: &NormalizedForeignCallBinding) -> Result<bool, TerminalRuntimeError> {
    let Some((locator, _)) = binding.import() else {
        return Ok(false);
    };
    let target = locator.target();
    let candidates = match target {
        TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 if cfg!(target_os = "linux") => {
            let host_arch = if target == TargetProfile::LinuxX64 {
                cfg!(target_arch = "x86_64")
            } else {
                cfg!(target_arch = "aarch64")
            };
            if !host_arch {
                return Ok(false);
            }
            vec![ForeignLocatorCandidate::ElfVersioned {
                object: b"libc.so.6".to_vec(),
                symbol: b"abs".to_vec(),
                version: if target == TargetProfile::LinuxX64 {
                    b"GLIBC_2.2.5".to_vec()
                } else {
                    b"GLIBC_2.17".to_vec()
                },
            }]
        }
        TargetProfile::MacosArm64 | TargetProfile::MacosX64 if cfg!(target_os = "macos") => {
            let host_arch = if target == TargetProfile::MacosArm64 {
                cfg!(target_arch = "aarch64")
            } else {
                cfg!(target_arch = "x86_64")
            };
            if !host_arch {
                return Ok(false);
            }
            [
                b"/usr/lib/libSystem.B.dylib".as_slice(),
                b"libSystem.B.dylib".as_slice(),
            ]
            .into_iter()
            .map(|library| ForeignLocatorCandidate::MachODylibSymbol {
                install_name: library.to_vec(),
                symbol: b"_abs".to_vec(),
            })
            .collect()
        }
        TargetProfile::WindowsX64 if cfg!(all(target_os = "windows", target_arch = "x86_64")) => {
            vec![ForeignLocatorCandidate::PeByName {
                library: b"msvcrt.dll".to_vec(),
                export: b"abs".to_vec(),
            }]
        }
        _ => return Ok(false),
    };
    for candidate in candidates {
        let expected = target::normalize_foreign_locator(candidate, target)
            .map_err(|_| TerminalRuntimeError::InvalidProvision("abs host locator is invalid"))?;
        if expected == *locator {
            return Ok(true);
        }
    }
    Ok(false)
}

import_signature_via_clone!(IntegerAbsoluteImport);

impl TerminalEffectHandler for IntegerAbsoluteImport {
    fn storage_arguments(&self, _: semantic_vocabulary::BoundaryMachineId) -> &[usize] {
        if self.payload.is_some() { &[0] } else { &[] }
    }

    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "abs requires exact typed result dispatch",
        ))
    }

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid =
            || TerminalEffectRejection::new("abs operands differ from its admitted signature");
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            result,
            ..
        } = effect
        else {
            return Err(invalid());
        };
        if !byte_buffers.is_empty() || *result != self.signature.result {
            return Err(TerminalEffectRejection::new(
                "abs has unexpected byte buffers or result contract",
            ));
        }
        let integer = IntegerType::new(IntegerSign::Signed, 32).map_err(|_| invalid())?;
        let value = if let Some((ty, path)) = &self.payload {
            let [argument] = structural_arguments.as_slice() else {
                return Err(invalid());
            };
            let [TerminalBoundaryStorageBuffer::Structural(buffer)] = storage else {
                return Err(TerminalEffectRejection::new(
                    "abs requires one structural storage buffer",
                ));
            };
            let [cell] = buffer.values() else {
                return Err(TerminalEffectRejection::new(format!(
                    "abs expected one scalar cell, got {}",
                    buffer.values().len()
                )));
            };
            if !arguments.is_empty()
                || buffer.argument_index() != 0
                || buffer.access() != StructuralAccess::Owned
                || buffer.structural_type() != *ty
                || argument.structural_type != *ty
                || cell.path != *path
            {
                return Err(TerminalEffectRejection::new(format!(
                    "abs owned storage differs: scalar-arguments={} index={} access={:?} buffer-type={:?} argument-type={:?} expected-type={:?} path={:?} expected-path={:?}",
                    arguments.len(),
                    buffer.argument_index(),
                    buffer.access(),
                    buffer.structural_type(),
                    argument.structural_type,
                    ty,
                    cell.path,
                    path
                )));
            }
            let TerminalStorageContents::Scalar {
                value,
                stored_scalar,
            } = cell.contents
            else {
                return Err(invalid());
            };
            if stored_scalar != ScalarType::Integer(integer) {
                return Err(invalid());
            }
            value
        } else {
            let [value] = arguments.as_slice() else {
                return Err(invalid());
            };
            if !structural_arguments.is_empty() || !storage.is_empty() {
                return Err(invalid());
            }
            *value
        };
        let TerminalScalarValue::Integer { scalar_type, value } = value else {
            return Err(TerminalEffectRejection::new(
                "abs payload is not an integer",
            ));
        };
        if scalar_type != integer || !integer.admits(value) {
            return Err(TerminalEffectRejection::new(
                "abs payload is not a valid signed C int",
            ));
        }
        // IntegerValue records mathematical sign independently of its carrier;
        // a positive literal in a signed field may use the unsigned variant.
        let value = match value {
            IntegerValue::Signed(value) => i32::try_from(value).ok(),
            IntegerValue::Unsigned(value) => i32::try_from(value).ok(),
        }
        .ok_or_else(invalid)?;
        let actual = runtime_abi::host_integer_absolute(value).ok_or_else(|| {
            TerminalEffectRejection::new("C abs is unavailable or has an undefined signed result")
        })?;
        if matches!(self.signature.result, BoundaryMachineResult::Unit) {
            Ok(TerminalEffectResult::Unit)
        } else {
            Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: integer,
                value: IntegerValue::Signed(actual.into()),
            }))
        }
    }
}
