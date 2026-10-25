//! An admitted libc lldiv call returns actual typed quotient/remainder fields.
//! Selection uses normalized target/library/symbol identity, never guest names.

use super::super::TerminalRuntimeError;
use calling_conventions::{
    BoundaryCallingApplication, BoundaryValueClass, BoundaryValueGraph, CallSignature,
};
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
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    StructuralMultiplicity, StructuralTypeShape,
};

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
        let invalid = TerminalRuntimeError::InvalidProvision;
        let signature = TerminalImportSignature::from(declaration);
        let integer =
            IntegerType::new(IntegerSign::Signed, 64).map_err(|_| invalid("lldiv requires i64"))?;
        let scalar = ScalarType::Integer(integer);
        let BoundaryMachineResult::Structural(result) = &signature.result else {
            return Err(invalid("lldiv requires a scalar record result"));
        };
        if signature.scalar_parameters != [scalar, scalar]
            || !signature.structural_parameters.is_empty()
            || declaration.parameter_order != [BoundaryParameterKind::Scalar; 2]
            || !matches!(
                result.multiplicity,
                StructuralMultiplicity::Unrestricted | StructuralMultiplicity::Affine
            )
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
        {
            return Err(invalid(
                "lldiv requires two i64 inputs and an unqualified nonlinear record",
            ));
        }
        let shape = program
            .structural_type(result.structural_type)
            .ok_or_else(|| invalid("lldiv result declaration is absent"))?;
        let StructuralTypeShape::Record { fields, .. } = &shape.shape else {
            return Err(invalid("lldiv result is not a record"));
        };
        if fields.len() != 2
            || fields.iter().zip([0, 8]).any(|(field, offset)| {
                field.relevance.is_erased()
                    || field.field_type != terminal_psi::StructuralFieldType::Scalar(scalar)
                    || !has_c_scalar_storage(field.storage.as_ref(), offset, scalar)
            })
        {
            return Err(invalid(
                "lldiv result must contain exactly two ordered i64 fields with their exact C scalar storage",
            ));
        }
        let application = application
            .ok_or_else(|| invalid("lldiv requires its complete native calling application"))?;
        if application.signature.parameters.len() != 2
            || application.plan.plan() != &binding.boundary_entry_plan
            || application.commitment == [0; 32]
            || binding
                .import()
                .is_none_or(|(locator, _)| application.target != locator.target().native_target())
            || !application.opaque_applications.is_empty()
        {
            return Err(invalid(
                "lldiv native application differs from its public C signature",
            ));
        }
        let native_parameters = application
            .signature
            .native_parameters
            .span(application.signature.native_parameter_rows)
            .ok_or_else(|| invalid("lldiv native parameter rows are absent"))?;
        if native_parameters.len() != 2 || native_parameters.iter().enumerate().any(|(ordinal, parameter)| {
            parameter.native_ordinal != ordinal as u32
                || parameter.origin != (calling_conventions::BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal: ordinal as u32 })
                || parameter.shape != calling_conventions::BoundaryNativeParameterShape::Semantic(application.signature.parameters[ordinal])
        }) {
            return Err(invalid("lldiv native parameter order differs from its semantic formals"));
        }
        let graph = BoundaryValueGraph {
            shapes: &application.signature.shapes,
            fields: &application.signature.fields,
        };
        for root in &application.signature.parameters {
            let shape = graph
                .shapes
                .get(usize::from(*root))
                .ok_or_else(|| invalid("lldiv input shape is absent"))?;
            if shape.class != BoundaryValueClass::Integer
                || shape.byte_size != 8
                || shape.alignment != 8
            {
                return Err(invalid(
                    "lldiv native inputs must be aligned eight-byte integers",
                ));
            }
        }
        let result_root = application
            .signature
            .result
            .ok_or_else(|| invalid("lldiv native result is absent"))?;
        let native_result = graph
            .shapes
            .get(usize::from(result_root))
            .ok_or_else(|| invalid("lldiv native result shape is absent"))?;
        let BoundaryValueClass::Record {
            first_field,
            field_count: 2,
        } = native_result.class
        else {
            return Err(invalid(
                "lldiv native result must retain the complete C record shape",
            ));
        };
        let start = usize::from(first_field);
        let members = graph
            .fields
            .get(start..start + 2)
            .ok_or_else(|| invalid("lldiv native record fields are absent"))?;
        if native_result.byte_size != 16 || native_result.alignment != 8 {
            return Err(invalid(
                "lldiv native result must be sixteen bytes aligned to eight",
            ));
        }
        for (ordinal, field) in members.iter().enumerate() {
            let leaf = graph
                .shapes
                .get(usize::from(field.shape))
                .ok_or_else(|| invalid("lldiv native record leaf is absent"))?;
            if field.byte_offset != (ordinal * 8) as u16
                || leaf.class != BoundaryValueClass::Integer
                || leaf.byte_size != 8
                || leaf.alignment != 8
            {
                return Err(invalid(
                    "lldiv native result field offsets or integer shapes differ",
                ));
            }
        }
        let policy = application.plan.plan().call.policy;
        let native = CallSignature {
            parameters: application
                .signature
                .parameters
                .iter()
                .map(|root| graph.classify(*root, policy))
                .collect::<Result<_, _>>()
                .map_err(|_| invalid("lldiv argument ABI classification failed"))?,
            result: Some(
                graph
                    .classify(result_root, policy)
                    .map_err(|_| invalid("lldiv result ABI classification failed"))?,
            ),
        };
        super::admit_exact_foreign_call(&signature, declaration, binding, &native)?;
        Ok(Some(Self {
            structural_type: result.structural_type,
            fields: [fields[0].id, fields[1].id],
            signature,
        }))
    }
}

