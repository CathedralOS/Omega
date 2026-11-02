//! Exact admitted libc provider-state references: `__errno_location` and
//! `gnu_get_libc_version`. Their C results are host pointers into provider
//! state; the interpreter's semantic model supplies the same contract without
//! an FFI address: a real materialized referent committed beneath the minted
//! reference carrier. `errno` begins zeroed; the libc version byte is a
//! stable nonzero guest-visible cell. Neither operation carries arguments.
//!
//! Admission keeps the checked signature authoritative: the boundary result
//! must be a reference whose access matches the operation's C meaning, and
//! the referent must be a single-field scalar record whose payload type is
//! the operation's exact semantic type. Everything else declines.

use super::TerminalRuntimeError;
use calling_conventions::{BoundaryCallingApplication, CallSignature, ValueShape};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalProgram, TerminalReferenceResult,
    TerminalScalarAggregateValue, TerminalScalarValue,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, StructuralAccess, StructuralTypeShape,
};

#[derive(Clone, Copy)]
enum ReferenceStateOperation {
    ErrnoLocation,
    LibcVersion,
}

pub(super) struct ReferenceStateImport {
    signature: TerminalImportSignature,
    referent: TerminalScalarAggregateValue,
}

impl ReferenceStateImport {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        application: Option<&BoundaryCallingApplication>,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        let Some(operation) = select(binding)? else {
            return Ok(None);
        };
        let signature = TerminalImportSignature::from(declaration);
        let referent = validate(program, declaration, binding, application, &signature, operation)?;
        Ok(Some(Self {
            signature,
            referent,
        }))
    }
}

impl TerminalImportAdapter for ReferenceStateImport {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for ReferenceStateImport {
    handle_effect_requires_dispatch!("provider reference state requires exact typed dispatch");

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid =
            super::refusal("provider reference operands differ from the admitted signature");
        let call = super::call_operands(effect, &invalid)?;
        if *call.result != self.signature.result
            || !call.arguments.is_empty()
            || !call.structural_arguments.is_empty()
            || !call.byte_sequence_arguments.is_empty()
        {
            return Err(invalid());
        }
        Ok(TerminalEffectResult::Reference(
            TerminalReferenceResult {
                referent: self.referent.clone(),
            },
        ))
    }
}

fn select(
    binding: &NormalizedForeignCallBinding,
) -> Result<Option<ReferenceStateOperation>, TerminalRuntimeError> {
    for (symbol, operation) in [
        (
            b"__errno_location".as_slice(),
            ReferenceStateOperation::ErrnoLocation,
        ),
        (
            b"gnu_get_libc_version".as_slice(),
            ReferenceStateOperation::LibcVersion,
        ),
    ] {
        // Both names are glibc imports; other C runtimes decline the offer.
        if super::matches_platform_c_symbol(binding, symbol, &[], None)? {
            return Ok(Some(operation));
        }
    }
    Ok(None)
}

fn validate(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    binding: &NormalizedForeignCallBinding,
    application: Option<&BoundaryCallingApplication>,
    signature: &TerminalImportSignature,
    operation: ReferenceStateOperation,
) -> Result<TerminalScalarAggregateValue, TerminalRuntimeError> {
    let invalid = TerminalRuntimeError::InvalidProvision;
    if !signature.scalar_parameters.is_empty()
        || !signature.structural_parameters.is_empty()
        || !declaration.parameter_order.is_empty()
    {
        return Err(invalid(
            "provider reference state carries no call arguments",
        ));
    }
    let BoundaryMachineResult::Structural(result) = &signature.result else {
        return Err(invalid(
            "provider reference state requires a structural reference result",
        ));
    };
    if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty() {
        return Err(invalid(
            "provider reference state cannot establish result qualifications",
        ));
    }
    let (access, payload_scalar, payload_value) = match operation {
        ReferenceStateOperation::ErrnoLocation => {
            let integer = IntegerType::new(IntegerSign::Signed, 32)
                .map_err(|_| invalid("errno requires a supported i32 carrier"))?;
            (
                StructuralAccess::MutableBorrow,
                ScalarType::Integer(integer),
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Signed(0),
                },
            )
        }
        ReferenceStateOperation::LibcVersion => {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8)
                .map_err(|_| invalid("libc version requires a supported u8 carrier"))?;
            (
                StructuralAccess::SharedBorrow,
                ScalarType::Integer(integer),
                // A stable nonzero byte: the semantic version cell this
                // provider owns for the invocation's life.
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Unsigned(2),
                },
            )
        }
    };
    let StructuralTypeShape::Reference {
        referent,
        access: declared_access,
    } = &program
        .structural_type(result.structural_type)
        .ok_or(invalid("provider reference result type is absent"))?
        .shape
    else {
        return Err(invalid("provider reference result is not a reference"));
    };
    if *declared_access != access {
        return Err(invalid(
            "provider reference access differs from its operation's semantics",
        ));
    }
    let path = super::native_word::scalar_payload_path(program, *referent, payload_scalar)?;
    let native = CallSignature {
        parameters: Vec::new(),
        result: Some(ValueShape::integer(8, 8)),
    };
    super::admit_exact_foreign_call(signature, declaration, binding, &native)?;
    if let Some(application) = application {
        if application.plan.plan() != &binding.boundary_entry_plan
            || application.commitment == [0; 32]
            || !application.opaque_applications.is_empty()
        {
            return Err(invalid(
                "provider reference calling application is unbound or contains opaque/callback transport",
            ));
        }
    }
    Ok(TerminalScalarAggregateValue {
        structural_type: *referent,
        result_case: None,
        fields: vec![(path, payload_value)],
    })
}
