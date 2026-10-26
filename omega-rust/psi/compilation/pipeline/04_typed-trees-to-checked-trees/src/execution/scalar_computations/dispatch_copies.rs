//! Selected structural payload captures use the ordinary copied-value producer.
//! The completed-flow receipt supplies the saved receiver and entry epoch;
//! the value graph retains one owned result before any target operand runs.

use super::builder::Builder;
use checked_trees::{
    CheckedStructuralAccess, CheckedStructuralValueKind, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralPathSegment,
};

impl Builder<'_, '_> {
    pub(super) fn retain_dispatch_copy_roots(&mut self) {
        let Some(state) = self.flow.control.unique_state(self.machine, self.state) else {
            return;
        };
        let copies = self
            .flow
            .control
            .outgoing
            .span_or_empty(state.outgoing)
            .iter()
            .filter(|outgoing| outgoing.statement_index == self.statement_index)
            .flat_map(|outgoing| {
                self.flow
                    .control
                    .operands
                    .span_or_empty(outgoing.copied_payloads)
                    .iter()
                    .copied()
            })
            .collect::<Vec<_>>();
        for copy in copies {
            if self
                .program
                .primitive_type_reference(copy.type_reference)
                .is_some()
                && validation::reference_shell(self.program, copy.type_reference).is_none()
            {
                continue;
            }
            let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } =
                copy.completion
            else {
                continue;
            };
            if self
                .structural_values
                .root_for_captured_copy(self.state, self.statement_index as u32, identity)
                .is_some()
            {
                continue;
            }
            let Some(kind) = self.dispatch_copy_kind(copy) else {
                continue;
            };
            let Some(value) =
                self.publish_structural_value(copy.expression, copy.type_reference, kind)
            else {
                continue;
            };
            self.structural_values
                .roots
                .append(checked_trees::CheckedStructuralValueRoot {
                    captured_copy: identity,
                    machine: self.machine,
                    state: self.state,
                    statement_ordinal: self.statement_index as u32,
                    expression: copy.expression,
                    type_reference: copy.type_reference,
                    root: value,
                });
        }
    }

    pub(super) fn captured_dispatch_value(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<&checked_trees::CheckedStructuralValueRoot> {
        let copy = self.captured_dispatch_operand(expression)?;
        let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } = copy.completion
        else {
            return None;
        };
        self.structural_values.root_for_captured_copy(
            self.state,
            self.statement_index as u32,
            identity,
        )
    }

    fn captured_dispatch_operand(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<&checked_trees::FlowOperandFact> {
        if !self.declaration_parameters.is_empty() {
            return None;
        }
        captured_dispatch_operand(
            self.program,
            self.flow,
            self.machine,
            self.state,
            self.statement_index,
            expression,
        )
    }

    pub(super) fn uses_captured_dispatch_value(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        uses_captured_dispatch_value(self.program, self.captured_dispatch_operand(expression))
    }

    /// A pure storage plan cannot stand in for a completed value occurrence.
    /// Formal/generated computations have no selected-copy flow use.
    pub(super) fn observes_captured_dispatch_value(
        &self,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> bool {
        self.declaration_parameters.is_empty()
            && observes_captured_dispatch_value(
                self.program,
                self.flow,
                self.machine,
                self.state,
                self.statement_index,
                expression,
            )
    }

    fn dispatch_copy_kind(
        &self,
        copy: checked_trees::FlowOperandFact,
    ) -> Option<CheckedStructuralValueKind> {
        let receiver = self.flow.control.operands.get(copy.backing);
        let place = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            receiver.expression,
        )?;
        // The completed-subject receipt already requires a whole-symbol
        // descriptor with its exact stored Read loan. Its backing keeps the
        // original indexed address and selectors; this receiver is the saved
        // descriptor, not another restriction on authored source geometry.
        if !place.segments.is_empty() {
            return None;
        }
        let symbol = place.root.as_symbol()?;
        let member = self.program.expression_table.member_node(copy.expression)?;
        let field_symbol =
            validation::effective_member_symbol(self.program, member.receiver, member);
        let case_symbol = facts::payload_variant_for_field(self.program, field_symbol)?;
        let (_, case) = self.program.variant_owner(case_symbol)?;
        let field = self
            .program
            .data_payload_field_by_symbol(case, field_symbol)?;
        let mut path = Vec::new();
        let mut reference = receiver.type_reference;
        while let Some(shell) = validation::reference_shell(self.program, reference) {
            if !shell.access.is_readable() {
                return None;
            }
            path.push(CheckedUnitStructuralPathSegment::Referent);
            reference = shell.referee;
        }
        path.push(CheckedUnitStructuralPathSegment::Case(
            checked_trees::CheckedMemberIdentity::of_member(case.identity, case.name.clone()),
        ));
        path.push(CheckedUnitStructuralPathSegment::Field(
            checked_trees::CheckedMemberIdentity::of_member(field.identity, field.name.clone()),
        ));
        Some(CheckedStructuralValueKind::CopiedStructuralPlace {
            source: CheckedUnitStructuralArgumentPlan {
                projection_evaluations: Default::default(),
                source: CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol },
                path,
                type_identity: validation::stored_structural_type_identity(
                    self.program,
                    copy.type_reference,
                    &[],
                    &[],
                ),
                access: CheckedStructuralAccess::SharedBorrow,
            },
            leaf: copy.type_reference,
        })
    }
}

