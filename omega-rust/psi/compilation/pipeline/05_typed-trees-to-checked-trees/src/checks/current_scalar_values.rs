//! Current scalar values and descriptor measures across statement transfers.
//!
//! A hypothesis captures its input roster when it is established. Assignment
//! captures the RHS under that roster before replacing its destination; later
//! reads never reinterpret an earlier name as the replacement. This is private
//! proof construction scratch, not an invocation-memory snapshot or a retained
//! certificate. Arrivals, exits and ranks share this transfer. Unknown effects
//! retire current bindings; they cannot revive an entry fact about new storage.
//! Saved Booleans retain conditional propositions under that same captured
//! roster. Guard selection consumes one polarity without rereading its numeric
//! sources; Boolean replacement retires the association like any other store.
//!
//! A fixed array's extent is metadata for every reference access and every
//! qualification. A stable slice descriptor retains its observed length under
//! the existing access and effect checks. Local descriptors use the same
//! replacement and effect retirement as scalar bindings; their captured
//! measures survive endpoint stores, never descriptor replacement.
//!
//! A match result has one length only when every arm's length is the same
//! observation, or each arm folds to the same exact integer. Differing arms
//! stay unknown rather than publishing one arm's extent. A plain record
//! value captures each slice field's length when that value is transferred;
//! a later overlapping write retires the affected field observations. A returned
//! record publishes the same field lengths on `result.field`, including when every match arm
//! builds that field from an equal window. An `ensures` bound that names a
//! parameter is that call argument's value, so `result.values.len == count`
//! arrives as the length the caller passed.
//!
//! An exact or trapping integer cast denotes its operand when the operand's
//! carrier already fits in the target, or the live facts at that statement
//! prove it does. Wrapping and saturating casts stay outside this equality:
//! they are different values. The window length `end - start` can then meet
//! `count as u64` for a nonnegative wrapping count, through a local, a field,
//! a parameter, or a call, because each spelling is that same integer.
use super::ranges::types::expression_integer_carrier_maximum;
use super::ranges::types::expression_integer_carrier_minimum;
use crate::checks::unsigned;
use crate::flow::expressions;
use crate::values::scalar::structural_fields;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetHandle};
use typed_trees::types::{FixedArrayLength, PrimitiveType, TypeReferenceHandle, TypeReferenceNode};
use typed_trees::{machine::Machine, state::State};
use validation::BigIntInterval;
use validation::{
    ScopedArithmeticBinder, ScopedArithmeticBinding, ScopedArithmeticExpression,
    ScopedArithmeticHypothesis, ScopedArithmeticValue, StrictArithmeticImplicationJudgment,
};

mod application_values;
mod call_contracts;
mod collections;
mod conditions;
mod declaration_reads;
mod entry_fields;
pub(crate) use entry_fields::parameter_projection_paths;
mod joins;
mod normal_completion;
use normal_completion::{CapturedResultFields, result_scalar_field_occurrence};
mod scalar_fields;
mod scalar_storage;
use conditions::ConditionBinding;
use scalar_fields::{ScalarField, ScalarProjection, scalar_field_occurrence};

#[derive(Clone)]
struct Binding {
    symbol: SymbolHandle,
    name: String,
    value: Option<ScopedArithmeticValue>,
    alternatives: Vec<ScopedArithmeticValue>,
    immutable_coordinate: bool,
    /// A by-value binding declared without `mut`. No store or call can reach
    /// it: a write needs a mutable place, and so does a `&mut` loan. Its
    /// current value therefore survives every effect even when the input has
    /// no storage graph to ask, as in build-time admission over bare typed
    /// trees. Unlike `immutable_coordinate`, it remains an ordinary store-
    /// alias candidate.
    unwritable: bool,
}

#[derive(Clone, PartialEq, Eq)]
enum LengthSubject {
    Storage(SymbolHandle),
    Result(SymbolHandle),
    /// One captured slice field, identified by its complete declaration path.
    /// The owner is the current storage binding, not the source producer.
    Field {
        owner: SymbolHandle,
        path: Vec<SymbolHandle>,
    },
    /// One slice field of the value returned from this machine. The owner is
    /// the machine, and `result.field.len` reads this observation.
    ResultField {
        owner: SymbolHandle,
        path: Vec<SymbolHandle>,
    },
}

/// A captured descriptor observation, not the capacity of its backing owner.
#[derive(Clone)]
struct Length {
    subject: LengthSubject,
    name: String,
    stable: bool,
    type_reference: TypeReferenceHandle,
    value: Option<ScopedArithmeticValue>,
}

/// The value a tracked entry holds, compared to find the entries a statement
/// established or changed.
#[derive(PartialEq)]
enum EntryValue {
    Binding(Option<ScopedArithmeticValue>, Vec<ScopedArithmeticValue>),
    Value(Option<ScopedArithmeticValue>),
    Condition(
        Vec<ScopedArithmeticHypothesis>,
        Vec<ScopedArithmeticHypothesis>,
    ),
}

/// A storage length's graph place. A result length names no storage.
fn length_place(subject: &LengthSubject) -> Option<validation::CanonicalPlace> {
    match subject {
        LengthSubject::Storage(symbol) => Some(validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(*symbol),
            segments: Vec::new(),
        }),
        LengthSubject::Field { owner, path } => Some(validation::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(*owner),
            segments: path
                .iter()
                .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                .collect(),
        }),
        LengthSubject::Result(_) | LengthSubject::ResultField { .. } => None,
    }
}

pub(crate) struct CurrentScalarValues<'program> {
    program: &'program dyn ::validation::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    statement_index: usize,
    initial: Vec<Binding>,
    current: Vec<Binding>,
    lengths: Vec<Length>,
    initial_lengths: Vec<Length>,
    fields: Vec<ScalarField>,
    result_fields: Option<CapturedResultFields>,
    conditions: Vec<ConditionBinding>,
    hypotheses: Vec<ScopedArithmeticHypothesis>,
    in_transition_target: bool,
    scalar_observations: std::cell::OnceCell<Vec<validation::CapturedScalarObservation>>,
    /// The snapshot each tracked place's entry was established at; absent is
    /// incoming storage.
    anchors: Vec<(
        validation::CanonicalPlace,
        validation::completed_storage::versions::Snapshot,
    )>,
}

impl<'program> CurrentScalarValues<'program> {
    pub(crate) fn machine(&self) -> &Machine {
        self.machine
    }
    pub(crate) fn state(&self) -> &State {
        self.state
    }

