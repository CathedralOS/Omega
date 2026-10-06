//! Checked reasoning about collection predicates.
//!
//! [Collection invariants](../../../../../../wiki/spec/language/dependent_values.md#collection-invariants)
//! constrain a collection with an ordinary pure, total, terminating predicate
//! machine used in a `where` or `requires` fact, for example
//! `where cells_valid(&cells) == true`. Naming the predicate grants nothing:
//! every consequence here comes from exploring that machine's own checked body
//! (`exploration.rs`) at the exact arguments of one fact. There is no
//! quantifier, no range-only domain and no decomposition rule that bypasses
//! the body.
//!
//! Three judgments consume the exploration:
//!
//! - Zero initialization: a data fact holds at the all-zero representation
//!   when the predicate returns `true` on every explored path with every
//!   argument field zero (`data_fact_holds_at_zero`).
//! - Consequences: under a fact `P(..) == true`, each array element and
//!   scalar argument lies in the join of its values over the paths that
//!   return `true` (`application_consequences`). Invariant windows use this for
//!   the facts standing before a store; helper theorems use it for premises.
//! - Proof: a goal holds when every path of its own evaluation, split on the
//!   values it reads, yields `true` (`premises_prove_expression`).
//!
//! Every judgment stands down (proves nothing) outside the explorer's small
//! body fragment or work budget. Failing to prove is never a refutation.

mod exploration;
pub(crate) mod integer_sets;

use exploration::{Binding, Frame, Stop, ValueShape, integer_carrier, value_shape};
pub(crate) use integer_sets::IntegerSet;
use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::statement::StatementNode;
use typed_trees::types::TypeReferenceHandle;

/// Initial knowledge of one predicate argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArgumentKnowledge {
    /// One set per element of a fixed integer array, in order.
    Array(Vec<IntegerSet>),
    Integer(IntegerSet),
    Boolean(bool),
}

/// `P(arguments) == true`, `true == P(arguments)` or a bare `P(arguments)`
/// whose callee is an admitted predicate and whose equality, when spelled,
/// has builtin meaning in `owner`'s scope.
pub(crate) struct PredicateApplication<'program> {
    pub(crate) machine: usize,
    pub(crate) arguments: &'program [ExpressionHandle],
}

pub(crate) fn predicate_application(
    program: &TypedTrees,
    owner: symbols::SymbolHandle,
    fact: ExpressionHandle,
) -> Option<PredicateApplication<'_>> {
    if !program.expression_table.expression_is_valid(fact) {
        return None;
    }
    let call_expression = match program.expression_table.expression(fact) {
        ExpressionNode::Call(_) => fact,
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::Equal => {
            let call = match (
                program.expression_table.expression(binary.left),
                program.expression_table.expression(binary.right),
            ) {
                (ExpressionNode::Call(_), ExpressionNode::Boolean(true)) => binary.left,
                (ExpressionNode::Boolean(true), ExpressionNode::Call(_)) => binary.right,
                _ => return None,
            };
            let boolean = crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                program,
                symbols::BuiltinTypeAtom::Bool,
            )?;
            if !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                owner,
                fact,
                language_core::OperatorSpelling::Equal,
                &[Some(boolean), Some(boolean)],
            ) {
                return None;
            }
            call
        }
        _ => return None,
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(call_expression) else {
        return None;
    };
    if call.receiver.is_valid()
        || call.static_machine_parameter.is_valid()
        || call.static_requirement_dispatch.is_some()
        || !call.machine_arguments.is_empty()
        || call.quotient_operation.is_some()
        || call.private_layout_operation.is_some()
        || call.layout_query.is_some()
        || !call.evidence_arguments.is_empty()
    {
        return None;
    }
    let machine = exploration::admitted_predicate_machine(program, call.target_symbol, false)?;
    Some(PredicateApplication {
        machine,
        arguments: program.expression_table.expression_handles(call.arguments),
    })
}

/// One exploration of a predicate at fixed initial knowledge.
pub(crate) struct Unfolding {
    outcomes: exploration::Outcomes,
    /// The atom range each argument occupies, in argument order.
    layout: Vec<std::ops::Range<usize>>,
}

impl Unfolding {
    /// `Some(result)` when every explored path returns `result`.
    pub(crate) fn decided(&self) -> Option<bool> {
        match (&self.outcomes.when_true, &self.outcomes.when_false) {
            (Some(_), None) => Some(true),
            (None, Some(_)) => Some(false),
            _ => None,
        }
    }

