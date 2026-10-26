//! Execute the selected provider's produced fixed-frame adapter. Admission
//! joins the declaration, native telescope, exact layout and emitted product;
//! the interpreter still checks requires, claims, guarantees and crash routes.
//! No loader name, fixture identity or observed C token selects semantics.
//!
//! This private sponsor supplies the selected native service only, with no
//! standard streams or additional service capabilities. Existing modeled host
//! services retain their explicit bindings before this route is considered.

mod original_homes;
mod record_fields;
mod values;
pub(super) use original_homes::NativeContext;
use original_homes::rejection;
use values::StorageCodec;

use super::super::TerminalRuntimeError;
use super::scalar_codec::{decode, encode, scalar_extent};
use calling_conventions::{
    BoundaryCallingApplication, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    BoundaryValueClass, ValueClass,
};
use native_artifact::TerminalRuntimeHostProvisions;
use native_provider_runtime::{NativeAdapterId, NativeArgument, NativeParameterStorage};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{BoundaryMachineId, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalOriginalScalarBackings, TerminalOriginalStorageRequest,
    TerminalProgram,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    TerminalPsiIdentity,
};

enum Parameter {
    Scalar {
        index: usize,
        scalar: ScalarType,
    },
    Structural {
        index: usize,
        access: StructuralAccess,
        codec: StorageCodec,
    },
}
enum ResultCodec {
    Unit,
    Scalar(ScalarType),
    Record(StorageCodec),
}

pub(super) struct NativeImport {
    signature: TerminalImportSignature,
    artifact: TerminalPsiIdentity,
    boundary: BoundaryMachineId,
    native: NativeContext,
    adapter: NativeAdapterId,
    parameters: Vec<Parameter>,
    storage_arguments: Vec<usize>,
    layouts: Vec<NativeBorrowedStorageLayout>,
    result: ResultCodec,
}

impl NativeImport {
    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        provisions: Option<&TerminalRuntimeHostProvisions>,
        native: &NativeContext,
    ) -> Result<Option<Self>, TerminalRuntimeError> {
        if !native.available() {
            return Ok(None);
        }
        let Some((locator, _)) = binding.import() else {
            return Ok(None);
        };
        let missing =
            |reason: String| TerminalRuntimeError::missing("native-provider-adapter", reason);
        if locator.target().native_target() != target::NativeTarget::host() {
            return Ok(None);
        }
        // Modeled descriptor operations own their explicit input/output
        // capability supply. An exact recognized but unoffered operation
        // must not fall through to this worker's private null streams.
        if runtime_abi::descriptor_import_identity(locator).is_some() {
            return Err(missing(
                "descriptor operation requires its explicit modeled I/O capability".into(),
            ));
        }
        if declaration
            .fixed_service_reach
            .iter()
            .chain(&declaration.published_service_ceiling)
            .any(|service| Some(*service) != declaration.owning_service)
        {
            return Err(missing(
                "private native sponsor supplies no additional service capability".into(),
            ));
        }
        if declaration.completion != terminal_psi::BoundaryCompletion::NormalReturn
            || !declaration.reference_outputs.is_empty()
            || !declaration.program_local_root_introductions.is_empty()
            || !binding.has_valid_entry_mechanism()
            || binding.provider_plan_report_identity() == 0
        {
            return Err(missing("native sponsor needs a normal exact call without introduced roots or reference outputs".into()));
        }
        let provisions = provisions
            .ok_or_else(|| missing("native adapter lost its producing provisions".into()))?;
        let application = provisions
            .host_calling_application(declaration.id)
            .ok_or_else(|| {
                missing("native adapter lost its complete calling application".into())
            })?;
        let product = provisions
            .host_native_foreign_adapter(declaration.id)
            .ok_or_else(|| missing("native adapter product is absent".into()))?
            .map_err(|reason| missing(reason.to_owned()))?;
        if application.target != locator.target().native_target()
            || application.plan.plan() != &binding.boundary_entry_plan
            || application.commitment == [0; 32]
            || *product.application() != application.commitment
            || product.locator() != locator
            || product.bridge().request().foreign != application.plan
            || !application.opaque_applications.is_empty()
            || !application
                .plan
                .plan()
                .call
                .callback_materializations
                .is_empty()
        {
            return Err(missing(
                "native adapter product differs from its complete selected calling application"
                    .into(),
            ));
        }
        let signature = TerminalImportSignature::from(declaration);
        let layouts = provisions.borrowed_storage_layouts();
        let parameters = admit_parameters(program, declaration, application, layouts)
            .map_err(|error| missing(error.reason))?;
        let result = match &declaration.result {
            BoundaryMachineResult::Unit => {
                if application.signature.result.is_some()
                    || application.plan.plan().call.result.is_some()
                {
                    return Err(missing(
                        "native Unit result retains physical payload".into(),
                    ));
                }
                ResultCodec::Unit
            }
            BoundaryMachineResult::Scalar(scalar) => {
                let shape = application
                    .signature
                    .result
                    .and_then(|root| application.signature.shapes.get(root as usize))
                    .ok_or_else(|| missing("native scalar result has no public shape".into()))?;
                validate_scalar_shape(*shape, scalar.scalar_type)
                    .map_err(|error| missing(error.reason))?;
                ResultCodec::Scalar(scalar.scalar_type)
            }
            BoundaryMachineResult::Structural(result) => {
                let codec = StorageCodec::admit(program, layouts, result.structural_type)
                    .map_err(|error| missing(error.reason))?;
                let shape = application
                    .signature
                    .result
                    .and_then(|root| application.signature.shapes.get(root as usize))
                    .ok_or_else(|| missing("native record result has no public shape".into()))?;
                let semantic = program
                    .structural_type(codec.structural_type)
                    .ok_or_else(|| {
                        missing("native result lost its semantic record declaration".into())
                    })?;
                validate_record_shape(&application.signature, *shape, &codec, semantic)
                    .map_err(|error| missing(error.reason))?;
                ResultCodec::Record(codec)
            }
        };
        // Loading can run provider constructors. It happens only after the
        // whole declaration and all input/result codecs have been admitted.
        let adapter = native
            .sponsor
            .borrow_mut()
            .with(|process| process.install(product, &parameters.iter().map(|parameter| match parameter { Parameter::Structural { access: StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow, codec, .. } => NativeParameterStorage::OriginalReference { byte_size: codec.extent, alignment: codec.alignment }, _ => NativeParameterStorage::Value }).collect::<Vec<_>>()))
            .map_err(|error| missing(error.reason))?;
        Ok(Some(Self {
            signature,
            artifact: program.terminal_psi(),
            boundary: declaration.id,
            native: native.clone(),
            adapter,
            parameters,
            storage_arguments: (0..declaration.structural_parameters.len()).collect(),
            layouts: layouts.to_vec(),
            result,
        }))
    }
}
import_signature_via_clone!(NativeImport);

