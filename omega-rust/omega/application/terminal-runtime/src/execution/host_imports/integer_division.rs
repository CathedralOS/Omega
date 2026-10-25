//! An admitted libc lldiv call returns actual typed quotient/remainder fields.
//! Selection uses normalized target/library/symbol identity, never guest names.

use super::super::TerminalRuntimeError;
use calling_conventions::BoundaryCallingApplication;
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralFieldId, StructuralTypeId,
};
use target::{ForeignLocatorCandidate, TargetProfile};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalProgram, TerminalScalarAggregateValue,
    TerminalScalarValue,
};
use terminal_psi::BoundaryMachineDeclaration;

pub(super) struct IntegerDivisionImport {
    signature: TerminalImportSignature,
    structural_type: StructuralTypeId,
    fields: [StructuralFieldId; 2],
}

impl IntegerDivisionImport {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        application: Option<&BoundaryCallingApplication>,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        if !matches_locator(binding)? {
            return Ok(None);
        }
        let integer = IntegerType::new(IntegerSign::Signed, 64)
            .map_err(|_| TerminalRuntimeError::InvalidProvision("lldiv requires i64"))?;
        let scalar = ScalarType::Integer(integer);
        let admitted = super::scalar_record::admit_c_word_record(
            program,
            declaration,
            binding,
            application,
            &[scalar, scalar],
            [scalar, scalar],
        )?;
        Ok(Some(Self {
            signature: admitted.signature,
            structural_type: admitted.structural_type,
            fields: admitted.fields,
        }))
    }
}

fn matches_locator(binding: &NormalizedForeignCallBinding) -> Result<bool, TerminalRuntimeError> {
    let Some((locator, _)) = binding.import() else {
        return Ok(false);
    };
    let target = locator.target();
    let (host, candidate) = match target {
        TargetProfile::MacosArm64 | TargetProfile::MacosX64 => (
            cfg!(target_os = "macos"),
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                symbol: b"_lldiv".to_vec(),
            },
        ),
        TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 => (
            cfg!(target_os = "linux"),
            ForeignLocatorCandidate::ElfVersioned {
                object: b"libc.so.6".to_vec(),
                symbol: b"lldiv".to_vec(),
                version: if target == TargetProfile::LinuxX64 {
                    b"GLIBC_2.2.5".to_vec()
                } else {
                    b"GLIBC_2.17".to_vec()
                },
            },
        ),
        TargetProfile::WindowsX64 => (
            cfg!(target_os = "windows"),
            ForeignLocatorCandidate::PeByName {
                library: b"api-ms-win-crt-utility-l1-1-0.dll".to_vec(),
                export: b"lldiv".to_vec(),
            },
        ),
        _ => return Ok(false),
    };
    let host_arch = match target {
        TargetProfile::MacosArm64 | TargetProfile::LinuxArm64 => cfg!(target_arch = "aarch64"),
        TargetProfile::MacosX64 | TargetProfile::LinuxX64 | TargetProfile::WindowsX64 => {
            cfg!(target_arch = "x86_64")
        }
        _ => false,
    };
    if !host || !host_arch {
        return Ok(false);
    }
    let expected = target::normalize_foreign_locator(candidate, target)
        .map_err(|_| TerminalRuntimeError::InvalidProvision("lldiv host locator is invalid"))?;
    Ok(*locator == expected)
}

import_signature_via_clone!(IntegerDivisionImport);

impl TerminalEffectHandler for IntegerDivisionImport {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "lldiv requires result-aware dispatch",
        ))
    }
    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid = || {
            TerminalEffectRejection::new(
                "lldiv operands or result differ from the admitted signature",
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
        let integer = IntegerType::new(IntegerSign::Signed, 64).map_err(|_| invalid())?;
        let [
            TerminalScalarValue::Integer {
                scalar_type: left_type,
                value: IntegerValue::Signed(left),
            },
            TerminalScalarValue::Integer {
                scalar_type: right_type,
                value: IntegerValue::Signed(right),
            },
        ] = arguments.as_slice()
        else {
            return Err(invalid());
        };
        if *left_type != integer
            || *right_type != integer
            || !structural_arguments.is_empty()
            || *result != self.signature.result
        {
            return Err(invalid());
        }
        let numerator = i64::try_from(*left).map_err(|_| invalid())?;
        let denominator = i64::try_from(*right).map_err(|_| invalid())?;
        let actual =
            runtime_abi::host_integer_division(numerator, denominator).ok_or_else(|| {
                TerminalEffectRejection::new("lldiv host unavailable or C quotient undefined")
            })?;
        Ok(TerminalEffectResult::ScalarAggregate(
            TerminalScalarAggregateValue {
                structural_type: self.structural_type,
                result_case: None,
                fields: vec![
                    (
                        self.fields[0],
                        TerminalScalarValue::Integer {
                            scalar_type: integer,
                            value: IntegerValue::Signed(actual.quotient.into()),
                        },
                    ),
                    (
                        self.fields[1],
                        TerminalScalarValue::Integer {
                            scalar_type: integer,
                            value: IntegerValue::Signed(actual.remainder.into()),
                        },
                    ),
                ],
            },
        ))
    }
}