    pub(crate) fn program(&self) -> &'program dyn ::validation::CheckingInput {
        self.program
    }

    pub(crate) fn at(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
        statement_index: usize,
        target: TransitionTargetHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
    ) -> Option<Self> {
        Self::at_observing_projections(
            program,
            machine,
            state,
            statement_index,
            target,
            frames,
            &[],
        )
    }

    pub(crate) fn at_observing_projections(
        program: &'program dyn ::validation::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
        statement_index: usize,
        target: TransitionTargetHandle,
        frames: Option<&validation::CallFrameResolver<'_>>,
        requested: &[Vec<facts::PlaceSegment>],
    ) -> Option<Self> {
        let mut initial = program
            .state_parameters(state)
            .iter()
            .filter_map(|parameter| {
                // A scalar parameter names either an owned value or the
                // current scalar in a stable borrowed place. Both use the
                // same store and call-frame invalidation below. Write-only
                // storage starts without an observation; its first completed
                // store must supply one before any postcondition can read it.
                let (primitive, readable) = if let Some(reference) =
                    validation::reference_shell(program, parameter.type_reference)
                {
                    if !validation::state_reference_parameter_binding_is_stable(
                        program,
                        machine,
                        state,
                        parameter.symbol,
                    ) {
                        return None;
                    }
                    (
                        integer_carrier(program, reference.referee)?,
                        reference.access.is_readable(),
                    )
                } else {
                    (integer_carrier(program, parameter.type_reference)?, true)
                };
                Some(Binding {
                    symbol: parameter.symbol,
                    name: parameter.name.to_string(),
                    value: readable.then(|| {
                        entry_fields::entry_value(
                            program,
                            validation::reference_shell(program, parameter.type_reference)
                                .map_or(parameter.type_reference, |shell| shell.referee),
                            None,
                            ScopedArithmeticValue::Atom {
                                identity: format!(
                                    "state:{:?}:parameter:{:?}",
                                    state.symbol, parameter.symbol
                                ),
                                unsigned: unsigned(primitive),
                            },
                        )
                    }),
                    alternatives: Vec::new(),
                    immutable_coordinate: false,
                    unwritable: !parameter.is_mutable
                        && validation::reference_shell(program, parameter.type_reference).is_none(),
                })
            })
            .collect::<Vec<_>>();
        // Machine Value/Const binders are immutable invocation coordinates.
        // Their exact symbols survive every subordinate state's scope; a
        // same-spelled state formal is a different captured value.
        initial.extend(
            program
                .machine_type_parameters(machine)
                .iter()
                .filter_map(|parameter| {
                    let reference = match parameter.kind {
                        typed_trees::data::TypeParameterKind::Value { type_reference }
                        | typed_trees::data::TypeParameterKind::Const { type_reference } => {
                            type_reference
                        }
                        _ => return None,
                    };
                    let primitive = integer_carrier(program, reference)?;
                    Some(Binding {
                        symbol: parameter.symbol,
                        name: parameter.name.to_string(),
                        value: Some(entry_fields::entry_value(
                            program,
                            reference,
                            None,
                            ScopedArithmeticValue::Atom {
                                identity: format!(
                                    "machine:{:?}:value:{:?}",
                                    machine.symbol, parameter.symbol
                                ),
                                unsigned: unsigned(primitive),
                            },
                        )),
                        alternatives: Vec::new(),
                        immutable_coordinate: true,
                        unwritable: true,
                    })
                }),
        );
        // Nested machines read the enclosing machine's immutable value and
        // const coordinates. Those symbols stay in lexical scope, so a helper
        // that decreases the enclosing rank can name them.
        let (declaration_scope, _) =
            program.machine_declaration_scope(machine, |left, right| left.symbol == right.symbol);
        for parameter in declaration_scope {
            if initial
                .iter()
                .any(|binding| binding.symbol == parameter.symbol)
            {
                continue;
            }
            let reference = match parameter.kind {
                typed_trees::data::TypeParameterKind::Value { type_reference }
                | typed_trees::data::TypeParameterKind::Const { type_reference } => type_reference,
                _ => continue,
            };
            let Some(primitive) = integer_carrier(program, reference) else {
                continue;
            };
            initial.push(Binding {
                symbol: parameter.symbol,
                name: parameter.name.to_string(),
                value: Some(entry_fields::entry_value(
                    program,
                    reference,
                    None,
                    ScopedArithmeticValue::Atom {
                        identity: format!(
                            "machine:{:?}:value:{:?}",
                            machine.symbol, parameter.symbol
                        ),
                        unsigned: unsigned(primitive),
                    },
                )),
                alternatives: Vec::new(),
                immutable_coordinate: true,
                unwritable: true,
            });
        }
        let lengths = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| {
                validation::state_reference_parameter_binding_is_stable(
                    program,
                    machine,
                    state,
                    parameter.symbol,
                )
            })
            .filter_map(|parameter| {
                Some(Length {
                    subject: LengthSubject::Storage(parameter.symbol),
                    name: parameter.name.to_string(),
                    stable: true,
                    type_reference: parameter.type_reference,
                    value: Some(entry_collection_length(
                        program,
                        parameter.type_reference,
                        || {
                            format!(
                                "state:{:?}:parameter:{:?}:length",
                                state.symbol, parameter.symbol
                            )
                        },
                    )?),
                })
            })
            .collect();
        let mut values = Self {
            program,
            machine,
            state,
            statement_index,
            current: initial.clone(),
            initial,
            lengths,
            initial_lengths: Vec::new(),
            fields: Vec::new(),
            result_fields: None,
            conditions: Vec::new(),
            hypotheses: Vec::new(),
            in_transition_target: target.is_valid(),
            scalar_observations: std::cell::OnceCell::new(),
            anchors: Vec::new(),
        };
        let owned_frames = frames
            .is_none()
            .then(|| validation::CallFrameResolver::new(program))
            .flatten();
        let frames = frames.or(owned_frames.as_ref());
        let mut field_paths = parameter_projection_paths(program, machine, state);
        for path in requested {
            if !field_paths.contains(path) {
                field_paths.push(path.clone());
            }
        }
        values.seed_parameter_fields(&field_paths, frames);
        values.initial_lengths = values.lengths.clone();
        let entry = program.machine_states(machine).first()?;
        let contracts = program.state_contracts(state).iter().chain(
            program
                .machine_contracts(machine)
                .iter()
                .filter(|_| entry.symbol == state.symbol),
        );
        for contract in contracts {
            if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
                continue;
            }
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                if let Some(expression) = fact.as_expression() {
                    values.assume(program, machine, state, *expression, true);
                }
            }
        }
        let statements = program.statement_table.statements(state.statement_nodes);
        if statement_index > statements.len() {
            return None;
        }
        // Labels can skip or reorder lexical windows. Replay the already
        // admitted path, retaining source ordinals for captured observations;
        // a textual prefix could establish facts from unexecuted stores.
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        let position = order.position(statement_index)?;
        for &ordinal in &order.indices()[..position] {
            let statement = &statements[ordinal];
            if validation::is_arm_pattern_marker(statement) {
                continue;
            }
            let before = values
                .program
                .storage_graph()
                .map(|_| values.anchored_entries());
            match statement {
                StatementNode::Transition(transition) => {
                    let TransitionGuardNode::When(guard) = transition.guard else {
                        return None;
                    };
                    values.retire_operand_effects(guard);
                    // A missing primary target preserves its true arm; a
                    // missing continuation preserves its false arm.
                    let holds = match (
                        transition.target.is_valid(),
                        transition.continuation.is_valid(),
                    ) {
                        (true, false) => false,
                        (false, true) => true,
                        _ => return None,
                    };
                    values.assume(program, machine, state, guard, holds);
                }
                // Reserved event results are not initialized mathematical
                // values. Their observed contents come from the event, not
                // from the parser's storage placeholder.
                StatementNode::LocalData(local) if !local.role.has_ordinary_initializer() => {}
                StatementNode::LocalData(local) => {
                    values.retire_operand_effects(local.initial_value);
                    let actuals = values.capture_call_actuals(local.initial_value);
                    let value = values.capture(program, machine, state, local.initial_value);
                    let conditions = values.capture_conditions(local.initial_value, 0);
                    values.replace_conditions(local.symbol, &[], conditions);
                    values.capture_collection(
                        local.symbol,
                        local.name.as_str(),
                        local.type_reference,
                        local.initial_value,
                        ordinal,
                    );
                    let scalar_fields = values.captured_scalar_fields_of_type(
                        local.initial_value,
                        local.type_reference,
                        0,
                    );
                    values.replace_scalar_fields(local.symbol, &[], scalar_fields);
                    values.capture_record_fields(local.symbol, &[], local.initial_value);
                    if integer_carrier(program, local.type_reference).is_none() {
                        values.assume_aggregate_call_ensures(
                            local.initial_value,
                            &actuals,
                            ordinal,
                        );
                    }
                    let reference_scalar =
                        validation::reference_shell(program, local.type_reference).and_then(
                            |shell| {
                                integer_carrier(program, shell.referee)
                                    .map(|primitive| (primitive, shell.access.is_readable()))
                            },
                        );
                    if let Some(primitive) = reference_scalar
                        .map(|(primitive, _)| primitive)
                        .or_else(|| integer_carrier(program, local.type_reference))
                    {
                        let value = if let Some((_, readable)) = reference_scalar {
                            let value = values.reference_scalar_initializer(
                                statement,
                                local.initial_value,
                                primitive,
                                frames,
                            );
                            readable.then_some(value).flatten()
                        } else {
                            value
                        };
                        // The executed initializer has one value even when its
                        // computation is unavailable to arithmetic reasoning.
                        // Keep that observation, not a replay of the initializer.
                        let alternatives = values.capture_alternatives(
                            program,
                            machine,
                            state,
                            local.initial_value,
                        );
                        let value = value.or_else(|| {
                            (!reference_scalar.is_some_and(|(_, readable)| !readable))
                                .then(|| observed_scalar(state, local.symbol, ordinal, primitive))
                        });
                        if let Some(result_value) = &value {
                            values.assume_call_ensures(
                                local.initial_value,
                                result_value,
                                &actuals,
                                ordinal,
                            );
                        }
                        values.current.push(Binding {
                            symbol: local.symbol,
                            name: local.name.to_string(),
                            value,
                            alternatives,
                            immutable_coordinate: false,
                            unwritable: !local.is_mutable && reference_scalar.is_none(),
                        });
                    }
                }
                StatementNode::Assignment(assignment) => {
                    values.retire_operand_effects(assignment.value);
                    let stored_at = values.statement_snapshot(ordinal);
                    let target_remains_current = frames.is_some_and(|frames| {
                        frames.assignment_destination_remains_current(machine, statement)
                    });
                    let actuals = values.capture_call_actuals(assignment.value);
                    let value = values.capture(program, machine, state, assignment.value);
                    let conditions = values.capture_conditions(assignment.value, 0);
                    let mut scalar_fields = validation::expression_result_type_reference(
                        program,
                        machine,
                        state,
                        assignment.target,
                    )
                    .map(|reference| {
                        values.captured_scalar_fields_of_type(assignment.value, reference, 0)
                    })
                    .unwrap_or_else(|| values.captured_scalar_fields(assignment.value, 0));
                    let scalar_store =
                        values.captured_scalar_store(assignment.target, value.clone(), ordinal);
                    let store_aliases = scalar_store.as_ref().map_or_else(Vec::new, |field| {
                        values.scalar_store_aliases(statement, ordinal, field.carrier, frames)
                    });
                    let stored_value = scalar_store.as_ref().map(|field| field.value.clone());
                    if let Some(field) = scalar_store
                        && values
                            .scalar_storage_path(assignment.target)
                            .is_some_and(|(_, path)| !path.is_empty())
                    {
                        scalar_fields.push(field);
                    }
                    let alternatives =
                        values.capture_alternatives(program, machine, state, assignment.value);
                    if stored_value.is_some() {
                        values.retire_unpreserved(stored_at);
                    }
                    let mut fields = values.captured_field_lengths(assignment.value, 0);
                    if let Some(value) = values.collection_length(assignment.value, 0)
                        && let Some(type_reference) = validation::expression_result_type_reference(
                            program,
                            machine,
                            state,
                            assignment.value,
                        )
                        && fixed_length(program, type_reference, String::new).is_some()
                    {
                        fields.push(collections::CapturedFieldLength {
                            path: Vec::new(),
                            name: String::new(),
                            type_reference,
                            value,
                        });
                    }
                    if target_remains_current
                        && let ExpressionNode::Name(name) =
                            program.expression_table.expression(assignment.target)
                        && name.symbol.is_valid()
                        && name.symbol == name.head_symbol
                        && values
                            .current
                            .iter()
                            .any(|binding| binding.symbol == name.symbol)
                    {
                        let next_value = value.or_else(|| stored_value.clone()).or_else(|| {
                            let reference = validation::expression_result_type_reference(
                                program,
                                machine,
                                state,
                                assignment.target,
                            )?;
                            let primitive = integer_carrier(program, reference)?;
                            Some(observed_scalar(state, name.symbol, ordinal, primitive))
                        });
                        if let Some(result_value) = &next_value {
                            values.assume_call_ensures(
                                assignment.value,
                                result_value,
                                &actuals,
                                ordinal,
                            );
                        }
                        if let Some(binding) = values
                            .current
                            .iter_mut()
                            .find(|binding| binding.symbol == name.symbol)
                        {
                            binding.value = next_value;
                            binding.alternatives = alternatives;
                        }
                    } else if target_remains_current
                        && let ExpressionNode::Name(name) =
                            program.expression_table.expression(assignment.target)
                        && name.symbol == name.head_symbol
                        && let Some(length) = values
                            .lengths
                            .iter()
                            .find(|length| length.subject == LengthSubject::Storage(name.symbol))
                            .cloned()
                    {
                        values.capture_collection(
                            name.symbol,
                            &length.name,
                            length.type_reference,
                            assignment.value,
                            ordinal,
                        );
                    } else {
                        values.retire_unpreserved(stored_at);
                    }
                    // The store uses the address captured before operand effects.
                    // A rebound selector can no longer name that home; publishing
                    // through its old spelling would manufacture a current rank.
                    if target_remains_current
                        && let Some((owner, path)) = values.scalar_storage_path(assignment.target)
                    {
                        values.replace_scalar_fields(owner, &path, scalar_fields);
                        if stored_value.is_none() {
                            values.assume_aggregate_call_ensures(
                                assignment.value,
                                &actuals,
                                ordinal,
                            );
                        }
                    }
                    if target_remains_current
                        && let Some((owner, path)) = field_storage_path(program, assignment.target)
                    {
                        values.replace_conditions(owner, &path, conditions);
                        values.replace_record_fields(owner, &path, fields);
                    }
                    if let Some(value) = stored_value {
                        values.publish_scalar_store(&store_aliases, &value);
                    }
                }
                StatementNode::Call(call) => {
                    let at = values.statement_snapshot(ordinal);
                    values.retire_unpreserved(at);
                    values.assume_statement_ensures(call, ordinal);
                }
                StatementNode::Expression(_) => {
                    let at = values.statement_snapshot(ordinal);
                    values.retire_unpreserved(at)
                }
                // Control metadata has no store; instruction statements above
                // retain their ordinary effects. Assertions are obligations,
                // never premises introduced by this value transfer.
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {}
            }
            if let Some(before) = before {
                values.anchor_changed_entries(&before, ordinal);
            }
        }
        if target.is_valid() {
            let transition = statements.get(statement_index)?.as_transition()?;
            if transition.target != target && transition.continuation != target {
                return None;
            }
            if let TransitionGuardNode::When(guard) = transition.guard {
                values.retire_operand_effects(guard);
                values.assume(program, machine, state, guard, transition.target == target);
            }
        }
        if target.is_valid()
            && let typed_trees::statement::TransitionTargetNode::Named { arguments, .. } =
                program.statement_table.transition_target(target)
        {
            for argument in program.statement_table.expression_handles(*arguments) {
                values.retire_operand_effects(*argument);
            }
        }
        Some(values)
    }

    /// Retire the entries the evaluation of `expression` may have written. An
    /// operand flow recorded no position for runs no call or atomic access, so
    /// its evaluation wrote nothing.
    pub(crate) fn retire_operand_effects(&mut self, expression: ExpressionHandle) {
        if let Some(at) = self.operand_snapshot(expression) {
            self.retire_unpreserved(Some(at));
        } else if validation::executable_subtree_any(
            self.program,
            expression,
            typed_trees::SubtreeEdges::VALUES,
            |_, node| match node {
                ExpressionNode::Call(_) | ExpressionNode::Atomic(_) => {
                    typed_trees::SubtreeVisit::Found
                }
                _ => typed_trees::SubtreeVisit::Descend,
            },
        ) {
            self.retire_unpreserved(None);
        }
    }

    fn operand_snapshot(
        &self,
        expression: ExpressionHandle,
    ) -> Option<validation::completed_storage::versions::Snapshot> {
        validation::completed_storage::versions::operand_snapshot(
            self.program.storage_graph()?,
            self.state.symbol,
            expression,
        )
    }

    fn statement_snapshot(
        &self,
        ordinal: usize,
    ) -> Option<validation::completed_storage::versions::Snapshot> {
        validation::completed_storage::versions::statement_snapshot(
            self.program,
            self.program.storage_graph()?,
            self.state.symbol,
            ordinal,
        )
    }

    /// Each tracked entry with a graph place, with the value it holds.
    fn anchored_entries(&self) -> Vec<(validation::CanonicalPlace, EntryValue)> {
        let place =
            |root: SymbolHandle, segments: Vec<facts::PlaceSegment>| validation::CanonicalPlace {
                root: facts::PlaceRoot::Symbol(root),
                segments,
            };
        let field_path = |path: &[SymbolHandle]| {
            path.iter()
                .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                .collect::<Vec<_>>()
        };
        let mut entries = Vec::new();
        for binding in &self.current {
            entries.push((
                place(binding.symbol, Vec::new()),
                EntryValue::Binding(binding.value.clone(), binding.alternatives.clone()),
            ));
        }
        for field in &self.fields {
            entries.push((
                place(field.owner, scalar_fields::source_path(&field.path)),
                EntryValue::Value(field.value.clone()),
            ));
        }
        for condition in &self.conditions {
            entries.push((
                place(condition.owner, field_path(&condition.path)),
                EntryValue::Condition(
                    condition.hypotheses(true).to_vec(),
                    condition.hypotheses(false).to_vec(),
                ),
            ));
        }
        for length in &self.lengths {
            let Some(length_place) = length_place(&length.subject) else {
                continue;
            };
            entries.push((length_place, EntryValue::Value(length.value.clone())));
        }
        entries
    }

    /// Anchor every entry this statement established or changed at its snapshot.
    fn anchor_changed_entries(
        &mut self,
        before: &[(validation::CanonicalPlace, EntryValue)],
        ordinal: usize,
    ) {
        let Some(snapshot) = self.statement_snapshot(ordinal) else {
            return;
        };
        for (place, value) in self.anchored_entries() {
            if before
                .iter()
                .any(|(earlier, earlier_value)| *earlier == place && *earlier_value == value)
            {
                continue;
            }
            self.anchor_at(place, snapshot);
        }
    }

    fn anchor_at(
        &mut self,
        place: validation::CanonicalPlace,
        snapshot: validation::completed_storage::versions::Snapshot,
    ) {
        match self
            .anchors
            .iter_mut()
            .find(|(anchored, _)| *anchored == place)
        {
            Some((_, anchor)) => *anchor = snapshot,
            None => self.anchors.push((place, snapshot)),
        }
    }

    fn anchor_of(
        &self,
        place: &validation::CanonicalPlace,
    ) -> validation::completed_storage::versions::Snapshot {
        self.anchors
            .iter()
            .find(|(anchored, _)| anchored == place)
            .map_or_else(arena::Handle::invalid, |(_, anchor)| *anchor)
    }

    /// Retire, at version `at`, every tracked entry whose place a write
    /// reached since its anchor. Machine Value/Const coordinates belong to this
    /// invocation, not addressable storage, and a stable descriptor's length
    /// survives endpoint stores. Without a graph position the effect is
    /// unknown: every other entry retires.
    ///
    /// A replay calls this at each write of the state prefix, in order. An
    /// entry that survives at `at` is re-anchored there, so the next call
    /// walks only the writes after `at` instead of the whole prefix again;
    /// a std program spent seconds repeating those walks. When `at` lies on
    /// every path to the next position, the shorter walk gives the same
    /// answer. When it does not, the walk runs past `at` to the state's
    /// start and the entry retires: a less precise answer, never an unsound
    /// one. An entry that already holds no value has nothing to retire and
    /// skips its walk.
    pub(crate) fn retire_unpreserved(
        &mut self,
        at: Option<validation::completed_storage::versions::Snapshot>,
    ) {
        let graph = self.program.storage_graph();
        let mut survived = Vec::new();
        let mut preserved = |place: validation::CanonicalPlace| match (at, graph) {
            (Some(at), Some(graph)) => {
                let kept = validation::completed_storage::versions::place_preserved_since(
                    self.program,
                    graph,
                    at,
                    &place,
                    self.anchor_of(&place),
                );
                if kept {
                    survived.push(place);
                }
                kept
            }
            _ => false,
        };
        let bindings = self
            .current
            .iter()
            .map(|binding| {
                binding.immutable_coordinate
                    || binding.unwritable
                    || (binding.value.is_none() && binding.alternatives.is_empty())
                    || preserved(validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(binding.symbol),
                        segments: Vec::new(),
                    })
            })
            .collect::<Vec<_>>();
        let fields = self
            .fields
            .iter()
            .map(|field| {
                field.value.is_none()
                    || preserved(validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(field.owner),
                        segments: scalar_fields::source_path(&field.path),
                    })
            })
            .collect::<Vec<_>>();
        let conditions = self
            .conditions
            .iter()
            .map(|condition| {
                preserved(validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(condition.owner),
                    segments: condition
                        .path
                        .iter()
                        .map(|symbol| facts::PlaceSegment::Field { symbol: *symbol })
                        .collect(),
                })
            })
            .collect::<Vec<_>>();
        let lengths = self
            .lengths
            .iter()
            .map(|length| {
                length.stable
                    || length.value.is_none()
                    || length_place(&length.subject).map_or(at.is_some(), &mut preserved)
            })
            .collect::<Vec<_>>();
        if let Some(at) = at {
            for place in survived {
                self.anchor_at(place, at);
            }
        }
        for (binding, preserved) in self.current.iter_mut().zip(bindings) {
            if !preserved {
                binding.value = None;
                binding.alternatives.clear();
            }
        }
        for (field, preserved) in self.fields.iter_mut().zip(fields) {
            if !preserved {
                field.value = None;
            }
        }
        let mut conditions = conditions.into_iter();
        self.conditions
            .retain(|_| conditions.next().unwrap_or(false));
        for (length, preserved) in self.lengths.iter_mut().zip(lengths) {
            if !preserved {
                length.value = None;
            }
        }
    }

    fn assume(
        &mut self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        holds: bool,
    ) {
        let hypotheses = self.capture_condition(program, machine, state, expression, holds, 0);
        self.hypotheses.extend(hypotheses);
    }

    fn capture_condition(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        holds: bool,
        depth: usize,
    ) -> Vec<ScopedArithmeticHypothesis> {
        if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
            return Vec::new();
        }
        if let Some((owner, path)) = field_storage_path(program, expression)
            && let Some(condition) = self
                .conditions
                .iter()
                .find(|condition| condition.owner == owner && condition.path == path)
        {
            return condition.hypotheses(holds).to_vec();
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Equal | BinaryOperator::NotEqual
            )
            && validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            let wrapped = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (_, ExpressionNode::Boolean(value)) => Some((binary.left, *value)),
                (ExpressionNode::Boolean(value), _) => Some((binary.right, *value)),
                _ => None,
            };
            if let Some((operand, value)) = wrapped {
                return self.capture_condition(
                    program,
                    machine,
                    state,
                    operand,
                    holds == (value == (binary.operator == BinaryOperator::Equal)),
                    depth + 1,
                );
            }
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
            && ((holds && binary.operator == BinaryOperator::And)
                || (!holds && binary.operator == BinaryOperator::Or))
            && validation::has_builtin_decomposed_guard_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            let mut hypotheses =
                self.capture_condition(program, machine, state, binary.left, holds, depth + 1);
            // Repeated copies of one Boolean do not create new evidence.
            // Deduplicate here so `saved && saved` chains cannot double the
            // retained predicate roster at every binding.
            for hypothesis in
                self.capture_condition(program, machine, state, binary.right, holds, depth + 1)
            {
                crate::shared::collections::push_unique(&mut hypotheses, hypothesis);
            }
            return hypotheses;
        }
        if let ExpressionNode::Unary(unary) = program.expression_table.expression(expression)
            && unary.operator == typed_trees::expression::UnaryOperator::LogicalNot
            && validation::has_builtin_bound_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        {
            return self.capture_condition(
                program,
                machine,
                state,
                unary.operand,
                !holds,
                depth + 1,
            );
        }
        let occurrence = self.occurrence_at(program, machine, state, &self.lengths);
        if admitted(occurrence, expression, &self.current, &self.fields, 0)
            && let Some(proposition) = scoped(occurrence, expression, &self.current, &self.fields)
        {
            return vec![ScopedArithmeticHypothesis { proposition, holds }];
        }
        Vec::new()
    }

    fn capture(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        self.capture_at_depth(program, machine, state, expression, 0)
    }

    /// Capture an executed primitive's normal value, not a specification term.
    /// Exact and Trapping addition/subtraction/multiplication have their
    /// mathematical result on this continuation; a trap or an unproved exact
    /// overflow never reaches the use. Selection and the result carrier are
    /// checked independently. A completed unsigned remainder records its normal
    /// denotation on the operation itself, irrespective of policy. Other
    /// Wrapping and Saturating operations retain the engine's policy checks.
    fn capture_at_depth(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<ScopedArithmeticValue> {
        if depth >= 64 {
            return None;
        }
        // A completed scalar copy retains the supplying value, including
        // compiler-introduced selector locals. Wrapping it in a new scoped
        // Name term would give two reads of one snapshot different identities.
        if let ExpressionNode::Name(name) = program.expression_table.expression(expression)
            && name.symbol == name.head_symbol
            && let Some(binding) = self
                .current
                .iter()
                .find(|binding| binding.symbol == name.symbol)
        {
            return binding.value.clone();
        }
        if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression)
            && let Some(value) = literal.value_i64()
        {
            return Some(ScopedArithmeticValue::Integer(
                numerics::bignum::BigInt::from_i64(value),
            ));
        }
        // A completed call retains its exact scalar actuals as well as its
        // result interval. An interval alone loses relational guarantees such
        // as result == input + 1. Capture only immutable owned integer formals:
        // a reference denotes storage, and a mutable formal's post-state is not
        // its incoming value. The existing contract importer owns those other
        // observations, selected identity and contract independence.
        // This is an executed value capture after the operand-write barrier,
        // not admission or formation of an unevaluated specification call.
        if let Some(call) = program.expression_table.call_node(expression)
            && let Some((low, high)) =
                validation::normal_return_integer_bounds(program, machine.symbol, expression, call)
        {
            if let (Some(low), Some(high)) = (low, high)
                && low == high
            {
                return Some(ScopedArithmeticValue::Integer(
                    numerics::bignum::BigInt::from_i64(low),
                ));
            }
            let reference =
                validation::expression_result_type_reference(program, machine, state, expression)?;
            let primitive = integer_carrier(program, reference)?;
            let arguments = program.expression_table.expression_handles(call.arguments);
            let actuals = validation::call_target_parameters(
                program,
                validation::call_target_symbol(program, call),
            )
            .filter(|parameters| parameters.len() == arguments.len())
            .map(|parameters| {
                parameters
                    .iter()
                    .zip(arguments)
                    .filter(|(parameter, _)| {
                        !parameter.is_mutable
                            && validation::reference_shell(program, parameter.type_reference)
                                .is_none()
                            && program
                                .primitive_type_reference(parameter.type_reference)
                                .is_some_and(PrimitiveType::is_fixed_integer)
                    })
                    .filter_map(|(_, argument)| {
                        Some(ScopedArithmeticBinding {
                            binder: ScopedArithmeticBinder::Occurrence(*argument),
                            value: self.capture_at_depth(
                                program,
                                machine,
                                state,
                                *argument,
                                depth + 1,
                            )?,
                        })
                    })
                    .collect()
            });
            let value = actuals.map_or_else(
                || ScopedArithmeticValue::Atom {
                    identity: format!("state:{:?}:call-result:{expression:?}", state.symbol),
                    unsigned: unsigned(primitive),
                },
                |bindings| {
                    ScopedArithmeticValue::Term(ScopedArithmeticExpression {
                        owner: state.symbol,
                        expression,
                        bindings,
                    })
                },
            );
            return Some(ScopedArithmeticValue::BoundedInteger {
                value: Box::new(value),
                interval: BigIntInterval {
                    low: low.map(numerics::bignum::BigInt::from_i64),
                    high: high.map(numerics::bignum::BigInt::from_i64),
                },
            });
        }
        if matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
        ) {
            if let Some(value) = length_occurrence(program, expression, &self.lengths)
                .and_then(|length| length.value.clone())
            {
                return Some(value);
            }
            if let Some(value) =
                scalar_field_occurrence(self.occurrence(), expression, &self.current, &self.fields)
            {
                return Some(value);
            }
            if let Some(value) = field_occurrence(program, machine, state, expression) {
                return Some(value);
            }
            if let Some(observation) = self
                .scalar_observations
                .get_or_init(|| {
                    validation::captured_numeric_observations(
                        program,
                        machine,
                        state,
                        self.statement_index,
                        self.in_transition_target,
                        validation::NumericObservationDemand {
                            scalar_values: validation::ScalarObservationDemand::StatePrefix,
                            integer_operations: false,
                            scalar_associations: false,
                        },
                    )
                    .scalar_values
                })
                .iter()
                .find(|observation| observation.expression == expression)
            {
                return Some(observation.value.clone());
            }
        }
        if let ExpressionNode::Cast(_) = program.expression_table.expression(expression) {
            // An admitted value-preserving cast transports the already
            // captured operand, including its operation evidence. It does not
            // replay a local initializer or infer meaning from a cast shape.
            let operand = preserving_integer_cast_operand(
                self.occurrence(),
                expression,
                &self.current,
                &self.fields,
            )?;
            return self.capture_at_depth(program, machine, state, operand, depth + 1);
        }
        if let Some(binary) = program.expression_table.binary_node(expression)
            && matches!(
                binary.operator,
                BinaryOperator::Add | BinaryOperator::Subtract | BinaryOperator::Multiply
            )
            && validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
            && let Some(reference) =
                validation::expression_result_type_reference(program, machine, state, expression)
            && integer_carrier(program, reference).is_some()
            && matches!(
                program.arithmetic_domain_for_type_reference(reference),
                ArithmeticDomain::Exact | ArithmeticDomain::Trapping
            )
        {
            return Some(ScopedArithmeticValue::IntegerOperation {
                operator: binary.operator,
                left: Box::new(self.capture_at_depth(
                    program,
                    machine,
                    state,
                    binary.left,
                    depth + 1,
                )?),
                right: Box::new(self.capture_at_depth(
                    program,
                    machine,
                    state,
                    binary.right,
                    depth + 1,
                )?),
            });
        }
        if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression)
            && let Some(value) =
                self.capture_join(program, machine, state, expression, dispatch, depth)
        {
            return Some(value);
        }
        // Resolve the captured term under this occurrence's exact callable;
        // admission alone does not supply the completed scalar value.
        let occurrence = self.occurrence();
        admitted(occurrence, expression, &self.current, &self.fields, 0)
            .then(|| scoped(occurrence, expression, &self.current, &self.fields))
            .flatten()
            .map(|term| {
                use crate::checks::contracts::prover::call_guarantees::{
                    arithmetic, callable::Callable,
                };
                arithmetic::completed_scalar_value(
                    occurrence.program,
                    &Callable::Machine {
                        machine: occurrence.machine,
                        state: occurrence.state,
                    },
                    term,
                )
            })
    }

    /// Exact value of a runtime index at this occurrence. Its declaration
    /// symbol remains storage identity; the returned roster value captures
    /// assignments and unknown write effects at the selected program point.
    pub(crate) fn current_symbol(&self, symbol: SymbolHandle) -> Option<ScopedArithmeticValue> {
        self.current
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .clone()
    }

    /// Value of a completed operand at this frame, retaining admitted
    /// operation evidence as well as captured stores and field values.
    /// Formation and ordered operand effects remain caller obligations.
    pub(crate) fn current_value(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticValue> {
        self.capture(self.program, self.machine, self.state, expression)
    }

    pub(crate) fn initial_symbol(&self, symbol: SymbolHandle) -> Option<ScopedArithmeticValue> {
        self.initial
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .clone()
    }
    /// Read an authored expression under the captured arrival roster. Later
    /// scalar replacement cannot reinterpret this input as current storage.
    pub(crate) fn initial(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticExpression> {
        let occurrence = self.occurrence();
        admitted(occurrence, expression, &self.initial, &[], 0)
            .then(|| scoped(occurrence, expression, &self.initial, &[]))
            .flatten()
    }
    pub(crate) fn current(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticExpression> {
        let occurrence = self.occurrence();
        admitted(occurrence, expression, &self.current, &self.fields, 0)
            .then(|| scoped(occurrence, expression, &self.current, &self.fields))
            .flatten()
    }
    fn capture_alternatives(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        state: &State,
        expression: ExpressionHandle,
    ) -> Vec<ScopedArithmeticValue> {
        let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression)
        else {
            return Vec::new();
        };
        let arms = program.expression_table.match_arms(dispatch.arms);
        if arms.is_empty() || arms.len() > 8 {
            return Vec::new();
        }
        let mut values = Vec::with_capacity(arms.len());
        for arm in arms {
            let Some(value) = self.capture(program, machine, state, arm.value) else {
                return Vec::new();
            };
            values.push(value);
        }
        values
    }

    /// Facts already captured in their establishing rosters. Consumers must
    /// preserve those rosters rather than reread the source predicates.
    pub(crate) fn captured_hypotheses(&self) -> &[ScopedArithmeticHypothesis] {
        &self.hypotheses
    }

    /// Bind a declaration recipe to current snapshots without treating its
    /// unevaluated calls as completed runtime occurrences. The formation owner
    /// must still discharge every operation before using the returned roster.
    pub(crate) fn integer_expression(
        &self,
        expression: ExpressionHandle,
    ) -> Option<ScopedArithmeticExpression> {
        scoped_operations(
            self.occurrence(),
            expression,
            &self.current,
            &self.fields,
            true,
        )
    }

    pub(crate) fn integer_expression_formation(
        &self,
        machine: &Machine,
        expression: &ScopedArithmeticExpression,
    ) -> Option<(
        validation::IntegerApplicationFormation,
        ScopedArithmeticValue,
    )> {
        validation::scoped_integer_expression_formation(
            self.program,
            machine,
            &self.hypotheses,
            expression,
        )
    }

    pub(crate) fn declared_integer_application_formation(
        &self,
        machine: &Machine,
        expression: &ScopedArithmeticExpression,
        input_type: typed_trees::types::TypeReferenceHandle,
        result: PrimitiveType,
    ) -> Option<(
        validation::IntegerApplicationFormation,
        ScopedArithmeticValue,
    )> {
        validation::scoped_declared_integer_application_formation(
            self.program,
            machine,
            &self.hypotheses,
            expression,
            input_type,
            result,
        )
    }

    pub(crate) fn proves_values_relation(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        operator: BinaryOperator,
        left: &ScopedArithmeticValue,
        right: &ScopedArithmeticValue,
    ) -> bool {
        validation::scoped_arithmetic_values_relation(
            program,
            machine,
            &self.hypotheses,
            operator,
            left,
            right,
        ) == StrictArithmeticImplicationJudgment::Proven
    }

    pub(crate) fn proves(
        &self,
        program: &dyn ::validation::CheckingInput,
        machine: &Machine,
        goal: &ScopedArithmeticExpression,
    ) -> bool {
        if validation::scoped_arithmetic_implication(program, machine, &self.hypotheses, goal)
            == StrictArithmeticImplicationJudgment::Proven
        {
            return true;
        }
        for current in &self.current {
            let Some(expected) = &current.value else {
                continue;
            };
            if current.alternatives.is_empty() {
                continue;
            }
            if substitute_symbol_in_expression(
                goal,
                current.symbol,
                expected,
                &current.alternatives[0],
            )
            .is_none()
            {
                continue;
            }
            if current.alternatives.iter().all(|alternative| {
                substitute_symbol_in_expression(goal, current.symbol, expected, alternative)
                    .is_some_and(|specialized| {
                        validation::scoped_arithmetic_implication(
                            program,
                            machine,
                            &self.hypotheses,
                            &specialized,
                        ) == StrictArithmeticImplicationJudgment::Proven
                    })
            }) {
                return true;
            }
        }
        false
    }
}

fn substitute_symbol_in_expression(
    expression: &ScopedArithmeticExpression,
    symbol: SymbolHandle,
    expected: &ScopedArithmeticValue,
    replacement: &ScopedArithmeticValue,
) -> Option<ScopedArithmeticExpression> {
    let mut updated = expression.clone();
    let mut changed = false;
    for binding in &mut updated.bindings {
        if binding.binder == ScopedArithmeticBinder::Symbol(symbol) && &binding.value == expected {
            binding.value = replacement.clone();
            changed = true;
        } else if let Some(next) =
            substitute_symbol_in_value(&binding.value, symbol, expected, replacement)
        {
            binding.value = next;
            changed = true;
        }
    }
    changed.then_some(updated)
}

fn substitute_symbol_in_value(
    value: &ScopedArithmeticValue,
    symbol: SymbolHandle,
    expected: &ScopedArithmeticValue,
    replacement: &ScopedArithmeticValue,
) -> Option<ScopedArithmeticValue> {
    match value {
        ScopedArithmeticValue::Term(term) => {
            substitute_symbol_in_expression(term, symbol, expected, replacement)
                .map(ScopedArithmeticValue::Term)
        }
        ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
            substitute_symbol_in_expression(term, symbol, expected, replacement)
                .map(ScopedArithmeticValue::AdmittedIntegerRemainder)
        }
        ScopedArithmeticValue::BoundedInteger {
            value: inner,
            interval,
        } => substitute_symbol_in_value(inner, symbol, expected, replacement).map(|next| {
            ScopedArithmeticValue::BoundedInteger {
                value: Box::new(next),
                interval: interval.clone(),
            }
        }),
        ScopedArithmeticValue::IntegerOperation {
            operator,
            left,
            right,
        } => {
            let next_left = substitute_symbol_in_value(left, symbol, expected, replacement);
            let next_right = substitute_symbol_in_value(right, symbol, expected, replacement);
            if next_left.is_none() && next_right.is_none() {
                return None;
            }
            Some(ScopedArithmeticValue::IntegerOperation {
                operator: *operator,
                left: Box::new(next_left.unwrap_or_else(|| (**left).clone())),
                right: Box::new(next_right.unwrap_or_else(|| (**right).clone())),
            })
        }
        _ => None,
    }
}

