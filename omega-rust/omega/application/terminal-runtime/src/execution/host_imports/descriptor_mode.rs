//! Trusted descriptor-mode implementation for MSVCRT `_setmode`.
//! The call moves no input or output: it only swaps a descriptor's
//! translation mode and returns the mode it replaced. The interpreter
//! therefore tracks a private per-descriptor mode table — each descriptor
//! starts in `_O_TEXT` (0x4000), matching how the CRT initializes the
//! standard descriptors — and reports the stored previous mode verbatim.
//! Descriptors outside the standard triple reject as failed (-1), never
//! as success or EOF, mirroring the provider's failed-read contract.
//! Exact locator, declaration and ABI authenticate the offer; the adapter
//! never manufactures padding, grants no buffers, and does no real host I/O.

use std::collections::BTreeMap;

use calling_conventions::{CallSignature, ValueShape};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use target::TargetProfile;
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalScalarValue,
};
use terminal_psi::{BoundaryMachineDeclaration, BoundaryParameterKind};

use super::TerminalRuntimeError;

const O_TEXT: i128 = 0x4000;

pub(super) struct DescriptorModeImport {
    signature: TerminalImportSignature,
    result_type: IntegerType,
    modes: BTreeMap<i128, i128>,
}

impl DescriptorModeImport {
    /// Absence is an unoffered binding, not an incompatible offered contract.
    pub(super) fn admit(
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some((locator, _)) = binding.import() else {
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
        let invalid = || {
            TerminalRuntimeError::InvalidProvision(
                "descriptor-mode adapter differs from its exact admitted declaration/ABI",
            )
        };
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
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "descriptor mode requires authenticated scalar dispatch",
        ))
    }
    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage_buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid = || {
            TerminalEffectRejection::new(
                "descriptor-mode operands differ from the installed contract",
            )
        };
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            return Err(invalid());
        };
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
        ] = arguments.as_slice()
        else {
            return Err(invalid());
        };
        if *descriptor_type != self.result_type
            || *mode_type != self.result_type
            || !self.result_type.admits(IntegerValue::Signed(*mode))
        {
            return Err(invalid());
        }
        // Only the standard descriptors exist in this runtime model.
        if !matches!(*descriptor, 0 | 1 | 2) {
            return self.result(-1);
        }
        let previous = self.modes.insert(*descriptor, *mode).unwrap_or(O_TEXT);
        self.result(previous)
    }
}
