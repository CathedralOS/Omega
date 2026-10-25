//! An admitted libc lldiv call returns actual typed quotient/remainder fields.
//! Selection uses normalized target/library/symbol identity, never guest names.

use super::super::TerminalRuntimeError;
use calling_conventions::BoundaryCallingApplication;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalProgram, TerminalScalarAggregateValue,
    TerminalScalarValue,
};
use terminal_psi::BoundaryMachineDeclaration;

/// The lldiv import is the shared two-word C record admission; the effect
/// handler below owns only the quotient/remainder call contract.
pub(super) type IntegerDivisionImport = super::scalar_record::ScalarRecordAdmission<2>;

impl IntegerDivisionImport {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        application: Option<&BoundaryCallingApplication>,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        if !super::matches_platform_c_symbol(
            binding,
            b"lldiv",
            &[b"/usr/lib/libSystem.B.dylib".as_slice()],
            Some(b"api-ms-win-crt-utility-l1-1-0.dll".as_slice()),
        )? {
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
        Ok(Some(admitted))
    }
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
