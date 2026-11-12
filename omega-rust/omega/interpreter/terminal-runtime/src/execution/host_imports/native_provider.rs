/*
We admit and execute the selected provider's produced fixed-frame native adapter.

Admission joins the boundary declaration, native telescope, exact borrowed
storage layout, and emitted native foreign adapter product while the interpreter
checks requires, claims, guarantees, and crash routes. No loader name, fixture
identity, or observed C token selects semantics.

The private worker supplies no additional service authority on its own: explicit
filesystem delegation admits exact OS operations through its guest descriptor
namespace, explicit native-service delegation admits package-local service
reach, and standard descriptors retain their finite input and buffered output
bindings selected before the invocation's storage presentation is captured.
*/

mod original_homes;
mod record_fields;
mod record_shape;
mod reference_outputs;
mod service_reach;
mod values;
pub(super) use original_homes::{NativeContext, rejection};
pub(super) use record_shape::{
    IntegerRecordField, matches_integer_record, matches_integer_storage,
    matches_signed_record_array,
};
use values::StorageCodec;

use super::super::TerminalRuntimeError;
use super::scalar_codec::{
    boundary_scalar_extent, decode, encode, is_host_address_carrier, scalar_extent,
};
use calling_conventions::{
    BoundaryCallingApplication, BoundaryNativeParameterOrigin, BoundaryNativeParameterShape,
    BoundaryValueClass, ValueClass,
};
use native_artifact::TerminalRuntimeHostProvisions;
use native_provider_runtime::{
    NativeAdapterId, NativeArgument, NativeHomeId, NativeParameterStorage,
};
use runtime_abi::NativeBorrowedStorageLayout;
use semantic_vocabulary::{BoundaryMachineId, CanonicalStructuralPathSegment as Path, ScalarType};
use target_operations::NormalizedForeignCallBinding;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalOriginalScalarBackings, TerminalOriginalStorageLoan,
    TerminalOriginalStorageRequest, TerminalProgram,
};
use terminal_psi::{
    BoundaryMachineDeclaration, BoundaryMachineResult, BoundaryParameterKind, StructuralAccess,
    TerminalPsiIdentity,
};

pub(in crate::execution::host_imports) struct ReadonlyRecord<'a> {
    native: &'a NativeContext,
    references: &'a reference_outputs::ReferenceOutputs,
    argument: usize,
    path: Vec<Path>,
    loan: &'a TerminalOriginalStorageLoan,
    codec: &'a StorageCodec,
    bytes: Vec<u8>,
}

impl<'a> ReadonlyRecord<'a> {
    fn new(
        native: &'a NativeContext,
        references: &'a reference_outputs::ReferenceOutputs,
        argument: usize,
        loan: &'a TerminalOriginalStorageLoan,
        codec: &'a StorageCodec,
    ) -> Result<Self, TerminalEffectRejection> {
        let (home, offset) = native.loan(loan, codec)?;
        let mut bytes = vec![0; codec.extent];
        native
            .sponsor
            .borrow_mut()
            .with(|process| process.read_home(home, offset, &mut bytes))?;
        Ok(Self {
            native,
            references,
            argument,
            path: Vec::new(),
            loan,
            codec,
            bytes,
        })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn sole_reference(&self) -> Result<Self, TerminalEffectRejection> {
        let [field] = self.codec.references.as_slice() else {
            return Err(rejection(
                "native input changed its complete single-reference roster",
            ));
        };
        let capture = self.references.input_reference(
            self.native,
            self.argument,
            &self.path,
            self.loan,
            field,
        )?;
        Ok(Self {
            native: self.native,
            references: self.references,
            argument: self.argument,
            path: capture.path,
            loan: capture.loan,
            codec: capture.codec,
            bytes: capture.bytes,
        })
    }
}

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
    ByteView {
        index: usize,
        access: StructuralAccess,
    },
}

enum ResultCodec {
    Unit,
    Scalar(ScalarType),
    Record(StorageCodec),
    Reference {
        referent: semantic_vocabulary::StructuralTypeId,
        access: StructuralAccess,
    },
}