fn validate_scalar_shape(
    shape: calling_conventions::BoundaryValueShape,
    scalar: ScalarType,
) -> Result<(), TerminalEffectRejection> {
    let class = if matches!(scalar, ScalarType::IeeeFloat(_)) {
        BoundaryValueClass::Float
    } else {
        BoundaryValueClass::Integer
    };
    if shape.class != class || usize::from(shape.byte_size) != scalar_extent(scalar)? {
        return Err(rejection(
            "native scalar shape lost its exact carrier class or width",
        ));
    }
    Ok(())
}
fn validate_record_shape(
    signature: &calling_conventions::BoundarySignatureGeometry,
    shape: calling_conventions::BoundaryValueShape,
    codec: &StorageCodec,
    semantic: &terminal_psi::StructuralTypeDeclaration,
) -> Result<(), TerminalEffectRejection> {
    // Borrowed storage can expose a complete nested scalar roster. Value ABI
    // transport and the scalar aggregate publisher still require the exact
    // direct-field semantic shape; equal flattened bytes cannot establish it.
    // Checking the declaration also retains nested empty fields with no leaves.
    let terminal_psi::StructuralTypeShape::Record {
        fields: semantic_fields,
        ..
    } = &semantic.shape
    else {
        return Err(rejection("native value ABI needs a semantic record"));
    };
    let semantic_fields = semantic_fields
        .iter()
        .filter(|field| !field.relevance.is_erased());
    if semantic.id != codec.structural_type || semantic_fields.clone().count() != codec.fields.len()
    {
        return Err(rejection(
            "native value ABI changes its semantic field roster",
        ));
    }
    for (semantic_field, field) in semantic_fields.zip(&codec.fields) {
        let logical = match semantic_field.field_type {
            terminal_psi::StructuralFieldType::Scalar(scalar) => scalar,
            terminal_psi::StructuralFieldType::IeeeFloat(format) => ScalarType::IeeeFloat(format),
            _ => {
                return Err(rejection(
                    "native value ABI needs direct scalar semantic fields",
                ));
            }
        };
        if !matches!(field.path.as_slice(), [semantic_vocabulary::CanonicalStructuralPathSegment::Field(identity)] if *identity == semantic_field.id)
            || field.logical != logical
        {
            return Err(rejection(
                "native value ABI changes its direct semantic field path",
            ));
        }
    }
    if !matches!(shape.class, BoundaryValueClass::Record { .. })
        || usize::from(shape.byte_size) != codec.extent
        || usize::from(shape.alignment) != codec.alignment
    {
        return Err(rejection(
            "native record public shape differs from its exact storage codec",
        ));
    }
    let BoundaryValueClass::Record {
        first_field,
        field_count,
    } = shape.class
    else {
        unreachable!("record class admitted above")
    };
    let first = usize::from(first_field);
    let end = first
        .checked_add(usize::from(field_count))
        .ok_or_else(|| rejection("native public record field span overflows"))?;
    let fields = signature
        .fields
        .get(first..end)
        .ok_or_else(|| rejection("native public record field span is absent"))?;
    if fields.len() != codec.fields.len() {
        return Err(rejection(
            "native public record changes its scalar field roster",
        ));
    }
    for field in &codec.fields {
        let mut matching = fields
            .iter()
            .filter(|actual| usize::from(actual.byte_offset) == field.offset);
        let actual = matching
            .next()
            .ok_or_else(|| rejection("native public record changes a semantic field offset"))?;
        if matching.next().is_some() {
            return Err(rejection(
                "native public record repeats a semantic field offset",
            ));
        }
        let shape = signature
            .shapes
            .get(actual.shape as usize)
            .ok_or_else(|| rejection("native public record field shape is absent"))?;
        validate_scalar_shape(*shape, field.stored)?;
    }
    Ok(())
}

