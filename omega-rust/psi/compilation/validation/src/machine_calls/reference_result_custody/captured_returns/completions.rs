//! Reference coverage needs every evaluated child, but only one selected arm.
//! Type-proven absence needs no callee origin equation; it proves neither
//! termination nor permission to omit evaluated inputs or their effects.

use arena::Handle;
use checked_trees::{FlowControlFacts, FlowOperandCompletion, FlowOperandFact};
use std::collections::BTreeMap;
use symbols::SymbolHandle;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Coverage {
    Call(SymbolHandle),
    All(Vec<usize>),
    Any(Vec<usize>),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct ReturnCoverage {
    nodes: Vec<Coverage>,
    root: usize,
}

impl ReturnCoverage {
    pub(super) fn captured(
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        calls: impl Fn(&FlowOperandFact) -> Option<Vec<SymbolHandle>>,
    ) -> Option<Self> {
        let mut builder = Builder::new(control, calls);
        builder.graph.root = builder.operand(operand)?;
        Some(builder.graph)
    }

    pub(super) fn invocation(
        control: &FlowControlFacts,
        call: &checked_trees::FlowCallFact,
        target: Option<SymbolHandle>,
        calls: impl Fn(&FlowOperandFact) -> Option<Vec<SymbolHandle>>,
    ) -> Option<Self> {
        let mut builder = Builder::new(control, calls);
        let mut required = Vec::new();
        control.operands.span(call.operands)?;
        for operand in call.operands.handles() {
            required.push(builder.value(operand)?);
        }
        if let Some(target) = target {
            required.push(builder.push(Coverage::Call(target)));
        }
        builder.graph.root = builder.push(Coverage::All(required));
        Some(builder.graph)
    }

    pub(super) fn dependencies(&self) -> impl Iterator<Item = SymbolHandle> + '_ {
        self.nodes.iter().filter_map(|node| match node {
            Coverage::Call(state) => Some(*state),
            Coverage::All(_) | Coverage::Any(_) => None,
        })
    }

    pub(super) fn established(&self, returning: impl Fn(SymbolHandle) -> bool) -> bool {
        let mut values = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            values.push(match node {
                Coverage::Call(state) => returning(*state),
                Coverage::All(children) => children.iter().all(|child| values[*child]),
                Coverage::Any(children) => children.iter().any(|child| values[*child]),
            });
        }
        values.get(self.root).copied().unwrap_or(false)
    }
}

struct Builder<'facts, Calls> {
    control: &'facts FlowControlFacts,
    calls: Calls,
    graph: ReturnCoverage,
    // None is an active capture, not an empty completed value. A malformed
    // cycle in the retained value graph must refuse independently of calls.
    values: BTreeMap<(u32, u32), Option<usize>>,
    prefixes: BTreeMap<(u32, u32), Option<usize>>,
}

