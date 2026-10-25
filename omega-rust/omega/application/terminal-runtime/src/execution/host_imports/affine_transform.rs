//! Exact CoreGraphics affine construction with a materialized C record result.

use super::super::TerminalRuntimeError;
use calling_conventions::BoundaryCallingApplication;
use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, ScalarType, StructuralFieldId, StructuralTypeId,
};
use target::{ForeignLocatorCandidate, TargetProfile};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalProgram, TerminalScalarAggregateValue,
    TerminalScalarValue,
};
use terminal_psi::BoundaryMachineDeclaration;

pub(super) struct AffineTransformImport {
    signature: TerminalImportSignature,
    structural_type: StructuralTypeId,
    fields: [StructuralFieldId; 6],
}

impl AffineTransformImport {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        application: Option<&BoundaryCallingApplication>,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some((locator, _)) = binding.import() else {
            return Ok(None);
        };
        if !cfg!(all(target_os = "macos", target_arch = "aarch64"))
            || locator.target() != TargetProfile::MacosArm64
        {
            return Ok(None);
        }
        let expected = target::normalize_foreign_locator(
            ForeignLocatorCandidate::MachODylibSymbol {
                install_name:
                    b"/System/Library/Frameworks/CoreGraphics.framework/Versions/A/CoreGraphics"
                        .to_vec(),
                symbol: b"_CGAffineTransformMake".to_vec(),
            },
            TargetProfile::MacosArm64,
        )
        .map_err(|_| {
            TerminalRuntimeError::InvalidProvision("CoreGraphics host locator is invalid")
        })?;
        if *locator != expected {
            return Ok(None);
        }
        let scalar = ScalarType::IeeeFloat(IeeeFloatFormat::Binary64);
        let admitted = super::scalar_record::admit_c_word_record(
            program,
            declaration,
            binding,
            application,
            &[scalar; 6],
            [scalar; 6],
        )?;
        Ok(Some(Self {
            signature: admitted.signature,
            structural_type: admitted.structural_type,
            fields: admitted.fields,
        }))
    }
}

import_signature_via_clone!(AffineTransformImport);

impl TerminalEffectHandler for AffineTransformImport {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "affine construction requires result-aware dispatch",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid = || {
            TerminalEffectRejection::new(
                "affine construction differs from its admitted six-double contract",
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
        if arguments.len() != 6
            || !structural_arguments.is_empty()
            || *result != self.signature.result
        {
            return Err(invalid());
        }
        let mut components = [0.0; 6];
        for (component, argument) in components.iter_mut().zip(arguments) {
            let TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(bits)) = argument else {
                return Err(invalid());
            };
            *component = f64::from_bits(*bits);
        }
        let actual = runtime_abi::host_affine_transform(components)
            .ok_or_else(|| TerminalEffectRejection::new("CoreGraphics C host is unavailable"))?;
        let values = [actual.a, actual.b, actual.c, actual.d, actual.tx, actual.ty];
        Ok(TerminalEffectResult::ScalarAggregate(
            TerminalScalarAggregateValue {
                structural_type: self.structural_type,
                result_case: None,
                fields: self
                    .fields
                    .into_iter()
                    .zip(values)
                    .map(|(field, value)| {
                        (
                            field,
                            TerminalScalarValue::IeeeFloat(IeeeFloatValue::Binary64(
                                value.to_bits(),
                            )),
                        )
                    })
                    .collect(),
            },
        ))
    }
}