impl Builder<'_, '_> {
    /// Copy a projected value from its already completed owner. This is the
    /// same ordinary copied-place operation as a local/parameter read, with
    /// its relative source supplied by the authenticated flow occurrence.
    pub(super) fn copied_dispatch_place(
        &mut self,
        expression: checked_trees::expression::ExpressionHandle,
        expected: typed_trees::types::TypeReferenceHandle,
    ) -> Option<CheckedStructuralValueKind> {
        if !validation::type_satisfies_declared_property(
            self.program,
            &[],
            expected,
            validation::DeclaredPropertyRequirement::Copy,
        ) {
            return None;
        }
        let state = self.program.state_by_symbol(self.state)?;
        let transition = self
            .program
            .statement_table
            .statement_at(state, self.statement_index)?
            .as_transition()?;
        let (_, copy) = crate::flow::copied_payload_at(
            self.program,
            self.flow,
            self.machine,
            self.state,
            self.statement_index,
            checked_trees::FlowPermissionEvaluation::TransitionTarget(transition.target),
            expression,
        )?;
        let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } = copy.completion
        else {
            return None;
        };
        let saved = self
            .structural_values
            .root_for_captured_copy(self.state, self.statement_index as u32, identity)?
            .root;
        let source = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            expression,
        )?;
        let prefix = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state,
            self.statement_index,
            copy.expression,
        )?;
        let place = validation::CanonicalPlace {
            root: facts::PlaceRoot::Expression(copy.expression),
            segments: source.segments[prefix.segments.len()..].to_vec(),
        };
        let receiver = validation::DispatchExtraction::payload_binding_receiver(
            self.program,
            expression,
            copy.expression,
        )?;
        let selectors =
            crate::values::authored_projection_selectors_below(self.program, expression, receiver)?;
        let selectors = self.retain_projection_selector_evaluations(expression, selectors)?;
        let (projected, mut path) =
            crate::execution::terminal_unit::calls::projected_argument_path(
                self.program,
                self.state,
                self.statement_index,
                &place,
                Some(self.machine),
                Some(&|expression| {
                    selectors.iter().find_map(|(selector, value)| {
                        (*selector == expression).then_some(
                            checked_trees::CheckedUnitStructuralPathSegment::ComputedIndex(*value),
                        )
                    })
                }),
            )?;
        if !validation::declared_carriers_match(self.program, projected, expected) {
            return None;
        }
        // Unlike borrowed formal roots, this root owns a saved descriptor.
        // Selecting below it crosses each actual stored reference shell.
        if !place.segments.is_empty() {
            let mut reference = copy.type_reference;
            let mut crossings = Vec::new();
            while let Some(shell) = validation::reference_shell(self.program, reference) {
                if !shell.access.is_readable() {
                    return None;
                }
                if !validation::stores_reference_shape(self.program, reference, &[]) {
                    break;
                }
                crossings.push(checked_trees::CheckedUnitStructuralPathSegment::Referent);
                reference = shell.referee;
            }
            crossings.append(&mut path);
            path = crossings;
        }
        for (ordinal, value) in selectors.iter().map(|(_, value)| *value).enumerate() {
            self.push_scalar_root(
                checked_trees::CheckedScalarExpressionRole::StructuralValueSelector {
                    expression,
                    selector_ordinal: u32::try_from(ordinal).ok()?,
                },
                value,
            )?;
        }
        let projection_evaluations = self.projection_selector_span(expression, &selectors)?;
        if !checked_trees::CheckedUnitStructuralArgumentPlan::projection_evaluations_match(
            &path,
            projection_evaluations,
            &self.plans.operands,
        ) {
            return None;
        }
        Some(CheckedStructuralValueKind::Projection {
            source: saved,
            logical_root_type_identity: Default::default(),
            path,
            projection_evaluations,
            copies_selected_value: true,
            type_identity: validation::stored_structural_type_identity(
                self.program,
                projected,
                &[],
                &[],
            ),
        })
    }
}

