//! Prove a thin scalar export against every physical ingress in this closed unit.
//!
//! Local origin transport remains with storage_origin_input. This query joins
//! parameter obligations to actual ordinary calls (including terminal calls),
//! preserving source coordinates and the original address. A cycle needs real
//! backing, and opaque ingress never becomes a native-width assumption.
use crate::SelectedInstructionError;
use legalized_operations::{
    LegalizedOperationPlan, LegalizedScalarArgument, LegalizedScalarCall, LegalizedScalarFunction,
    LegalizedScalarInstructionKind as I, LegalizedScalarTerminator as T,
};
use semantic_vocabulary::{MachineId, StructuralPlaceKind};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralArgument, StructuralTypeShape};

pub(super) struct ForeignScalarBackings<'program> {
    program: &'program LegalizedOperationPlan,
    incoming: BTreeMap<MachineId, Vec<(usize, &'program LegalizedScalarCall)>>,
    opaque_ingress: BTreeSet<MachineId>,
}
struct Coordinate {
    function: usize,
    source: StructuralArgument,
    dependencies: Vec<usize>,
    backing: bool,
}
impl<'program> ForeignScalarBackings<'program> {
    pub(super) fn new(program: &'program LegalizedOperationPlan) -> Self {
        let mut context = Self {
            program,
            incoming: BTreeMap::new(),
            opaque_ingress: BTreeSet::from([program.entry]),
        };
        for (position, function) in program.scalar_functions.iter().enumerate() {
            for instruction in function.instructions() {
                if let I::Call(call) = &instruction.kind {
                    context.retain_call(position, call);
                }
            }
            for block in &function.blocks {
                if let T::InvokeNoNormal { call, .. } = &block.terminator {
                    context.retain_call(position, call);
                }
            }
        }
        context
    }
    fn retain_call(&mut self, caller: usize, call: &'program LegalizedScalarCall) {
        match call.target {
            target_operations::NativeCallTarget::Machine(callee) => {
                self.incoming
                    .entry(callee)
                    .or_default()
                    .push((caller, call));
            }
            target_operations::NativeCallTarget::Dynamic { .. } => {
                // This reduced unit has no selector table. Every compatible
                // entry is a possible opaque ingress, rather than a guessed
                // closed direct-call roster. Callback thunks are independent
                // units and receive the same entry fence above.
                self.opaque_ingress.extend(
                    self.program
                        .scalar_functions
                        .iter()
                        .filter(|function| function.call_plan == call.call_plan)
                        .map(|function| function.machine),
                );
            }
        }
    }
    pub(super) fn matches(
        &self,
        function: &LegalizedScalarFunction,
        source: &StructuralArgument,
        width: u32,
    ) -> Result<bool, SelectedInstructionError> {
        let function = self
            .program
            .scalar_functions
            .iter()
            .position(|candidate| std::ptr::eq(candidate, function))
            .ok_or_else(|| {
                SelectedInstructionError::custody_because(
                    "foreign backing query lost its exact closed function",
                )
            })?;
        let mut coordinates = vec![Coordinate {
            function,
            source: source.clone(),
            dependencies: Vec::new(),
            backing: false,
        }];
        let mut cursor = 0;
        while cursor < coordinates.len() {
            let function_index = coordinates[cursor].function;
            let function = &self.program.scalar_functions[function_index];
            let Some(origins) = super::storage_origin_input::scalar_storage_origins(
                function,
                &coordinates[cursor].source,
            )?
            else {
                return Ok(false);
            };
            let signature = function
                .structural
                .as_ref()
                .ok_or_else(SelectedInstructionError::custody)?;
            for origin in origins {
                let origin = match origin {
                    super::storage_origin_input::ScalarStorageOrigin::Width(actual) => {
                        if actual != width {
                            return Ok(false);
                        }
                        coordinates[cursor].backing = true;
                        continue;
                    }
                    super::storage_origin_input::ScalarStorageOrigin::Source(origin) => origin,
                };
                let declaration = signature
                    .structural_place(origin.place)
                    .ok_or_else(SelectedInstructionError::custody)?;
                let pointer_ingress =
                    super::reference_input::borrowed_scalar_source(function, origin.place)
                        .is_some()
                        || origin
                            .path
                            .contains(&terminal_psi::StructuralPathSegment::Referent);
                // Preserve exact declaration-local storage geometry (including
                // fields of borrowed records). Only a scalar descriptor or an
                // actual pointer crossing has an unknown incoming codec.
                if !pointer_ingress {
                    let root = super::reference_input::root_type(function, origin.place)
                        .ok_or_else(SelectedInstructionError::custody)?;
                    let footprint = inline_footprint(function, root, &origin.path);
                    if let Some(actual) = footprint {
                        if actual != width {
                            return Ok(false);
                        }
                        coordinates[cursor].backing = true;
                        continue;
                    }
                }
                if let StructuralPlaceKind::Parameter { position } = declaration.kind {
                    if self.opaque_ingress.contains(&function.machine) {
                        return Ok(false);
                    }
                    let Some(calls) = self
                        .incoming
                        .get(&function.machine)
                        .filter(|calls| !calls.is_empty())
                    else {
                        return Ok(false);
                    };
                    for (caller, call) in calls {
                        let actual = call
                            .arguments
                            .iter()
                            .filter_map(|argument| match argument {
                                LegalizedScalarArgument::Structural { semantic, .. } => {
                                    Some(semantic)
                                }
                                _ => None,
                            })
                            .nth(position as usize)
                            .ok_or_else(|| {
                                SelectedInstructionError::custody_because(
                                    "foreign backing ingress lost its actual structural argument",
                                )
                            })?;
                        let mut source = actual.clone();
                        source.path.extend_from_slice(&origin.path);
                        let existing = coordinates.iter().position(|previous| {
                            previous.function == *caller
                                && terminal_semantics::same_descriptor_coordinate(
                                    &previous.source,
                                    &source,
                                )
                        });
                        let dependency = if let Some(existing) = existing {
                            existing
                        } else {
                            // A growing recursive projection has an unbounded
                            // origin relation. Keep it unknown; a finite cycle
                            // with an actual backing remains expressible below.
                            if coordinates.iter().any(|previous| {
                                previous.function == *caller
                                    && previous.source.place == source.place
                                    && source.path.len() > previous.source.path.len()
                                    && source.path.starts_with(&previous.source.path)
                            }) {
                                return Ok(false);
                            }
                            let dependency = coordinates.len();
                            coordinates.push(Coordinate {
                                function: *caller,
                                source,
                                dependencies: Vec::new(),
                                backing: false,
                            });
                            dependency
                        };
                        coordinates[cursor].dependencies.push(dependency);
                    }
                } else {
                    return Ok(false);
                }
            }
            cursor += 1;
        }
        // Every discovered alternative was checked above. Ground all cycles
        // in concrete storage; recursion alone cannot establish a footprint.
        let mut grounded = coordinates
            .iter()
            .map(|coordinate| coordinate.backing)
            .collect::<Vec<_>>();
        loop {
            let mut changed = false;
            for (position, coordinate) in coordinates.iter().enumerate() {
                if !grounded[position]
                    && coordinate
                        .dependencies
                        .iter()
                        .any(|dependency| grounded[*dependency])
                {
                    grounded[position] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        Ok(grounded.into_iter().all(|grounded| grounded))
    }
}

fn inline_footprint(
    function: &LegalizedScalarFunction,
    root: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<u32> {
    let signature = function.structural.as_ref()?;
    let codec = crate::structural_inputs::structural_reference_input::projected_scalar_codec(
        root,
        path,
        &signature.structural_types,
    );
    if let Some(codec) = codec {
        return Some(u32::from(codec.byte_size()));
    }
    // A view's conservative backing roster can retain a whole homogeneous
    // array. Query its exact element storage without inventing an index,
    // address, access permission or a new default layout.
    if !path.is_empty() {
        return None;
    }
    let StructuralTypeShape::FixedArray { element, .. } = signature.structural_type(root)?.shape
    else {
        return None;
    };
    crate::structural_inputs::structural_reference_input::projected_scalar_codec(
        element,
        &[],
        &signature.structural_types,
    )
    .map(|codec| u32::from(codec.byte_size()))
}
