//! Returned descriptor origins follow the completed value/store graph and its
//! body-proven descriptor forwarding associations. Lifetime roots never stand in for an address;
//! an unchanged opaque ingress retains its parameter-relative identity.
use super::{Captures, TypeReferenceHandle};
use arena::Handle;
use checked_trees::FlowOperandFact;
use facts::PlaceSegment;

impl Captures<'_> {
    pub(super) fn completed_descriptor_sources(
        &self,
        state: &typed_trees::state::State,
        operand: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        expected: TypeReferenceHandle,
        suffix: &crate::LocalWriteSource,
        active: &mut Vec<(Handle<FlowOperandFact>, Vec<PlaceSegment>)>,
    ) -> Option<Vec<crate::LocalWriteSource>> {
        let query = (operand, path.to_vec());
        let control = &self.facts.flow.control;
        if active.len() >= super::super::MAX_REFERENCE_ORIGIN_DEPTH
            || active.contains(&query)
            || !control.operands.is_valid(operand)
        {
            return None;
        }
        let actual = control.operands.get(operand);
        let supplied =
            crate::project_type_reference_from_segments(self.program, actual.type_reference, path)?;
        let supplied = crate::reference_shell(self.program, supplied)?;
        let expected_shell = crate::reference_shell(self.program, expected)?;
        if !(supplied.access == expected_shell.access
            || supplied.access == language_semantics::ReferenceAccess::Mutable)
            || !crate::storage_origin_types_match(
                self.program,
                supplied.referee,
                expected_shell.referee,
            )
        {
            return None;
        }
        active.push(query);
        let selected = (|| {
            let Some(stored) = crate::completed_storage::reference_captures_at_path(
                self.program,
                control,
                operand,
                path,
                actual.storage_snapshot,
            ) else {
                // The selector authenticates this exact unchanged input cut.
                // A newer store or unavailable alternative cannot reach here.
                if actual.state_symbol != state.symbol {
                    return None;
                }
                let mut place = crate::canonical_place_from_expression_in_state(
                    self.program,
                    state.symbol,
                    actual.statement_index,
                    actual.expression,
                )?;
                let root = place.root.as_symbol()?;
                if !self.ingress_present(state, root)? {
                    return None;
                }
                place.segments.extend_from_slice(path);
                let origin = crate::LocalWriteSource {
                    invocation_output: None,
                    is_image_storage: false,
                    recasts: Vec::new(),
                    representation: None,
                    path: self.program.symbols.name(root).to_owned(),
                    collection_coarse: false,
                    root,
                    segments: place.segments,
                };
                return Some(vec![origin.substitute_storage(self.program, suffix)?]);
            };
            let descriptors = stored.captures()?;
            if descriptors.is_empty() {
                return None;
            }
            let mut output = Vec::new();
            for selected in descriptors {
                let descriptor = control.operands.get(selected.capture);
                let sources = if !selected.projection.is_empty() {
                    self.completed_descriptor_sources(
                        state,
                        selected.capture,
                        &selected.projection,
                        expected,
                        suffix,
                        active,
                    )?
                } else if !descriptor.referents.is_empty() {
                    let mut sources = Vec::new();
                    for referent in control.operand_referents.span(descriptor.referents)? {
                        let root = referent.root.as_symbol()?;
                        let origin = crate::LocalWriteSource {
                            invocation_output: None,
                            is_image_storage: false,
                            recasts: Vec::new(),
                            representation: None,
                            path: self.program.symbols.name(root).to_owned(),
                            collection_coarse: false,
                            root,
                            segments: control
                                .operand_referent_segments
                                .span(referent.segments)?
                                .to_vec(),
                        };
                        sources.push(origin.substitute_storage(self.program, suffix)?);
                    }
                    sources
                } else {
                    if !control.operand_retentions.is_valid(descriptor.retention) {
                        return None;
                    }
                    control.retained_roots(descriptor.retention)?;
                    let profile = control.operand_retentions.get(descriptor.retention);
                    let suppliers = control
                        .operand_retained_suppliers
                        .span(profile.descriptor_sources)?;
                    if suppliers.is_empty() {
                        return None;
                    }
                    let mut sources = Vec::new();
                    for supplier in suppliers {
                        sources.extend(self.completed_descriptor_sources(
                            state,
                            supplier.operand,
                            control.operand_retained_paths.span(supplier.projection)?,
                            expected,
                            suffix,
                            active,
                        )?);
                    }
                    sources
                };
                if sources.is_empty() {
                    return None;
                }
                for source in sources {
                    if !output.contains(&source) {
                        output.push(source);
                    }
                }
                if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                    return None;
                }
            }
            Some(output)
        })();
        active.pop();
        selected
    }
}
