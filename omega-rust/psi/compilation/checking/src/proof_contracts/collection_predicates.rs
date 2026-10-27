//! Checked reasoning about collection predicates.
//!
//! [Collection invariants](../../../../../../wiki/spec/language/dependent_values.md#collection-invariants)
//! constrain a collection with an ordinary pure, total, terminating predicate
//! machine used in a `where` or `requires` fact, for example
//! `where cells_valid(&cells) == true`. Naming the predicate grants nothing:
//! every consequence here comes from that machine's own checked body at the
//! exact arguments of one fact. There is no
//! quantifier, no range-only domain and no decomposition rule that bypasses
//! the body.
//!
//! Concrete exploration and symbolic implication have distinct jobs:
//!
//! - Zero initialization: a data fact holds at the all-zero representation
//!   when the predicate returns `true` on every explored path with every
//!   argument field zero (`data_fact_holds_at_zero`).
//! - Consequences: under a fact `P(..) == true`, each array element and
//!   scalar argument lies in the join of its values over the paths that
//!   return `true` (`unfold` / `when_true`). Invariant windows use this for
//!   the facts standing before a store; live guards project the same checked
//!   consequences onto their exact scalar arguments and descriptor extents.
//!   Extent knowledge never supplies element contents (`exploration.rs`).
//! - Integer relations: the same symbolic implication judgment proves ordering
//!   consequences on every successful return, then publishes exact caller
//!   actuals for borrow compatibility. Independent interval hulls cannot retain
//!   correlations such as two unrestricted indices being unequal.
//! - Proof: helper-theorem implication and predicate handback use symbolic
//!   descriptors, selected read coordinates and functional store versions.
//!   Recursive cases owe a complete substituted induction-premise roster and
//!   a well-founded carrier step (`implication.rs`), not enumeration of every
//!   possible collection length or unchecked predicate preservation.
//!
//! Every judgment stands down (proves nothing) outside the explorer's small
//! body fragment or work budget. Failing to prove is never a refutation.

mod exploration;
mod implication;
pub(crate) mod integer_sets;

use exploration::{Binding, ValueShape, integer_carrier, value_shape};
pub(crate) use integer_sets::IntegerSet;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::types::TypeReferenceHandle;
/// The member expression when `expression` is a `<collection>.length` read —
/// the bound shape length-guard seeding rules gate on identically.
pub fn length_member<'program>(
    program: &'program dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<&'program typed_trees::expression::TableMemberExpression> {
    let member = program.expression_table.member_node(expression)?;
    (language_semantics::declaration_selection::CollectionMeasure::from_authored_spelling(
        member.member.as_str(),
    ) == Some(language_semantics::declaration_selection::CollectionMeasure::Length))
    .then_some(member)
}

/// Initial knowledge of one predicate argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArgumentKnowledge {
    /// One set per element of a fixed integer array, in order.
    Array(Vec<IntegerSet>),
    Integer(IntegerSet),
    /// A runtime descriptor's extent, without invented element contents.
    CollectionExtent(IntegerSet),
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
    program: &dyn crate::CheckingInput,
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
    predicate_call(program, call_expression)
}

fn predicate_call(
    program: &dyn crate::CheckingInput,
    call_expression: ExpressionHandle,
) -> Option<PredicateApplication<'_>> {
    let call = program.expression_table.call_node(call_expression)?;
    if !call.is_ordinary_positional_call() {
        return None;
    }
    let machine = exploration::admitted_predicate_machine(program, call.target_symbol, false)?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != call.arguments.len() {
        return None;
    }
    Some(PredicateApplication { machine, arguments })
}