fn admit_parameters(
    program: &TerminalProgram,
    declaration: &BoundaryMachineDeclaration,
    application: &BoundaryCallingApplication,
    layouts: &[NativeBorrowedStorageLayout],
) -> Result<Vec<Parameter>, TerminalEffectRejection> {
    if !declaration.has_valid_parameter_order()
        || application.signature.parameters.len() != declaration.parameter_order.len()
    {
        return Err(rejection("native semantic formal roster is incomplete"));
    }
    let rows = application
        .signature
        .native_parameters
        .span(application.signature.native_parameter_rows)
        .ok_or_else(|| rejection("native telescope storage is absent"))?;
    if rows.len() != declaration.parameter_order.len()
        || rows.len() != application.plan.plan().call.parameters.len()
    {
        return Err(rejection(
            "native import needs supported exact semantic formals without private ABI materializations",
        ));
    }
    let mut parameters = Vec::with_capacity(rows.len());
    let mut seen = vec![false; rows.len()];
    for ordinal in 0..rows.len() {
        let mut matching = rows
            .iter()
            .filter(|row| row.native_ordinal as usize == ordinal);
        let row = matching
            .next()
            .ok_or_else(|| rejection("native telescope lost an ordinal"))?;
        let BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal } = row.origin else {
            return Err(rejection(
                "native private parameter requires its own materialization codec",
            ));
        };
        let formal = formal_ordinal as usize;
        let repeated = seen
            .get_mut(formal)
            .ok_or_else(|| rejection("native semantic formal exceeds its declaration"))?;
        if matching.next().is_some()
            || std::mem::replace(repeated, true)
            || row.shape
                != BoundaryNativeParameterShape::Semantic(application.signature.parameters[formal])
        {
            return Err(rejection(
                "native semantic/native formal correspondence is repeated or changed",
            ));
        }
        let shape = application
            .signature
            .shapes
            .get(application.signature.parameters[formal] as usize)
            .ok_or_else(|| rejection("native semantic formal shape is absent"))?;
        let physical = &application.plan.plan().call.parameters[ordinal].shape;
        match declaration.parameter_order[formal] {
            BoundaryParameterKind::Scalar => {
                let index = declaration.parameter_order[..formal]
                    .iter()
                    .filter(|kind| **kind == BoundaryParameterKind::Scalar)
                    .count();
                let scalar = declaration.scalar_parameters[index];
                validate_scalar_shape(*shape, scalar)?;
                if matches!(
                    physical.class,
                    ValueClass::BorrowedReference | ValueClass::BorrowedScalarReference
                ) || physical.byte_size as usize != scalar_extent(scalar)?
                {
                    return Err(rejection(
                        "native scalar placement changes its value transport",
                    ));
                }
                parameters.push(Parameter::Scalar { index, scalar });
            }
            BoundaryParameterKind::Structural => {
                let index = declaration.parameter_order[..formal]
                    .iter()
                    .filter(|kind| **kind == BoundaryParameterKind::Structural)
                    .count();
                let parameter = &declaration.structural_parameters[index];
                let codec = StorageCodec::admit(program, layouts, parameter.structural_type)?;
                match parameter.access {
                    StructuralAccess::Owned => {
                        let semantic =
                            program
                                .structural_type(codec.structural_type)
                                .ok_or_else(|| {
                                    rejection(
                                        "native value input lost its semantic record declaration",
                                    )
                                })?;
                        validate_record_shape(&application.signature, *shape, &codec, semantic)?;
                        if matches!(
                            physical.class,
                            ValueClass::BorrowedReference | ValueClass::BorrowedScalarReference
                        ) {
                            return Err(rejection("native owned record became a borrowed pointer"));
                        }
                    }
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow => {
                        if shape.class != BoundaryValueClass::Reference
                            || shape.byte_size != 8
                            || shape.alignment != 8
                            || *physical != calling_conventions::ValueShape::integer(8, 8)
                        {
                            return Err(rejection(
                                "native loan requires its exact thin foreign pointer transport",
                            ));
                        }
                    }
                    _ => {
                        return Err(rejection(
                            "native import requires an initialized readable input codec",
                        ));
                    }
                }
                if parameter.access == StructuralAccess::Owned
                    && (physical.byte_size as usize != codec.extent
                        || usize::from(physical.alignment) != codec.alignment)
                {
                    return Err(rejection(
                        "native formal placement changes the complete referent geometry",
                    ));
                }
                parameters.push(Parameter::Structural {
                    index,
                    access: parameter.access,
                    codec,
                });
            }
        }
    }
    Ok(parameters)
}

