//! A checked view keeps its field namespace when passed to another function.
//! Follow the completed descriptor to its actual formation, then carry that
//! formation's recorded interpretation before the callee's result projection.
//! The enclosure supplies storage coordinates only; returned loans still
//! authenticate every source, access and lifetime in the caller.

use super::{CapturedReturnPresentation, Captures, CheckedReferenceOriginProjection};
use arena::Handle;
use checked_trees::{
    CheckedStructuralValueKind, CheckedUnitStructuralPathSegment, FlowOperandFact,
};
use semantic_vocabulary::ReferenceOriginExtent;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;

#[derive(PartialEq, Eq)]
pub(super) struct CapturedArgumentOrigin {
    pub(super) source: crate::LocalWriteSource,
    pub(super) presentation: CapturedReturnPresentation,
    pub(super) extent: ReferenceOriginExtent,
}

impl Captures<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn argument_return_sources(
        &self,
        state: &typed_trees::state::State,
        reference: TypeReferenceHandle,
        operand: Handle<FlowOperandFact>,
        current: Handle<checked_trees::FlowStorageSnapshot>,
        relative: &crate::LocalWriteSource,
        presentation: &CapturedReturnPresentation,
        active: &mut Vec<SymbolHandle>,
        visiting: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Option<Vec<CapturedArgumentOrigin>> {
        let control = &self.facts.flow.control;
        if !control.operands.is_valid(operand)
            || visiting.contains(&operand)
            || visiting.len() >= super::super::MAX_REFERENCE_ORIGIN_DEPTH
        {
            return None;
        }
        // Owned carriers retain their existing leaf-by-leaf descriptor query.
        // This route transports the interpretation of the outer pointer.
        if crate::reference_shell(self.program, reference).is_none() {
            return self.ordinary_argument_sources(
                state,
                reference,
                operand,
                current,
                relative,
                presentation,
                active,
            );
        }
        let _actual = control.operands.get(operand);
        let stored = crate::completed_storage::reference_sources_at_path(
            self.program,
            control,
            operand,
            &[],
            current,
        );
        let mut selections = Vec::new();
        descriptor_alternatives(stored, &mut selections)?;
        visiting.push(operand);
        let result = (|| {
            let mut output = Vec::new();
            for selection in selections {
                let selection = match selection {
                    crate::completed_storage::StoredOperand::Incoming(place) => {
                        let source = self.incoming_argument_source(
                            state,
                            reference,
                            place,
                            relative,
                            presentation,
                        )?;
                        if !output.contains(&source) {
                            output.push(source);
                        }
                        if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                            return None;
                        }
                        continue;
                    }
                    crate::completed_storage::StoredOperand::Value {
                        capture,
                        reference,
                        projection,
                        slot,
                    } => crate::completed_storage::StoredCapture {
                        capture,
                        reference,
                        projection,
                        slot,
                    },
                    crate::completed_storage::StoredOperand::Unavailable
                    | crate::completed_storage::StoredOperand::Alternatives(_) => return None,
                };
                let capture = control.operands.get(selection.capture);
                let (path, _) = super::super::source_paths::structural_path(
                    self.program,
                    capture.type_reference,
                    &selection.projection,
                )?;
                let descriptor =
                    self.completed_operand_source(self.completed_operand_leaf(capture, &path)?)?;
                let enclosure = if control.operand_retentions.is_valid(descriptor.retention) {
                    control
                        .operand_retentions
                        .get(descriptor.retention)
                        .enclosing_storage
                } else {
                    Handle::invalid()
                };
                let values = &self.facts.values.structural_values;
                let formation = values.with_state_nodes(descriptor.state_symbol, |handles| {
                    let mut nodes = handles
                        .iter()
                        .map(|handle| values.nodes.get(*handle))
                        .filter(|value| {
                            value.expression == descriptor.expression
                                && value.statement_index as usize == descriptor.statement_index
                        });
                    let value = nodes.next()?;
                    nodes.next().is_none().then_some(value)
                });
                let view = formation.filter(|value| {
                    matches!(
                        value.kind,
                        CheckedStructuralValueKind::Reference {
                            storage: checked_trees::CheckedReferenceStorage::View { .. },
                            ..
                        }
                    )
                });
                if let Some(view) = view {
                    if !control.operands.is_valid(enclosure)
                        || view.state != descriptor.state_symbol
                        || view.machine != self.flow(descriptor.state_symbol)?.machine_symbol
                        || view.statement_index as usize != descriptor.statement_index
                        || self.program.normalized_type_identity(view.type_reference)
                            != self
                                .program
                                .normalized_type_identity(descriptor.type_reference)
                    {
                        return None;
                    }
                    let CheckedStructuralValueKind::Reference {
                        storage,
                        backing_type,
                        ref source,
                        ..
                    } = view.kind
                    else {
                        return None;
                    };
                    let physical = control.operands.get(enclosure);
                    let physical_shell =
                        crate::reference_shell(self.program, physical.type_reference)?;
                    let target = crate::reference_shell(self.program, view.type_reference)?.referee;
                    if source.type_identity
                        != crate::stored_structural_type_identity(
                            self.program,
                            backing_type,
                            &[],
                            &[],
                        )
                        || self
                            .program
                            .normalized_type_identity(physical_shell.referee)
                            != self.program.normalized_type_identity(backing_type)
                    {
                        return None;
                    }
                    let (suffix, endpoint, extent) = super::super::reference_origin_path(
                        self.program,
                        target,
                        &relative.segments,
                    )?;
                    if self.program.normalized_type_identity(endpoint) != presentation.source_type {
                        return None;
                    }
                    let mut path = vec![CheckedReferenceOriginProjection::Exact(
                        CheckedUnitStructuralPathSegment::RepresentationView {
                            type_identity: crate::stored_structural_type_identity(
                                self.program,
                                target,
                                &[],
                                &[],
                            ),
                            storage,
                        },
                    )];
                    path.extend(suffix);
                    path.extend(presentation.path.iter().cloned());
                    let presented = CapturedReturnPresentation {
                        path,
                        source_type: self.program.normalized_type_identity(backing_type),
                        endpoint_type: presentation.endpoint_type,
                        result_type: presentation.result_type,
                    };
                    let mut physical_relative = relative.clone();
                    physical_relative.segments.clear();
                    for mut source in self.argument_return_sources(
                        state,
                        physical.type_reference,
                        enclosure,
                        current,
                        &physical_relative,
                        &presented,
                        active,
                        visiting,
                    )? {
                        if extent == ReferenceOriginExtent::WithinRegion {
                            source.extent = extent;
                        }
                        if !output.contains(&source) {
                            output.push(source);
                        }
                    }
                } else {
                    // A retained enclosure without its checked formation is
                    // unavailable, not a license to infer a codec from types.
                    if enclosure.is_valid() && formation.is_none() {
                        return None;
                    }
                    let sources = if selection.projection.is_empty() {
                        self.captured_input_sources(
                            state,
                            reference,
                            selection.capture,
                            current,
                            relative,
                            active,
                        )?
                    } else {
                        self.completed_descriptor_sources(
                            state,
                            selection.capture,
                            &selection.projection,
                            reference,
                            relative,
                            current,
                            &mut Vec::new(),
                        )?
                    };
                    for source in sources {
                        let source = CapturedArgumentOrigin {
                            source,
                            presentation: presentation.clone(),
                            extent: ReferenceOriginExtent::Projected,
                        };
                        if !output.contains(&source) {
                            output.push(source);
                        }
                    }
                }
                if output.len() > super::super::MAX_REFERENCE_ORIGIN_LEAVES {
                    return None;
                }
            }
            // A complete input query may prove that this result leaf is
            // inactive. Preserve that known-empty result, not an unknown source.
            Some(output)
        })();
        visiting.pop();
        result
    }

    /// An unchanged entry descriptor keeps that exact namespace. It is not
    /// replaced with another branch's produced pointer or a guessed address.
    fn incoming_argument_source(
        &self,
        state: &typed_trees::state::State,
        reference: TypeReferenceHandle,
        place: crate::CanonicalPlace,
        relative: &crate::LocalWriteSource,
        presentation: &CapturedReturnPresentation,
    ) -> Option<CapturedArgumentOrigin> {
        let root = place.root.as_symbol()?;
        if !self.ingress_present(state, root)? {
            return None;
        }
        let parameter = self.program.state_parameter_by_symbol(state, root)?;
        let slot = crate::project_type_reference_from_segments(
            self.program,
            parameter.type_reference,
            &place.segments,
        )?;
        let supplied = crate::reference_shell(self.program, slot)?;
        let expected = crate::reference_shell(self.program, reference)?;
        if !crate::storage_origin_types_match(self.program, supplied.referee, expected.referee)
            || !super::super::reference_access_can_supply(
                supplied.access.into(),
                expected.access.into(),
            )
        {
            return None;
        }
        let source = crate::LocalWriteSource {
            invocation_output: None,
            invocation_result: None,
            is_image_storage: false,
            recasts: Vec::new(),
            representation: None,
            path: self.program.symbols.name(root).to_owned(),
            collection_coarse: false,
            root,
            segments: place.segments,
        }
        .substitute_storage(self.program, relative)?;
        Some(CapturedArgumentOrigin {
            source,
            presentation: presentation.clone(),
            extent: ReferenceOriginExtent::Projected,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn ordinary_argument_sources(
        &self,
        state: &typed_trees::state::State,
        reference: TypeReferenceHandle,
        operand: Handle<FlowOperandFact>,
        current: Handle<checked_trees::FlowStorageSnapshot>,
        relative: &crate::LocalWriteSource,
        presentation: &CapturedReturnPresentation,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<CapturedArgumentOrigin>> {
        Some(
            self.captured_input_sources(state, reference, operand, current, relative, active)?
                .into_iter()
                .map(|source| CapturedArgumentOrigin {
                    source,
                    presentation: presentation.clone(),
                    extent: ReferenceOriginExtent::Projected,
                })
                .collect(),
        )
    }
}

/// Keep every predecessor, including unchanged ingress. An unavailable
/// predecessor cannot disappear behind a successfully captured branch.
fn descriptor_alternatives(
    selected: crate::completed_storage::StoredOperand,
    output: &mut Vec<crate::completed_storage::StoredOperand>,
) -> Option<()> {
    use crate::completed_storage::StoredOperand;
    match selected {
        StoredOperand::Unavailable => return None,
        StoredOperand::Alternatives(alternatives) => {
            if alternatives.is_empty() {
                return None;
            }
            for alternative in alternatives {
                descriptor_alternatives(alternative, output)?;
            }
        }
        selected => output.push(selected),
    }
    Some(())
}
