//! Correlate an actual worker pointer with a typed endpoint of this invocation.
//! Equality only selects a candidate. Core still authenticates capture identity,
//! every lifetime anchor, access and transactional descriptor restoration.
//! Pointers outside this closed captured graph remain explicitly unsupported.

use super::{NativeContext, StorageCodec, rejection};
use native_provider_runtime::NativePointerTarget;
use semantic_vocabulary::{CanonicalStructuralPathSegment as Path, StructuralTypeId};
use std::collections::BTreeMap;
use terminal_interpreter::{
    TerminalBoundaryStorageBuffer, TerminalEffectRejection, TerminalOriginalStorageLoan,
    TerminalOriginalStorageRequest, TerminalProgram,
};
use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryMachineDeclaration, BoundaryReferenceDestination,
    BoundaryReferenceOutput, StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration,
};

struct Capture {
    argument: usize,
    path: Vec<Path>,
    access: StructuralAccess,
    view: StructuralTypeId,
    loan: TerminalOriginalStorageLoan,
}
pub(super) struct ReferenceOutputs {
    types: Vec<StructuralTypeDeclaration>,
    parameters: Vec<StructuralTypeId>,
    outputs: Vec<BoundaryReferenceOutput>,
    pub codecs: BTreeMap<StructuralTypeId, StorageCodec>,
    captures: Vec<Capture>,
}
impl ReferenceOutputs {
    pub fn admit(
        program: &TerminalProgram,
        declaration: &BoundaryMachineDeclaration,
        layouts: &[runtime_abi::NativeBorrowedStorageLayout],
    ) -> Self {
        Self {
            types: program.module().structural_types.clone(),
            parameters: declaration
                .structural_parameters
                .iter()
                .map(|parameter| parameter.structural_type)
                .collect(),
            outputs: declaration.reference_outputs.clone(),
            // Unsupported unrelated layouts do not become codecs. Every
            // captured view below must find its own complete admitted codec.
            codecs: layouts
                .iter()
                .filter_map(|layout| {
                    StorageCodec::admit(program, layouts, layout.structural_type)
                        .ok()
                        .map(|codec| (layout.structural_type, codec))
                })
                .collect(),
            captures: Vec::new(),
        }
    }
    pub fn capture(&mut self, requests: &[TerminalOriginalStorageRequest]) {
        self.captures = requests
            .iter()
            .map(|request| Capture {
                argument: request.argument_index(),
                path: request.argument_path().to_vec(),
                access: request.access(),
                view: request.view_type(),
                loan: request.loan(),
            })
            .collect();
    }
    fn path(
        &self,
        capture: &Capture,
    ) -> Result<Vec<StructuralPathSegment>, TerminalEffectRejection> {
        terminal_semantics::canonical_structural_storage_path(
            self.types.iter(),
            self.parameters[capture.argument],
            &capture.path,
        )
        .ok_or_else(|| rejection("native reference candidate lost its exact typed selection path"))
    }
    fn anchored(
        &self,
        output: &BoundaryReferenceOutput,
        capture: &Capture,
    ) -> Result<bool, TerminalEffectRejection> {
        let path = self.path(capture)?;
        Ok(output
            .dependencies
            .iter()
            .any(|dependency| match dependency {
                BoundaryLifetimeDependency::Referent {
                    argument_index,
                    access,
                } => {
                    *argument_index as usize == capture.argument
                        && path
                            .iter()
                            .all(|step| !matches!(step, StructuralPathSegment::Referent))
                        && covers(*access, output.access)
                }
                BoundaryLifetimeDependency::Stored {
                    argument_index,
                    path: anchor,
                    access,
                } => {
                    *argument_index as usize == capture.argument
                        && covers(*access, output.access)
                        && path
                            .iter()
                            .rposition(|step| matches!(step, StructuralPathSegment::Referent))
                            .is_some_and(|position| &path[..position] == anchor.as_slice())
                }
            }))
    }
    pub fn complete(
        &self,
        native: &NativeContext,
        buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<(), TerminalEffectRejection> {
        for output in &self.outputs {
            let BoundaryReferenceDestination::Parameter(index) = output.destination;
            let destination = self
                .captures
                .iter()
                .find(|capture| capture.argument == index as usize && capture.path.is_empty())
                .ok_or_else(|| {
                    rejection("native reference output lost its captured destination")
                })?;
            let codec = self
                .codecs
                .get(&destination.view)
                .ok_or_else(|| rejection("native reference destination has no exact codec"))?;
            let field = codec
                .references
                .iter()
                .find(|field| {
                    terminal_semantics::canonical_structural_storage_path(
                        self.types.iter(),
                        destination.view,
                        &field.path,
                    )
                    .as_ref()
                        == Some(&output.path)
                })
                .ok_or_else(|| {
                    rejection("native reference output has no direct produced pointer slot")
                })?;
            if field.access != output.access {
                return Err(rejection(
                    "native reference output changed its declared access",
                ));
            }
            let slot = native.reference_slot(&destination.loan, &field.path)?;
            let mut selected = None;
            for capture in &self.captures {
                if capture.view != field.referent
                    || !covers(capture.access, output.access)
                    || !self.anchored(output, capture)?
                {
                    continue;
                }
                let codec = self
                    .codecs
                    .get(&capture.view)
                    .ok_or_else(|| rejection("native reference candidate has no exact codec"))?;
                let (home, offset) = native.loan(&capture.loan, codec)?;
                let target = NativePointerTarget {
                    home,
                    offset,
                    byte_size: codec.extent,
                    alignment: codec.alignment,
                };
                if native
                    .sponsor
                    .borrow_mut()
                    .with(|process| process.pointer_matches(slot, target))?
                {
                    selected = Some((capture, target));
                    break;
                }
            }
            let (capture, target) = selected.ok_or_else(|| {
                rejection("native reference output is outside its captured typed endpoints")
            })?;
            let source = buffers
                .iter()
                .find(|buffer| argument(buffer) == capture.argument)
                .ok_or_else(|| rejection("native reference selection lost its invocation buffer"))?
                .select_reference(&capture.path, output.access)?;
            let buffer = buffers
                .iter_mut()
                .find(|buffer| argument(buffer) == index as usize)
                .ok_or_else(|| {
                    rejection("native reference destination lost its invocation buffer")
                })?;
            let TerminalBoundaryStorageBuffer::Structural(buffer) = buffer else {
                return Err(rejection(
                    "native reference destination is not structural storage",
                ));
            };
            buffer.replace_reference(&output.path, source)?;
            native.record_reference(slot, target)?;
        }
        native.validate_references()
    }
}
fn argument(buffer: &TerminalBoundaryStorageBuffer) -> usize {
    match buffer {
        TerminalBoundaryStorageBuffer::Scalar(buffer) => buffer.argument_index(),
        TerminalBoundaryStorageBuffer::Structural(buffer) => buffer.argument_index(),
    }
}
fn covers(available: StructuralAccess, requested: StructuralAccess) -> bool {
    available == requested
        || available == StructuralAccess::MutableBorrow
            && requested == StructuralAccess::SharedBorrow
}
