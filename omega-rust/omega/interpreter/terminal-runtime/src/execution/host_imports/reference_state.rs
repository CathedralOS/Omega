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
    TerminalImportAdapter, TerminalImportSignature, TerminalProgram, TerminalReferencePayload,
    TerminalReferenceResult, TerminalScalarAggregateValue, TerminalScalarValue,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, StructuralAccess, StructuralTypeShape,
};

#[derive(Clone, Copy)]
enum ReferenceStateOperation {
    ErrnoLocation,
    LibcVersion,
    Localeconv,
}

pub(super) struct ReferenceStateImport {
    signature: TerminalImportSignature,
    result: TerminalReferenceResult,
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
        let result = validate(
            program,
            declaration,
            binding,
            application,
            &signature,
            operation,
        )?;
        Ok(Some(Self { signature, result }))
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
        let call = super::call_operands(effect, invalid)?;
        if *call.result != self.signature.result
            || !call.arguments.is_empty()
            || !call.structural_arguments.is_empty()
            || !call.byte_sequence_arguments.is_empty()
        {
            return Err(invalid());
        }
        Ok(TerminalEffectResult::Reference(self.result.clone()))
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
        (
            b"localeconv".as_slice(),
            ReferenceStateOperation::Localeconv,
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
) -> Result<TerminalReferenceResult, TerminalRuntimeError> {
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
    let access = match operation {
        ReferenceStateOperation::ErrnoLocation => StructuralAccess::MutableBorrow,
        ReferenceStateOperation::LibcVersion | ReferenceStateOperation::Localeconv => {
            StructuralAccess::SharedBorrow
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
    let native = CallSignature {
        parameters: Vec::new(),
        result: Some(ValueShape::integer(8, 8)),
    };
    super::admit_exact_foreign_call(signature, declaration, binding, &native)?;
    if let Some(application) = application
        && (application.plan.plan() != &binding.boundary_entry_plan
            || application.commitment == [0; 32]
            || !application.opaque_applications.is_empty())
    {
        return Err(invalid(
            "provider reference calling application is unbound or contains opaque/callback transport",
        ));
    }
    match operation {
        ReferenceStateOperation::ErrnoLocation => {
            let integer = IntegerType::new(IntegerSign::Signed, 32)
                .map_err(|_| invalid("errno requires a supported i32 carrier"))?;
            scalar_state(
                program,
                *referent,
                ScalarType::Integer(integer),
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value: IntegerValue::Signed(0),
                },
            )
        }
        ReferenceStateOperation::LibcVersion => {
            // The libc version cell is one scalar leaf; admitted referents
            // may wrap it in layout storage, so the leaf's declared carrier —
            // not the operation — chooses the integer width. The semantic
            // model supplies the same stable small version for every
            // carrier.
            let integer = super::native_word::scalar_leaf_integer(program, *referent)?;
            let value = match integer.sign() {
                IntegerSign::Signed => IntegerValue::Signed(2),
                IntegerSign::Unsigned => IntegerValue::Unsigned(2),
            };
            scalar_state(
                program,
                *referent,
                ScalarType::Integer(integer),
                TerminalScalarValue::Integer {
                    scalar_type: integer,
                    value,
                },
            )
        }
        ReferenceStateOperation::Localeconv => locale_view(program, *referent),
    }
}

/// A host-owned referent whose entire observable content is one scalar leaf:
/// the provider state record beneath the result descriptor.
fn scalar_state(
    program: &TerminalProgram,
    referent: semantic_vocabulary::StructuralTypeId,
    scalar: ScalarType,
    value: TerminalScalarValue,
) -> Result<TerminalReferenceResult, TerminalRuntimeError> {
    let path = super::native_word::scalar_payload_path(program, referent, scalar)?;
    Ok(TerminalReferenceResult {
        referent: TerminalReferencePayload::ScalarAggregate(TerminalScalarAggregateValue {
            structural_type: referent,
            result_case: None,
            fields: vec![(path, value)],
        }),
        references: Vec::new(),
    })
}

/// `localeconv` returns a stable pointer to C's `lconv`, every member a shared
/// `char *` into provider locale state. The admitted view keeps that exact
/// shape: each non-erased field is a shared reference to a single-u8 record,
/// supplied the "C" locale's decimal point byte.
fn locale_view(
    program: &TerminalProgram,
    referent: semantic_vocabulary::StructuralTypeId,
) -> Result<TerminalReferenceResult, TerminalRuntimeError> {
    let invalid = TerminalRuntimeError::InvalidProvision;
    let byte = IntegerType::new(IntegerSign::Unsigned, 8)
        .map_err(|_| invalid("locale members require a supported u8 carrier"))?;
    let declaration = program
        .structural_type(referent)
        .ok_or(invalid("locale view referent type is absent"))?;
    let StructuralTypeShape::Record { fields, .. } = &declaration.shape else {
        return Err(invalid("locale view referent is not a record"));
    };
    let mut references = Vec::new();
    for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
        let Some(member) = field.field_type.as_structural() else {
            return Err(invalid("locale view members are provider references"));
        };
        let StructuralTypeShape::Reference {
            referent: nested,
            access,
        } = &program
            .structural_type(member)
            .ok_or(invalid("locale view member type is absent"))?
            .shape
        else {
            return Err(invalid("locale view members are provider references"));
        };
        if *access != StructuralAccess::SharedBorrow {
            return Err(invalid("locale view members are shared references"));
        }
        let path =
            super::native_word::scalar_payload_path(program, *nested, ScalarType::Integer(byte))?;
        references.push((
            vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                field.id,
            )],
            TerminalReferenceResult {
                referent: TerminalReferencePayload::ScalarAggregate(TerminalScalarAggregateValue {
                    structural_type: *nested,
                    result_case: None,
                    // "." under the "C" locale.
                    fields: vec![(
                        path,
                        TerminalScalarValue::Integer {
                            scalar_type: byte,
                            value: IntegerValue::Unsigned(46),
                        },
                    )],
                }),
                references: Vec::new(),
            },
        ));
    }
    if references.is_empty() {
        return Err(invalid("locale view exposes no provider references"));
    }
    Ok(TerminalReferenceResult {
        referent: TerminalReferencePayload::Zeroed,
        references,
    })
}
