//! Boundary applications as ordered computation operands.
//!
//! Whole-value and nested scalar calls share one computation owner. Its
//! authority completes first, then ordinary operands complete left to right,
//! and the exact source call runs once. Its scalar result feeds the enclosing
//! consumer and its authority carrier retires on normal completion.
//! That keeps the authored
//! evaluation schedule of `wiki/spec/language/expressions.md`: ordered,
//! single invocation, and no invocation on a skipped short-circuit path.
//!
//! Mutable primitive storage consumes a computed value, not a separate call
//! result binding. Its initializer and whole-place assignments therefore keep
//! the boundary application as a computation even when it is the entire RHS.
//! Reads of the old storage complete before the same ordinary store replaces it.
//!
//! Applications share the ordinary computation operand evaluator:
//! structural places, selector snapshots, and scalar computations retain their
//! formal order. Their published boundary signature still owns access, claims,
//! completion and contract requirements independently of evaluation.
use super::call_arguments::ComputedCallOperands;
use super::{
    Builder, CheckedScalarComputationHandle, CheckedScalarComputationKind, ExpressionHandle,
    ExpressionNode, PrimitiveType,
};

impl Builder<'_, '_> {
    pub(super) fn is_bodyless_boundary_call(&self, mut expression: ExpressionHandle) -> bool {
        let call = loop {
            if !self
                .program
                .expression_table
                .expression_is_valid(expression)
            {
                return false;
            }
            match self.program.expression_table.expression(expression) {
                ExpressionNode::Atomic(atomic) => expression = atomic.value,
                // A statement owns only a whole call. Every enclosing cast
                // belongs to the computation graph, which sequences the call
                // once and then retains its conversion or qualification edge.
                // Peeling a semantic cast here drops that outer operation.
                ExpressionNode::Cast(_) => return false,
                ExpressionNode::Call(call) => break call,
                _ => return false,
            }
        };
        bodyless_boundary_target(
            self.program,
            validation::call_target_symbol(self.program, call),
        )
        .is_some()
    }

    /// The resolved declaration owns the signature; having an authored body
    /// state is not a prerequisite for entering the shared operand evaluator.
    pub(super) fn boundary_call(
        &mut self,
        expression: ExpressionHandle,
        call: &typed_trees::expression::TableCallExpression,
        target: (symbols::SymbolHandle, symbols::SymbolHandle),
        expected_type: PrimitiveType,
    ) -> Option<CheckedScalarComputationHandle> {
        let program = self.program;
        let authored_target = validation::call_target_symbol(self.program, call);
        // Boundary authority is admitted through the same source relation as
        // statement calls. A parameter projection must retain its ordinary
        // input. Entry establishment and provider selection are independent
        // realization obligations, not implicit receiver attachment authority.
        // Operand-directed checking may settle a declaration while the typed
        // call retains its provisional zero target. Namespace classification
        // must rejoin that checked identity, not interpret the qualifier as
        // a runtime value merely because early selection was deferred.
        let mut selected_call = call.clone();
        selected_call.target_symbol = authored_target;
        let mut authority_value = checked_trees::CheckedStructuralValueHandle::invalid();
        if call.receiver.is_valid()
            && !program.call_has_no_runtime_receiver_for_target(&selected_call)
        {
            let mut owners = program.traits().iter().filter(|owner| {
                owner.is_boundary
                    && program
                        .trait_machine_instances(owner)
                        .any(|signature| signature.symbol == authored_target)
            });
            let owner = owners.next()?;
            if owners.next().is_some() {
                return None;
            }
            let state = validation::find_state_in_machine(program, self.machine, self.state)?;
            let (source, _) = self.call_ordinal(expression, authored_target)?;
            let source = self.flow.control.calls.get(source);
            let site = validation::CallSite::Expression { expression, call };
            use crate::execution::terminal_unit::calls::{
                BoundaryTraitReceiver, boundary_trait_receiver_source,
            };
            let receiver =
                boundary_trait_receiver_source(program, state, source, &site, owner.symbol)?;
            if let BoundaryTraitReceiver::Parameters(sources) = receiver
                && sources.iter().any(|source| {
                    !self
                        .authored_parameters
                        .iter()
                        .any(|parameter| parameter.symbol == *source)
                })
            {
                return None;
            }
            let machine = program.machine_by_symbol(self.machine)?;
            let reference = validation::expression_result_type_reference(
                program,
                machine,
                state,
                call.receiver,
            )?;
            if validation::canonical_place_from_expression_in_state(
                program,
                self.state,
                self.statement_index,
                call.receiver,
            )
            .is_some_and(|place| matches!(place.root, facts::PlaceRoot::Expression(_)))
            {
                authority_value = self.structural_value(call.receiver, reference)?;
                if self
                    .structural_values
                    .root_for_expression(
                        self.state,
                        u32::try_from(self.statement_index).ok()?,
                        call.receiver,
                    )
                    .is_none()
                {
                    self.structural_values.roots.append(
                        checked_trees::CheckedStructuralValueRoot {
                            machine: self.machine,
                            state: self.state,
                            statement_ordinal: u32::try_from(self.statement_index).ok()?,
                            expression: call.receiver,
                            type_reference: reference,
                            root: authority_value,
                        },
                    );
                }
            }
        }
        let operands = program
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        self.boundary_application(
            expression,
            authored_target,
            target,
            // External leaves retain their authored formal identities while
            // the public requirement owns the execution/result contract.
            validation::call_target_parameters(program, authored_target)?,
            validation::call_target_return_type(program, target.1)?,
            &operands,
            expected_type,
            authority_value,
        )
    }