/// The full declared carrier of an unconditional integer proof formal.
/// Representation-only queries unwrap qualifications; they cannot establish a
/// callee's additional domain or reference-formation obligations.
pub(crate) fn bare_integer_parameter_type(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<typed_trees::types::PrimitiveType> {
    if !program
        .type_reference_table
        .contains_type_reference(reference)
    {
        return None;
    }
    if !matches!(
        program.type_reference_table.type_reference(reference),
        typed_trees::types::TypeReferenceNode::Named { .. }
    ) {
        return None;
    }
    program
        .primitive_type_reference(reference)
        .filter(|primitive| primitive.is_fixed_integer())
}

/// Judge a complete ordinary predicate at exact closed integer actuals using
/// the same checked-body explorer as construction and implication. Canonical
/// bytes supply values, never membership. False and undecided outcomes remain
/// distinct; no interval consequence substitutes for the complete predicate.
pub(crate) fn closed_integer_predicate_value(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    values: &[(i128, Option<TypeReferenceHandle>)],
) -> Option<bool> {
    let application = predicate_call(program, expression)?;
    let machine = program.machines().get(application.machine)?;
    let states = program.machine_states(machine);
    if states.len() != machine.states.len() {
        return None;
    }
    let entry = states.first()?;
    let parameters = program.state_parameters(entry);
    if parameters.len() != entry.parameters.len()
        || parameters.len() != values.len()
        || parameters.len() != application.arguments.len()
    {
        return None;
    }
    let knowledge = parameters
        .iter()
        .zip(application.arguments)
        .zip(values)
        .map(|((parameter, actual), (value, actual_type))| {
            let primitive = bare_integer_parameter_type(program, parameter.type_reference)?;
            if let Some(actual_type) = actual_type {
                if program.primitive_type_reference(*actual_type) != Some(primitive) {
                    return None;
                }
            } else {
                let literal = program.expression_table.integer_node(*actual)?;
                if literal.landing().is_some_and(|landing| {
                    typed_trees::types::PrimitiveType::from_name(landing.landed_type.name())
                        != Some(primitive)
                        || landing.domain != numerics::arithmetic::ArithmeticDomain::Exact
                }) {
                    return None;
                }
            }
            let (minimum, maximum) = primitive.integer_bounds()?;
            if *value < minimum || *value > maximum {
                return None;
            }
            Some(ArgumentKnowledge::Integer(IntegerSet::singleton(*value)))
        })
        .collect::<Option<Vec<_>>>()?;
    unfold(program, application.machine, &knowledge)?.decided()
}

/// One exact caller actual or a checked mathematical integer bound.
#[derive(Clone)]
pub struct PredicateIntegerSubject {
    pub expression: ExpressionHandle,
    pub projection: Vec<facts::PlaceSegment>,
}

/// An inert observation recipe in the predicate's exact declared telescope.
/// The source handle stays in that telescope; consumers substitute both the
/// collection and every selector before comparing caller value identities.
#[derive(Clone)]
pub struct PredicateIntegerReadSubject {
    pub predicate: symbols::SymbolHandle,
    pub expression: ExpressionHandle,
    pub arguments: Vec<ExpressionHandle>,
}

#[derive(Clone)]
pub enum PredicateIntegerOperand {
    Argument(PredicateIntegerSubject),
    Read(PredicateIntegerReadSubject),
    Integer(i128),
}

/// A relation proved on every true return, retaining exact caller actuals.
pub struct PredicateOrderingConsequence {
    pub left: PredicateIntegerOperand,
    pub right: PredicateIntegerOperand,
    pub operator: BinaryOperator,
}

/// A whole predicate may be replaced by one relation only after both
/// directions are proved over its complete formal carriers. Entry formation
/// and exact readable actuals remain obligations of the consuming contract.
pub fn exact_predicate_ordering(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> Option<PredicateOrderingConsequence> {
    let application = predicate_application(program, owner, expression)?;
    implication::exact_ordering(program, &application)
}

/// A complete conjunction, not a list of independent ambient facts. The shared
/// interpreter proves both directions and each partial read from earlier guards.
pub fn exact_predicate_conjunction(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> Option<Vec<PredicateOrderingConsequence>> {
    let application = predicate_application(program, owner, expression)?;
    implication::exact_conjunction(program, &application)
}

/// Rejoin a callee integer expression to its exact declared actual telescope.
/// No source expression is manufactured and a read remains an inert recipe.
pub fn predicate_integer_operand(
    program: &dyn crate::CheckingInput,
    predicate: symbols::SymbolHandle,
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> Option<PredicateIntegerOperand> {
    let position = exploration::admitted_predicate_machine(program, predicate, false)?;
    let machine = &program.machines()[position];
    let entry = program.machine_states(machine).first()?;
    if program.state_parameters(entry).len() != arguments.len() {
        return None;
    }
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(expression) {
        return Some(PredicateIntegerOperand::Integer(
            literal.decimal_value_i128()?,
        ));
    }
    if program.expression_table.indexed_node(expression).is_some() {
        if !crate::immutable_entry_indexed_read_identity(program, expression)?
            .has_scope(machine.symbol, entry.symbol)
        {
            return None;
        }
        return Some(PredicateIntegerOperand::Read(PredicateIntegerReadSubject {
            predicate,
            expression,
            arguments: arguments.to_vec(),
        }));
    }
    let (position, projection) =
        crate::immutable_entry_integer_projection(program, predicate, expression)?;
    Some(PredicateIntegerOperand::Argument(PredicateIntegerSubject {
        expression: *arguments.get(position)?,
        projection,
    }))
}

/// Recover integer relations from a positive, already-available predicate
/// occurrence. The shared symbolic implication engine interprets its checked
/// body, including calls and branches; naming a predicate supplies no relation.
/// Unsupported bodies and exhausted proof work contribute no evidence.
pub fn predicate_ordering_consequences(
    program: &dyn crate::CheckingInput,
    call_expression: ExpressionHandle,
) -> Vec<PredicateOrderingConsequence> {
    let Some(application) = predicate_call(program, call_expression) else {
        return Vec::new();
    };
    let mut consequences = implication::ordering_consequences(program, &application);
    // Scalar hulls are necessary consequences, never equivalent replacements
    // of the predicate. The exact public-envelope judgment below additionally
    // checks its converse over the complete Cartesian product.
    if let Some(intervals) = interval_consequences(program, &application) {
        for interval in intervals.into_iter().filter(|row| !row.extent) {
            for (bound, operator) in [
                (interval.low, BinaryOperator::GreaterOrEqual),
                (interval.high, BinaryOperator::LessOrEqual),
            ] {
                consequences.push(PredicateOrderingConsequence {
                    left: PredicateIntegerOperand::Argument(PredicateIntegerSubject {
                        expression: interval.argument,
                        projection: Vec::new(),
                    }),
                    right: PredicateIntegerOperand::Integer(bound),
                    operator,
                });
            }
        }
    }
    consequences
}

/// One interval observation of an exact actual argument. Consumers separately
/// establish whether it is a consequence or a complete public envelope.
/// Descriptor extents describe metadata only, never collection contents.
#[derive(Debug, Clone)]
pub struct PredicateInterval {
    pub argument: ExpressionHandle,
    pub extent: bool,
    pub low: i128,
    pub high: i128,
}

/// Describe an admitted application using the existing explorer, in both
/// directions. True outcomes imply these hulls; re-exploration of their full
/// Cartesian product must also return true on every path. A mere necessary
/// consequence cannot replace an authored public requirement. Unsupported
/// carriers, empty truth sets and non-exact hulls grant no representation.
pub fn exact_predicate_intervals(
    program: &dyn crate::CheckingInput,
    owner: symbols::SymbolHandle,
    expression: ExpressionHandle,
) -> Option<Vec<PredicateInterval>> {
    let application = predicate_application(program, owner, expression)?;
    let rows = interval_consequences(program, &application)?;
    if rows.len() != application.arguments.len() {
        return None;
    }
    let narrowed = rows
        .iter()
        .map(|row| {
            let hull = IntegerSet::range(row.low, row.high);
            if row.extent {
                ArgumentKnowledge::CollectionExtent(hull)
            } else {
                ArgumentKnowledge::Integer(hull)
            }
        })
        .collect::<Vec<_>>();
    (unfold(program, application.machine, &narrowed)?.decided() == Some(true)).then_some(rows)
}

/// One-way true-return bounds shared by guard narrowing and borrow relations.
/// Complete declared carriers seed the existing explorer. Unsupported actual
/// shapes contribute no row; an empty truth set grants no ambient contradiction.
/// Consumers retain availability, source meaning and storage epochs separately.
pub(crate) fn interval_consequences(
    program: &dyn crate::CheckingInput,
    application: &PredicateApplication<'_>,
) -> Option<Vec<PredicateInterval>> {
    let entry = program
        .machine_states(&program.machines()[application.machine])
        .first()?;
    let knowledge = program
        .state_parameters(entry)
        .iter()
        .map(|parameter| unknown_knowledge(program, parameter.type_reference))
        .collect::<Option<Vec<_>>>()?;
    if knowledge.len() != application.arguments.len() {
        return None;
    }
    let unfolding = unfold(program, application.machine, &knowledge)?;
    let mut rows = Vec::new();
    for (position, (argument, knowledge)) in
        application.arguments.iter().zip(&knowledge).enumerate()
    {
        let extent = match knowledge {
            ArgumentKnowledge::Integer(_) => false,
            ArgumentKnowledge::CollectionExtent(_) => true,
            _ => continue,
        };
        let Some(values) = unfolding.when_true(position) else {
            continue;
        };
        let [values] = values.as_slice() else {
            continue;
        };
        let Some((low, high)) = values.bounds() else {
            continue;
        };
        rows.push(PredicateInterval {
            argument: *argument,
            extent,
            low,
            high,
        });
    }
    Some(rows)
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
    program: &dyn crate::CheckingInput,
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
                ValueShape::Collection {
                    length,
                    element_type,
                },
                ArgumentKnowledge::Array(elements),
            ) if length.is_none_or(|length| elements.len() == length) => {
                let (low, high) = integer_carrier(program, element_type)?;
                let carrier = IntegerSet::range(low, high);
                atoms.extend(elements.iter().map(|element| element.intersect(&carrier)));
                bindings.push(Binding::Array {
                    first,
                    length: elements.len(),
                    element_type,
                });
            }
            (
                ValueShape::Collection { length: None, .. },
                ArgumentKnowledge::CollectionExtent(set),
            ) => {
                atoms.push(set.intersect(&IntegerSet::range(0, i128::from(u64::MAX))));
                bindings.push(Binding::CollectionExtent { atom: first });
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
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<ArgumentKnowledge> {
    Some(match value_shape(program, type_reference)? {
        ValueShape::Collection {
            length: Some(length),
            element_type,
        } => {
            let (low, high) = integer_carrier(program, element_type)?;
            ArgumentKnowledge::Array(vec![IntegerSet::range(low, high); length])
        }
        ValueShape::Integer(carrier) => {
            let (low, high) = integer_carrier(program, carrier)?;
            ArgumentKnowledge::Integer(IntegerSet::range(low, high))
        }
        ValueShape::Collection { length: None, .. } => {
            ArgumentKnowledge::CollectionExtent(IntegerSet::range(0, i128::from(u64::MAX)))
        }
        ValueShape::Boolean | ValueShape::Record(_) => return None,
    })
}

/// The zero representation of a parameter or field type.
fn zero_knowledge(
    program: &dyn crate::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<ArgumentKnowledge> {
    Some(match value_shape(program, type_reference)? {
        ValueShape::Collection {
            length: Some(length),
            ..
        } => ArgumentKnowledge::Array(vec![IntegerSet::singleton(0); length]),
        ValueShape::Integer(_) => ArgumentKnowledge::Integer(IntegerSet::singleton(0)),
        ValueShape::Boolean => ArgumentKnowledge::Boolean(false),
        ValueShape::Collection { length: None, .. } | ValueShape::Record(_) => return None,
    })
}

/// The field a data-fact argument names: `field` or `&field`.
pub(crate) fn fact_argument_field<'program>(
    program: &'program dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    definition: &DataDefinition,
    field: &DataField,
) -> Option<Vec<IntegerSet>> {
    let ArgumentKnowledge::Array(mut elements) = unknown_knowledge(program, field.type_reference)?
    else {
        return None;
    };
    let mut constrained = false;
    for fact in program.proof_facts.span_or_empty(definition.where_facts) {
        let Some(fact) = fact.as_expression() else {
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
    program: &dyn crate::CheckingInput,
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
        let is_array = match unknown_knowledge(program, field_type(program, definition, *field)?)? {
            ArgumentKnowledge::Array(_) => true,
            ArgumentKnowledge::Integer(_) => false,
            // Descriptor metadata is not the value stored in this field.
            // These rows certify element/scalar field domains only.
            ArgumentKnowledge::CollectionExtent(_) | ArgumentKnowledge::Boolean(_) => return None,
        };
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
    program: &dyn crate::CheckingInput,
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

/// Whether a machine's arrival premises and supported body prove `goal` at
/// every normal exit. Stores keep the original predicate's logical version
/// separate from the current collection; unknown effects grant nothing.
///
/// Premises retain the machine's exact formal identities. A predicate's
/// successful paths establish the selected read through declared coordinate
/// meaning and checked recursive implication. Unsupported cases grant nothing.
pub fn premises_prove_expression(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    goal: ExpressionHandle,
) -> bool {
    implication::proves(program, machine, goal)
}
