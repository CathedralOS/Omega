//! An external-completion provider must reach an original native leaf on
//! every accepted CFG path and pass the same status input to that leaf.
//! A no-normal public signature alone does not establish either fact.

use std::sync::Arc;

use diagnostics::Diagnostic;
use semantic_vocabulary::{BlockId, BoundaryMachineId, MachineId, ValueId};
use symbols::SymbolHandle;
use terminal_psi::{OperationKind, TerminalMachine, Terminator};

use crate::PublishedProviderSource;

type Origin = Option<usize>;

/// One accepted machine's actual value declarations, shared by all CFG paths.
/// An unset row differs from a computed value with no retained status origin.
#[derive(Clone)]
struct ValueOrigins {
    machine: MachineId,
    declarations: Arc<[ValueId]>,
    origins: Vec<Option<Origin>>,
}
impl ValueOrigins {
    fn prepare(machine: &TerminalMachine, arguments: Vec<Origin>) -> Result<Self, String> {
        let mut declarations = machine
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .chain(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| block.parameters.iter().map(|parameter| parameter.id)),
            )
            .chain(machine.blocks.iter().flat_map(|block| {
                block
                    .operations
                    .iter()
                    .filter_map(|operation| operation.result.as_scalar().map(|result| result.id))
            }))
            .collect::<Vec<_>>();
        declarations.sort_unstable();
        declarations.dedup();
        let mut values = Self {
            machine: machine.id,
            origins: vec![None; declarations.len()],
            declarations: declarations.into(),
        };
        for (parameter, argument) in machine.parameters.iter().zip(arguments) {
            values.insert(parameter.id, argument)?;
        }
        Ok(values)
    }

    fn get(&self, value: &ValueId) -> Option<&Origin> {
        let position = self.declarations.binary_search(value).ok()?;
        self.origins.get(position)?.as_ref()
    }

    fn insert(&mut self, value: ValueId, origin: Origin) -> Result<(), String> {
        let position = self
            .declarations
            .binary_search(&value)
            .map_err(|_| "completion value lost its accepted declaration")?;
        self.origins[position] = Some(origin);
        Ok(())
    }
}

pub(crate) fn capture(
    source: &PublishedProviderSource,
    provider: &crate::ProviderPlanProvenance,
    target: Option<&str>,
    candidates: &[&crate::DerivedProviderPlan],
) -> Result<Vec<Vec<SymbolHandle>>, Vec<Diagnostic>> {
    if provider.row_requirements.len() != provider.row_realizations.len() {
        return Err(vec![Diagnostic::error(
            "completion review lost exact provider row correspondence",
        )]);
    }
    provider.row_requirements.iter().zip(&provider.row_realizations).map(|(&requirement, realization)| {
        let Some(realization) = realization.authored() else { return Ok(Vec::new()) };
        let completion = typed_trees::boundary_completions::boundary_requirement_completion(&source.typed, requirement);
        if !matches!(completion, semantic_vocabulary::BoundaryCompletion::External(_)) { return Ok(Vec::new()) }
        let mut review = CompletionReview { source, requirement, target, candidates, active: Vec::new() };
        let outcome = review.source_machine(realization, vec![Some(0)]).map_err(|reason| {
            vec![Diagnostic::error(format!("selected completion provider `{}`: {reason}", source.typed.symbols.display_path(realization, "::")))]
        })?;
        if outcome.returns || outcome.leaves.is_empty() {
            return Err(vec![Diagnostic::error("selected external-completion provider has a normal return path or no original leaf")]);
        }
        Ok(outcome.leaves)
    }).collect()
}

#[derive(Default)]
struct Outcome {
    returns: bool,
    result: Origin,
    leaves: Vec<SymbolHandle>,
}
impl Outcome {
    fn returning(result: Origin) -> Self {
        Self {
            returns: true,
            result,
            leaves: Vec::new(),
        }
    }
    fn merge(&mut self, other: Self) {
        if other.returns {
            self.result = if self.returns && self.result != other.result {
                None
            } else {
                other.result
            };
        }
        self.returns |= other.returns;
        for leaf in other.leaves {
            if !self.leaves.contains(&leaf) {
                self.leaves.push(leaf);
            }
        }
    }
}

