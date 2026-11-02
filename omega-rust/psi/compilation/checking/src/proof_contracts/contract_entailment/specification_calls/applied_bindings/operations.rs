//! Project the callable heads a predicate actually mentions through its
//! already authenticated static application. Namespace correspondence comes
//! from the same owner used when materializing public signature expressions.

use super::{AppliedProofBindings, MachineBindings};
use crate::frozen_program::expression_table;
use typed_trees::expression::{ExpressionHandle, StaticMachineArgument};

impl AppliedProofBindings {
    pub fn predicate_machine_bindings(
        &self,
        program: &dyn crate::CheckingInput,
        owner: symbols::SymbolHandle,
        parameters: &[typed_trees::signature::StateParameter],
        fact: ExpressionHandle,
    ) -> Result<MachineBindings, &'static str> {
        let mut result = self.machines.clone();
        if self.implementations.is_empty() {
            return Ok(result);
        }
        let mut expressions = Vec::new();
        crate::expression_traversal::collect_expression_tree(program, fact, &mut expressions);
        for expression in expressions {
            let Some(call) = expression_table(program).call_node(expression) else {
                continue;
            };
            let source = crate::proof_contracts::contract_entailment::structural_terms::structural_call_target_in_frame(
                program, call, Some((owner, parameters)),
            );
            if !source.is_valid() {
                continue;
            }
            let binder = program.symbols.get(source).parent;
            let Some((_, selected)) = self
                .implementations
                .iter()
                .find(|(parameter, _)| *parameter == binder)
            else {
                continue;
            };
            if result.iter().any(|(known, _, _)| *known == source) {
                continue;
            }
            let symbols = program.selected_implementation_namespace_symbols(binder, selected)?;
            let target = symbols
                .iter()
                .find(|(known, _)| *known == source)
                .map(|(_, target)| *target)
                .ok_or("predicate implementation lost its exact selected requirement child")?;
            let (target, result_type, name) = if let Some(conformance) =
                program.conformance_by_symbol(selected.symbol)
            {
                let mut rows = conformance
                    .rows
                    .iter()
                    .filter(|row| row.requirement == target);
                let row = rows
                    .next()
                    .ok_or("predicate implementation has no selected requirement row")?;
                if rows.next().is_some() {
                    return Err("predicate implementation has ambiguous selected requirement rows");
                }
                let machine = program
                    .machine_by_symbol(row.realization_machine)
                    .ok_or("predicate implementation lost its selected callable")?;
                let state = program
                    .state_in_machine(machine, row.realization_state)
                    .ok_or("predicate implementation lost its selected entry")?;
                let public = typed_trees::trait_definition::AppliedTraitScope::from_conformance(
                    program.typed_program(),
                    conformance,
                )
                .and_then(|scope| {
                    scope.declaring_scope(program.typed_program(), row.declaring_trait)
                })
                .ok_or("predicate implementation lost its complete public application")?;
                let signature = program
                    .applied_callable_signature(&public, row.requirement)
                    .or_else(|| {
                        (public.substitutions.is_empty()
                            && public.declaration.lifetime_parameters.is_empty())
                        .then(|| {
                            program.trait_machine_signature_by_symbol(
                                public.declaration,
                                row.requirement,
                            )
                        })
                        .flatten()
                    })
                    .ok_or("predicate implementation lost its applied public result")?;
                // A named map keeps its actual selected entry identity.
                // The public requirement symbol is shared by other maps
                // and cannot identify this implementation's value. Its public
                // result recipe remains authoritative over the private body.
                (
                    state.symbol,
                    signature.return_type,
                    row.requirement_name.clone(),
                )
            } else {
                let (_, signature) = crate::selected_requirement_signature(program, target)
                    .ok_or("predicate forwarded implementation lost its public signature")?;
                (target, signature.return_type, signature.name.clone())
            };
            if super::super::super::struct_trace() {
                eprintln!(
                    "STRUCT predicate-operation source={:?} selected={:?} target={target:?} result={result_type:?}",
                    source, selected.symbol
                );
            }
            result.push((
                source,
                result_type,
                StaticMachineArgument {
                    symbol: target,
                    path: Box::from([name]),
                    ..StaticMachineArgument::default()
                },
            ));
        }
        Ok(result)
    }
}
