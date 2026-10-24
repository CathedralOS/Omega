//! Value production and its publication boundaries. Ordinary locals publish
//! their checked binding only after the common producer returns. Scalar operands
//! give the same producer a private invocation result namespace and consume its
//! actual completed binding; physical storage and checked claim identity remain
//! shared.

use super::super::argument_evaluation::Evaluation;
use super::super::structural_values;
use super::{OperationEnvironment, OperationFrame, StructuralResults};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::emission::operation_emission::calls::CallEmissionContext;
use crate::expression_preparation::bindings::StructuralLocalBinding;
use crate::unit::{
    CheckedUnitEffectOperationPlan, LoweringError, StructuralTypeId, ValueDeclaration,
    lookup_type_id, unsupported,
};

impl OperationFrame<'_, '_> {
    pub(super) fn establish_structural_value(
        self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,

        operation: &CheckedUnitEffectOperationPlan,
    ) -> Result<(), LoweringError> {
        let CheckedUnitEffectOperationPlan::EstablishLocal(
            checked_trees::CheckedUnitLocalBindingPlan::StructuralValue {
                result,
                value,
                discard_result_on_return,
                calls: operand_calls,
                ..
            },
        ) = operation
        else {
            return unsupported("structural value producer missing");
        };
        let (mut environment, emission) = self.split();
        let checked = environment.checked;
        let state = environment.state;
        let structural_types = environment.structural_types;
        let type_ids = environment.type_ids;
        let super::OperationEmission {
            evaluation,
            values,
            next_value,
            next_block,
            next_edge,
            calls,
            operations,
            ..
        } = emission;
        // Fresh construction carries the claims the checker established at
        // the binding statement. Resolve them before the operand emitter
        // borrows the caller's claim namespace; the producing operation's
        // result publishes them below.
        //
        // The binding's own multiplicity does not decide whether there are
        // claims to carry. A transparent affine aggregate contributes no root
        // claim and holds each contained linear member as an independent
        // path-indexed claim, so an affine result can own several linear
        // claims and a linear result owns exactly its whole-root one.
        // A completed value has its own expression root unless the exact
        // local initializer receives it. Use the retained value occurrence,
        // not an assumption that every structural producer is a declaration.
        let occurrence = checked.facts.values.structural_values.nodes.get(*value);
        let owner = if result.local.is_named() {
            facts::PlaceRoot::Symbol(result.local.symbol)
        } else {
            facts::PlaceRoot::Expression(occurrence.expression)
        };
        let completed = structural_values::emit(
            identities,
            structural_values::ValueProduction {
                value: *value,
                owner,
                destination: structural_values::ValueDestination::Binding,
                statement: result.statement_index,
                structural_type: lookup_type_id(type_ids, &result.type_identity)?,
                multiplicity: result.multiplicity,
                binding_type_reference: result.local.declared_type,
                access: crate::expression_preparation::bindings::structural_paths::terminal_access(
                    result.local.access,
                ),
                selection: checked
                    .facts
                    .flow
                    .ownership
                    .owned_selection_at(state, result.statement_index)
                    .map(|(handle, _)| handle)
                    .unwrap_or_default(),
                calls: operand_calls,
            },
            &mut environment,
            calls,
            evaluation,
            values,
            next_value,
            next_block,
            next_edge,
            operations,
        )?;
        evaluation.establish_structural_binding(
            result,
            completed.binding,
            structural_types,
            operations,
        )?;
        environment.results.publish_value_binding(
            result,
            completed.declaration,
            *discard_result_on_return,
            operations,
        )?;
        Ok(())
    }
}