impl<'facts, Calls> Builder<'facts, Calls>
where
    Calls: Fn(&FlowOperandFact) -> Option<Vec<SymbolHandle>>,
{
    fn new(control: &'facts FlowControlFacts, calls: Calls) -> Self {
        Self {
            control,
            calls,
            graph: ReturnCoverage::default(),
            values: BTreeMap::new(),
            prefixes: BTreeMap::new(),
        }
    }

    fn push(&mut self, completion: Coverage) -> usize {
        let index = self.graph.nodes.len();
        self.graph.nodes.push(completion);
        index
    }

    fn value(&mut self, value: Handle<FlowOperandFact>) -> Option<usize> {
        let key = (value.arena_index(), value.generation());
        if let Some(result) = self.values.get(&key) {
            return *result;
        }
        if !self.control.operands.is_valid(value) {
            return None;
        }
        self.values.insert(key, None);
        let result = self.operand(self.control.operands.get(value))?;
        self.values.insert(key, Some(result));
        Some(result)
    }

    fn prefix(&mut self, arm: Handle<checked_trees::FlowOperandAlternative>) -> Option<usize> {
        let key = (arm.arena_index(), arm.generation());
        if let Some(result) = self.prefixes.get(&key) {
            return *result;
        }
        if !self.control.operand_alternatives.is_valid(arm) {
            return None;
        }
        self.prefixes.insert(key, None);
        let arm = *self.control.operand_alternatives.get(arm);
        let mut inputs = Vec::new();
        if arm.prior_evaluation.is_valid() {
            inputs.push(self.prefix(arm.prior_evaluation)?);
        } else if arm.evaluation_inputs.is_empty() {
            // A first arm without its subject is missing evaluation evidence.
            return None;
        }
        self.control.operands.span(arm.evaluation_inputs)?;
        for input in arm.evaluation_inputs.handles() {
            inputs.push(self.value(input)?);
        }
        let result = self.push(Coverage::All(inputs));
        self.prefixes.insert(key, Some(result));
        Some(result)
    }

    fn operand(&mut self, operand: &FlowOperandFact) -> Option<usize> {
        let mut required = Vec::new();
        for state in (self.calls)(operand)? {
            required.push(self.push(Coverage::Call(state)));
        }
        let inputs = self.control.operands.span(operand.evaluated_inputs)?;
        let guaranteed = operand.conditional_inputs_from.unwrap_or(inputs.len());
        inputs.get(..guaranteed)?;
        for input in operand.evaluated_inputs.handles().take(guaranteed) {
            required.push(self.value(input)?);
        }
        for source in [operand.backing, operand.value_source] {
            if source.is_valid() {
                required.push(self.value(source)?);
            }
        }
        match operand.completion {
            FlowOperandCompletion::Constructed(children) => {
                for child in self.control.operand_children.span(children)? {
                    required.push(self.value(child.value)?);
                }
            }
            FlowOperandCompletion::Selected(alternatives) => {
                self.control.operand_alternatives.span(alternatives)?;
                let mut selected = Vec::new();
                for alternative in alternatives.handles() {
                    let prefix = self.prefix(alternative)?;
                    let value =
                        self.value(self.control.operand_alternatives.get(alternative).value)?;
                    selected.push(self.push(Coverage::All(vec![prefix, value])));
                }
                required.push(self.push(Coverage::Any(selected)));
            }
            FlowOperandCompletion::Copied { copy, .. } => {
                if !self.control.copied_values.is_valid(copy) {
                    return None;
                }
                required.push(self.value(self.control.copied_values.get(copy).source)?);
            }
            FlowOperandCompletion::InvocationResult { invocation, .. } => {
                if !self.control.calls.is_valid(invocation) {
                    return None;
                }
                // A projected returned descriptor may omit the aggregate's
                // evaluated-input span. The exact performed call still owns
                // all actual captures, including recursively returned inputs.
                let call = self.control.calls.get(invocation);
                self.control.operands.span(call.operands)?;
                for input in call.operands.handles() {
                    required.push(self.value(input)?);
                }
            }
            FlowOperandCompletion::Observation | FlowOperandCompletion::IncomingContents { .. } => {
            }
        }
        Some(self.push(Coverage::All(required)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::{FlowCallFact, FlowOperandAlternative, FlowOperandChild};

    fn call(control: &mut FlowControlFacts, state: u32) -> Handle<FlowOperandFact> {
        let invocation = control.calls.insert(FlowCallFact {
            target_symbol: SymbolHandle::from_parts(state, 1),
            ..FlowCallFact::default()
        });
        control.operands.insert(FlowOperandFact {
            completion: FlowOperandCompletion::InvocationResult {
                invocation,
                result_type: Handle::invalid(),
                path: arena::HandleSpan::empty(),
            },
            ..FlowOperandFact::default()
        })
    }

    fn captured(control: &FlowControlFacts, value: &FlowOperandFact) -> ReturnCoverage {
        ReturnCoverage::captured(control, value, |operand| {
            Some(match operand.completion {
                FlowOperandCompletion::InvocationResult { invocation, .. } => {
                    vec![control.calls.get(invocation).target_symbol]
                }
                _ => Vec::new(),
            })
        })
        .expect("the actual capture graph is complete")
    }

    #[test]
    fn captured_return_completion_requires_every_constructed_child() {
        let mut control = FlowControlFacts::default();
        let recursive = call(&mut control, 1);
        let grounded = call(&mut control, 2);
        let inner = control.operand_children.insert_many([FlowOperandChild {
            value: recursive,
            ..FlowOperandChild::default()
        }]);
        let inner = control.operands.insert(FlowOperandFact {
            completion: FlowOperandCompletion::Constructed(inner),
            ..FlowOperandFact::default()
        });
        // Some(record { recursive: A(), other: B() }) must not use B's
        // completed result to hide A's empty construction approximation.
        let children = control.operand_children.insert_many([
            FlowOperandChild {
                value: inner,
                ..FlowOperandChild::default()
            },
            FlowOperandChild {
                value: grounded,
                ..FlowOperandChild::default()
            },
        ]);
        let value = FlowOperandFact {
            completion: FlowOperandCompletion::Constructed(children),
            ..FlowOperandFact::default()
        };
        let completion = captured(&control, &value);
        assert!(!completion.established(|state| state.arena_index() == 2));
        assert!(completion.established(|_| true));
        assert_eq!(completion.dependencies().count(), 2);
    }

    #[test]
    fn captured_return_completion_keeps_selected_prefix_separate_from_arm_values() {
        let mut control = FlowControlFacts::default();
        let subject = call(&mut control, 1);
        let recursive = call(&mut control, 2);
        let constant = control.operands.insert(FlowOperandFact::default());
        let prefix = control
            .operands
            .insert_many([*control.operands.get(subject)]);
        let mut alternatives = arena::HandleSpan::empty();
        let prior = control.operand_alternatives.append_to_span(
            &mut alternatives,
            FlowOperandAlternative {
                evaluation_inputs: prefix,
                value: recursive,
                ..FlowOperandAlternative::default()
            },
        );
        control.operand_alternatives.append_to_span(
            &mut alternatives,
            FlowOperandAlternative {
                prior_evaluation: prior,
                value: constant,
                ..FlowOperandAlternative::default()
            },
        );
        let value = FlowOperandFact {
            completion: FlowOperandCompletion::Selected(alternatives),
            ..FlowOperandFact::default()
        };
        let completion = captured(&control, &value);
        // The constant arm still owes the subject, but not the first arm's
        // recursive value. Earlier pattern evaluation stays in the prefix.
        assert!(!completion.established(|state| state.arena_index() == 2));
        assert!(completion.established(|state| state.arena_index() == 1));
    }

    #[test]
    fn captured_return_completion_requires_pattern_before_selected_constant() {
        let mut control = FlowControlFacts::default();
        let subject = control.operands.insert(FlowOperandFact::default());
        let pattern = call(&mut control, 1);
        let constant = control.operands.insert(FlowOperandFact::default());
        let inputs = control.operands.insert_many([
            *control.operands.get(subject),
            *control.operands.get(pattern),
        ]);
        let alternatives = control
            .operand_alternatives
            .insert_many([FlowOperandAlternative {
                evaluation_inputs: inputs,
                value: constant,
                ..FlowOperandAlternative::default()
            }]);
        let value = FlowOperandFact {
            completion: FlowOperandCompletion::Selected(alternatives),
            ..FlowOperandFact::default()
        };
        assert!(!captured(&control, &value).established(|_| false));
        assert!(captured(&control, &value).established(|_| true));
    }

    #[test]
    fn captured_return_completion_distinguishes_formed_empty_from_missing_selection() {
        let control = FlowControlFacts::default();
        let empty = FlowOperandFact {
            completion: FlowOperandCompletion::Constructed(arena::HandleSpan::empty()),
            ..FlowOperandFact::default()
        };
        assert!(captured(&control, &empty).established(|_| false));
        let missing = FlowOperandFact {
            completion: FlowOperandCompletion::Selected(arena::HandleSpan::empty()),
            ..FlowOperandFact::default()
        };
        assert!(!captured(&control, &missing).established(|_| false));
    }
    #[test]
    fn captured_return_coverage_known_empty_result_keeps_evaluated_inputs() {
        let mut control = FlowControlFacts::default();
        let child = call(&mut control, 1);
        let no_descriptors = call(&mut control, 3);
        let inputs = control.operands.insert_many([*control.operands.get(child)]);
        let value = *control.operands.get(no_descriptors);
        let FlowOperandCompletion::InvocationResult { invocation, .. } = value.completion else {
            panic!("the descriptor retains its actual invocation");
        };
        control.calls.get_mut(invocation).operands = inputs;
        let coverage = ReturnCoverage::captured(&control, &value, |operand| {
            let FlowOperandCompletion::InvocationResult { invocation, .. } = operand.completion
            else {
                return Some(Vec::new());
            };
            let target = control.calls.get(invocation).target_symbol;
            // The caller supplies this conclusion only after a complete
            // declared leaf query proves the result has no descriptors.
            Some(if target.arena_index() == 3 {
                Vec::new()
            } else {
                vec![target]
            })
        })
        .expect("both completed inputs are retained");
        assert!(!coverage.established(|_| false));
        assert!(coverage.established(|state| state.arena_index() == 1));
        assert_eq!(
            coverage.dependencies().collect::<Vec<_>>(),
            vec![SymbolHandle::from_parts(1, 1)]
        );
        // A type-proven empty result does not require a reference equation
        // for state 3. This is no assertion that state 3 terminates.
    }
}
