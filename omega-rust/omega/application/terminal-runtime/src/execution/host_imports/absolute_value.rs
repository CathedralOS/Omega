//! Interpret exact C `abs` bindings over their admitted signed integer word.
//! A public one-scalar record is a by-value ABI envelope, not an opaque token
//! or a borrowed address. Its exact captured payload is read through the normal
//! boundary storage interface. Unit explicitly discards the C return register;
//! neither that discard nor the host result establishes claims or qualifications.

use calling_conventions::{BoundaryCallingApplication, CallSignature, ValueShape};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerType, IntegerValue, ScalarType,
};
use target::TargetProfile;
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalProgram, TerminalScalarValue, TerminalStorageContents,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
};

use super::TerminalRuntimeError;

pub(super) struct AbsoluteValueImport {
    signature: TerminalImportSignature,
    word: IntegerType,
    payload: Option<Vec<CanonicalStructuralPathSegment>>,
}

impl AbsoluteValueImport {
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
        let word = IntegerType::new(IntegerSign::Signed, 32)
            .map_err(|_| invalid("abs requires a signed 32-bit integer carrier"))?;
        let scalar = ScalarType::Integer(word);
        let signature = TerminalImportSignature::from(declaration);
        let result = if signature.result.is_unit() {
            // A Unit declaration observes no C return register. It does not
            // change the native operation or suppress validation of its input.
            None
        } else if signature.result == BoundaryMachineResult::Scalar(scalar.into()) {
            Some(ValueShape::integer(4, 4))
        } else {
            return Err(invalid(
                "abs result must be i32 or an explicitly discarded Unit",
            ));
        };
        let payload = if signature.scalar_parameters == [scalar]
            && signature.structural_parameters.is_empty()
            && declaration.parameter_order == [BoundaryParameterKind::Scalar]
        {
            None
        } else {
            let [parameter] = signature.structural_parameters.as_slice() else {
                return Err(invalid(
                    "abs requires exactly one signed integer-word argument",
                ));
            };
            if !signature.scalar_parameters.is_empty()
                || declaration.parameter_order != [BoundaryParameterKind::Structural]
                || parameter.access != StructuralAccess::Owned
            {
                return Err(invalid(
                    "abs cannot receive borrowed or multiple argument lanes",
                ));
            }
            Some(super::native_word::scalar_payload_path(
                program,
                parameter.structural_type,
                scalar,
            )?)
        };
        let application = application.ok_or(invalid(
            "abs requires its complete admitted calling application",
        ))?;
        if application.plan.plan() != &binding.boundary_entry_plan
            || application.commitment == [0; 32]
            || !application.opaque_applications.is_empty()
            || application.signature.parameters.len() != 1
            || application.signature.result.is_some() != result.is_some()
        {
            return Err(invalid(
                "abs calling application differs from its admitted signature",
            ));
        }
        let envelope = super::native_word::native_word_envelope(
            &application.signature,
            application.signature.parameters[0],
        )
        .map_err(invalid)?;
        if envelope.byte_size != 4 || envelope.alignment != 4 {
            return Err(invalid(
                "abs native argument must occupy one aligned 32-bit integer word",
            ));
        }
        if let Some(root) = application.signature.result {
            let envelope = super::native_word::native_word_envelope(&application.signature, root)
                .map_err(invalid)?;
            if envelope.byte_size != 4 || envelope.alignment != 4 || envelope.contains_record {
                return Err(invalid(
                    "abs native result must be one signed integer scalar",
                ));
            }
        }
        super::admit_exact_foreign_call(
            &signature,
            declaration,
            binding,
            &CallSignature {
                parameters: vec![ValueShape::integer(4, 4)],
                result,
            },
        )?;
        Ok(Some(Self {
            signature,
            word,
            payload,
        }))
    }
}

import_signature_via_clone!(AbsoluteValueImport);

impl TerminalEffectHandler for AbsoluteValueImport {
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
        let invalid = || {
            TerminalEffectRejection::new(
                "abs operands differ from the admitted integer-word contract",
            )
        };
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            result,
            ..
        } = effect
        else {
            return Err(invalid());
        };
        if *result != self.signature.result || !byte_buffers.is_empty() {
            return Err(invalid());
        }
        let value = if let Some(path) = &self.payload {
            let [TerminalBoundaryStorageBuffer::Structural(buffer)] = storage else {
                return Err(invalid());
            };
            let [argument] = structural_arguments.as_slice() else {
                return Err(invalid());
            };
            let [cell] = buffer.values() else {
                return Err(invalid());
            };
            if !arguments.is_empty()
                || buffer.argument_index() != 0
                || buffer.access() != StructuralAccess::Owned
                || buffer.structural_type()
                    != self.signature.structural_parameters[0].structural_type
                || buffer.structural_type() != argument.structural_type
                || cell.path != *path
            {
                return Err(invalid());
            }
            let TerminalStorageContents::Scalar {
                value,
                stored_scalar,
            } = cell.contents
            else {
                return Err(invalid());
            };
            if stored_scalar != ScalarType::Integer(self.word) {
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
        let TerminalScalarValue::Integer {
            scalar_type,
            value: IntegerValue::Signed(value),
        } = value
        else {
            return Err(invalid());
        };
        if scalar_type != self.word || !self.word.admits(IntegerValue::Signed(value)) {
            return Err(invalid());
        }
        let absolute = value
            .checked_abs()
            .filter(|value| self.word.admits(IntegerValue::Signed(*value)))
            .ok_or_else(|| {
                TerminalEffectRejection::new(
                    "abs argument has no representable C signed integer result",
                )
            })?;
        if self.signature.result.is_unit() {
            Ok(TerminalEffectResult::Unit)
        } else {
            Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: self.word,
                value: IntegerValue::Signed(absolute),
            }))
        }
    }
}

fn matches_locator(binding: &NormalizedForeignCallBinding) -> Result<bool, TerminalRuntimeError> {
    let Some((locator, _)) = binding.import() else {
        return Ok(false);
    };
    let target = locator.target();
    let candidates = match target {
        TargetProfile::WindowsX64 => vec![target::ForeignLocatorCandidate::PeByName {
            library: b"msvcrt.dll".to_vec(),
            export: b"abs".to_vec(),
        }],
        TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 => {
            vec![target::ForeignLocatorCandidate::ElfVersioned {
                object: b"libc.so.6".to_vec(),
                symbol: b"abs".to_vec(),
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
                symbol: b"_abs".to_vec(),
            },
        )
        .collect(),
        _ => return Ok(false),
    };
    for candidate in candidates {
        let offered = target::normalize_foreign_locator(candidate, target).map_err(|_| {
            TerminalRuntimeError::InvalidProvision(
                "abs implementation has an invalid native locator",
            )
        })?;
        if offered == *locator {
            return Ok(true);
        }
    }
    Ok(false)
}
