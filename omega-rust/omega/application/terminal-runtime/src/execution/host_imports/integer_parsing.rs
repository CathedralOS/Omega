//! Exact C strtol binding over captured typed storage. Native C returns an
//! offset into a private input snapshot; the interpreter's completion owner
//! authenticates that cell and the declared output lifetime before publication.

use super::TerminalRuntimeError;
use super::borrowed_storage::{CapturedStorage, PointerOutputPlan, StorageTransfer};
use calling_conventions::{CallSignature, ValueShape};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarType};
use target::TargetProfile;
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalProgram, TerminalScalarValue,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind,
    BoundaryReferenceDestination, StructuralAccess, StructuralMultiplicity, StructuralPathSegment,
    StructuralTypeShape,
};

pub(super) struct IntegerParsingImport {
    signature: TerminalImportSignature,
    input_layout: Option<NativeBorrowedStorageLayout>,
    output_path: Vec<StructuralPathSegment>,
    output_access: StructuralAccess,
    output_plan: PointerOutputPlan,
    base_type: IntegerType,
    result_type: IntegerType,
}

impl IntegerParsingImport {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        layouts: &[NativeBorrowedStorageLayout],
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        if !matches_locator(binding)? {
            return Ok(None);
        }
        let invalid = TerminalRuntimeError::InvalidProvision;
        let base_type = IntegerType::new(IntegerSign::Signed, 32)
            .map_err(|_| invalid("strtol base requires i32"))?;
        let result_type = IntegerType::new(IntegerSign::Signed, 64)
            .map_err(|_| invalid("strtol result requires Unix C long"))?;
        let signature = TerminalImportSignature::from(declaration);
        let [input, output] = signature.structural_parameters.as_slice() else {
            return Err(invalid(
                "strtol requires input and output storage arguments",
            ));
        };
        if signature.scalar_parameters != [ScalarType::Integer(base_type)]
            || signature.result
                != BoundaryMachineResult::Scalar(ScalarType::Integer(result_type).into())
            || declaration.parameter_order
                != [
                    BoundaryParameterKind::Structural,
                    BoundaryParameterKind::Structural,
                    BoundaryParameterKind::Scalar,
                ]
            || !matches!(
                input.access,
                StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
            )
            || output.access != StructuralAccess::MutableBorrow
            || [input, output].iter().any(|parameter| {
                parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    || !parameter.qualifications.is_empty()
                    || !parameter.projected_qualifications.is_empty()
            })
        {
            return Err(invalid(
                "strtol declaration differs from its exact borrowed-pointer, pointer-output and base ABI",
            ));
        }
        let [reference_output] = declaration.reference_outputs.as_slice() else {
            return Err(invalid(
                "strtol needs one authenticated normal-completion reference output",
            ));
        };
        let types = program.module().structural_types.iter();
        let endpoint = terminal_semantics::runtime_structural_path_tip(
            types,
            output.structural_type,
            &reference_output.path,
        )
        .ok_or(invalid("strtol output has no exact typed storage endpoint"))?;
        let StructuralTypeShape::Reference { referent, access } = endpoint.shape else {
            return Err(invalid(
                "strtol output storage endpoint is not a reference descriptor",
            ));
        };
        if reference_output.destination != BoundaryReferenceDestination::Parameter(1)
            || access != reference_output.access
            || !matches!(
                access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
            || (access != StructuralAccess::SharedBorrow
                && input.access != StructuralAccess::MutableBorrow)
            || reference_output.dependencies.is_empty()
            || !matches!(program.structural_type(referent).map(|row| &row.shape),
                Some(StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(word)))
                    if word.bits() == 8 && !word.is_address())
        {
            return Err(invalid(
                "strtol must publish a byte reference under its declared lifetime anchors",
            ));
        }
        let input_layout = if matches!(
            program
                .structural_type(input.structural_type)
                .map(|row| &row.shape),
            Some(StructuralTypeShape::PrimitiveScalar(_))
        ) {
            None
        } else {
            let mut matching = layouts
                .iter()
                .filter(|layout| layout.structural_type == input.structural_type);
            let layout = matching
                .next()
                .ok_or(invalid(
                    "strtol input has no admitted native storage layout",
                ))?
                .clone();
            if matching.next().is_some() {
                return Err(invalid("strtol input has ambiguous native storage layouts"));
            }
            Some(layout)
        };
        let mut output_layouts = layouts
            .iter()
            .filter(|layout| layout.structural_type == output.structural_type);
        let output_layout = output_layouts.next().ok_or(invalid(
            "strtol output has no admitted native storage layout",
        ))?;
        if output_layouts.next().is_some() {
            return Err(invalid(
                "strtol output has ambiguous native storage layouts",
            ));
        }
        let output_plan = PointerOutputPlan::prepare(
            program,
            output_layout,
            &reference_output.path,
            8,
        )
        .map_err(|reason| {
            TerminalRuntimeError::missing(
                "runtime-host-reference-output-layout",
                format!(
                    "strtol reference output differs from its native root pointer write: {reason:?}"
                ),
            )
        })?;
        super::admit_exact_foreign_call(
            &signature,
            declaration,
            binding,
            &CallSignature {
                parameters: vec![
                    ValueShape::integer(8, 8),
                    ValueShape::integer(8, 8),
                    ValueShape::integer(4, 4),
                ],
                result: Some(ValueShape::integer(8, 8)),
            },
        )?;
        Ok(Some(Self {
            signature,
            input_layout,
            output_path: reference_output.path.clone(),
            output_access: reference_output.access,
            output_plan,
            base_type,
            result_type,
        }))
    }
}