/// The flow receipt, not equal canonical storage, selects the completed value.
/// Pure-root publication and expanded-root construction share this exact query.
fn captured_dispatch_operand<'flow>(
    program: &dyn validation::CheckingInput,
    flow: &'flow checked_trees::FlowFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<&'flow checked_trees::FlowOperandFact> {
    let typed_state = program.state_by_symbol(state)?;
    let transition = program
        .statement_table
        .statement_at(typed_state, statement_index)?
        .as_transition()?;
    let (_, copy) = crate::flow::copied_payload_at(
        program,
        flow,
        machine,
        state,
        statement_index,
        checked_trees::FlowPermissionEvaluation::TransitionTarget(transition.target),
        expression,
    )?;
    Some(copy)
}

fn uses_captured_dispatch_value(
    program: &dyn validation::CheckingInput,
    copy: Option<&checked_trees::FlowOperandFact>,
) -> bool {
    copy.is_some_and(|copy| {
        program
            .primitive_type_reference(copy.type_reference)
            .is_none()
            || validation::reference_shell(program, copy.type_reference).is_some()
    })
}

pub(super) fn observes_captured_dispatch_value(
    program: &dyn validation::CheckingInput,
    flow: &checked_trees::FlowFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: usize,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let mut pending = vec![expression];
    let mut visited = Vec::new();
    while let Some(expression) = pending.pop() {
        if visited.contains(&expression) {
            continue;
        }
        visited.push(expression);
        if uses_captured_dispatch_value(
            program,
            captured_dispatch_operand(program, flow, machine, state, statement_index, expression),
        ) {
            return true;
        }
        if program.expression_table.expression_is_valid(expression) {
            validation::expression_traversal::children(
                program,
                program.expression_table.expression(expression),
                |child| pending.push(child),
            );
        }
    }
    false
}

/// Source flow already captured its operands at their real read epochs. At the
/// execution-plan boundary, an actual completed-value dependency belongs solely
/// to the expanded producer. Withdraw the exact pure root even if construction
/// of its replacement later fails: the old live read is never a fallback.
/// Conflicting custody rows remain published so ordinary ambiguity checks refuse.
pub(super) fn withdraw_captured_pure_roots(
    program: &dyn validation::CheckingInput,
    flow: &checked_trees::FlowFacts,
    pure: &mut checked_trees::CheckedScalarExpressionPlans,
) {
    let mut withdrawn = Vec::new();
    for (_, binding) in pure.source_bindings.iter() {
        let Some((retained, _)) =
            pure.bound_expression_at(binding.state, binding.statement_ordinal, binding.role)
        else {
            continue;
        };
        if retained != binding {
            continue;
        }
        let Some((machine, _)) = validation::find_state(program, binding.state) else {
            continue;
        };
        if observes_captured_dispatch_value(
            program,
            flow,
            machine.symbol,
            binding.state,
            binding.statement_ordinal as usize,
            binding.expression,
        ) {
            withdrawn.push((binding.state, binding.statement_ordinal, binding.role));
        }
    }
    if withdrawn.is_empty() {
        return;
    }
    pure.expressions
        .retain(|root| !withdrawn.contains(&(root.state, root.statement_ordinal, root.role)));
    // No published Flow row holds a source-binding handle; FlowBuildContext's
    // indexes have completed. The declaration symbol arena stays unchanged.
    let mut bindings = arena::Arena::default();
    for (_, binding) in pure.source_bindings.iter() {
        if !withdrawn.contains(&(binding.state, binding.statement_ordinal, binding.role)) {
            bindings.append(binding.clone());
        }
    }
    pure.source_bindings = bindings;
    pure.state_index = Default::default();
}