struct CompletionReview<'a> {
    source: &'a PublishedProviderSource,
    requirement: SymbolHandle,
    target: Option<&'a str>,
    candidates: &'a [&'a crate::DerivedProviderPlan],
    active: Vec<MachineId>,
}
impl CompletionReview<'_> {
    fn source_machine(
        &mut self,
        symbol: SymbolHandle,
        arguments: Vec<Origin>,
    ) -> Result<Outcome, String> {
        let context = crate::ProviderPlanningContext::new(
            &self.source.typed,
            source::DependencyScope::Product,
        );
        let actual = crate::target_machines::realized_row_machine(context, symbol, self.target)
            .ok_or("completion call has no exact target realization")?;
        if !actual.body_is_present {
            return self.original_leaf(actual.symbol, arguments);
        }
        let source = self.source;
        let machine = source.accepted_machine(actual.symbol)?;
        self.machine(machine, arguments)
    }

    fn original_leaf(
        &self,
        symbol: SymbolHandle,
        arguments: Vec<Origin>,
    ) -> Result<Outcome, String> {
        let typed = &self.source.typed;
        let context = crate::ProviderPlanningContext::new(typed, source::DependencyScope::Product);
        let machine = crate::target_machines::realized_row_machine(context, symbol, self.target)
            .ok_or("completion leaf has no exact target declaration")?;
        let matched = self.candidates.iter().any(|candidate| {
            candidate
                .plan
                .rows
                .iter()
                .enumerate()
                .any(|(position, row)| {
                    let original = match candidate.provenance.row_realizations.get(position) {
                        Some(crate::ProviderRowRealization::Authored(actual)) => {
                            *actual == machine.symbol
                        }
                        Some(crate::ProviderRowRealization::BuildNative(origin)) => typed
                            .machine_trait_conformances(machine)
                            .iter()
                            .any(|edge| edge.requirement_symbol == origin.requirement),
                        _ => false,
                    };
                    original
                        && candidate.provenance.row_requirements.get(position)
                            == Some(&self.requirement)
                        && matches!(
                            row.binding,
                            effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                                | effects::provider_plan::ProviderBinding::Import { .. }
                        )
                })
        });
        if !matched {
            return Err("completion call has no original external satisfaction leaf (service redispatch is not a leaf)".into());
        }
        if arguments != [Some(0)] {
            return Err(
                "external-completion provider does not preserve its exact status argument".into(),
            );
        }
        Ok(Outcome {
            returns: false,
            result: None,
            leaves: vec![machine.symbol],
        })
    }

    fn boundary(
        &self,
        boundary: BoundaryMachineId,
        arguments: Vec<Origin>,
    ) -> Result<Outcome, String> {
        let declaration = self
            .source
            .module()
            .boundary_machine(boundary)
            .ok_or("completion call has no accepted boundary declaration")?;
        if !matches!(
            declaration.completion,
            semantic_vocabulary::BoundaryCompletion::External(_)
        ) {
            return Err(
                "completion review cannot treat a normal boundary as external completion".into(),
            );
        }
        let symbol = self
            .source
            .correspondence()
            .boundaries
            .get(boundary)
            .copied()
            .ok_or("completion call lost its exact boundary source")?;
        self.original_leaf(symbol, arguments)
    }

    fn machine(
        &mut self,
        machine: &TerminalMachine,
        arguments: Vec<Origin>,
    ) -> Result<Outcome, String> {
        if self.active.contains(&machine.id) {
            return Err(
                "external-completion provider has a cyclic accepted invocation path".into(),
            );
        }
        if arguments.len() != machine.parameters.len() || !machine.structural_parameters.is_empty()
        {
            return Err(
                "completion review cannot close this call's complete parameter namespace".into(),
            );
        }
        let values = ValueOrigins::prepare(machine, arguments)?;
        self.active.push(machine.id);
        let outcome = self.block(machine, machine.entry, values, &mut Vec::new());
        self.active.pop();
        outcome
    }

    fn block(
        &mut self,
        machine: &TerminalMachine,
        id: BlockId,
        mut values: ValueOrigins,
        active: &mut Vec<BlockId>,
    ) -> Result<Outcome, String> {
        if values.machine != machine.id {
            return Err("completion origins belong to another accepted machine".into());
        }
        if active.contains(&id) {
            return Err("external-completion provider has a cyclic accepted control path".into());
        }
        let block = machine
            .blocks
            .iter()
            .find(|block| block.id == id)
            .ok_or("completion edge has no accepted destination block")?;
        active.push(id);
        let mut leaves = Outcome::default();
        for operation in &block.operations {
            let origin = if let Some(call) = operation.kind.module_invocation() {
                if !call.structural_arguments.is_empty()
                    || !call.erased_arguments.is_empty()
                    || !call.erased_proof_arguments.is_empty()
                {
                    return Err(
                        "completion review cannot close this call's structural or erased operands"
                            .into(),
                    );
                }
                let source = self.source;
                let callee = source
                    .module()
                    .machine(call.callee)
                    .ok_or("completion call lost its accepted callee")?;
                let arguments = call
                    .arguments
                    .iter()
                    .map(|value| values.get(value).copied().flatten())
                    .collect();
                let outcome = self.machine(callee, arguments)?;
                let returns = outcome.returns;
                let result = outcome.result;
                let mut completed_paths = outcome;
                completed_paths.returns = false;
                completed_paths.result = None;
                leaves.merge(completed_paths);
                if !returns {
                    active.pop();
                    return Ok(leaves);
                }
                result
            } else if operation.kind.callable_invocation().is_some() {
                return Err("a runtime callable public contract does not establish an original external-completion leaf".into());
            } else if matches!(operation.kind, OperationKind::BoundaryCall { .. }) {
                return Err("completion review does not substitute service redispatch for an original external leaf".into());
            } else {
                // The accepted operation may compute another value. Its
                // result is not the status input merely because it has the
                // same carrier or happens to equal it at one caller.
                None
            };
            if let Some(result) = operation.result.as_scalar() {
                values.insert(result.id, origin)?;
            }
        }
        let outcome = match &block.terminator {
            Terminator::Return { value, .. } => {
                Outcome::returning(values.get(value).copied().flatten())
            }
            Terminator::ReturnUnit { .. } => Outcome::returning(None),
            Terminator::ReturnStructural { .. } => {
                return Err("completion review cannot close a structural return".into());
            }
            Terminator::Crash { .. } => {
                return Err("aborting provider does not establish external completion".into());
            }
            Terminator::ExternalCompletion {
                boundary,
                arguments,
                structural_arguments,
                ..
            } => {
                if !structural_arguments.is_empty() {
                    return Err("completion leaf retains structural operands".into());
                }
                self.boundary(
                    *boundary,
                    arguments
                        .iter()
                        .map(|value| values.get(value).copied().flatten())
                        .collect(),
                )?
            }
            Terminator::InvokeNoNormal {
                callee,
                arguments,
                structural_arguments,
                erased_arguments,
                erased_proof_arguments,
                ..
            } => {
                if !structural_arguments.is_empty()
                    || !erased_arguments.is_empty()
                    || !erased_proof_arguments.is_empty()
                {
                    return Err(
                        "completion transfer retains unsupported structural or erased operands"
                            .into(),
                    );
                }
                let source = self.source;
                let callee = source
                    .module()
                    .machine(*callee)
                    .ok_or("completion transfer lost its accepted callee")?;
                let outcome = self.machine(
                    callee,
                    arguments
                        .iter()
                        .map(|value| values.get(value).copied().flatten())
                        .collect(),
                )?;
                if outcome.returns {
                    return Err("no-normal completion transfer has a returning callee".into());
                }
                outcome
            }
            Terminator::Jump {
                target,
                arguments,
                structural_arguments,
                ..
            } => {
                if !structural_arguments.is_empty() {
                    return Err("completion edge passes unrepresented structural operands".into());
                }
                self.successor(machine, *target, arguments, &values, active)?
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                if !when_true.structural_arguments.is_empty()
                    || !when_false.structural_arguments.is_empty()
                {
                    return Err("completion edge passes unrepresented structural operands".into());
                }
                let mut outcome = self.successor(
                    machine,
                    when_true.target,
                    &when_true.arguments,
                    &values,
                    active,
                )?;
                outcome.merge(self.successor(
                    machine,
                    when_false.target,
                    &when_false.arguments,
                    &values,
                    active,
                )?);
                outcome
            }
            Terminator::StructuralCase { .. } => {
                return Err("completion review cannot close a structural dispatch".into());
            }
        };
        active.pop();
        leaves.merge(outcome);
        Ok(leaves)
    }

    fn successor(
        &mut self,
        machine: &TerminalMachine,
        target: BlockId,
        arguments: &[ValueId],
        values: &ValueOrigins,
        active: &mut Vec<BlockId>,
    ) -> Result<Outcome, String> {
        let block = machine
            .blocks
            .iter()
            .find(|block| block.id == target)
            .ok_or("completion edge lost its accepted target")?;
        if block.parameters.len() != arguments.len() {
            return Err("completion edge changes its exact parameter arity".into());
        }
        let mut next = values.clone();
        for (parameter, argument) in block.parameters.iter().zip(arguments) {
            next.insert(parameter.id, values.get(argument).copied().flatten())?;
        }
        self.block(machine, target, next, active)
    }
}
