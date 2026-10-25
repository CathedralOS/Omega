//! A descriptor result's measure is a private normal-return observation.
//! All readable published clauses participate, not one recognized equality
//! spelling. Actual observations are captured by the source value owner;
//! neither a range-shaped argument nor an implementation body supplies facts.
//! The scalar and collection routes share selected-contract identity,
//! dependency exclusion and guarantee transport.

use super::{
    CallOccurrence, ScopedCalls, SelectedCallContract, owner_parameters, owner_symbol,
    selected_call_contract,
};
use super::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticValue,
};
use crate::value_custody::places::collection_operations::{
    MeasureReceiver, collection_measure_member,
};
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

impl<'program> ScopedCalls<'program> {
    pub(super) fn collection_length(
        &mut self,
        source: &ScopedArithmeticExpression,
    ) -> Option<ScopedArithmeticValue> {
        self.step()?;
        if self.depth >= 64
            || !self
                .program
                .expression_table
                .expression_is_valid(source.expression)
        {
            return None;
        }
        if let Some((_, value)) = self.lengths.iter().find(|(original, _)| original == source) {
            return Some(value.clone());
        }
        let ExpressionNode::Call(call) =
            self.program.expression_table.expression(source.expression)
        else {
            return None;
        };
        let owner = selected_call_contract(self.program, source.owner, call)?;
        if matches!(owner, SelectedCallContract::DynamicSlot { .. })
            || !crate::value_custody::type_shapes::has_slice_base(
                self.program,
                crate::proof_contracts::contract_entailment::specification_calls::owner_return_type(
                    &owner,
                ),
            )
        {
            return None;
        }
        let result = ScopedArithmeticValue::Atom {
            identity: format!(
                "\0scoped-length:{}:{:?}:{:?}",
                self.lengths.len(),
                source.expression,
                owner_symbol(&owner)
            ),
            unsigned: true,
        };
        // Reserve identity before preparing nested actual observations. A
        // recursive proof dependency may retain an unknown, never its own
        // guarantee as evidence.
        self.lengths.push((source.clone(), result.clone()));
        let parameters = owner_parameters(self.program, &owner);
        let arguments = self
            .program
            .expression_table
            .expression_handles(call.arguments);
        if parameters.len() != arguments.len()
            || !crate::machine_calls::contract_dependencies::independent_call_application(
                self.program,
                self.context,
                owner_symbol(&owner),
                typed_trees::references::StaticArgumentCall::Expression(source.expression, call),
            )
            || !self
                .independent(owner, &mut Vec::new(), &mut Vec::new())
                .unwrap_or(false)
        {
            return Some(result);
        }
        self.depth += 1;
        let transported = (|| {
            let mut actuals = Vec::new();
            for actual in &source.bindings {
                actuals.push(ScopedArithmeticBinding {
                    binder: actual.binder,
                    value: self.value(&actual.value)?,
                });
            }
            let bindings = self.completed_bindings(
                owner,
                arguments,
                CallOccurrence::Expression(source.expression),
                &actuals,
                None,
                self.lengths.len() - 1,
            )?;
            self.transport_guarantees(owner, &bindings, Some(&result))
        })();
        self.depth -= 1;
        transported?;
        Some(result)
    }

    pub(super) fn bind_length_projections(
        &mut self,
        owner: SelectedCallContract<'program>,
        guarantee: ExpressionHandle,
        result: Option<&ScopedArithmeticValue>,
        bindings: &mut Vec<ScopedArithmeticBinding>,
    ) -> Option<()> {
        let mut occurrences = Vec::new();
        self.postorder(guarantee, &mut occurrences)?;
        let parameters = owner_parameters(self.program, &owner);
        // The arithmetic scope is the entry state, but the reserved result
        // occurrence belongs to the callable declaration, not that state.
        let result_owner = match owner {
            SelectedCallContract::Concrete(machine, _) => machine.symbol,
            _ => owner_symbol(&owner),
        };
        for expression in occurrences {
            let Some(member) = self.program.expression_table.member_node(expression) else {
                continue;
            };
            let observation = if let Some((symbol, reference)) =
                crate::reserved_result_owner(self.program, member.receiver)
            {
                if symbol != result_owner {
                    continue;
                }
                result.map(|value| (reference, value.clone()))
            } else {
                self.program
                    .expression_table
                    .name_path_node(member.receiver)
                    .and_then(|name| {
                        let parameter = parameters.iter().find(|parameter| {
                            parameter.symbol == name.symbol && name.symbol == name.head_symbol
                        })?;
                        if parameter.is_mutable {
                            return None;
                        }
                        let value = bindings.iter().find(|binding| {
                            binding.binder == ScopedArithmeticBinder::Symbol(parameter.symbol)
                        })?;
                        Some((parameter.type_reference, value.value.clone()))
                    })
            };
            if let Some((reference, value)) = observation
                && collection_measure_member(
                    self.program,
                    member,
                    MeasureReceiver::Declared(reference),
                ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length)
            {
                bindings.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(expression),
                    value,
                });
            }
        }
        // Noninteger formals are observation subjects, not scalar symbols.
        bindings.retain(|binding| {
            let ScopedArithmeticBinder::Symbol(symbol) = binding.binder else {
                return true;
            };
            !parameters.iter().any(|parameter| {
                parameter.symbol == symbol
                    && !self
                        .program
                        .primitive_type_reference(parameter.type_reference)
                        .is_some_and(|primitive| primitive.is_fixed_integer())
            })
        });
        Some(())
    }
}