impl TerminalEffectHandler for NativeImport {
    fn requires_original_storage(&self, boundary: BoundaryMachineId) -> bool {
        boundary == self.boundary
    }
    fn storage_arguments(&self, boundary: BoundaryMachineId) -> &[usize] {
        if boundary == self.boundary {
            &self.storage_arguments
        } else {
            &[]
        }
    }
    fn prepare_original_storage(
        &mut self,
        boundary: BoundaryMachineId,
        requests: &[TerminalOriginalStorageRequest],
    ) -> Result<TerminalOriginalScalarBackings, TerminalEffectRejection> {
        if boundary != self.boundary
            || requests.iter().any(|request| {
                request.root().artifact() != self.artifact
                    || !self.storage_arguments.contains(&request.argument_index())
            })
        {
            return Err(rejection(
                "native original storage belongs to another invocation or artifact",
            ));
        }
        self.native.prepare(requests, &self.layouts)
    }
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(rejection(
            "native import requires complete result and storage dispatch",
        ))
    }
    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        bytes: &mut [TerminalBoundaryByteBuffer],
        buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall {
            boundary,
            arguments,
            structural_arguments,
            result,
            ..
        } = effect
        else {
            return Err(rejection("native adapter received a non-call effect"));
        };
        if *boundary != self.boundary
            || *result != self.signature.result
            || arguments.len() != self.signature.scalar_parameters.len()
            || structural_arguments.len() != self.signature.structural_parameters.len()
            || !bytes.is_empty()
            || buffers.len() != self.storage_arguments.len()
        {
            return Err(rejection(
                "native call differs from its admitted complete signature",
            ));
        }
        enum Input {
            Value(Vec<u8>),
            Loan(native_provider_runtime::NativeHomeId, usize),
        }
        let mut inputs = Vec::with_capacity(self.parameters.len());
        for parameter in &self.parameters {
            inputs.push(match parameter {
                Parameter::Scalar { index, scalar } => {
                    let value = arguments[*index];
                    if value.scalar_type() != *scalar { return Err(rejection("native scalar actual changed its declared carrier")); }
                    let encoded = encode(value, *scalar)?;
                    Input::Value(encoded[..scalar_extent(*scalar)?].to_vec())
                }
                Parameter::Structural { index, access, codec } => {
                    let mut matching = buffers.iter().filter(|buffer| match buffer { TerminalBoundaryStorageBuffer::Scalar(buffer) => buffer.argument_index() == *index, TerminalBoundaryStorageBuffer::Structural(buffer) => buffer.argument_index() == *index });
                    let buffer = matching.next().ok_or_else(|| rejection("native structural argument lost its captured payload"))?;
                    if matching.next().is_some() { return Err(rejection("native structural argument has repeated captures")); }
                    match (access, buffer) {
                        (StructuralAccess::Owned, TerminalBoundaryStorageBuffer::Structural(buffer)) if buffer.access() == *access && buffer.structural_type() == codec.structural_type && buffer.original_storage().is_none() => Input::Value(codec.encode(buffer.values())?),
                        (StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow, TerminalBoundaryStorageBuffer::Structural(buffer)) if buffer.access() == *access && buffer.structural_type() == codec.structural_type => {
                            let loan = buffer.original_storage().ok_or_else(|| rejection("native borrowed record has no original allocation"))?;
                            let (home, offset) = self.native.loan(loan, codec)?; Input::Loan(home, offset)
                        }
                        (StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow, TerminalBoundaryStorageBuffer::Scalar(buffer)) if buffer.access() == *access => {
                            let loan = buffer.original_storage().ok_or_else(|| rejection("native borrowed scalar has no original allocation"))?;
                            let (home, offset) = self.native.loan(loan, codec)?; Input::Loan(home, offset)
                        }
                        _ => return Err(rejection("native structural argument changes access or payload representation")),
                    }
                }
            });
        }
        let native_arguments = inputs
            .iter()
            .map(|input| match input {
                Input::Value(bytes) => NativeArgument::Value(bytes),
                Input::Loan(home, offset) => NativeArgument::Borrowed {
                    home: *home,
                    offset: *offset,
                },
            })
            .collect::<Vec<_>>();
        let returned = match self
            .native
            .sponsor
            .borrow_mut()
            .call(self.adapter, &native_arguments)
        {
            Ok(bytes) => bytes,
            Err(error) if actual_trap(&error) => {
                return Ok(TerminalEffectResult::Crash(terminal_psi::CrashCause::Trap));
            }
            Err(error) => return Err(rejection(error.to_string())),
        };
        // Mutable captures are completion observations of the same mapped
        // homes, never a write-back copy. Core validates them and preserves its
        // ordinary qualification/provenance and claim completion joins.
        for parameter in &self.parameters {
            let Parameter::Structural {
                index,
                access: StructuralAccess::MutableBorrow,
                codec,
            } = parameter
            else {
                continue;
            };
            let buffer = buffers
                .iter_mut()
                .find(|buffer| match buffer {
                    TerminalBoundaryStorageBuffer::Scalar(buffer) => {
                        buffer.argument_index() == *index
                    }
                    TerminalBoundaryStorageBuffer::Structural(buffer) => {
                        buffer.argument_index() == *index
                    }
                })
                .ok_or_else(|| rejection("native mutable completion lost its original input"))?;
            let loan = match &*buffer {
                TerminalBoundaryStorageBuffer::Scalar(buffer) => buffer.original_storage(),
                TerminalBoundaryStorageBuffer::Structural(buffer) => buffer.original_storage(),
            }
            .ok_or_else(|| rejection("native mutable completion lost its original loan"))?;
            let (home, offset) = self.native.loan(loan, codec)?;
            let mut observed = vec![0; codec.extent];
            self.native
                .sponsor
                .borrow_mut()
                .with(|process| process.read_home(home, offset, &mut observed))?;
            match buffer {
                TerminalBoundaryStorageBuffer::Scalar(buffer) => {
                    let [field] = codec.fields.as_slice() else {
                        return Err(rejection("native scalar completion is not one scalar"));
                    };
                    buffer.replace(decode(&observed, field.stored, field.logical)?)?;
                }
                TerminalBoundaryStorageBuffer::Structural(buffer) => {
                    buffer.replace(codec.decode_fields(&observed)?)?
                }
            }
        }
        match &self.result {
            ResultCodec::Unit if returned.is_empty() => Ok(TerminalEffectResult::Unit),
            ResultCodec::Unit => Err(rejection("native Unit completion returned a payload")),
            ResultCodec::Scalar(scalar) => Ok(TerminalEffectResult::Scalar(decode(
                &returned, *scalar, *scalar,
            )?)),
            ResultCodec::Record(codec) => Ok(TerminalEffectResult::ScalarAggregate(
                codec.aggregate(&returned)?,
            )),
        }
    }
}