fn integer_carrier(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let primitive = program.primitive_type_reference(reference)?;
    // A stored payload has an integer denotation under every arithmetic policy.
    // This admits observation, not arithmetic with an erased policy.
    if !primitive.is_fixed_integer() {
        return None;
    }
    Some(primitive)
}
/// A collection type's length when no store or reseat can change it during
/// the activation: a literal fixed array's own length, or one unsigned unknown.
/// Entry metadata includes inline fixed arrays as well as borrowed descriptors.
/// Only the descriptor owner below may create a live length atom; a declared
/// extent supplies its exact constant without treating owned storage as a loan.
fn entry_collection_length(
    program: &dyn validation::CheckingInput,
    reference: TypeReferenceHandle,
    identity: impl FnOnce() -> String,
) -> Option<ScopedArithmeticValue> {
    if let Some(extent) = validation::fixed_array_type_length(program, reference) {
        return Some(ScopedArithmeticValue::Integer(
            numerics::bignum::BigInt::from_u64(u64::try_from(extent).ok()?),
        ));
    }
    fixed_length(program, reference, identity)
}

/// A descriptor length is read as a `u64` count, so a private length atom lies
/// in the `u64` interval whenever it is transported into a scoped expression.
/// Without that interval an exact `items.len - 1` or `items.len + 1` in a
/// declaration (such as a rank endpoint) could never prove it fits its carrier,
/// even under a `requires items.len > 0` hypothesis. A known length is already
/// an exact integer and needs no interval.
fn carrier_bounded_length(length: ScopedArithmeticValue) -> ScopedArithmeticValue {
    let ScopedArithmeticValue::Atom { .. } = length else {
        return length;
    };
    ScopedArithmeticValue::BoundedInteger {
        value: Box::new(length),
        interval: validation::BigIntInterval {
            low: Some(numerics::bignum::BigInt::zero()),
            high: Some(numerics::bignum::BigInt::from_u64(u64::MAX)),
        },
    }
}