import_signature_via_clone!(IntegerParsingImport);

impl TerminalEffectHandler for IntegerParsingImport {
    fn storage_arguments(&self, _: semantic_vocabulary::BoundaryMachineId) -> &[usize] {
        &[0, 1]
    }

    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "strtol needs captured storage and typed completion",
        ))
    }

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let invalid = || {
            TerminalEffectRejection::new(
                "strtol invocation differs from its admitted storage/ABI contract",
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
        let [
            TerminalScalarValue::Integer {
                scalar_type,
                value: IntegerValue::Signed(base),
            },
        ] = arguments.as_slice()
        else {
            return Err(invalid());
        };
        if *scalar_type != self.base_type
            || *result != self.signature.result
            || structural_arguments.len() != 2
            || !byte_buffers.is_empty()
            || buffers.len() != 2
        {
            return Err(invalid());
        }
        let TerminalBoundaryStorageBuffer::Structural(output) = &buffers[1] else {
            return Err(invalid());
        };
        if output.argument_index() != 1
            || output.structural_type() != self.signature.structural_parameters[1].structural_type
        {
            return Err(invalid());
        }
        self.output_plan.validate(output)?;
        let base = i32::try_from(*base).map_err(|_| invalid())?;
        let captured = CapturedStorage::capture(
            &buffers[..1],
            self.signature.structural_parameters[0].access,
            self.input_layout.as_ref(),
            0,
            StorageTransfer::Observe,
        )?;
        let (value, offset) = runtime_abi::parse_host_long(captured.terminated_prefix()?, base)
            .ok_or_else(|| {
                TerminalEffectRejection::new("strtol native result has no captured input endpoint")
            })?;
        let path = captured.byte_path_at(offset)?;
        let selection = buffers[0].select_reference(&path, self.output_access)?;
        let TerminalBoundaryStorageBuffer::Structural(output) = &mut buffers[1] else {
            return Err(invalid());
        };
        if output.argument_index() != 1
            || output.structural_type() != self.signature.structural_parameters[1].structural_type
        {
            return Err(invalid());
        }
        output.replace_reference(&self.output_path, selection)?;
        Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
            scalar_type: self.result_type,
            value: IntegerValue::Signed(i128::from(value)),
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
            if target == TargetProfile::MacosArm64 {
                cfg!(all(target_os = "macos", target_arch = "aarch64"))
            } else {
                cfg!(all(target_os = "macos", target_arch = "x86_64"))
            },
            target::ForeignLocatorCandidate::MachODylibSymbol {
                install_name: b"/usr/lib/libSystem.B.dylib".to_vec(),
                symbol: b"_strtol".to_vec(),
            },
        ),
        TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 => (
            if target == TargetProfile::LinuxX64 {
                cfg!(all(target_os = "linux", target_arch = "x86_64"))
            } else {
                cfg!(all(target_os = "linux", target_arch = "aarch64"))
            },
            target::ForeignLocatorCandidate::ElfVersioned {
                object: b"libc.so.6".to_vec(),
                symbol: b"strtol".to_vec(),
                version: if target == TargetProfile::LinuxX64 {
                    b"GLIBC_2.2.5".to_vec()
                } else {
                    b"GLIBC_2.17".to_vec()
                },
            },
        ),
        _ => return Ok(false),
    };
    if !host {
        return Ok(false);
    }
    let offered = target::normalize_foreign_locator(candidate, target).map_err(|_| {
        TerminalRuntimeError::InvalidProvision(
            "strtol implementation has an invalid native locator",
        )
    })?;
    Ok(offered == *locator)
}