/// ABI classification retains physical width/class, not signed storage codecs.
/// Rejoin the declaration's effective codec before treating C's signed words as
/// semantic fields. Explicit identical layouts remain admitted; an equal-sized
/// unsigned codec or a bit-fragment recipe does not become C long long.
fn has_c_scalar_storage(
    storage: Option<&semantic_vocabulary::AuthoredFieldStorage>,
    offset: u64,
    scalar: ScalarType,
) -> bool {
    use semantic_vocabulary::AuthoredFieldStorage;
    match storage {
        None => true,
        Some(AuthoredFieldStorage::Scalar {
            byte_offset,
            stored_scalar,
        }) => *byte_offset == offset && stored_scalar.is_none_or(|stored| stored == scalar),
        Some(AuthoredFieldStorage::Direct { byte_offset }) => *byte_offset == offset,
        Some(AuthoredFieldStorage::Repeated { .. } | AuthoredFieldStorage::Fragmented(_)) => false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::AuthoredFieldStorage;

    #[test]
    fn c_record_admission_retains_signed_codec_not_only_equal_width() {
        let signed = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 64).unwrap());
        let unsigned = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        assert!(has_c_scalar_storage(None, 0, signed));
        for stored_scalar in [None, Some(signed)] {
            assert!(has_c_scalar_storage(
                Some(&AuthoredFieldStorage::Scalar {
                    byte_offset: 8,
                    stored_scalar,
                }),
                8,
                signed
            ));
        }
        assert!(has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Direct { byte_offset: 8 }),
            8,
            signed
        ));
        assert!(!has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Scalar {
                byte_offset: 8,
                stored_scalar: Some(unsigned),
            }),
            8,
            signed
        ));
        assert!(!has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Scalar {
                byte_offset: 0,
                stored_scalar: Some(signed),
            }),
            8,
            signed
        ));
        assert!(!has_c_scalar_storage(
            Some(&AuthoredFieldStorage::Repeated {
                byte_offset: 8,
                element_stride: 8,
            }),
            8,
            signed
        ));
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