    /// The values argument `position` can hold when the predicate returns
    /// `true`: one set per array element, or one set for a scalar. `None`
    /// when no path returns `true`, which grants nothing.
    pub(crate) fn when_true(&self, position: usize) -> Option<Vec<IntegerSet>> {
        let atoms = self.outcomes.when_true.as_ref()?;
        Some(atoms.get(self.layout.get(position)?.clone())?.to_vec())
    }
}

/// Explore `machine` from its entry with one knowledge row per parameter.
pub(crate) fn unfold(
    program: &TypedTrees,
    machine: usize,
    arguments: &[ArgumentKnowledge],
) -> Option<Unfolding> {
    let entry = program
        .machine_states(program.machines().get(machine)?)
        .first()?;
    let parameters = program.state_parameters(entry);
    if parameters.len() != arguments.len() {
        return None;
    }
    let mut atoms = Vec::new();
    let mut bindings = Vec::with_capacity(arguments.len());
    let mut layout = Vec::with_capacity(arguments.len());
    for (parameter, knowledge) in parameters.iter().zip(arguments) {
        let first = atoms.len();
        match (value_shape(program, parameter.type_reference)?, knowledge) {
            (
                ValueShape::Array {
                    length,
                    element_type,
                },
                ArgumentKnowledge::Array(elements),
            ) if elements.len() == length => {
                let (low, high) = integer_carrier(program, element_type)?;
                let carrier = IntegerSet::range(low, high);
                atoms.extend(elements.iter().map(|element| element.intersect(&carrier)));
                bindings.push(Binding::Array {
                    first,
                    length,
                    element_type,
                });
            }
            (ValueShape::Integer(carrier), ArgumentKnowledge::Integer(set)) => {
                let (low, high) = integer_carrier(program, carrier)?;
                atoms.push(set.intersect(&IntegerSet::range(low, high)));
                bindings.push(Binding::Atom {
                    atom: first,
                    carrier,
                });
            }
            (ValueShape::Boolean, ArgumentKnowledge::Boolean(value)) => {
                bindings.push(Binding::Boolean(*value));
            }
            _ => return None,
        }
        layout.push(first..atoms.len());
    }
    // An argument outside its own carrier cannot be passed at all.
    if atoms.iter().any(IntegerSet::is_empty) {
        return None;
    }
    let outcomes = exploration::explore(program, machine, bindings, atoms)?;
    Some(Unfolding { outcomes, layout })
}

/// The full carrier knowledge of a parameter or field type: nothing known.
pub(crate) fn unknown_knowledge(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<ArgumentKnowledge> {
    Some(match value_shape(program, type_reference)? {
        ValueShape::Array {
            length,
            element_type,
        } => {
            let (low, high) = integer_carrier(program, element_type)?;
            ArgumentKnowledge::Array(vec![IntegerSet::range(low, high); length])
        }
        ValueShape::Integer(carrier) => {
            let (low, high) = integer_carrier(program, carrier)?;
            ArgumentKnowledge::Integer(IntegerSet::range(low, high))
        }
        ValueShape::Boolean => return None,
    })
}

/// The zero representation of a parameter or field type.
fn zero_knowledge(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<ArgumentKnowledge> {
    Some(match value_shape(program, type_reference)? {
        ValueShape::Array { length, .. } => {
            ArgumentKnowledge::Array(vec![IntegerSet::singleton(0); length])
        }
        ValueShape::Integer(_) => ArgumentKnowledge::Integer(IntegerSet::singleton(0)),
        ValueShape::Boolean => ArgumentKnowledge::Boolean(false),
    })
}

/// The field a data-fact argument names: `field` or `&field`.
pub(crate) fn fact_argument_field<'program>(
    program: &'program TypedTrees,
    definition: &DataDefinition,
    argument: ExpressionHandle,
) -> Option<&'program DataField> {
    let symbol = crate::expression_traversal::single_member_argument_symbol(program, argument)?;
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.symbol == symbol => Some(field),
            _ => None,
        })
}

/// A data fact that applies a predicate to fields of `definition`, with each
/// argument's knowledge supplied by `field_knowledge`.
fn data_fact_unfolding(
    program: &TypedTrees,
    definition: &DataDefinition,
    fact: ExpressionHandle,
    field_knowledge: &mut dyn FnMut(&DataField) -> Option<ArgumentKnowledge>,
) -> Option<(Unfolding, Vec<Option<symbols::SymbolHandle>>)> {
    let application = predicate_application(program, definition.symbol, fact)?;
    let mut knowledge = Vec::with_capacity(application.arguments.len());
    let mut fields = Vec::with_capacity(application.arguments.len());
    for argument in application.arguments {
        if let Some(field) = fact_argument_field(program, definition, *argument) {
            knowledge.push(field_knowledge(field)?);
            fields.push(Some(field.symbol));
            continue;
        }
        let value = program.expression_table.constant_integer_value(*argument)?;
        knowledge.push(ArgumentKnowledge::Integer(IntegerSet::singleton(
            i128::from(value),
        )));
        fields.push(None);
    }
    Some((unfold(program, application.machine, &knowledge)?, fields))
}