fn fixed_length(
    program: &dyn ::validation::CheckingInput,
    reference: TypeReferenceHandle,
    identity: impl FnOnce() -> String,
) -> Option<ScopedArithmeticValue> {
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = program.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    let shared = *access == language_core::ReferenceAccess::Shared;
    let mut referee = *referee;
    // Fixed-array geometry is independent of qualification and loan access.
    // Dynamic slice observations retain their existing shared-qualified gate.
    let mut qualified = false;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(referee)
    {
        qualified = true;
        referee = *base_type;
    }
    match program.type_reference_table.type_reference(referee) {
        TypeReferenceNode::FixedArray {
            length: FixedArrayLength::Literal(length),
            ..
        } => Some(ScopedArithmeticValue::Integer(
            numerics::bignum::BigInt::from_u64(u64::try_from(*length).ok()?),
        )),
        TypeReferenceNode::Slice { .. } if qualified && !shared => None,
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => {
            Some(ScopedArithmeticValue::Atom {
                identity: identity(),
                unsigned: true,
            })
        }
        _ => None,
    }
}

/// Exact length of `collection[start..end].len` when both bounds are closed
/// integer literals. A fixed array's own `.len` stays on the declared extent;
/// this is the same `end - start` a plain window comparison already uses.
fn literal_window_length(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    let member = program.expression_table.member_node(expression)?;
    if member.case_variant.is_some() || member.member_symbol.is_valid() {
        return None;
    }
    let mut receiver = member.receiver;
    while let Some(borrow) = program.expression_table.borrow_expression(receiver) {
        receiver = borrow.target;
    }
    let indexed = program.expression_table.indexed_node(receiver)?;
    if !program.expression_table.indexed_on_range(indexed) {
        return None;
    }
    if validation::collection_operations::collection_measure_member(
        program,
        member,
        validation::collection_operations::MeasureReceiver::Window,
    ) != Some(language_semantics::declaration_selection::CollectionMeasure::Length)
    {
        return None;
    }
    let range = program.expression_table.range_node(indexed.index)?;
    let start = if range.start.is_valid() {
        closed_integer_literal(program, range.start)?
    } else {
        numerics::bignum::BigInt::zero()
    };
    if !range.end.is_valid() {
        return None;
    }
    let mut end = closed_integer_literal(program, range.end)?;
    if range.end_inclusive {
        end = end.add(&numerics::bignum::BigInt::from_u64(1));
    }
    if start > end {
        return None;
    }
    Some(ScopedArithmeticValue::Integer(end.sub(&start)))
}