fn actual_trap(error: &native_provider_runtime::NativeProviderError) -> bool {
    let Some(status) = error.status else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        matches!(status.signal(), Some(4 | 5))
    }
    #[cfg(windows)]
    {
        matches!(
            status.code().map(|code| code as u32),
            Some(0xc000001d | 0x80000003)
        )
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = status;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use calling_conventions::{BoundarySignatureGeometry, BoundaryValueField, BoundaryValueShape};
    use semantic_vocabulary::{
        CanonicalStructuralPathSegment, IntegerSign, IntegerType, StructuralFieldId,
        StructuralTypeId,
    };

    #[test]
    fn equal_record_extent_does_not_admit_changed_scalar_class_or_field_offset() {
        let scalar = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        let mut codec = StorageCodec {
            structural_type: StructuralTypeId::new(1).unwrap(),
            extent: 8,
            alignment: 8,
            fields: vec![values::ScalarField {
                path: vec![CanonicalStructuralPathSegment::Field(
                    StructuralFieldId::new(1).unwrap(),
                )],
                offset: 0,
                logical: scalar,
                stored: scalar,
            }],
        };
        let mut semantic = terminal_psi::StructuralTypeDeclaration {
            quotient_carrier: None,
            id: codec.structural_type,
            identity: terminal_psi::TerminalIdentityIndex(1),
            owns_linear_claim: false,
            copyable: false,
            no_code_affine_cleanup: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: terminal_psi::StructuralTypeShape::Record {
                fields: vec![terminal_psi::StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: terminal_psi::TerminalIdentityIndex(2),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(scalar),
                    storage: None,
                }],
                storage: terminal_psi::StructuralRecordStorage::Packed,
            },
        };
        let record = BoundaryValueShape {
            class: BoundaryValueClass::Record {
                first_field: 0,
                field_count: 1,
            },
            byte_size: 8,
            alignment: 8,
        };
        let mut signature = BoundarySignatureGeometry {
            shapes: vec![
                record,
                BoundaryValueShape {
                    class: BoundaryValueClass::Integer,
                    byte_size: 8,
                    alignment: 8,
                },
                BoundaryValueShape {
                    class: BoundaryValueClass::Float,
                    byte_size: 8,
                    alignment: 8,
                },
            ],
            fields: vec![BoundaryValueField {
                shape: 1,
                byte_offset: 0,
            }],
            parameters: Vec::new(),
            result: Some(0),
            native_parameters: Default::default(),
            native_parameter_rows: Default::default(),
            callback_layouts: Default::default(),
            callback_layout_rows: Default::default(),
        };
        assert!(validate_record_shape(&signature, record, &codec, &semantic).is_ok());
        // Equal flat public bytes do not authenticate a nested semantic path.
        codec.fields[0]
            .path
            .push(CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(2).unwrap(),
            ));
        assert!(validate_record_shape(&signature, record, &codec, &semantic).is_err());
        codec.fields[0].path.pop();
        // A nested empty field has no scalar leaf. Its semantic declaration
        // must still prevent publication through a flat public record graph.
        let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut semantic.shape else {
            unreachable!()
        };
        fields.push(terminal_psi::StructuralFieldDeclaration {
            id: StructuralFieldId::new(2).unwrap(),
            identity: terminal_psi::TerminalIdentityIndex(3),
            relevance: terminal_psi::BindingRelevance::Relevant,
            field_type: terminal_psi::StructuralFieldType::Structural(
                StructuralTypeId::new(2).unwrap(),
            ),
            storage: None,
        });
        assert!(validate_record_shape(&signature, record, &codec, &semantic).is_err());
        let terminal_psi::StructuralTypeShape::Record { fields, .. } = &mut semantic.shape else {
            unreachable!()
        };
        fields.pop();
        assert!(validate_record_shape(&signature, record, &codec, &semantic).is_ok());
        signature.fields[0].shape = 2;
        assert!(validate_record_shape(&signature, record, &codec, &semantic).is_err());
        signature.fields[0].shape = 1;
        signature.fields[0].byte_offset = 1;
        assert!(validate_record_shape(&signature, record, &codec, &semantic).is_err());
        signature.fields[0].byte_offset = 0;
        let changed_span = BoundaryValueShape {
            class: BoundaryValueClass::Record {
                first_field: 1,
                field_count: 1,
            },
            ..record
        };
        assert!(validate_record_shape(&signature, changed_span, &codec, &semantic).is_err());
    }
}