/// Whether one data `where` fact is a collection-predicate application that
/// holds at the all-zero representation of its fields.
pub(crate) fn data_fact_holds_at_zero(
    program: &TypedTrees,
    definition: &DataDefinition,
    fact: ExpressionHandle,
) -> bool {
    data_fact_unfolding(program, definition, fact, &mut |field| {
        zero_knowledge(program, field.type_reference)
    })
    .and_then(|(unfolding, _)| unfolding.decided())
        == Some(true)
}

/// Whether a data fact mentions `field` as a predicate argument.
pub(crate) fn data_fact_applies_to_field(
    program: &TypedTrees,
    definition: &DataDefinition,
    fact: ExpressionHandle,
    field: symbols::SymbolHandle,
) -> Option<bool> {
    let application = predicate_application(program, definition.symbol, fact)?;
    Some(application.arguments.iter().any(|argument| {
        fact_argument_field(program, definition, *argument)
            .is_some_and(|argument_field| argument_field.symbol == field)
    }))
}

/// Decide one data fact with the caller's knowledge of its argument fields.
pub(crate) fn data_fact_decided(
    program: &TypedTrees,
    definition: &DataDefinition,
    fact: ExpressionHandle,
    field_knowledge: &mut dyn FnMut(&DataField) -> Option<ArgumentKnowledge>,
) -> Option<bool> {
    data_fact_unfolding(program, definition, fact, field_knowledge)?
        .0
        .decided()
}

/// The element values of array `field` implied by every standing data fact
/// of `definition` that applies a predicate to it, each fact explored with no
/// other knowledge. `None` when no fact constrains the field.
pub(crate) fn standing_field_elements(
    program: &TypedTrees,
    definition: &DataDefinition,
    field: &DataField,
) -> Option<Vec<IntegerSet>> {
    let ArgumentKnowledge::Array(mut elements) = unknown_knowledge(program, field.type_reference)?
    else {
        return None;
    };
    let mut constrained = false;
    for fact in program.proof_facts.span_or_empty(definition.where_facts) {
        let ProofFact::Expression(fact) = fact else {
            continue;
        };
        let Some((unfolding, fields)) =
            data_fact_unfolding(program, definition, *fact, &mut |argument| {
                unknown_knowledge(program, argument.type_reference)
            })
        else {
            continue;
        };
        for (position, argument_field) in fields.iter().enumerate() {
            if *argument_field != Some(field.symbol) {
                continue;
            }
            let Some(implied) = unfolding.when_true(position) else {
                continue;
            };
            if implied.len() != elements.len() {
                continue;
            }
            for (element, implied) in elements.iter_mut().zip(implied) {
                *element = element.intersect(&implied);
            }
            constrained = true;
        }
    }
    constrained.then_some(elements)
}

/// The interval shape of one argument field's consequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConsequenceShape {
    /// One inclusive interval per element of a fixed integer array.
    Array(Vec<(i128, i128)>),
    Integer((i128, i128)),
}

/// The consequence one data fact grants one of its argument fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldConsequence {
    pub(crate) field: symbols::SymbolHandle,
    pub(crate) elements: ConsequenceShape,
}

/// The checked elementwise consequence of one collection-predicate data fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FactConsequence {
    /// One row per distinct argument field, in argument order.
    pub(crate) fields: Vec<FieldConsequence>,
    /// Whether the intervals also imply the fact: exploring the predicate
    /// with every argument field confined to its intervals returns `true` on
    /// every path. Rows bounding each element then carry the complete fact.
    pub(crate) exact: bool,
}