fn closed_integer_literal(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<numerics::bignum::BigInt> {
    expressions(program)
        .integer_node(expression)?
        .value_bignum()
}

/// Static declaration fields retain their exact storage root and complete path.
fn field_storage_path(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Vec<SymbolHandle>)> {
    let place = validation::canonical_place_from_expression(program, expression)?;
    let owner = place.root.as_symbol()?;
    Some((owner, declaration_field_path(&place.segments)?))
}

fn declaration_field_path(segments: &[facts::PlaceSegment]) -> Option<Vec<SymbolHandle>> {
    segments
        .iter()
        .map(|segment| match segment {
            facts::PlaceSegment::Field { symbol } if symbol.is_valid() => Some(*symbol),
            _ => None,
        })
        .collect()
}

/// The fixed length a `collection.len` occurrence reads, if it reads one.
fn length_occurrence<'lengths>(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    lengths: &'lengths [Length],
) -> Option<&'lengths Length> {
    let member = program.expression_table.member_node(expression)?;
    if let Some(length) = lengths.iter().find(|length| {
        if length.value.is_none() {
            return false;
        }
        match &length.subject {
            LengthSubject::Field { owner, path } => field_storage_path(program, member.receiver)
                .is_some_and(|(actual_owner, actual_path)| {
                    actual_owner == *owner && actual_path == *path
                }),
            LengthSubject::ResultField { owner, path } => {
                validation::reserved_result_place(program, member.receiver).is_some_and(|place| {
                    place.machine_symbol == *owner
                        && declaration_field_path(&place.segments)
                            .is_some_and(|actual| actual == *path)
                })
            }
            _ => false,
        }
    }) {
        return (validation::collection_operations::collection_measure_member(
            program,
            member,
            validation::collection_operations::MeasureReceiver::Declared(length.type_reference),
        ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length))
        .then_some(length);
    }
    let subject =
        if let Some((owner, _)) = validation::reserved_result_owner(program, member.receiver) {
            LengthSubject::Result(owner)
        } else {
            let name = program.expression_table.name_path_node(member.receiver)?;
            if !program.expression_table.name_path_is_single_member(name) {
                return None;
            }
            LengthSubject::Storage(name.symbol)
        };
    let length = lengths
        .iter()
        .find(|length| length.subject == subject && length.value.is_some())?;
    (validation::collection_operations::collection_measure_member(
        program,
        member,
        validation::collection_operations::MeasureReceiver::Declared(length.type_reference),
    ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length))
    .then_some(length)
}

