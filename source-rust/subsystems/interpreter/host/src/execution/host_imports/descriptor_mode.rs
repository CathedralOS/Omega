// We provide a trusted descriptor-mode implementation for MSVCRT _setmode.
// The call swaps a standard descriptor's translation mode in a private
// per-descriptor table (each initialized to _O_TEXT = 0x4000) and returns the
// previous translation mode. Invalid modes and unsupported descriptors return -1.

use std::collections::BTreeMap;

use calling_conventions::{CallSignature, ValueShape};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use target::TargetProfile;
use target_vocabulary::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalScalarValue,
};
use terminal_psi::{BoundaryMachineDeclaration, BoundaryParameterKind};

use super::TerminalRuntimeError;

const O_TEXT: i128 = 0x4000;
const O_BINARY: i128 = 0x8000;
const O_WTEXT: i128 = 0x10000;
const O_U16TEXT: i128 = 0x20000;
const O_U8TEXT: i128 = 0x40000;

pub(super) struct DescriptorModeImport {
    signature: TerminalImportSignature,
    result_type: IntegerType,
    modes: BTreeMap<i128, i128>,
}

impl DescriptorModeImport {
    pub(super) fn admit(
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some(locator) = binding.foreign_import_locator() else {
            return Ok(None);
        };
        if locator.target() != TargetProfile::WindowsX64 {
            return Ok(None);
        }
        let offered = target::normalize_foreign_locator(
            target::ForeignLocatorCandidate::PeByName {
                library: b"msvcrt.dll".to_vec(),
                export: b"_setmode".to_vec(),
            },
            locator.target(),
        )
        .map_err(|_| {
            TerminalRuntimeError::InvalidProvision("mode implementation has an invalid locator")
        })?;
        if offered != *locator {
            return Ok(None);
        }
        let invalid = super::unprovisioned(
            "descriptor-mode adapter differs from its exact admitted declaration/ABI",
        );
        let word = IntegerType::new(IntegerSign::Signed, 32).map_err(|_| invalid())?;
        let signature = TerminalImportSignature::from(declaration);
        let native_signature = CallSignature {
            parameters: vec![ValueShape::integer(4, 4), ValueShape::integer(4, 4)],
            result: Some(ValueShape::integer(4, 4)),
        };
        if signature.scalar_parameters != [ScalarType::Integer(word), ScalarType::Integer(word)]
            || !signature.structural_parameters.is_empty()
            || signature.result
                != terminal_psi::BoundaryMachineResult::Scalar(ScalarType::Integer(word).into())
            || declaration.parameter_order
                != [BoundaryParameterKind::Scalar, BoundaryParameterKind::Scalar]
        {
            return Err(invalid());
        }
        super::admit_exact_foreign_call(&signature, declaration, binding, &native_signature)?;
        Ok(Some(Self {
            signature,
            result_type: word,
            modes: BTreeMap::new(),
        }))
    }

    fn result(&self, value: i128) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if !self.result_type.admits(IntegerValue::Signed(value)) {
            return Err(TerminalEffectRejection::new(
                "descriptor-mode result differs from the installed contract",
            ));
        }
        Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: self.result_type,
            value: IntegerValue::Signed(value),
        }))
    }
}

impl TerminalImportAdapter for DescriptorModeImport {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for DescriptorModeImport {
    handle_effect_requires_dispatch!("descriptor mode requires authenticated scalar dispatch");
    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage_buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid = super::refusal("descriptor-mode operands differ from the installed contract");
        let call = super::call_operands(effect, invalid)?;
        if !byte_buffers.is_empty() || !storage_buffers.is_empty() {
            return Err(invalid());
        }
        let [
            TerminalScalarValue::Integer {
                scalar_type: descriptor_type,
                value: IntegerValue::Signed(descriptor),
            },
            TerminalScalarValue::Integer {
                scalar_type: mode_type,
                value: IntegerValue::Signed(mode),
            },
        ] = call.arguments
        else {
            return Err(invalid());
        };
        if *descriptor_type != self.result_type
            || *mode_type != self.result_type
            || !self.result_type.admits(IntegerValue::Signed(*mode))
        {
            return Err(invalid());
        }
        let previous = set_mode(&mut self.modes, *descriptor, *mode);
        self.result(previous)
    }
}

fn set_mode(modes: &mut BTreeMap<i128, i128>, descriptor: i128, mode: i128) -> i128 {
    if !matches!(descriptor, 0..=2)
        || !matches!(mode, O_BINARY | O_TEXT | O_WTEXT | O_U16TEXT | O_U8TEXT)
    {
        return -1;
    }
    match modes.insert(descriptor, mode).unwrap_or(O_TEXT) {
        // MSVCRT reports a previous Unicode translation mode as wide text.
        O_U16TEXT | O_U8TEXT => O_WTEXT,
        previous => previous,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_modes_leave_the_previous_descriptor_mode_intact() {
        let mut modes = BTreeMap::new();
        assert_eq!(set_mode(&mut modes, 1, 0x8000), O_TEXT);
        for invalid in [0, 0x1234, -1, 0x4001] {
            assert_eq!(set_mode(&mut modes, 1, invalid), -1);
        }
        assert_eq!(set_mode(&mut modes, 2, O_TEXT), O_TEXT);
        assert_eq!(set_mode(&mut modes, 1, O_TEXT), 0x8000);
        assert_eq!(set_mode(&mut modes, 3, 0x8000), -1);
    }

    #[test]
    fn all_unicode_translation_modes_report_wide_text_as_the_previous_mode() {
        let mut modes = BTreeMap::new();
        for unicode in [0x10000, 0x20000, 0x40000] {
            assert_eq!(set_mode(&mut modes, 1, unicode), O_TEXT);
            assert_eq!(set_mode(&mut modes, 1, O_TEXT), 0x10000);
        }
    }
}