/// Under a data fact `P(&field, ..) == true`, the interval hull of the values
/// each argument field's elements hold on the paths of `P`'s own body that
/// return `true`, explored with no other knowledge. `None` outside the
/// explorer's fragment or budget, or when no path returns `true`.
pub(crate) fn data_fact_element_consequences(
    program: &TypedTrees,
    definition: &DataDefinition,
    fact: ExpressionHandle,
) -> Option<FactConsequence> {
    let (unfolding, fields) = data_fact_unfolding(program, definition, fact, &mut |field| {
        unknown_knowledge(program, field.type_reference)
    })?;
    let mut consequences: Vec<(symbols::SymbolHandle, Vec<IntegerSet>, bool)> = Vec::new();
    for (position, field) in fields.iter().enumerate() {
        let Some(field) = field else {
            continue;
        };
        let implied = unfolding.when_true(position)?;
        let is_array = matches!(
            unknown_knowledge(program, field_type(program, definition, *field)?)?,
            ArgumentKnowledge::Array(_)
        );
        match consequences
            .iter_mut()
            .find(|(symbol, _, _)| symbol == field)
        {
            Some((_, existing, _)) if existing.len() == implied.len() => {
                for (element, implied) in existing.iter_mut().zip(&implied) {
                    *element = element.intersect(implied);
                }
            }
            Some(_) => return None,
            None => consequences.push((*field, implied, is_array)),
        }
    }
    let mut rows = Vec::with_capacity(consequences.len());
    for (field, elements, is_array) in &consequences {
        let intervals = elements
            .iter()
            .map(IntegerSet::bounds)
            .collect::<Option<Vec<_>>>()?;
        rows.push(FieldConsequence {
            field: *field,
            elements: if *is_array {
                ConsequenceShape::Array(intervals)
            } else {
                ConsequenceShape::Integer(*intervals.first()?)
            },
        });
    }
    let exact = data_fact_decided(program, definition, fact, &mut |field| {
        let row = rows.iter().find(|row| row.field == field.symbol)?;
        Some(match &row.elements {
            ConsequenceShape::Array(intervals) => ArgumentKnowledge::Array(
                intervals
                    .iter()
                    .map(|(low, high)| IntegerSet::range(*low, *high))
                    .collect(),
            ),
            ConsequenceShape::Integer((low, high)) => {
                ArgumentKnowledge::Integer(IntegerSet::range(*low, *high))
            }
        })
    }) == Some(true);
    Some(FactConsequence {
        fields: rows,
        exact,
    })
}

fn field_type(
    program: &TypedTrees,
    definition: &DataDefinition,
    field: symbols::SymbolHandle,
) -> Option<TypeReferenceHandle> {
    program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(candidate) if candidate.symbol == field => {
                Some(candidate.type_reference)
            }
            _ => None,
        })
}

/// Whether a machine's own premises prove `goal` at an exit that cannot have
/// changed any parameter: the machine is a single entry state with no
/// assignment or local declaration, and every parameter is read-only.
///
/// Premises are the machine's `requires` conjuncts. A predicate application
/// over parameters contributes its `true`-path consequences; any other
/// conjunct refines the parameters by exploring it as a guard and keeping the
/// forks where it holds. A conjunct outside the fragment is dropped, which
/// only weakens the premises. The goal then holds when every fork of its own
/// evaluation is `true`.
pub fn premises_prove_expression(
    program: &TypedTrees,
    machine: &Machine,
    goal: ExpressionHandle,
) -> bool {
    let [entry] = program.machine_states(machine) else {
        return false;
    };
    if program
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .any(|statement| {
            matches!(
                statement,
                StatementNode::Assignment(_)
                    | StatementNode::LocalData(_)
                    | StatementNode::RootBinding(_)
            )
        })
    {
        return false;
    }
    let parameters = program.state_parameters(entry);
    if parameters.iter().any(|parameter| parameter.is_mutable) {
        return false;
    }
    let mut atoms = Vec::new();
    let mut bindings = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        let first = atoms.len();
        match unknown_knowledge(program, parameter.type_reference) {
            Some(ArgumentKnowledge::Array(elements)) => {
                let Some(ValueShape::Array { element_type, .. }) =
                    value_shape(program, parameter.type_reference)
                else {
                    return false;
                };
                let length = elements.len();
                atoms.extend(elements);
                bindings.push(Binding::Array {
                    first,
                    length,
                    element_type,
                });
            }
            Some(ArgumentKnowledge::Integer(set)) => {
                let Some(ValueShape::Integer(carrier)) =
                    value_shape(program, parameter.type_reference)
                else {
                    return false;
                };
                atoms.push(set);
                bindings.push(Binding::Atom {
                    atom: first,
                    carrier,
                });
            }
            // A parameter the fragment cannot read stays unreadable: a
            // premise or goal that reads it stands down.
            _ => bindings.push(Binding::Unreadable),
        }
    }
    let frame = Frame {
        machine,
        state: entry,
        bindings: &bindings,
        locals: &[],
    };
    let mut premises = Vec::new();
    for contract in program.machine_contracts(machine) {
        if contract.kind != SignatureContractKind::Requires {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            if let ProofFact::Expression(expression) = fact {
                collect_conjuncts(program, *expression, &mut premises);
            }
        }
    }
    for premise in premises {
        if let Some(refined) = assume_application(program, &frame, &atoms, premise)
            .or_else(|| assume_guard(program, &frame, &atoms, premise))
        {
            atoms = refined;
        }
    }
    holds_on_every_fork(program, &frame, atoms, goal)
}

