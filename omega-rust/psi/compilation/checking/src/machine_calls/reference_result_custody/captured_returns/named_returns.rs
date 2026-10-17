//! Named completion joins the finite target result relation to this edge's
//! completed operands and exact returned-loan group. Destination formals never
//! cross into the caller namespace without substitution; declaration lifetime
//! candidates do not establish a present result or choose its backing.

use super::super::{
    MAX_REFERENCE_ORIGIN_LEAVES, declared_carrier_references, origin_place_segments,
};
use super::{
    Captures, CheckedReferenceOriginProjection, CheckedReturnedReferencePlan,
    CheckedUnitStructuralPathSegment, SymbolHandle, TypeReferenceHandle, frame_origin_fits_loan,
    loan_owner_path, loan_place_segments,
};

impl Captures<'_> {
    /// An internal edge consumes a complete target relation, not a recursive
    /// executable summary. Its formal roots are substituted through this
    /// occurrence's completed operands before joining source-state custody.
    pub(super) fn named_return_sources(
        &self,
        state: &typed_trees::state::State,
        transition_target: typed_trees::statement::TransitionTargetHandle,
        call: &checked_trees::FlowCallFact,
        loans: &[checked_trees::BorrowReturnedLoanFact],
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<CheckedReturnedReferencePlan>> {
        let target = crate::call_target_state(self.program, call.target_symbol)?;
        let frames = crate::CallFrameResolver::new(self.program)?;
        let relative = frames.reference_result_leaf_origins(target.symbol)?;
        let leaves = self.reference_leaves(state)?;
        let parameters = self.program.state_parameters(target);
        let operands = self
            .facts
            .flow
            .control
            .operands
            .span(call.argument_operands())?;
        if parameters.len() != operands.len() {
            return None;
        }
        let mut origins = Vec::new();
        for candidate in relative {
            let (result_path, _) =
                self.structural_path(target.return_type, &candidate.result_path)?;
            if !leaves.iter().any(|leaf| leaf.path == result_path) {
                return None;
            }
            let result_path = loan_place_segments(self.program, state.return_type, &result_path)?;
            if candidate.source.is_image_storage {
                // Immutable image storage carries no invocation-relative loan.
                // Its checked formation retains the backing independently.
                continue;
            }
            let (position, parameter) = parameters
                .iter()
                .enumerate()
                .find(|(_, parameter)| parameter.symbol == candidate.source.root)?;
            let operand = operands.get(position)?;
            if operand.state_symbol != state.symbol
                || operand.statement_index != call.statement_index
            {
                return None;
            }
            for source in self.captured_input_sources(
                state,
                parameter.type_reference,
                operand,
                &candidate.source,
                active,
            )? {
                let origin = crate::ReferenceResultLeafOrigin {
                    result_path: result_path.clone(),
                    source,
                };
                if !origins.contains(&origin) {
                    origins.push(origin);
                    if origins.len() > MAX_REFERENCE_ORIGIN_LEAVES {
                        return None;
                    }
                }
            }
        }
        let site_loans: Vec<_> = loans
            .iter()
            .filter(|loan| {
                loan.statement_index == call.statement_index
                    && loan.transition_target == transition_target
            })
            .cloned()
            .collect();
        let mut output = Vec::new();
        for leaf in leaves {
            let result_path = loan_place_segments(self.program, state.return_type, &leaf.path)?;
            let owner_path = loan_owner_path(self.program, state.return_type, &leaf.path)?;
            for origin in origins
                .iter()
                .filter(|origin| origin.result_path == result_path)
            {
                let mut covered = false;
                for loan in site_loans.iter().filter(|loan| {
                    loan.root == facts::PlaceRoot::Symbol(origin.source.root)
                        && self.facts.borrow.owner_segments.span(loan.owner_path)
                            == Some(owner_path.as_slice())
                        && self
                            .facts
                            .borrow
                            .access_segments
                            .span(loan.segments)
                            .is_some_and(|segments| {
                                frame_origin_fits_loan(&origin.source, segments)
                            })
                }) {
                    if leaf.source.access.borrow_access_kind()? != loan.kind {
                        return None;
                    }
                    covered = true;
                    for source in self.return_origin_with_frame(
                        state,
                        loan,
                        &leaf,
                        self.facts.borrow.access_segments.span(loan.segments)?,
                        &site_loans,
                        Some(&origins),
                    )? {
                        let returned = CheckedReturnedReferencePlan {
                            path: leaf.path.clone(),
                            source,
                        };
                        if !output.contains(&returned) {
                            output.push(returned);
                        }
                    }
                }
                if !covered {
                    return None;
                }
            }
        }
        Some(output)
    }

    fn captured_input_sources(
        &self,
        state: &typed_trees::state::State,
        reference: TypeReferenceHandle,
        operand: &checked_trees::FlowOperandFact,
        relative: &crate::LocalWriteSource,
        active: &mut Vec<SymbolHandle>,
    ) -> Option<Vec<crate::LocalWriteSource>> {
        let mut relative = relative.clone();
        let mut selected = operand;
        if crate::reference_shell(self.program, reference).is_none() {
            let carrier = declared_carrier_references(self.program, reference)?;
            let mut boundary = None;
            for leaf in carrier {
                let segments =
                    origin_place_segments(self.program, reference, leaf.path.iter().cloned())?;
                if relative.segments.starts_with(&segments) {
                    if boundary.is_some() {
                        return None;
                    }
                    boundary = Some((leaf, segments));
                }
            }
            let (leaf, segments) = boundary?;
            let path = leaf
                .path
                .iter()
                .map(|projection| match projection {
                    CheckedReferenceOriginProjection::Exact(segment) => Some(segment.clone()),
                    CheckedReferenceOriginProjection::Element => None,
                })
                .collect::<Option<Vec<_>>>()?;
            if !self.expression_leaf_present(
                state,
                operand.statement_index,
                operand.expression,
                reference,
                &path,
                Some(operand),
                active,
            )? {
                return Some(Vec::new());
            }
            if let Some(child) = self.completed_operand_leaf(operand, &path) {
                selected = child;
                relative.segments = relative
                    .segments
                    .strip_prefix(segments.as_slice())?
                    .to_vec();
            } else {
                // An opaque owned input retains its own descriptor identities.
                // Presence checked the captured write prefix; never turn a
                // detached carrier's empty referents into an empty result.
                let place = crate::canonical_place_from_expression_in_state(
                    self.program,
                    state.symbol,
                    operand.statement_index,
                    operand.expression,
                )?;
                let root = place.root.as_symbol()?;
                if !self
                    .program
                    .state_parameters(state)
                    .iter()
                    .any(|parameter| parameter.symbol == root)
                {
                    return None;
                }
                let source = crate::LocalWriteSource {
                    is_image_storage: false,
                    recasts: Vec::new(),
                    representation: None,
                    path: self.program.symbols.name(root).to_owned(),
                    collection_coarse: false,
                    root,
                    segments: place.segments,
                };
                return Some(vec![source.substitute_storage(self.program, &relative)?]);
            }
        }
        let referents = self
            .facts
            .flow
            .control
            .operand_referents
            .span(selected.referents)?;
        if referents.is_empty() {
            return None;
        }
        let mut output = Vec::new();
        for referent in referents {
            let root = referent.root.as_symbol()?;
            let source = crate::LocalWriteSource {
                is_image_storage: false,
                recasts: Vec::new(),
                representation: None,
                path: self.program.symbols.name(root).to_owned(),
                collection_coarse: false,
                root,
                segments: self
                    .facts
                    .flow
                    .control
                    .operand_referent_segments
                    .span(referent.segments)?
                    .to_vec(),
            };
            let source = source.substitute_storage(self.program, &relative)?;
            if !output.contains(&source) {
                output.push(source);
            }
        }
        Some(output)
    }

    pub(super) fn completed_operand_leaf<'fact>(
        &'fact self,
        mut operand: &'fact checked_trees::FlowOperandFact,
        mut path: &[CheckedUnitStructuralPathSegment],
    ) -> Option<&'fact checked_trees::FlowOperandFact> {
        while !path.is_empty() {
            let mut selected = None;
            for child in self
                .facts
                .flow
                .control
                .operand_children
                .span(operand.children)?
            {
                let (destination, _) = self.structural_path(
                    operand.type_reference,
                    self.facts
                        .flow
                        .control
                        .operand_child_paths
                        .span(child.path)?,
                )?;
                if !destination.is_empty() && path.starts_with(&destination) {
                    if selected.is_some() || !self.facts.flow.control.operands.is_valid(child.value)
                    {
                        return None;
                    }
                    selected = Some((
                        self.facts.flow.control.operands.get(child.value),
                        destination.len(),
                    ));
                }
            }
            let (child, length) = selected?;
            if child.state_symbol != operand.state_symbol
                || child.statement_index != operand.statement_index
            {
                return None;
            }
            operand = child;
            path = &path[length..];
        }
        Some(operand)
    }
}