    /// Build the node once the declaration is known: every formal must be an
    /// supported call operand and the result must be the expected scalar.
    #[allow(clippy::too_many_arguments)]
    fn boundary_application(
        &mut self,
        expression: ExpressionHandle,
        call_target: symbols::SymbolHandle,
        (target_machine, target_state): (symbols::SymbolHandle, symbols::SymbolHandle),
        parameters: &[typed_trees::signature::StateParameter],
        return_type: typed_trees::types::TypeReferenceHandle,
        operands: &[ExpressionHandle],
        expected_type: PrimitiveType,
        authority_value: checked_trees::CheckedStructuralValueHandle,
    ) -> Option<CheckedScalarComputationHandle> {
        // The scalar carrier is the executed payload. A policy-only cast may
        // change later arithmetic, but this call retains its own source use.
        if policy_carrier(self.program, return_type)? != expected_type
            || operands.len() != parameters.len()
        {
            return None;
        }
        let (source_call, call_ordinal) = self.call_ordinal(expression, call_target)?;
        let ComputedCallOperands {
            arguments,
            structural_arguments,
            mut temporary_cleanups,
            claim_transfers,
        } = self.call_operands(source_call, operands, parameters)?;
        if authority_value.is_valid() {
            let mut cleanups = self
                .plans
                .temporary_cleanups
                .span(temporary_cleanups)?
                .to_vec();
            // Reverse establishment order: authority is evaluated first and
            // its captured carrier retires after all ordinary temporaries.
            cleanups.push(checked_trees::CheckedScalarComputationTemporaryCleanup {
                argument_ordinal: None,
                actions: Default::default(),
            });
            temporary_cleanups = self.plans.temporary_cleanups.insert_many(cleanups);
        }
        Some(self.insert(
            expected_type,
            CheckedScalarComputationKind::Call {
                static_requirement_dispatch: None,
                source_call,
                callee: checked_trees::CheckedScalarCallee::Boundary {
                    machine: target_machine,
                    state: target_state,
                    authority_value,
                },
                call_ordinal,
                arguments,
                structural_arguments,
                temporary_cleanups,
                claim_transfers,
            },
        ))
    }
}

/// Classify by callable ownership rather than syntax or a representative
/// provider. Authored boundary bodies remain ordinary checked callees.
pub(super) fn bodyless_boundary_target(
    program: &dyn ::validation::CheckingInput,
    target: symbols::SymbolHandle,
) -> Option<(symbols::SymbolHandle, symbols::SymbolHandle)> {
    if let Some((requirement, _)) =
        validation::exact_external_leaf_boundary_requirement(program, target)
    {
        return Some((requirement, requirement));
    }
    if let Some((machine, state)) = validation::find_machine_head(program, target) {
        // Every bodyless callable owns an opaque signature, including a
        // Requirement. Supply/trust classification is not executable ownership:
        // lowering must not search an opaque requirement for a private body.
        return machine
            .has_bodyless_signature()
            .then_some((machine.symbol, state.symbol));
    }
    crate::values::scalar::call_lowering::call_is_boundary(program, target)
        .then_some((target, target))
}

/// The primitive carrier of a type that is bare or qualified only by an
/// arithmetic policy. Declared domains and ranges keep their own custody.
fn policy_carrier(
    program: &dyn ::validation::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> Option<PrimitiveType> {
    if let Some(primitive) = program.primitive_type_reference(reference) {
        return Some(primitive);
    }
    let typed_trees::types::TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    program
        .type_reference_table
        .constraints(*constraints)
        .iter()
        .all(|constraint| {
            matches!(
                constraint,
                typed_trees::types::TypeConstraintNode::ArithmeticDomain(_)
            )
        })
        .then(|| program.primitive_type_reference(*base_type))
        .flatten()
}