fn collect_conjuncts(
    program: &TypedTrees,
    expression: ExpressionHandle,
    into: &mut Vec<ExpressionHandle>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) if binary.operator == BinaryOperator::And => {
            collect_conjuncts(program, binary.left, into);
            collect_conjuncts(program, binary.right, into);
        }
        _ => into.push(expression),
    }
}

/// Refine the frame's atoms by the `true`-path consequences of one predicate
/// application whose arguments are the frame's own parameters or literals.
fn assume_application(
    program: &TypedTrees,
    frame: &Frame<'_>,
    atoms: &[IntegerSet],
    premise: ExpressionHandle,
) -> Option<Vec<IntegerSet>> {
    let application = predicate_application(program, frame.machine.symbol, premise)?;
    let mut knowledge = Vec::with_capacity(application.arguments.len());
    let mut targets = Vec::with_capacity(application.arguments.len());
    for argument in application.arguments {
        match exploration::evaluate(program, frame, atoms, *argument) {
            Ok(exploration::Value::Array { first, length, .. }) => {
                knowledge.push(ArgumentKnowledge::Array(
                    atoms.get(first..first + length)?.to_vec(),
                ));
                targets.push(Some(first..first + length));
            }
            Ok(exploration::Value::Integer { set, atom, .. }) => {
                knowledge.push(ArgumentKnowledge::Integer(set));
                targets.push(atom.map(|atom| atom..atom + 1));
            }
            Ok(exploration::Value::Boolean(value)) => {
                knowledge.push(ArgumentKnowledge::Boolean(value));
                targets.push(None);
            }
            Err(_) => return None,
        }
    }
    let unfolding = unfold(program, application.machine, &knowledge)?;
    let mut refined = atoms.to_vec();
    for (position, target) in targets.into_iter().enumerate() {
        let Some(target) = target else {
            continue;
        };
        let implied = unfolding.when_true(position)?;
        for (atom, implied) in target.zip(implied) {
            refined[atom] = refined[atom].intersect(&implied);
        }
    }
    Some(refined)
}

/// Refine the frame's atoms by one ordinary guard: explore its forks and
/// join the atoms of those where it holds.
fn assume_guard(
    program: &TypedTrees,
    frame: &Frame<'_>,
    atoms: &[IntegerSet],
    premise: ExpressionHandle,
) -> Option<Vec<IntegerSet>> {
    let mut remaining = 1usize << 12;
    let mut stack = vec![atoms.to_vec()];
    let mut joined: Option<Vec<IntegerSet>> = None;
    while let Some(atoms) = stack.pop() {
        remaining = remaining.checked_sub(1)?;
        match exploration::evaluate_boolean(program, frame, &atoms, premise) {
            Ok(true) => {
                joined = Some(match joined {
                    None => atoms,
                    Some(existing) => existing
                        .iter()
                        .zip(&atoms)
                        .map(|(left, right)| left.union(right))
                        .collect(),
                });
            }
            Ok(false) => {}
            Err(Stop::Split { atom, parts }) => {
                for part in parts {
                    let mut refined = atoms.clone();
                    refined[atom] = part;
                    stack.push(refined);
                }
            }
            Err(Stop::Unsupported) => return None,
        }
    }
    joined
}

fn holds_on_every_fork(
    program: &TypedTrees,
    frame: &Frame<'_>,
    atoms: Vec<IntegerSet>,
    goal: ExpressionHandle,
) -> bool {
    let mut remaining = 1usize << 12;
    let mut stack = vec![atoms];
    while let Some(atoms) = stack.pop() {
        let Some(left) = remaining.checked_sub(1) else {
            return false;
        };
        remaining = left;
        // An atom without any value is an unreachable fork.
        if atoms.iter().any(IntegerSet::is_empty) {
            continue;
        }
        match exploration::evaluate_boolean(program, frame, &atoms, goal) {
            Ok(true) => {}
            Ok(false) | Err(Stop::Unsupported) => return false,
            Err(Stop::Split { atom, parts }) => {
                for part in parts {
                    let mut refined = atoms.clone();
                    refined[atom] = part;
                    stack.push(refined);
                }
            }
        }
    }
    true
}