pub(super) struct NativeImport<'host> {
    signature: TerminalImportSignature,
    artifact: TerminalPsiIdentity,
    boundary: BoundaryMachineId,
    native: NativeContext,
    adapter: NativeAdapterId,
    parameters: Vec<Parameter>,
    storage_arguments: Vec<usize>,
    layouts: Vec<NativeBorrowedStorageLayout>,
    result: ResultCodec,
    references: reference_outputs::ReferenceOutputs,
    filesystem: Option<super::filesystem::FilesystemCall>,
    filesystem_header: Option<NativeHomeId>, // Private marshalling storage reused across synchronous calls.
    modeled_descriptor: Option<super::descriptor_io::DescriptorIoImport<'host>>,
}

impl<'host> NativeImport<'host> {
    fn scalar_result(
        &self,
        returned: &[u8],
        scalar: ScalarType,
    ) -> Result<terminal_interpreter::TerminalScalarValue, TerminalEffectRejection> {
        if is_host_address_carrier(scalar) {
            self.native.host_address_result(returned, scalar)
        } else {
            decode(returned, scalar, scalar)
        }
    }

    pub(super) fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        binding: &NormalizedForeignCallBinding,
        provisions: Option<&TerminalRuntimeHostProvisions>,
        native: &NativeContext,
        filesystem: Option<super::filesystem::FilesystemCall>,
        input: Option<std::sync::Arc<std::sync::Mutex<Box<dyn std::io::Read + Send + 'host>>>>,
        output: super::buffered_output::BufferedStreams,
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
        if runtime_abi::descriptor_import_identity(locator).is_some() && filesystem.is_none() {
            return Err(missing(
                "descriptor operation requires its explicit modeled I/O capability".into(),
            ));
        }
        if filesystem.is_none() {
            service_reach::admit_additional_services(
                program.module(),
                declaration.owning_service,
                declaration
                    .fixed_service_reach
                    .iter()
                    .chain(&declaration.published_service_ceiling)
                    .copied(),
                native.services(),
            )
            .map_err(|reason| {
                missing(format!(
                    "{reason} for boundary `{}`",
                    program
                        .module()
                        .identities
                        .text(declaration.identity)
                        .map_or("<missing boundary identity>", service_reach::identity_path),
                ))
            })?;
        }
        if declaration.completion != terminal_psi::BoundaryCompletion::NormalReturn
            || !declaration.program_local_root_introductions.is_empty()
            || !binding.has_valid_entry_mechanism()
            || binding.provider_plan_report_identity() == 0
        {
            return Err(missing(
                "native sponsor needs a normal exact call without introduced roots".into(),
            ));
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
        let modeled_descriptor = if filesystem
            .as_ref()
            .is_some_and(|filesystem| filesystem.descriptor_io())
        {
            Some(
                super::descriptor_io::DescriptorIoImport::for_filesystem(
                    program,
                    declaration,
                    binding,
                    layouts,
                    input,
                    output,
                )?
                .ok_or_else(|| {
                    missing("filesystem descriptor lost its exact stream contract".into())
                })?,
            )
        } else {
            None
        };
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
            BoundaryMachineResult::Structural(result)
                if matches!(
                    program
                        .structural_type(result.structural_type)
                        .map(|row| &row.shape),
                    Some(terminal_psi::StructuralTypeShape::Reference { .. })
                ) =>
            {
                let terminal_psi::StructuralTypeShape::Reference { referent, access } = program
                    .structural_type(result.structural_type)
                    .unwrap()
                    .shape
                else {
                    unreachable!()
                };
                let shape = application
                    .signature
                    .result
                    .and_then(|root| application.signature.shapes.get(root as usize))
                    .ok_or_else(|| missing("native reference result has no public shape".into()))?;
                if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty()
                    || !matches!(access, StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow)
                    || shape.class != BoundaryValueClass::Reference || shape.byte_size != 8 || shape.alignment != 8
                    || application.plan.plan().call.result.as_ref().map(|p| p.shape)
                        != Some(calling_conventions::ValueShape::integer(8, 8))
                    || declaration.result_reference_regions.is_empty()
                    || declaration.result_reference_regions.iter().any(|region| {
                        !region.path.is_empty() || region.access != access
                            || !matches!(region.origin, terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index }
                                if declaration.structural_parameters.get(argument_index as usize).is_some_and(|p|
                                    matches!(p.access, StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow)))
                    }) {
                    return Err(missing("native reference result lacks exact captured-input pointer custody".into()));
                }
                StorageCodec::admit(program, layouts, referent)
                    .map_err(|error| missing(error.reason))?;
                ResultCodec::Reference { referent, access }
            }
            BoundaryMachineResult::Structural(result) => {
                let codec = StorageCodec::admit(program, layouts, result.structural_type)
                    .map_err(|error| missing(error.reason))?;
                let shape = application
                    .signature
                    .result
                    .ok_or_else(|| missing("native record result has no public shape".into()))?;
                record_shape::validate(&application.signature, shape, &codec, |identity| {
                    program.structural_type(identity)
                })
                .map_err(|error| missing(error.reason))?;
                ResultCodec::Record(codec)
            }
        };
        let adapter = native
            .sponsor
            .borrow_mut()
            .with(|process| process.install(product, &parameters.iter().map(|parameter| match parameter { Parameter::Structural { access: StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow, codec, .. } => NativeParameterStorage::OriginalReference { byte_size: codec.extent, alignment: codec.alignment }, Parameter::ByteView { .. } => NativeParameterStorage::OriginalByteView, _ => NativeParameterStorage::Value }).collect::<Vec<_>>()))
            .map_err(|error| missing(error.reason))?;
        let references = reference_outputs::ReferenceOutputs::admit(program, declaration, layouts);
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
            references,
            filesystem,
            filesystem_header: None,
            modeled_descriptor,
        }))
    }
}