impl OperationEnvironment<'_> {
    /// Invocation-local result ordinals never escape to the authored namespace.
    /// Every produced declaration still belongs to the one physical allocator.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn produce_value(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        value: checked_trees::CheckedStructuralValueHandle,
        structural_type: StructuralTypeId,
        evaluation: &mut Evaluation,
        values: &mut Vec<ValueDeclaration>,
        next_value: &mut u64,
        next_block: &mut u64,
        next_edge: &mut u64,
        calls: &mut CallEmissionContext<'_>,
        operations: &mut OperationBuffer,
    ) -> Result<(), LoweringError> {
        let checked = self.checked;
        let node = checked.facts.values.structural_values.nodes.get(value);
        let root = checked
            .facts
            .values
            .structural_values
            .root_for_expression(node.state, node.statement_index, node.expression)
            .filter(|root| root.root == value)
            .ok_or(LoweringError::Unsupported(
                "computed value lost its complete checked root",
            ))?;
        let (preceding_bindings, operand_calls) = checked
            .facts
            .values
            .structural_values
            .call_schedule(value)
            .ok_or(LoweringError::Unsupported(
                "computed value lost its checked call namespace",
            ))?;
        let source_value_count = values.len();
        let parent_bindings = operations.structural_values.clone();
        let mut results = self.results.completed_namespace(operations)?.into_owned();
        let inherited_results = preceding_bindings as usize;
        if inherited_results > results.len() {
            return unsupported("computed value lost its checked predecessor bindings");
        }
        results.truncate(inherited_results);
        operations
            .structural_values
            .retain(|(ordinal, _)| *ordinal < preceding_bindings);
        let scalar_bindings = evaluation.scalar_bindings.clone();
        let parent_homes = evaluation
            .structural_locals
            .iter()
            .map(|binding| (binding.root, binding.result_binding_ordinal))
            .collect::<Vec<_>>();
        // Enclosing same-statement operands retain their physical homes, but
        // their private ordinals do not belong to this root's checked scope.
        for binding in &mut evaluation.structural_locals {
            if binding
                .result_binding_ordinal
                .is_some_and(|ordinal| ordinal >= preceding_bindings)
            {
                binding.result_binding_ordinal = None;
            }
        }
        evaluation.scalar_bindings = scalar_bindings
            .clone()
            .with_structural_locals(&evaluation.structural_locals);
        let produced = {
            let mut environment = self.reborrow();
            environment.results = StructuralResults::Dense(&mut results);
            let selection = checked
                .facts
                .flow
                .ownership
                .owned_selection_at(node.state, node.statement_index)
                .filter(|(_, receipt)| receipt.expression == node.expression)
                .map(|(handle, _)| handle)
                .unwrap_or_default();
            structural_values::emit(
                identities,
                structural_values::ValueProduction {
                    value,
                    owner: facts::PlaceRoot::Expression(node.expression),
                    destination: structural_values::ValueDestination::Operand,
                    statement: node.statement_index,
                    structural_type,
                    multiplicity: crate::reference_custody::result_multiplicity(
                        checked,
                        root.type_reference,
                    ),
                    binding_type_reference: root.type_reference,
                    access: terminal_psi::StructuralAccess::Owned,
                    selection,
                    calls: operand_calls,
                },
                &mut environment,
                calls,
                evaluation,
                values,
                next_value,
                next_block,
                next_edge,
                operations,
            )
        };
        operations.structural_values = parent_bindings;
        evaluation.scalar_bindings = scalar_bindings;
        for binding in &mut evaluation.structural_locals {
            binding.result_binding_ordinal = parent_homes
                .iter()
                .find(|(root, _)| *root == binding.root)
                .and_then(|(_, ordinal)| *ordinal);
        }
        let completed = produced?;
        for (declaration, _) in results.into_iter().skip(inherited_results) {
            if let Some(declaration) = declaration {
                if !self
                    .private_places
                    .iter()
                    .any(|existing| existing.id == declaration.id)
                {
                    self.private_places.push(declaration);
                }
            }
        }
        if let Some(declaration) = completed.declaration {
            self.private_places.push(declaration);
        }
        let argument = completed.argument;
        let binding = crate::emission::operation_emission::buffer::ComputedValueBinding {
            argument: argument.clone(),
            structural_type: completed.binding.structural_type(),
        };
        if let Some((_, existing)) = operations
            .computed_values
            .iter_mut()
            .find(|(root, _)| *root == value)
        {
            *existing = binding;
        } else {
            operations.computed_values.push((value, binding));
        }
        // The receiving home is the completed value, not a replacement for the
        // raw call's operation result or a nested operand's separate expression.
        let root = facts::PlaceRoot::Expression(node.expression);
        evaluation
            .structural_locals
            .retain(|binding| binding.root != root);
        crate::expression_preparation::bindings::retain_completed_structural_homes(
            &mut evaluation.structural_locals,
            &[StructuralLocalBinding {
                result_binding_ordinal: None,
                root,
                argument,
                structural_type,
            }],
        )?;
        values.truncate(source_value_count);
        Ok(())
    }
}
