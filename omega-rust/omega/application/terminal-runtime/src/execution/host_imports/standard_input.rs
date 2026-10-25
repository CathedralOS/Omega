//! The exact selected hosted byte leaf over one explicit finite input snapshot.
//! Checked line readers still execute as guest code. The shape join below
//! describes the selected builtin's result; it never selects host behavior from
//! a provider name, a sum spelling, or signature resemblance.

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, StructuralCaseId, StructuralFieldId, StructuralTypeId,
};
use std::io::Read;
use std::sync::{Arc, Mutex};
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalProgram, TerminalScalarAggregateValue,
    TerminalScalarValue,
};
use terminal_psi::{
    BoundaryCompletion, BoundaryMachineDeclaration, BoundaryMachineResult, CrashCause,
    StructuralMultiplicity, StructuralTypeShape,
};

use super::super::TerminalRuntimeError;

pub(super) struct ReadByteImport<'input> {
    signature: TerminalImportSignature,
    input: Arc<Mutex<Box<dyn Read + Send + 'input>>>,
    structural_type: StructuralTypeId,
    empty_case: StructuralCaseId,
    byte_case: StructuralCaseId,
    field: StructuralFieldId,
    carrier: IntegerType,
}

impl<'input> ReadByteImport<'input> {
    pub(super) fn for_import(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        input: Arc<Mutex<Box<dyn Read + Send + 'input>>>,
    ) -> Result<Self, TerminalRuntimeError> {
        let invalid = || {
            TerminalRuntimeError::InvalidProvision(
                "stdin realization differs from its admitted sum contract",
            )
        };
        let signature = TerminalImportSignature::from(declaration);
        let BoundaryMachineResult::Structural(result) = &signature.result else {
            return Err(invalid());
        };
        let Some(StructuralTypeShape::Sum { cases }) = program
            .structural_type(result.structural_type)
            .map(|declaration| &declaration.shape)
        else {
            return Err(invalid());
        };
        let [empty, byte] = cases.as_slice() else {
            return Err(invalid());
        };
        let [field] = byte.fields.as_slice() else {
            return Err(invalid());
        };
        let Some(semantic_vocabulary::ScalarType::Integer(carrier)) =
            field.field_type.scalar_type()
        else {
            return Err(invalid());
        };
        if signature.completion != BoundaryCompletion::NormalReturn
            || !signature.scalar_parameters.is_empty()
            || !signature.structural_parameters.is_empty()
            || result.multiplicity != StructuralMultiplicity::Affine
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
            || !empty.fields.is_empty()
            || field.relevance.is_erased()
            || carrier.sign() != IntegerSign::Signed
            || carrier.bits() != 32
            || !match field.field_type {
                terminal_psi::StructuralFieldType::Scalar(_) => true,
                terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
                    bounds.contains(IntegerValue::Signed(0))
                        && bounds.contains(IntegerValue::Signed(255))
                }
                _ => false,
            }
        {
            return Err(invalid());
        }
        let structural_type = result.structural_type;
        Ok(Self {
            signature,
            input,
            structural_type,
            empty_case: empty.id,
            byte_case: byte.id,
            field: field.id,
            carrier,
        })
    }
}

import_signature_via_clone!(ReadByteImport<'_>);

impl TerminalEffectHandler for ReadByteImport<'_> {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "stdin import requires result-aware dispatch",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(TerminalEffectRejection::new(
                "stdin import received a non-boundary effect",
            ));
        };
        if !arguments.is_empty() || !structural_arguments.is_empty() {
            return Err(TerminalEffectRejection::new(
                "stdin operands differ from the bound contract",
            ));
        }
        let mut input = self
            .input
            .lock()
            .map_err(|_| TerminalEffectRejection::new("stdin snapshot lock is poisoned"))?;
        let mut byte = [0u8];
        // This leaf promises one read attempt. Even an interrupted attempt
        // follows its declared Trap route; retrying would hide a host failure
        // and consume extra effects outside the guest operation's fuel charge.
        let read = input.read(&mut byte);
        let (result_case, fields) = match read {
            Ok(0) => (self.empty_case, Vec::new()),
            Ok(1) => (
                self.byte_case,
                vec![(
                    self.field,
                    TerminalScalarValue::Integer {
                        scalar_type: self.carrier,
                        value: IntegerValue::Signed(i128::from(byte[0])),
                    },
                )],
            ),
            // Exhaustion alone is EOF. Failed reads keep the leaf's declared
            // Trap outcome; the core independently checks its published route.
            _ => return Ok(TerminalEffectResult::Crash(CrashCause::Trap)),
        };
        Ok(TerminalEffectResult::ScalarAggregate(
            TerminalScalarAggregateValue {
                structural_type: self.structural_type,
                result_case: Some(result_case),
                fields,
            },
        ))
    }
}