import_signature_via_clone!(NativeImport<'_>);

fn validate_scalar_shape(
    shape: calling_conventions::BoundaryValueShape,
    scalar: ScalarType,
) -> Result<(), TerminalEffectRejection> {
    let class = if matches!(scalar, ScalarType::IeeeFloat(_)) {
        BoundaryValueClass::Float
    } else {
        BoundaryValueClass::Integer
    };
    if shape.class != class || usize::from(shape.byte_size) != boundary_scalar_extent(scalar)? {
        return Err(rejection(
            "native scalar shape lost its exact carrier class or width",
        ));
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
                ) || physical.byte_size as usize != boundary_scalar_extent(scalar)?
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
                if matches!(program.structural_type(parameter.structural_type).map(|row| &row.shape),
                    Some(terminal_psi::StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView { access })) if access.is_none_or(|access| access == parameter.access))
                {
                    if !matches!(
                        parameter.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    ) || shape.class != BoundaryValueClass::Reference
                        || shape.byte_size != 16
                        || shape.alignment != 8
                        || *physical != calling_conventions::ValueShape::integer(16, 8)
                    {
                        return Err(rejection(
                            "native byte view lost its exact borrowed pointer/length transport",
                        ));
                    }
                    parameters.push(Parameter::ByteView {
                        index,
                        access: parameter.access,
                    });
                    continue;
                }
                let codec = StorageCodec::admit(program, layouts, parameter.structural_type)?;
                match parameter.access {
                    StructuralAccess::Owned => {
                        record_shape::validate(
                            &application.signature,
                            application.signature.parameters[formal],
                            &codec,
                            |identity| program.structural_type(identity),
                        )?;
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

impl TerminalEffectHandler for NativeImport<'_> {
    fn accepts_reference_results(&self, boundary: BoundaryMachineId) -> bool {
        boundary == self.boundary && matches!(self.result, ResultCodec::Reference { .. })
    }

    fn storage_requirements(
        &self,
        boundary: BoundaryMachineId,
        arguments: &[terminal_interpreter::TerminalScalarValue],
    ) -> terminal_interpreter::TerminalCallStorageRequirements<'_> {
        if boundary != self.boundary {
            return terminal_interpreter::TerminalCallStorageRequirements {
                arguments: &[],
                original: false,
            };
        }
        if self
            .filesystem
            .as_ref()
            .is_some_and(|filesystem| filesystem.modeled_stream(arguments))
            && let Some(modeled) = &self.modeled_descriptor
        {
            return modeled.storage_requirements(boundary, arguments);
        }
        terminal_interpreter::TerminalCallStorageRequirements {
            arguments: &self.storage_arguments,
            original: true,
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
        let backings = self
            .native
            .prepare(requests, &self.layouts, &self.references.codecs)?;
        self.references.capture(requests);
        Ok(backings)
    }

    handle_effect_requires_dispatch!(rejection(
        "native import requires complete result and storage dispatch"
    ));

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        bytes: &mut [TerminalBoundaryByteBuffer],
        buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let call = super::call_operands(effect, || {
            rejection("native adapter received a non-call effect")
        })?;
        if self
            .filesystem
            .as_ref()
            .is_some_and(|filesystem| filesystem.modeled_stream(call.arguments))
        {
            return self
                .modeled_descriptor
                .as_mut()
                .ok_or_else(|| rejection("filesystem stream lost its modeled binding"))?
                .handle_effect_with_buffers(effect, bytes, buffers);
        }
        if *call.boundary != self.boundary
            || *call.result != self.signature.result
            || call.arguments.len() != self.signature.scalar_parameters.len()
            || call.structural_arguments.len() != self.signature.structural_parameters.len()
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
            ByteView(native_provider_runtime::NativeHomeId, usize, usize),
        }
        if let Some(descriptor) = &self.modeled_descriptor {
            descriptor.validate_native_transfer(call.arguments, buffers)?;
        }
        let mut inputs = Vec::with_capacity(self.parameters.len());
        let supplied_arguments = self
            .filesystem
            .as_ref()
            .map(|filesystem| filesystem.arguments(call.arguments))
            .transpose()?;
        let native_scalars = supplied_arguments.as_deref().unwrap_or(call.arguments);
        for parameter in &self.parameters {
            inputs.push(match parameter {
                Parameter::ByteView { index, access } => {
                    let mut matching = buffers.iter().filter_map(|buffer| match buffer {
                        TerminalBoundaryStorageBuffer::ByteView(buffer) if buffer.argument_index() == *index => Some(buffer),
                        _ => None,
                    });
                    let buffer = matching.next().ok_or_else(|| rejection("native byte view lost its captured original window"))?;
                    if matching.next().is_some() || buffer.access() != *access {
                        return Err(rejection("native byte view changed access or repeated its capture"));
                    }
                    let loan = buffer.original_storage().ok_or_else(|| rejection("native byte view has no original allocation"))?;
                    let (home, offset, length) = self.native.byte_loan(loan)?;
                    if length as u64 != buffer.length() { return Err(rejection("native byte view changed its captured live length")); }
                    Input::ByteView(home, offset, length)
                }
                Parameter::Scalar { index, scalar } => {
                    let value = native_scalars[*index];
                    if value.scalar_type() != *scalar { return Err(rejection("native scalar actual changed its declared carrier")); }
                    if is_host_address_carrier(*scalar) {
                        Input::Value(self.native.host_address_argument(value)?)
                    } else {
                        let encoded = encode(value, *scalar)?;
                        Input::Value(encoded[..scalar_extent(*scalar)?].to_vec())
                    }
                }
                Parameter::Structural { index, access, codec } => {
                    let mut matching = buffers.iter().filter(|buffer| match buffer { TerminalBoundaryStorageBuffer::Scalar(buffer) => buffer.argument_index() == *index, TerminalBoundaryStorageBuffer::Structural(buffer) => buffer.argument_index() == *index, TerminalBoundaryStorageBuffer::ByteView(_) => false });
                    let buffer = matching.next().ok_or_else(|| rejection("native structural argument lost its captured payload"))?;
                    if matching.next().is_some() { return Err(rejection("native structural argument has repeated captures")); }
                    if let Some(filesystem) = &self.filesystem
                        && filesystem.path_argument(*index) {
                            let TerminalBoundaryStorageBuffer::Structural(buffer) = buffer else { return Err(rejection("filesystem path lost its captured array")); };
                            filesystem.validate_path(*index, &codec.encode(buffer.values())?)?;
                        }
                    match (access, buffer) {
                        (StructuralAccess::Owned, TerminalBoundaryStorageBuffer::Structural(buffer)) if buffer.access() == *access && buffer.structural_type() == codec.structural_type && buffer.original_storage().is_none() => Input::Value(codec.encode(buffer.values())?),
                        (StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow, TerminalBoundaryStorageBuffer::Structural(buffer)) if buffer.access() == *access && buffer.structural_type() == codec.structural_type => {
                            let loan = buffer.original_storage().ok_or_else(|| rejection("native borrowed record has no original allocation"))?;
                            let (home, offset) = self.native.loan(loan, codec)?;
                            if let Some(filesystem) = &self.filesystem {
                                if *access == StructuralAccess::SharedBorrow && filesystem.readonly_record_argument(*index) {
                                    let view = ReadonlyRecord::new(&self.native, &self.references, *index, loan, codec)?;
                                    if let Some(bytes) = filesystem.prepare_readonly_record(*index, &view)? {
                                        let header = match self.filesystem_header {
                                            Some(header) => header,
                                            None => {
                                                let header = self.native.sponsor.borrow_mut().with(|process| process.create_home(codec.extent, codec.alignment))?;
                                                self.filesystem_header = Some(header);
                                                header
                                            }
                                        };
                                        self.native.sponsor.borrow_mut().with(|process| process.write_home(header, 0, &bytes))?;
                                        Input::Loan(header, 0)
                                    } else { Input::Loan(home, offset) }
                                } else { Input::Loan(home, offset) }
                            } else { Input::Loan(home, offset) }
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
                Input::ByteView(home, offset, length) => NativeArgument::ByteView {
                    home: *home,
                    offset: *offset,
                    length: *length,
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
        self.references.complete(&self.native, buffers)?;
        let scalar_result = match self.result {
            ResultCodec::Scalar(scalar) => Some(self.scalar_result(&returned, scalar)?),
            _ => None,
        };
        for parameter in &self.parameters {
            let Parameter::ByteView {
                index,
                access: StructuralAccess::MutableBorrow,
            } = parameter
            else {
                continue;
            };
            let buffer = buffers
                .iter_mut()
                .find_map(|buffer| match buffer {
                    TerminalBoundaryStorageBuffer::ByteView(buffer)
                        if buffer.argument_index() == *index =>
                    {
                        Some(buffer)
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    rejection("native mutable byte completion lost its original window")
                })?;
            buffer.acknowledge_original_write()?;
        }
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
                    TerminalBoundaryStorageBuffer::ByteView(_) => false,
                })
                .ok_or_else(|| rejection("native mutable completion lost its original input"))?;
            let loan = match &*buffer {
                TerminalBoundaryStorageBuffer::Scalar(buffer) => buffer.original_storage(),
                TerminalBoundaryStorageBuffer::Structural(buffer) => buffer.original_storage(),
                TerminalBoundaryStorageBuffer::ByteView(_) => None,
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
                    let native_value = decode(&observed, field.stored, field.logical)?;
                    let replacement = match &self.filesystem {
                        Some(filesystem) => {
                            filesystem.output_scalar(*index, native_value, scalar_result)?
                        }
                        None => None,
                    };
                    if let Some(value) = replacement {
                        let encoded = encode(value, field.stored)?;
                        let encoded = encoded.get(..codec.extent).ok_or_else(|| {
                            rejection("virtualized native scalar exceeds its exact mutable codec")
                        })?;
                        self.native
                            .sponsor
                            .borrow_mut()
                            .with(|process| process.write_home(home, offset, encoded))?;
                    }
                    buffer.replace(replacement.unwrap_or(native_value))?;
                }
                TerminalBoundaryStorageBuffer::Structural(buffer) => {
                    buffer.replace(codec.decode_fields(&observed)?)?
                }
                TerminalBoundaryStorageBuffer::ByteView(_) => {
                    return Err(rejection("fixed codec completion received a byte view"));
                }
            }
        }
        match &self.result {
            ResultCodec::Unit if returned.is_empty() => Ok(TerminalEffectResult::Unit),
            ResultCodec::Unit => Err(rejection("native Unit completion returned a payload")),
            ResultCodec::Scalar(scalar) => {
                let value = self.scalar_result(&returned, *scalar)?;
                let value = match &self.filesystem {
                    Some(filesystem) => filesystem.result(call.arguments, value)?,
                    None => value,
                };
                Ok(TerminalEffectResult::Scalar(value))
            }
            ResultCodec::Record(codec) => Ok(TerminalEffectResult::ScalarAggregate(
                codec.aggregate(&returned)?,
            )),
            ResultCodec::Reference { referent, access } => Ok(
                TerminalEffectResult::CapturedReference(self.references.returned_reference(
                    &self.native,
                    buffers,
                    &returned,
                    *referent,
                    *access,
                )?),
            ),
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