/// Frozen integer fields retain canonical root and complete declaration path.
/// Only stable shared parameter loans participate; owned or exclusive storage
/// needs an effect-sensitive value transfer before it can retain entry atoms.
fn frozen_field(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(TypeReferenceHandle, String)> {
    let parameters = program.state_parameters(state);
    let (index, path, selected_type) =
        structural_fields::structural_parameter_place(program, parameters, expression)?;
    if path.is_empty()
        || path.iter().any(|segment| {
            !matches!(
                segment,
                checked_trees::CheckedStructuralPredicatePathSegment::Field(_)
            )
        })
    {
        return None;
    }
    let parameter = parameters.get(usize::try_from(index).ok()?)?;
    let shell = validation::reference_shell(program, parameter.type_reference)?;
    if shell.access != language_core::ReferenceAccess::Shared
        || parameter.is_mutable
        || !validation::state_reference_parameter_binding_is_stable(
            program,
            machine,
            state,
            parameter.symbol,
        )
    {
        return None;
    }
    let (_, segments, _, frozen) =
        structural_fields::resolve_structural_parameter_path(program, parameters, index, &path)?;
    if !frozen {
        return None;
    }
    Some((
        selected_type,
        format!(
            "state:{:?}:parameter:{:?}:field:{segments:?}",
            state.symbol, parameter.symbol
        ),
    ))
}

fn field_collection_length(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    let (reference, identity) = frozen_field(program, machine, state, expression)?;
    fixed_length(program, reference, || format!("{identity}:length"))
}

fn field_occurrence(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ScopedArithmeticValue> {
    if let Some(member) = program.expression_table.member_node(expression)
        && let Some((reference, _)) = frozen_field(program, machine, state, member.receiver)
        && validation::collection_operations::collection_measure_member(
            program,
            member,
            validation::collection_operations::MeasureReceiver::Declared(reference),
        ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length)
    {
        return field_collection_length(program, machine, state, member.receiver);
    }
    let (reference, identity) = frozen_field(program, machine, state, expression)?;
    let primitive = integer_carrier(program, reference)?;
    let value = ScopedArithmeticValue::Atom {
        identity,
        unsigned: unsigned(primitive),
    };
    // Standing declaration bounds constrain this same frozen field identity.
    // They neither replay an initializer nor import a caller's changing facts.
    Some(
        match validation::immutable_integer_expression_bounds(program, machine, state, expression) {
            Some((minimum, maximum)) => ScopedArithmeticValue::BoundedInteger {
                value: Box::new(value),
                interval: BigIntInterval {
                    low: Some(numerics::bignum::BigInt::from_i64(minimum)),
                    high: Some(numerics::bignum::BigInt::from_i64(maximum)),
                },
            },
            None => value,
        },
    )
}

/// Identity belongs to this executed store on the current state activation.
/// Distinct stores never share an unknown merely because they have the same
/// destination or initializer; subsequent effects still retire the binding.
fn observed_scalar(
    state: &State,
    symbol: SymbolHandle,
    statement: usize,
    primitive: PrimitiveType,
) -> ScopedArithmeticValue {
    ScopedArithmeticValue::Atom {
        identity: format!(
            "state:{:?}:store:{statement}:scalar:{symbol:?}",
            state.symbol
        ),
        unsigned: unsigned(primitive),
    }
}

/// Operand of an exact or trapping fixed-integer value cast. This is formation
/// only: wrapping and saturating casts are different values, and representability
/// is decided separately from the live hypotheses.
fn integer_cast_operand(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let ExpressionNode::Cast(cast) = program.expression_table.expression(expression) else {
        return None;
    };
    if cast.form.is_recast() || !cast.semantic_domain.is_empty() {
        return None;
    }
    let target_type = program.primitive_type_reference(cast.target_type)?;
    // Anonymous integer literals have no fixed source carrier. The conversion
    // keeps their denotation only when the exact literal fits the target; the
    // preserving owner below checks that value, never an inferred endpoint.
    if target_type.is_fixed_integer()
        && matches!(
            program.expression_table.expression(cast.value),
            ExpressionNode::Integer(_)
        )
    {
        return Some(cast.value);
    }
    let operand_type =
        validation::expression_result_type_reference(program, machine, state, cast.value)
            .and_then(|reference| program.primitive_type_reference(reference))?;
    (operand_type.is_fixed_integer()
        && target_type.is_fixed_integer()
        && (matches!(
            cast.domain,
            ArithmeticDomain::Exact | ArithmeticDomain::Trapping
        ) || operand_type == target_type))
        .then_some(cast.value)
}

/// The occurrence a scoped walk reads: the program point plus the roster it
/// admits against. A callee's entry and a matched alternative's state are
/// occurrences too, so the pair moves while the rest stays the caller's.
#[derive(Clone, Copy)]
struct ScopedOccurrence<'roster, 'tree> {
    program: &'tree dyn ::validation::CheckingInput,
    machine: &'tree Machine,
    state: &'tree State,
    lengths: &'roster [Length],
    hypotheses: &'roster [ScopedArithmeticHypothesis],
    result_fields: Option<&'roster CapturedResultFields>,
}

impl<'program> CurrentScalarValues<'program> {
    fn occurrence(&self) -> ScopedOccurrence<'_, 'program> {
        ScopedOccurrence {
            program: self.program,
            machine: self.machine,
            state: self.state,
            lengths: &self.lengths,
            hypotheses: &self.hypotheses,
            result_fields: None,
        }
    }

    /// This occurrence read through an explicitly supplied program point: a
    /// callee's entry or a selected alternative's state, against the roster
    /// that scope re-derived. The caller's hypotheses and statement still apply.
    fn occurrence_at<'a>(
        &'a self,
        program: &'a dyn ::validation::CheckingInput,
        machine: &'a Machine,
        state: &'a State,
        lengths: &'a [Length],
    ) -> ScopedOccurrence<'a, 'a> {
        ScopedOccurrence {
            program,
            machine,
            state,
            lengths,
            hypotheses: &self.hypotheses,
            result_fields: None,
        }
    }
}

/// Operand of an exact or trapping integer cast that denotes that operand.
/// A total carrier inclusion always does. A partial conversion does only when
/// the same hypotheses prove the operand fits in the target.
fn preserving_integer_cast_operand(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
) -> Option<ExpressionHandle> {
    let ScopedOccurrence {
        program,
        machine,
        state,
        hypotheses,
        ..
    } = occurrence;
    let operand = integer_cast_operand(program, machine, state, expression)?;
    let (target_minimum, target_maximum) =
        integer_carrier_span(program, machine, state, expression)?;
    if let Some(value) = expressions(program)
        .integer_node(operand)
        .and_then(|literal| literal.value_bignum())
    {
        return (value >= target_minimum && value <= target_maximum).then_some(operand);
    }
    let (source_minimum, source_maximum) = integer_carrier_span(program, machine, state, operand)?;
    if source_minimum >= target_minimum && source_maximum <= target_maximum {
        return Some(operand);
    }
    let operand_value = ScopedArithmeticValue::Term(scoped(occurrence, operand, bindings, fields)?);
    let minimum = ScopedArithmeticValue::Integer(target_minimum);
    let maximum = ScopedArithmeticValue::Integer(target_maximum);
    let fits = validation::scoped_arithmetic_values_relation(
        program,
        machine,
        hypotheses,
        BinaryOperator::GreaterOrEqual,
        &operand_value,
        &minimum,
    ) == StrictArithmeticImplicationJudgment::Proven
        && validation::scoped_arithmetic_values_relation(
            program,
            machine,
            hypotheses,
            BinaryOperator::LessOrEqual,
            &operand_value,
            &maximum,
        ) == StrictArithmeticImplicationJudgment::Proven;
    fits.then_some(operand)
}

fn integer_carrier_span(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    let minimum = expression_integer_carrier_minimum(program, machine, state, expression)?;
    let maximum = expression_integer_carrier_maximum(program, machine, state, expression)?;
    Some((minimum, numerics::bignum::BigInt::from_u64(maximum)))
}

/// Whether the occurrence's scoped engine admits `expression`: its syntax is
/// integer vocabulary, its leaves resolve against the occurrence's own roster.
fn admitted(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
    depth: usize,
) -> bool {
    let ScopedOccurrence {
        program,
        machine,
        state,
        lengths,
        ..
    } = occurrence;
    if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        // Boolean leaves belong to the scoped engine's predicate language.
        // Admission grants only their syntax; the engine still distinguishes
        // true from false when judging a conjunction or a whole predicate.
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Cast(_) => integer_cast_operand(program, machine, state, expression)
            .is_some_and(|operand| admitted(occurrence, operand, bindings, fields, depth + 1)),
        ExpressionNode::Call(_) => validation::integer_embedding_argument(program, expression)
            .is_some_and(|(_, argument)| {
                admitted(occurrence, argument, bindings, fields, depth + 1)
            }),
        ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            result_scalar_field_occurrence(occurrence, expression).is_some()
                || length_occurrence(program, expression, lengths).is_some()
                || scalar_field_occurrence(occurrence, expression, bindings, fields).is_some()
                || field_occurrence(program, machine, state, expression).is_some()
                || literal_window_length(program, expression).is_some()
        }
        ExpressionNode::Name(name) => {
            (name.symbol == name.head_symbol
                && bindings
                    .iter()
                    .any(|binding| binding.symbol == name.symbol && binding.value.is_some()))
                || validation::reserved_result_owner(program, expression)
                    .is_some_and(|(owner, _)| owner == machine.symbol)
        }
        ExpressionNode::Binary(binary) => {
            matches!(
                binary.operator,
                BinaryOperator::Add
                    | BinaryOperator::Subtract
                    | BinaryOperator::Multiply
                    | BinaryOperator::Divide
                    | BinaryOperator::Modulo
                    | BinaryOperator::BitwiseAnd
                    | BinaryOperator::BitwiseOr
                    | BinaryOperator::BitwiseXor
                    | BinaryOperator::ShiftLeft
                    | BinaryOperator::ShiftRight
                    | BinaryOperator::Equal
                    | BinaryOperator::NotEqual
                    | BinaryOperator::Less
                    | BinaryOperator::LessOrEqual
                    | BinaryOperator::Greater
                    | BinaryOperator::GreaterOrEqual
                    | BinaryOperator::And
                    | BinaryOperator::Or
            ) && validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            ) && admitted(occurrence, binary.left, bindings, fields, depth + 1)
                && admitted(occurrence, binary.right, bindings, fields, depth + 1)
        }
        _ => false,
    }
}

