//! Invalidate live arithmetic reads using their captured storage suppliers.
//! A scalar copied into a local keeps its value; an equality with a live
//! reference projection survives only when the store preserves that read.

use super::Operand;
use super::storage::{ReadSegment, compatible_destination, disjoint};
use facts::PlaceSegment;
use typed_trees::expression::ExpressionHandle;

impl Operand {
    /// None identifies an inline-owned observation handled by the ordinary
    /// path rules. Reference reads supply a complete preservation judgment;
    /// an unavailable supplier or descriptor load cannot prove preservation.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::proof_contracts::arithmetic_domains) fn assignment_reference_read_preserved(
        &self,
        program: &dyn crate::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: Option<&typed_trees::state::State>,
        statement: &typed_trees::statement::StatementNode,
        frames: Option<&crate::CallFrameResolver<'_>>,
        storage: Option<&crate::AssignmentStorageOrigins>,
    ) -> Option<bool> {
        let mut segments = Vec::new();
        let root = self.read_coordinate(&mut segments)?;
        let Some(state) = state else {
            return Some(false);
        };
        let result = (|| {
            let Some(mut reference) = crate::state_root_type(program, state, root) else {
                return if self.is_immutable_snapshot(program, state) {
                    Ok(None)
                } else {
                    Err("root-type")
                };
            };
            let mut path = Vec::new();
            let mut reference_read = false;
            let mut dynamic = None;
            let mut measure = false;
            let mut slice_descriptor_measure = false;
            for (ordinal, segment) in segments.iter().enumerate() {
                while let Some(shell) = crate::reference_shell(program, reference) {
                    if dynamic.is_some() {
                        return Err("uncaptured-inner-selector");
                    }
                    if matches!(segment, ReadSegment::Length)
                        && crate::slice_view_shell(program, reference, &[]).is_some()
                    {
                        slice_descriptor_measure = true;
                    }
                    reference_read = true;
                    reference = shell.referee;
                }
                match segment {
                    ReadSegment::Field(symbol) => {
                        let start = path.len();
                        let parent = program.symbols.get(*symbol).parent;
                        if program.variant_owner(parent).is_some() {
                            path.push(PlaceSegment::Case { variant: parent });
                        }
                        path.push(PlaceSegment::Field { symbol: *symbol });
                        reference = crate::project_type_reference_from_segments(
                            program,
                            reference,
                            &path[start..],
                        )
                        .ok_or("field-type")?;
                    }
                    ReadSegment::Element(index) => {
                        reference =
                            crate::value_custody::places::collection_element_type_reference(
                                program, reference,
                            )
                            .ok_or("element-type")?;
                        path.push(match index {
                            Some(index) => PlaceSegment::FixedIndex { index: *index },
                            None => {
                                dynamic.get_or_insert(path.len());
                                // Geometry only: this marker never reaches an
                                // exact read query or grants an element access.
                                PlaceSegment::Index {
                                    expression: ExpressionHandle::invalid(),
                                }
                            }
                        });
                    }
                    ReadSegment::Length => {
                        if ordinal + 1 != segments.len() {
                            return Err("measure-suffix");
                        }
                        measure = true;
                    }
                    ReadSegment::Case(_) => return Err("unexpected-case-coordinate"),
                }
            }
            if crate::reference_shell(program, reference).is_some() {
                if dynamic.is_some() {
                    return Err("uncaptured-inner-selector");
                }
                reference_read = true;
            }
            if !reference_read {
                return Ok(None);
            }
            if measure
                && crate::stable_slice_descriptor_place(program, state, root, &path).is_some()
            {
                // This owner proves that the private descriptor home is not
                // replaced or exposed. Its extent does not read the backing
                // bytes, including when the view denotes a window.
                return Ok(Some(true));
            }
            let index = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .position(|candidate| std::ptr::eq(candidate, statement))
                .ok_or("statement-owner")?;
            let frames = frames.ok_or("read-owner")?;
            let cutoff = dynamic.unwrap_or(path.len());
            let backing = frames
                .storage_read_backing_before_statement(machine, state, index, root, &path[..cutoff])
                .ok_or("read-backing")?;
            if backing.sources.is_empty() {
                return Err("empty-suppliers");
            }
            let storage = storage.ok_or("write-destinations")?;
            let replaced_binding = if storage.destinations.is_empty() {
                // The write owner uses an empty destination set only for a
                // direct local descriptor replacement. It changes that home,
                // not the storage reached through its previous pointer.
                let assignment = statement.as_assignment().ok_or("binding-assignment")?;
                let place = crate::canonical_place_from_expression(program, assignment.target)
                    .ok_or("binding-home")?;
                let replaced = place.root.as_symbol().ok_or("binding-root")?;
                if !place.segments.is_empty()
                    || program.state_local_declaration(state, replaced).is_none()
                    || !matches!(storage.references.as_ref(), Some(references) if references.is_empty())
                {
                    return Err("binding-role");
                }
                if root == replaced {
                    return Err("replaced-read-binding");
                }
                Some(replaced)
            } else {
                None
            };
            for binding in &backing.bindings {
                if !source_namespace_is_exact(binding) {
                    return Err("binding-namespace");
                }
                // A reference formal's root denotes its incoming referent,
                // not a private home for that formal's descriptor value.
                let borrowed_root = program
                    .state_parameter_by_symbol(state, binding.root)
                    .is_some_and(|parameter| {
                        crate::reference_shell(program, parameter.type_reference).is_some()
                    });
                if !borrowed_root && crate::value_custody::places::binding_observations::stable_reference_descriptor_place(
                    program, state, binding.root, &binding.segments,
                ).is_some() {
                    continue;
                }
                if replaced_binding.is_some() {
                    // An unstable loaded slot needs a physical comparison.
                    // A private replacement supplies no pointee destination
                    // with which to establish that comparison.
                    return Err("descriptor-binding-replacement");
                }
                let read = read_segments(&binding.segments).ok_or("binding-coordinate")?;
                if !storage.destinations.iter().all(|destination| {
                    compatible_destination(binding.root, destination)
                        && disjoint(&read, &destination.segments)
                }) {
                    return Err("descriptor-binding-write");
                }
            }
            if slice_descriptor_measure {
                // Slice extent belongs to the descriptor just authenticated
                // above. An element store cannot change it; window backing
                // coordinates are not dependencies of this metadata read.
                return Ok(Some(true));
            }
            let mut suffix = read_segments(&path[cutoff..]).ok_or("owned-suffix")?;
            if measure {
                suffix.push(ReadSegment::Length);
            }
            for source in &backing.sources {
                if !source_namespace_is_exact(source) {
                    return Err("supplier-namespace");
                }
                if let Some(replaced) = replaced_binding {
                    if source.root == replaced {
                        return Err("replaced-supplier-root");
                    }
                    continue;
                }
                let mut read = read_segments(&source.segments).ok_or("supplier-coordinate")?;
                read.extend_from_slice(&suffix);
                if !storage.destinations.iter().all(|destination| {
                    compatible_destination(source.root, destination)
                        && disjoint(&read, &destination.segments)
                }) {
                    return Err("overlapping-store");
                }
            }
            Ok(Some(true))
        })();
        match result {
            Ok(preserved) => preserved,
            Err(_) => Some(false),
        }
    }
}

fn source_namespace_is_exact(source: &crate::LocalWriteSource) -> bool {
    source.root.is_valid()
        && source.invocation_output.is_none()
        && source.invocation_result.is_none()
        && !source.is_image_storage
        && source.recasts.is_empty()
        && source.representation.is_none()
        && !source.collection_coarse
}

fn read_segments(path: &[PlaceSegment]) -> Option<Vec<ReadSegment>> {
    path.iter()
        .map(|segment| match segment {
            PlaceSegment::Field { symbol } => Some(ReadSegment::Field(*symbol)),
            PlaceSegment::Case { variant } => Some(ReadSegment::Case(*variant)),
            PlaceSegment::FixedIndex { index } => Some(ReadSegment::Element(Some(*index))),
            PlaceSegment::Index { .. } => Some(ReadSegment::Element(None)),
            _ => None,
        })
        .collect()
}
