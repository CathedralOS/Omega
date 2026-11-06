//! Ordinary call writes relocate exact descriptor permissions. The executable
//! callee graph supplies slot-to-slot effects; the caller's pre-call frontier
//! supplies identities and captured backing. Replacing carrier storage does
//! not redirect a detached child's historical parent to the new descriptor.
use super::*;
use std::borrow::Cow;
use terminal_psi::{BoundaryMachineDeclaration, StructuralPathSegment};
use terminal_semantics::descriptor_lifetimes::descriptor_path;
use terminal_semantics::descriptor_results::{DescriptorAnalysis, DescriptorStoredOrigin};

struct SlotEffect {
    destination: StructuralArgument,
    sources: Vec<SlotSource>,
}

enum SlotSource {
    Incoming(StructuralArgument),
    BorrowedStorage(StructuralArgument),
}

/// Prepared with the other call effects once for a current optimization unit.
pub(crate) struct ReferenceCallEffects {
    functions: BTreeMap<MachineId, Option<Vec<SlotEffect>>>,
    declarations: Vec<StructuralTypeDeclaration>,
    outputs: Option<crate::reference_outputs::ReferenceOutputPreparation>,
}

impl ReferenceCallEffects {
    pub(crate) fn new(
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        boundaries: &[&BoundaryMachineDeclaration],
        dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Self {
        let declarations = types
            .values()
            .map(|value| (**value).clone())
            .collect::<Vec<_>>();
        let graph = crate::descriptor_results::optimized_reference_catalog_graph(
            functions.values().copied(),
            boundaries.iter().copied(),
            &declarations,
            dynamic_dispatch,
            |_, node| Some(Cow::Borrowed(&node.operation)),
        );
        let mut analysis = graph
            .as_ref()
            .map(|graph| DescriptorAnalysis::with_reference_slots(&declarations, graph));
        let functions = functions
            .iter()
            .map(|(machine, function)| {
                let effects = (|| {
                    analysis
                        .as_mut()?
                        .stored_effects(*machine)?
                        .into_iter()
                        .map(|effect| {
                            let parameter = function.structural_parameters.get(effect.parameter)?;
                            let path = descriptor_path(
                                &declarations,
                                parameter.structural_type,
                                &effect.exact_path()?,
                            )?;
                            let access = if let Some(crossing) = path
                                .iter()
                                .rposition(|segment| *segment == StructuralPathSegment::Referent)
                            {
                                let reference = access_path_type(
                                    types,
                                    parameter.structural_type,
                                    &path[..crossing],
                                )?;
                                reference_access(types, reference)?
                            } else {
                                parameter.access
                            };
                            let destination = StructuralArgument {
                                place: parameter.place,
                                path,
                                access,
                            };
                            let sources = effect
                                .origins
                                .into_iter()
                                .map(|origin| {
                                    match origin {
                                        DescriptorStoredOrigin::IncomingSlot { position, path } => {
                                            let parameter = function.structural_parameters.get(position)?;
                                            Some(SlotSource::Incoming(StructuralArgument {
                                                place: parameter.place,
                                                path: descriptor_path(&declarations, parameter.structural_type, &path)?,
                                                access: parameter.access,
                                            }))
                                        }
                                        DescriptorStoredOrigin::Parameter { position, projection } => {
                                            // Conservative overlap geometry is not an exact
                                            // address from which a new permission can be born.
                                            if !projection.exact || projection.indirect
                                                || projection != terminal_semantics::borrowed_storage::Projection::new(&projection.path) {
                                                return None;
                                            }
                                            let source_parameter = function.structural_parameters.get(position)?;
                                            let reference = access_path_type(types, parameter.structural_type, &destination.path)?;
                                            let source = StructuralArgument {
                                                place: source_parameter.place,
                                                path: projection.path,
                                                access: reference_access(types, reference)?,
                                            };
                                            if reference_source_type(function, types, &source)
                                                != leaf_referent(types, reference, &[]) {
                                                return None;
                                            }
                                            Some(SlotSource::BorrowedStorage(source))
                                        }
                                        // A possible contents set cannot mint an exact
                                        // reference permission at an executable address.
                                        DescriptorStoredOrigin::Invocation { .. }
                                        | DescriptorStoredOrigin::IncomingContents { .. }
                                        | DescriptorStoredOrigin::Provider(_)
                                        | DescriptorStoredOrigin::Empty | DescriptorStoredOrigin::Literal { .. } | DescriptorStoredOrigin::AccessFormation { .. } => None,
                                    }
                                })
                                .collect::<Option<Vec<_>>>()?;
                            (!sources.is_empty()).then_some(SlotEffect {
                                destination,
                                sources,
                            })
                        })
                        .collect::<Option<Vec<_>>>()
                })();
                (*machine, effects)
            })
            .collect();
        Self {
            functions,
            declarations,
            outputs: None,
        }
    }

    pub(crate) fn use_output_preparation(
        &mut self,
        outputs: crate::reference_outputs::ReferenceOutputPreparation,
    ) {
        self.outputs = Some(outputs);
    }

    fn completed_outputs(
        &self,
        machine: MachineId,
    ) -> Option<std::sync::Arc<crate::reference_outputs::ReferenceBodyOutputs>> {
        self.outputs.as_ref()?.borrow().body(machine)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn stage(
        &self,
        function: &PsiOptimizationFunction,
        functions: &BTreeMap<MachineId, &PsiOptimizationFunction>,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        borrowed_sources: &BorrowedReferenceSources,
        block: BlockId,
        node: u32,
        operation: &O,
        live: &[LiveReference],
        moved: &BTreeSet<ReferenceOccurrenceIdentity>,
    ) -> Result<StoredReferences, OptimizationUnitValidationError> {
        let O::CallStructural {
            callee,
            structural_arguments,
            ..
        } = operation
        else {
            return Ok(StoredReferences::default());
        };
        let fail = |reason| invalid(function, block, Some(node), reason);
        let needs_outputs = structural_arguments.iter().any(|argument| {
            structural_source_contract(function, argument.place)
                .and_then(|signature| {
                    access_path_type(types, signature.structural_type, &argument.path)
                })
                .is_none_or(|selected| {
                    argument.access == StructuralAccess::Owned
                        && contains_reference(types, selected)
                        || terminal_semantics::descriptor_results::DescriptorSlotKind::References
                            .writable_slots(
                                &self.declarations,
                                argument.place,
                                selected,
                                argument.access,
                            )
                            .is_none_or(|slots| !slots.is_empty())
                })
        }) || matches!(operation, O::CallStructural {
            result: AbstractStructuralCallResult::Structural { result, .. }, ..
        } if contains_reference(types, result.structural_type));
        if self.outputs.is_some()
            && needs_outputs
            && let Some(machine) = callee.machine()
        {
            let body = self.completed_outputs(machine).ok_or_else(|| {
                OptimizationUnitValidationError::ReferenceOutputCompletion {
                    machine: function.machine,
                    callee: machine,
                    block,
                    node,
                    reason: self
                        .outputs
                        .as_ref()
                        .and_then(|outputs| {
                            outputs
                                .borrow()
                                .unavailable_reason(machine)
                                .map(str::to_owned)
                        })
                        .unwrap_or_else(|| {
                            "no independently completed reference-output body".to_owned()
                        }),
                }
            })?;
            let callee = functions
                .get(&machine)
                .ok_or_else(|| fail("completed reference callee is absent"))?;
            let result = match operation {
                O::CallStructural {
                    result: AbstractStructuralCallResult::Structural { result, .. },
                    ..
                } => Some(result),
                _ => None,
            };
            return super::call_outputs::bind(
                function,
                callee,
                &body,
                types,
                borrowed_sources,
                block,
                node,
                structural_arguments,
                result,
                live,
                moved,
            );
        }
        let effects = callee
            .machine()
            .and_then(|machine| self.functions.get(&machine))
            .and_then(Option::as_ref);
        let Some(effects) = effects else {
            if structural_arguments.iter().any(|argument| {
                structural_source_contract(function, argument.place)
                    .and_then(|signature| {
                        access_path_type(types, signature.structural_type, &argument.path)
                    })
                    .is_none_or(|selected| {
                        terminal_semantics::descriptor_results::DescriptorSlotKind::References
                            .writable_slots(
                                &self.declarations,
                                argument.place,
                                selected,
                                argument.access,
                            )
                            .is_none_or(|slots| !slots.is_empty())
                    })
            }) {
                return Err(fail(
                    "ordinary call has no complete stored reference effects",
                ));
            }
            return Ok(StoredReferences::default());
        };
        if effects.is_empty() {
            return Ok(StoredReferences::default());
        }
        let callee = callee
            .machine()
            .and_then(|machine| functions.get(&machine))
            .ok_or_else(|| fail("stored reference callee is absent"))?;
        let mut replacements = Vec::new();
        let mut established = BTreeSet::new();
        for effect in effects {
            let destination = call_source(
                &callee.structural_parameters,
                structural_arguments,
                &effect.destination,
            )
            .ok_or_else(|| fail("stored reference destination argument is absent"))?;
            if !matches!(
                destination.access,
                StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
            ) {
                return Err(fail(
                    "stored reference destination lacks exclusive carrier access",
                ));
            }
            let destination = resolve_slot(function, types, block, node, live, &destination)?;
            let residents = live
                .iter()
                .filter(|reference| {
                    reference.carrier == destination.place
                        && reference.carrier_path == destination.path
                })
                .count();
            // Exact writable external storage may receive a blind replacement.
            // No old permission is fabricated: any incoming/self source below
            // still has to identify one genuinely live descriptor.
            if residents > 1
                || (residents == 0
                    && !(destination.path.contains(&StructuralPathSegment::Referent)
                        && function.structural_parameter(destination.place).is_some()))
            {
                return Err(fail("stored reference destination is absent or ambiguous"));
            }
            let expected = slot_type(function, types, &destination)
                .ok_or_else(|| fail("stored reference destination type is absent"))?;
            let mut alternatives = Vec::new();
            for source in &effect.sources {
                let formal = match source {
                    SlotSource::Incoming(formal) | SlotSource::BorrowedStorage(formal) => formal,
                };
                let actual =
                    call_source(&callee.structural_parameters, structural_arguments, formal)
                        .ok_or_else(|| fail("stored reference source argument is absent"))?;
                let reference = match source {
                    SlotSource::Incoming(_) => {
                        let source_slot =
                            resolve_slot(function, types, block, node, live, &actual)?;
                        if slot_type(function, types, &source_slot) != Some(expected) {
                            return Err(fail("stored reference transfer changes descriptor type"));
                        }
                        let mut matching = live.iter().filter(|reference| {
                            reference.carrier == source_slot.place
                                && reference.carrier_path == source_slot.path
                        });
                        let mut reference = matching
                            .next()
                            .cloned()
                            .ok_or_else(|| fail("stored reference source is not live"))?;
                        if matching.next().is_some() {
                            return Err(fail("stored reference source is ambiguous"));
                        }
                        if source_slot != destination && !moved.contains(&reference.identity) {
                            if reference_access(types, expected)
                                != Some(StructuralAccess::SharedBorrow)
                            {
                                return Err(fail(
                                    "stored exclusive reference is not transferred by its argument",
                                ));
                            }
                            reference.identity = formed_identity(
                                function,
                                block,
                                node,
                                destination.place,
                                destination.path.clone(),
                            )?;
                            established.insert(reference.identity.clone());
                        }
                        reference.carrier = destination.place;
                        reference.carrier_path = destination.path.clone();
                        reference
                    }
                    SlotSource::BorrowedStorage(_) => {
                        if reference_source_type(function, types, &actual)
                            != leaf_referent(types, expected, &[])
                            || !leaf_access_within(types, expected, &[], actual.access)
                        {
                            return Err(fail(
                                "stored reference birth changes source type or permission",
                            ));
                        }
                        let (root, parent) = normalized_source(
                            function,
                            types,
                            borrowed_sources,
                            block,
                            node,
                            live,
                            &actual,
                        )?;
                        let identity = formed_identity(
                            function,
                            block,
                            node,
                            destination.place,
                            destination.path.clone(),
                        )?;
                        established.insert(identity.clone());
                        LiveReference {
                            identity,
                            carrier: destination.place,
                            carrier_path: destination.path.clone(),
                            root,
                            parent,
                        }
                    }
                };
                if !alternatives.contains(&reference) {
                    alternatives.push(reference);
                }
            }
            let replacement = combine_alternative_references(function, block, node, alternatives)?;
            if replacements.iter().any(|existing: &LiveReference| {
                existing.carrier == replacement.carrier
                    && existing.carrier_path == replacement.carrier_path
            }) {
                return Err(fail(
                    "ordinary call writes one reference slot more than once",
                ));
            }
            // A parent's identity names the permission a child captured, even
            // after the parent's old carrier bytes have been replaced.
            if live.iter().any(|old| old.identity == replacement.identity && old != &replacement
                && !moved.contains(&old.identity)
                && live.iter().any(|child| matches!(&child.parent, ReferenceParent::Reference(identity) if identity == &old.identity))) {
                return Err(fail("stored reference replacement reuses a historical parent identity"));
            }
            replacements.push(replacement);
        }
        Ok(StoredReferences {
            replacements,
            returned: None,
            established,
            retained_identities: live
                .iter()
                .filter(|reference| !moved.contains(&reference.identity))
                .map(|reference| reference.identity.clone())
                .collect(),
        })
    }
}

pub(super) fn slot_type(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    slot: &ReferenceIdentity,
) -> Option<StructuralTypeId> {
    let selected = leaf_type(
        types,
        structural_source_contract(function, slot.place)?.structural_type,
        &slot.path,
    )?;
    reference_access(types, selected)?;
    Some(selected)
}

/// Resolve descriptor storage through actual references, preserving each pointer
/// crossing as a permission use. The final slot itself is never dereferenced.
pub(super) fn resolve_slot(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    block: BlockId,
    node: u32,
    live: &[LiveReference],
    argument: &StructuralArgument,
) -> Result<ReferenceIdentity, OptimizationUnitValidationError> {
    let Some(crossing) = argument
        .path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
    else {
        return Ok(ReferenceIdentity {
            place: argument.place,
            path: argument.path.clone(),
        });
    };
    let reject = |reason| invalid(function, block, Some(node), reason);
    let custody = access_reference_custody(function, types, block, node, live, argument)?;
    let root = structural_source_contract(function, argument.place)
        .ok_or_else(|| reject("stored reference root is absent"))?;
    let reference = access_path_type(types, root.structural_type, &argument.path[..crossing])
        .ok_or_else(|| reject("stored reference crossing is untyped"))?;
    let referent = leaf_referent(types, reference, &[])
        .ok_or_else(|| reject("stored reference crossing has no referent"))?;
    let mut slots = Vec::new();
    for origin in custody.origins {
        let mut slot = match origin {
            ReferenceOrigin::Primitive(place) => ReferenceIdentity {
                place,
                path: Vec::new(),
            },
            ReferenceOrigin::IngressLeaf(identity) => identity
                .exact_identity()
                .ok_or_else(|| reject("stored reference carrier has no exact storage identity"))?,
            ReferenceOrigin::Invocation { .. } | ReferenceOrigin::Alternatives { .. } => {
                return Err(reject("stored reference carrier has alternative storage"));
            }
        };
        slot = storage_cursor(function, types, slot, referent)
            .ok_or_else(|| reject("stored reference origin has no exact storage cursor"))?;
        slot.path.extend_from_slice(&argument.path[crossing + 1..]);
        if !slots.contains(&slot) {
            slots.push(slot);
        }
    }
    if slots.len() != 1 {
        return Err(reject("stored reference requires one exact carrier slot"));
    }
    Ok(slots.remove(0))
}

#[derive(Default)]
pub(super) struct StoredReferences {
    pub(super) replacements: Vec<LiveReference>,
    pub(super) returned: Option<Vec<LiveReference>>,
    pub(super) established: BTreeSet<ReferenceOccurrenceIdentity>,
    pub(super) retained_identities: BTreeSet<ReferenceOccurrenceIdentity>,
}

impl StoredReferences {
    pub(super) fn commit(
        self,
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        block: BlockId,
        node: u32,
        live: &mut Vec<LiveReference>,
    ) -> Result<(), OptimizationUnitValidationError> {
        if self.replacements.is_empty() {
            return Ok(());
        }
        let mut graph = live.iter().chain(&self.replacements).collect::<Vec<_>>();
        graph.sort_by(|left, right| left.identity.cmp(&right.identity));
        for replacement in &self.replacements {
            if live.iter().any(|old| {
                old.identity == replacement.identity
                    && (self.established.contains(&replacement.identity) || old != replacement)
            }) && graph.iter().any(|child| {
                matches!(&child.parent,
                    ReferenceParent::Reference(parent) if parent == &replacement.identity)
            }) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "stored reference replacement reuses a historical parent occurrence",
                ));
            }
        }
        let ancestor = |ancestor: &ReferenceOccurrenceIdentity, child: &LiveReference| {
            let mut parent = &child.parent;
            for _ in 0..=graph.len() {
                let ReferenceParent::Reference(identity) = parent else {
                    return false;
                };
                if identity == ancestor {
                    return true;
                }
                let Ok(position) = graph.binary_search_by(|row| row.identity.cmp(identity)) else {
                    return false;
                };
                parent = &graph[position].parent;
            }
            false
        };
        let replaced = |reference: &LiveReference| {
            self.replacements.iter().any(|replacement| {
                replacement.carrier == reference.carrier
                    && replacement.carrier_path == reference.carrier_path
            })
        };
        let mut staged = live
            .iter()
            .filter(|reference| !replaced(reference))
            .cloned()
            .collect::<Vec<_>>();
        let mut publications = staged
            .iter()
            .filter(|reference| !self.retained_identities.contains(&reference.identity))
            .cloned()
            .collect::<Vec<_>>();
        for replacement in self.replacements.iter().cloned() {
            let permission = |reference: &LiveReference| {
                slot_type(
                    function,
                    types,
                    &ReferenceIdentity {
                        place: reference.carrier,
                        path: reference.carrier_path.clone(),
                    },
                )
                .and_then(|selected| reference_access(types, selected))
            };
            if publications.iter().any(|reference| {
                origins_overlap(&reference.root, &replacement.root)
                    && (permission(reference) != Some(StructuralAccess::SharedBorrow)
                        || permission(&replacement) != Some(StructuralAccess::SharedBorrow))
                    && !ancestor(&reference.identity, &replacement)
                    && !ancestor(&replacement.identity, reference)
            }) {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "stored and returned reference effects alias an exclusive origin",
                ));
            }

            if staged
                .iter()
                .any(|reference| reference.identity == replacement.identity)
            {
                return Err(invalid(
                    function,
                    block,
                    Some(node),
                    "stored and returned reference effects duplicate an identity",
                ));
            }
            publications.push(replacement.clone());
            staged.push(replacement);
        }
        staged.sort_by(|left, right| left.identity.cmp(&right.identity));
        *live = staged;
        Ok(())
    }
}