/// The scoped projection of `expression` at this occurrence.
fn scoped(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
) -> Option<ScopedArithmeticExpression> {
    scoped_operations(occurrence, expression, bindings, fields, false)
}

/// Bind the same exact read roster for proof consumers and declaration
/// formation. Formation preserves operations for the child-first checker;
/// completed proof consumers retain their prior preserving-cast projection.
fn scoped_operations(
    occurrence: ScopedOccurrence<'_, '_>,
    expression: ExpressionHandle,
    bindings: &[Binding],
    fields: &[ScalarField],
    form_operations: bool,
) -> Option<ScopedArithmeticExpression> {
    let ScopedOccurrence {
        program,
        machine,
        state,
        lengths,
        ..
    } = occurrence;
    let mut symbols = Vec::new();
    let mut projections = Vec::new();
    let mut pending = vec![(expression, 0)];
    let mut budget = 4096usize;
    while let Some((expression, depth)) = pending.pop() {
        budget = budget.checked_sub(1)?;
        if depth >= 64 || !program.expression_table.expression_is_valid(expression) {
            return None;
        }
        match program.expression_table.expression(expression) {
            ExpressionNode::Name(_)
                if program.expression_table.names_reserved_result(expression) => {}
            ExpressionNode::Name(name) if name.symbol == name.head_symbol => {
                crate::shared::collections::push_unique(&mut symbols, name.symbol);
            }
            ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => {}
            ExpressionNode::Atomic(atomic) if form_operations => {
                pending.push((atomic.value, depth + 1));
            }
            ExpressionNode::Cast(cast) if form_operations => {
                pending.push((cast.value, depth + 1));
            }
            ExpressionNode::Call(call) if form_operations => {
                if call.receiver.is_valid() {
                    pending.push((call.receiver, depth + 1));
                }
                pending.extend(
                    program
                        .expression_table
                        .expression_handles(call.arguments)
                        .iter()
                        .map(|argument| (*argument, depth + 1)),
                );
            }
            ExpressionNode::Cast(_) => {
                let operand =
                    preserving_integer_cast_operand(occurrence, expression, bindings, fields)?;
                let value = scoped(occurrence, operand, bindings, fields)?;
                projections.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(expression),
                    value: ScopedArithmeticValue::Term(value),
                });
            }
            ExpressionNode::Call(_) => {
                let (_, argument) = validation::integer_embedding_argument(program, expression)?;
                pending.push((argument, depth + 1));
            }
            ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                let value = result_scalar_field_occurrence(occurrence, expression)
                    .or_else(|| {
                        length_occurrence(program, expression, lengths)
                            .and_then(|length| length.value.clone())
                            .map(carrier_bounded_length)
                    })
                    .or_else(|| scalar_field_occurrence(occurrence, expression, bindings, fields))
                    .or_else(|| field_occurrence(program, machine, state, expression))
                    .or_else(|| literal_window_length(program, expression))?;
                projections.push(ScopedArithmeticBinding {
                    binder: ScopedArithmeticBinder::Occurrence(expression),
                    value,
                });
            }
            ExpressionNode::Binary(binary) => {
                pending.push((binary.left, depth + 1));
                pending.push((binary.right, depth + 1));
            }
            _ => return None,
        }
    }
    let mut selected = Vec::new();
    for symbol in symbols {
        let value = bindings
            .iter()
            .find(|binding| binding.symbol == symbol)?
            .value
            .as_ref()?;
        charge(value, &mut budget, 0)?;
        selected.push(ScopedArithmeticBinding {
            binder: ScopedArithmeticBinder::Symbol(symbol),
            value: value.clone(),
        });
    }
    selected.extend(projections);
    Some(ScopedArithmeticExpression {
        owner: state.symbol,
        expression,
        bindings: selected,
    })
}
fn charge(value: &ScopedArithmeticValue, budget: &mut usize, depth: usize) -> Option<()> {
    *budget = budget.checked_sub(1)?;
    if depth >= 64 {
        return None;
    }
    match value {
        ScopedArithmeticValue::Term(term)
        | ScopedArithmeticValue::CollectionLength(term)
        | ScopedArithmeticValue::AdmittedIntegerRemainder(term) => {
            for binding in &term.bindings {
                charge(&binding.value, budget, depth + 1)?;
            }
        }
        ScopedArithmeticValue::BoundedInteger { value, .. } => {
            charge(value, budget, depth + 1)?;
        }
        ScopedArithmeticValue::IntegerOperation { left, right, .. } => {
            charge(left, budget, depth + 1)?;
            charge(right, budget, depth + 1)?;
        }
        ScopedArithmeticValue::TypeReference(_)
        | ScopedArithmeticValue::OwnedSnapshot(_)
        | ScopedArithmeticValue::CapturedInteger(_)
        | ScopedArithmeticValue::Atom { .. }
        | ScopedArithmeticValue::Integer(_) => {}
    }
    Some(())
}

/// The current descriptor observation's proven upper bound, after every
/// preceding store and call has retired or replaced its captured facts.
pub(crate) fn byte_length_bound(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<u64> {
    let machine = program.machine_by_own_or_state_symbol(state.symbol)?;
    CurrentScalarValues::at(
        program,
        machine,
        state,
        statement_index,
        TransitionTargetHandle::invalid(),
        None,
    )?
    .byte_length_bound(expression)
}
