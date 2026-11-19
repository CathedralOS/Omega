// We correlate actual worker pointers with captured typed endpoints of the
// current invocation. Pointer equality only selects a candidate while Core
// authenticates capture identity, lifetime anchors, access permissions, and
// transactional descriptor restoration.

use super::values::ReferenceField;
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

pub(super) struct CapturedInput<'a> {
    pub path: Vec<Path>,
    pub loan: &'a TerminalOriginalStorageLoan,
    pub codec: &'a StorageCodec,
    pub bytes: Vec<u8>,
}

pub(super) struct ReferenceOutputs {
    types: Vec<StructuralTypeDeclaration>,
    parameters: Vec<StructuralTypeId>,
    outputs: Vec<BoundaryReferenceOutput>,
    result_regions: Vec<terminal_psi::TerminalDynamicReferenceRegion>,
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
            result_regions: declaration.result_reference_regions.clone(),
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

    pub fn input_reference<'a>(
        &'a self,
        native: &NativeContext,
        argument: usize,
        parent_path: &[Path],
        parent: &TerminalOriginalStorageLoan,
        field: &ReferenceField,
    ) -> Result<CapturedInput<'a>, TerminalEffectRejection> {
        let mut path = parent_path.to_vec();
        path.extend_from_slice(&field.path);
        path.push(Path::Referent);
        let mut matching = self
            .captures
            .iter()
            .filter(|capture| capture.argument == argument && capture.path == path);
        let capture = matching
            .next()
            .ok_or_else(|| rejection("native input reference is outside the captured graph"))?;
        if matching.next().is_some()
            || capture.view != field.referent
            || !StructuralAccess::SharedBorrow.admits_referent(capture.access)
        {
            return Err(rejection(
                "native input reference changed its exact typed/readable capture",
            ));
        }
        let codec = self
            .codecs
            .get(&capture.view)
            .ok_or_else(|| rejection("native input reference lost its complete codec"))?;
        let (home, offset) = native.loan(&capture.loan, codec)?;
        let target = NativePointerTarget {
            home,
            offset,
            byte_size: codec.extent,
            alignment: codec.alignment,
        };
        let slot = native.reference_slot(parent, &field.path)?;
        if !native
            .sponsor
            .borrow_mut()
            .with(|process| process.pointer_matches(slot, target))?
        {
            return Err(rejection(
                "native input pointer differs from its captured typed endpoint",
            ));
        }
        let mut bytes = vec![0; codec.extent];
        native
            .sponsor
            .borrow_mut()
            .with(|process| process.read_home(home, offset, &mut bytes))?;
        Ok(CapturedInput {
            path,
            loan: &capture.loan,
            codec,
            bytes,
        })
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
                        && output.access.admits_referent(*access)
                }
                BoundaryLifetimeDependency::Stored {
                    argument_index,
                    path: anchor,
                    access,
                } => {
                    *argument_index as usize == capture.argument
                        && output.access.admits_referent(*access)
                        && path
                            .iter()
                            .rposition(|step| matches!(step, StructuralPathSegment::Referent))
                            .is_some_and(|position| &path[..position] == anchor.as_slice())
                }
            }))
    }

    pub fn returned_reference(
        &self,
        native: &NativeContext,
        buffers: &[TerminalBoundaryStorageBuffer],
        returned: &[u8],
        referent: StructuralTypeId,
        access: StructuralAccess,
    ) -> Result<terminal_interpreter::TerminalBoundaryReferenceSelection, TerminalEffectRejection>
    {
        for capture in &self.captures {
            if capture.view != referent || !access.admits_referent(capture.access) {
                continue;
            }
            let path = self.path(capture)?;
            let allowed = self.result_regions.iter().any(|region| {
                region.path.is_empty()
                    && region.access == access
                    && region.origin
                        == (terminal_psi::TerminalDynamicReferenceOrigin::Parameter {
                            argument_index: capture.argument as u32,
                        })
                    && path.starts_with(terminal_semantics::reference_source_storage_path(
                        self.types.iter(),
                        self.parameters[capture.argument],
                        &region.source_path,
                    ))
            });
            if !allowed {
                continue;
            }
            let codec = self
                .codecs
                .get(&capture.view)
                .ok_or_else(|| rejection("returned reference candidate has no exact codec"))?;
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
                .with(|process| process.returned_pointer_matches(returned, target))?
            {
                return buffers
                    .iter()
                    .find(|buffer| argument(buffer) == capture.argument)
                    .ok_or_else(|| rejection("returned reference lost its captured input buffer"))?
                    .select_reference(&capture.path, access);
            }
        }
        Err(rejection(
            "native returned reference is outside its captured typed input endpoints",
        ))
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
                    || !output.access.admits_referent(capture.access)
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
        TerminalBoundaryStorageBuffer::ByteView(buffer) => buffer.argument_index(),
    }
}
