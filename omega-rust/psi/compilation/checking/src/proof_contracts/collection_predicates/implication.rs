//! Symbolic consequences of a predicate's normal Boolean result.
//!
//! A collection descriptor has an identity and a symbolic extent, not a list
//! of elements. Reads split only on equality with previously observed indices;
//! equal coordinates share one value. Thus proving a selected-cell theorem
//! does not enumerate every possible length or index.
//!
//! Each generalized activation is a lemma: its successful result implies the
//! demanded goal under its captured numeric premises. Reusing an unfinished
//! lemma owes every premise under simultaneous argument substitution and a
//! strict, consistently directed change of one fixed-width integer formal.
//! The carrier supplies a finite lower/upper bound, hence a well-founded
//! measure. State transfers carrying an enclosing lemma's formal stay in that
//! scope; generalizing them would erase the very relationship proving descent.
//! No body name, declared ranking promise, or current ensures supplies evidence.
//! Mutating helpers use functional collection versions. A read of an updated
//! version chooses the stored value only at the equal coordinate; other reads
//! retain the previous version's identity. Proving a resulting predicate true
//! checks every reachable return, using original predicate consequences where
//! necessary. Its recursive obligations use the same checked induction domain
//! as readonly consequences, never an assumption that predicates are hereditary.
//! Unsupported operations and exhausted work prove nothing.
//!
//! Setting `OMEGA_COLLECTION_PROOF_TRACE` to a file path appends one
//! `theorem=<machine> goal=<fact> elapsed_micros=<n> spent=<n> proved=<bool>`
//! line per judgment to that file -- the opt-in observation convention of
//! `OMEGA_PROOF_MEASUREMENTS`, written to a file because a package review
//! does not surface the checker's stderr. Normal runs pay one env-var check.

use super::exploration::{ValueShape, admitted_predicate_machine, value_shape};
use crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning;
use crate::{
    ScopedArithmeticComparison as Comparison, ScopedArithmeticValue as Integer,
    scoped_arithmetic_comparisons_unsatisfiable,
};
use numerics::bignum::BigInt;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Clone, Debug, PartialEq, Eq)]
enum Value {
    Integer {
        value: Integer,
        carrier: Option<TypeReferenceHandle>,
    },
    Collection {
        identity: usize,
        length: Integer,
        element: TypeReferenceHandle,
    },
    Boolean(Boolean),
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Boolean {
    Constant(bool),
    Comparison(Comparison),
    And(Box<Self>, Box<Self>),
    Or(Box<Self>, Box<Self>),
    Not(Box<Self>),
    Call {
        machine: usize,
        arguments: Vec<Value>,
    },
}

#[derive(Clone)]
struct Observation {
    collection: usize,
    selector: Integer,
    value: Value,
}

/// One disjunction of comparisons; an empty clause is `false`.
type Clause = Vec<Comparison>;

/// What one symbolic path has assumed: a conjunction of comparisons, the
/// disjunctions it has not yet split, and the element reads it has observed.
/// A disjunction stays a clause until a decision touches one of its atoms;
/// splitting it at assumption time would fork the path once per disjunct,
/// and a predicate that guards every element in its own state would then
/// cost one path per combination of elements the goal never reads.
#[derive(Clone, Default)]
struct Path {
    comparisons: Vec<Comparison>,
    clauses: Vec<Clause>,
    observations: Vec<Observation>,
}

#[derive(Clone)]
struct Frame {
    machine: usize,
    state: usize,
    bindings: Vec<(SymbolHandle, Value)>,
}

struct Lemma {
    machine: usize,
    state: usize,
    arguments: Vec<Value>,
    premises: Vec<Comparison>,
    premise_clauses: Vec<Clause>,
    /// Intersection of the measures justified by every recurrent arrival.
    measures: Vec<(usize, BinaryOperator)>,
    judgment: Judgment,
    goal: Boolean,
}

/// Consequences restrict attention to successful predicate returns; a required
/// result must establish truth on every reachable return. These are different
/// obligations, not alternative source-shape recognition paths.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Judgment {
    Consequence,
    RequiredResult,
}

struct CollectionUpdate {
    identity: usize,
    previous: usize,
    selector: Integer,
    value: Value,
}

struct Implication<'program> {
    program: &'program dyn crate::CheckingInput,
    context: &'program Machine,
    remaining: usize,
    next_identity: usize,
    lemmas: Vec<Lemma>,
    updates: Vec<CollectionUpdate>,
    antecedents: Vec<Boolean>,
}

pub(super) fn proves(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    goal: ExpressionHandle,
) -> bool {
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let Some(machine_position) = program
        .machines()
        .iter()
        .position(|candidate| candidate.symbol == machine.symbol)
    else {
        return false;
    };
    let mut proof = Implication {
        program,
        context: machine,
        remaining: WORK_BUDGET,
        next_identity: 0,
        lemmas: Vec::new(),
        updates: Vec::new(),
        antecedents: Vec::new(),
    };
    let started = std::time::Instant::now();
    let proved = proof.theorem(entry, machine_position, goal);
    if let Some(trace) = std::env::var_os("OMEGA_COLLECTION_PROOF_TRACE") {
        use std::io::Write as _;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(trace)
        {
            let _ = writeln!(
                file,
                "theorem={} goal={} elapsed_micros={} spent={} proved={proved}",
                machine.name,
                program.expression_table.display_name(goal),
                started.elapsed().as_micros(),
                WORK_BUDGET - proof.remaining,
            );
        }
    }
    proved
}

/// Steps one theorem judgment may spend across its activations, reads,
/// assumptions, expression evaluations and statement visits before it
/// stands down.
const WORK_BUDGET: usize = 8192;

impl Implication<'_> {
    fn theorem(
        &mut self,
        entry: &typed_trees::state::State,
        machine_position: usize,
        goal: ExpressionHandle,
    ) -> bool {
        let program = self.program;
        let machine = self.context;
        let mut path = Path::default();
        let mut bindings = Vec::new();
        for parameter in program.state_parameters(entry) {
            let Some(value) = self.parameter(parameter.type_reference, &mut path) else {
                return false;
            };
            bindings.push((parameter.symbol, value));
        }
        let frame = Frame {
            machine: machine_position,
            state: 0,
            bindings,
        };
        let mut candidates = vec![(path, Vec::<Boolean>::new())];
        for premise in program.contract_expressions(
            program.machine_contracts(machine).iter(),
            SignatureContractKind::Requires,
        ) {
            let mut next = Vec::new();
            for (path, predicates) in candidates {
                let Some(values) = self.expression(&frame, path, premise, 0) else {
                    continue;
                };
                for (path, value) in values {
                    let Value::Boolean(value) = value else {
                        continue;
                    };
                    let Some(branches) = self.premise(path, &value, true) else {
                        return false;
                    };
                    for (path, additional) in branches {
                        let mut combined = predicates.clone();
                        combined.extend(additional);
                        next.push((path, combined));
                    }
                }
            }
            // Dropping an unread premise weakens the theorem, never its conclusion.
            if next.is_empty() {
                return false;
            }
            candidates = next;
        }
        for (path, predicates) in candidates {
            self.antecedents = predicates;
            let Some(exits) = self.body_exits(frame.clone(), path) else {
                return false;
            };
            for (mut frame, path) in exits {
                // Ensures expressions are authored in entry scope even when a
                // different state returns. Bindings retain exact symbol identity
                // and current collection versions across the state transfer.
                frame.state = 0;
                let Some(goals) = self.expression(&frame, path, goal, 0) else {
                    return false;
                };
                for (path, goal) in goals {
                    let Value::Boolean(goal) = goal else {
                        return false;
                    };
                    if self.establish(&path, &goal, 0) != Some(true) {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn establish(&mut self, path: &Path, goal: &Boolean, depth: usize) -> Option<bool> {
        self.spend()?;
        if depth >= 64 {
            return None;
        }
        if self.entails(path, goal) {
            return Some(true);
        }
        match goal {
            Boolean::Call { machine, arguments } => {
                if self.antecedents.iter().any(|antecedent| antecedent == goal) {
                    return Some(true);
                }
                return self.activation(
                    *machine,
                    0,
                    arguments.clone(),
                    path.clone(),
                    &Boolean::Constant(true),
                    Judgment::RequiredResult,
                    depth + 1,
                );
            }
            Boolean::And(left, right) => {
                return Some(
                    self.establish(path, left, depth + 1)?
                        && self.establish(path, right, depth + 1)?,
                );
            }
            Boolean::Or(left, right)
                if self.establish(path, left, depth + 1) == Some(true)
                    || self.establish(path, right, depth + 1) == Some(true) =>
            {
                return Some(true);
            }
            _ => {}
        }
        for antecedent in self.antecedents.clone() {
            let Boolean::Call { machine, arguments } = antecedent else {
                continue;
            };
            if self.activation(
                machine,
                0,
                arguments,
                path.clone(),
                goal,
                Judgment::Consequence,
                depth + 1,
            ) == Some(true)
            {
                return Some(true);
            }
        }
        Some(false)
    }

    /// Execute the supported body once to obtain its actual normal-return
    /// versions. Preconditions retain the arrival versions; assignments update
    /// all live aliases, not those immutable logical snapshots. Unsupported
    /// effects stand down rather than asserting parameter preservation. A
    /// recurrent mutating state needs a store invariant; it is not unrolled
    /// here. Predicate recurrence below has its separate checked induction.
    fn body_exits(&mut self, frame: Frame, path: Path) -> Option<Vec<(Frame, Path)>> {
        let mut pending = vec![(0usize, frame, path, vec![0usize])];
        let mut exits = Vec::new();
        while let Some((position, frame, path, visited)) = pending.pop() {
            self.spend()?;
            let machine = &self.program.machines()[frame.machine];
            let state = &self.program.machine_states(machine)[frame.state];
            let statements = self
                .program
                .statement_table
                .statements(state.statement_nodes);
            let Some(statement) = statements.get(position) else {
                exits.push((frame, path));
                continue;
            };
            match statement {
                StatementNode::LocalData(local)
                    if local.ordinary_value_initializer().is_some()
                        && local.relevance == language_core::BindingRelevance::Relevant =>
                {
                    for (path, value) in self.expression(&frame, path, local.initial_value, 0)? {
                        let mut frame = frame.clone();
                        frame.bindings.push((
                            local.symbol,
                            declared_value(self.program, value, local.type_reference)?,
                        ));
                        pending.push((position + 1, frame, path, visited.clone()));
                    }
                }
                StatementNode::Assignment(assignment) => {
                    for (frame, path) in
                        self.assignment(&frame, path, assignment.target, assignment.value)?
                    {
                        pending.push((position + 1, frame, path, visited.clone()));
                    }
                }
                StatementNode::Transition(transition)
                    if transition.exit == TransitionExit::Ordinary
                        && !transition.continuation.is_valid() =>
                {
                    let guards = match transition.guard {
                        TransitionGuardNode::Always => {
                            vec![(path, Value::Boolean(Boolean::Constant(true)))]
                        }
                        TransitionGuardNode::When(guard) => {
                            self.expression(&frame, path, guard, 0)?
                        }
                    };
                    for (path, guard) in guards {
                        let Value::Boolean(guard) = guard else {
                            return None;
                        };
                        if let Some(path) = self.assume(path.clone(), &guard, false)? {
                            pending.push((position + 1, frame.clone(), path, visited.clone()));
                        }
                        if let Some(path) = self.assume(path, &guard, true)? {
                            match self
                                .program
                                .statement_table
                                .transition_target(transition.target)
                            {
                                TransitionTargetNode::Terminal => exits.push((frame.clone(), path)),
                                TransitionTargetNode::Value(value) => {
                                    for (path, _) in self.expression(&frame, path, *value, 0)? {
                                        exits.push((frame.clone(), path));
                                    }
                                }
                                TransitionTargetNode::Named {
                                    path: target,
                                    arguments,
                                    static_machine_parameter,
                                    evidence_arguments,
                                    ..
                                } if !static_machine_parameter.is_valid()
                                    && evidence_arguments.is_empty() =>
                                {
                                    let destination = typed_trees::state::state_position(
                                        self.program.machine_states(machine),
                                        target.symbol,
                                    )?;
                                    if visited.contains(&destination) {
                                        return None;
                                    }
                                    let parameters = self.program.state_parameters(
                                        &self.program.machine_states(machine)[destination],
                                    );
                                    for (path, arguments) in self.arguments(
                                        &frame,
                                        path,
                                        self.program.statement_table.expression_handles(*arguments),
                                        0,
                                    )? {
                                        if parameters.len() != arguments.len() {
                                            return None;
                                        }
                                        let mut frame = frame.clone();
                                        frame.state = destination;
                                        for (parameter, value) in parameters.iter().zip(arguments) {
                                            frame.bindings.push((
                                                parameter.symbol,
                                                declared_value(
                                                    self.program,
                                                    value,
                                                    parameter.type_reference,
                                                )?,
                                            ));
                                        }
                                        let mut visited = visited.clone();
                                        visited.push(destination);
                                        pending.push((0, frame, path, visited));
                                    }
                                }
                                _ => return None,
                            }
                        }
                    }
                }
                StatementNode::Expression(expression) if position + 1 == statements.len() => {
                    for (path, _) in self.expression(&frame, path, *expression, 0)? {
                        exits.push((frame.clone(), path));
                    }
                }
                _ => return None,
            }
        }
        Some(exits)
    }

    fn assignment(
        &mut self,
        frame: &Frame,
        path: Path,
        target: ExpressionHandle,
        source: ExpressionHandle,
    ) -> Option<Vec<(Frame, Path)>> {
        let machine = &self.program.machines()[frame.machine];
        let state = &self.program.machine_states(machine)[frame.state];
        let ExpressionNode::Indexed(indexed) = self.program.expression_table.expression(target)
        else {
            return None;
        };
        if !crate::value_custody::places::place_has_builtin_coordinates(
            self.program,
            machine,
            Some(state),
            target,
        ) {
            return None;
        }
        let mut result = Vec::new();
        for (path, collection) in self.expression(frame, path, indexed.collection, 0)? {
            let Value::Collection {
                identity: previous,
                length,
                element,
            } = collection
            else {
                return None;
            };
            for (path, selector) in self.expression(frame, path, indexed.index, 0)? {
                let Value::Integer {
                    value: selector, ..
                } = selector
                else {
                    return None;
                };
                if !self.implies(
                    &path,
                    &compare(BinaryOperator::GreaterOrEqual, selector.clone(), number(0)),
                ) || !self.implies(
                    &path,
                    &compare(BinaryOperator::Less, selector.clone(), length.clone()),
                ) {
                    return None;
                }
                for (path, value) in self.expression(frame, path, source, 0)? {
                    let value = declared_value(self.program, value, element)?;
                    let identity = self.next_identity;
                    self.next_identity += 1;
                    self.updates.push(CollectionUpdate {
                        identity,
                        previous,
                        selector: selector.clone(),
                        value,
                    });
                    let mut frame = frame.clone();
                    for (_, value) in &mut frame.bindings {
                        if let Value::Collection {
                            identity: current, ..
                        } = value
                            && *current == previous
                        {
                            *current = identity;
                        }
                    }
                    result.push((frame, path));
                }
            }
        }
        Some(result)
    }

    fn spend(&mut self) -> Option<()> {
        self.remaining = self.remaining.checked_sub(1)?;
        Some(())
    }

    fn atom(&mut self, unsigned: bool) -> Integer {
        let identity = self.next_identity;
        self.next_identity += 1;
        Integer::Atom {
            identity: format!("collection-proof:{identity}"),
            unsigned,
        }
    }

    fn integer(&mut self, carrier: TypeReferenceHandle, path: &mut Path) -> Option<Value> {
        let (minimum, maximum) = self
            .program
            .primitive_type_reference(carrier)?
            .integer_bounds()?;
        let value = self.atom(minimum >= 0);
        path.comparisons.push(compare(
            BinaryOperator::GreaterOrEqual,
            value.clone(),
            number(minimum),
        ));
        path.comparisons.push(compare(
            BinaryOperator::LessOrEqual,
            value.clone(),
            number(maximum),
        ));
        Some(Value::Integer {
            value,
            carrier: Some(carrier),
        })
    }

    fn parameter(&mut self, reference: TypeReferenceHandle, path: &mut Path) -> Option<Value> {
        match readable_shape(self.program, reference)? {
            ValueShape::Integer(carrier) => self.integer(carrier, path),
            ValueShape::Collection {
                length,
                element_type,
            } => {
                let identity = self.next_identity;
                self.next_identity += 1;
                let length = length
                    .map(|length| number(length as i128))
                    .unwrap_or_else(|| self.atom(true));
                path.comparisons.push(compare(
                    BinaryOperator::GreaterOrEqual,
                    length.clone(),
                    number(0),
                ));
                path.comparisons.push(compare(
                    BinaryOperator::LessOrEqual,
                    length.clone(),
                    number(i128::from(u64::MAX)),
                ));
                Some(Value::Collection {
                    identity,
                    length,
                    element: element_type,
                })
            }
            ValueShape::Boolean => None,
        }
    }

    fn unsatisfiable(&self, comparisons: &[Comparison]) -> bool {
        scoped_arithmetic_comparisons_unsatisfiable(self.program, self.context, comparisons)
    }

    /// Whether `comparisons` together with `clauses` admit no values. A
    /// comparison over two literals decides itself. Everything else is
    /// decided one atom-connected component at a time: a contradiction can
    /// only combine comparisons that share atoms, so the solver never
    /// closes bounds over the cells a decision does not mention, and its
    /// superlinear cost follows the component rather than the path. A
    /// clause is split case by case inside its component. With a `focus`,
    /// only the components the focus touches are consulted, because every
    /// other component was consistent when its clauses were assumed.
    fn contradictory(
        &self,
        comparisons: &[Comparison],
        clauses: &[Clause],
        focus: Option<&[Comparison]>,
    ) -> bool {
        if comparisons
            .iter()
            .any(|comparison| constant_truth(comparison) == Some(false))
        {
            return true;
        }
        let focus_atoms = focus.map(|focus| {
            let mut atoms = Vec::new();
            for comparison in focus {
                comparison_atoms(comparison, &mut atoms);
            }
            atoms
        });
        let mut consultations = SPLIT_CONSULTATIONS;
        components(comparisons, clauses)
            .iter()
            .filter(|component| {
                focus_atoms
                    .as_ref()
                    .is_none_or(|atoms| atoms.iter().any(|atom| component.atoms.contains(atom)))
            })
            .any(|component| {
                self.split(
                    component.comparisons.clone(),
                    &component.clauses,
                    &mut consultations,
                )
            })
    }

    /// Decide the conjunction, then case-split the first clause and decide
    /// each case under the rest. Exhausting the consultation allowance
    /// decides nothing.
    fn split(
        &self,
        comparisons: Vec<Comparison>,
        clauses: &[Clause],
        consultations: &mut usize,
    ) -> bool {
        let Some(remaining) = consultations.checked_sub(1) else {
            return false;
        };
        *consultations = remaining;
        if self.unsatisfiable(&comparisons) {
            return true;
        }
        let Some((clause, rest)) = clauses.split_first() else {
            return false;
        };
        clause.iter().all(|literal| {
            let mut comparisons = comparisons.clone();
            comparisons.push(literal.clone());
            self.split(comparisons, rest, consultations)
        })
    }

    fn impossible(&self, path: &Path) -> bool {
        self.contradictory(&path.comparisons, &path.clauses, None)
    }

    /// Whether `path` admits no values at which `clause` is false: every
    /// literal's negation is added, so a one-literal clause is an implied
    /// comparison and a lemma's disjunctive premise is an implied clause.
    /// A literal the path already states, or one true on its face, needs
    /// no solver. A literal false on its face, or one whose negation the
    /// path already states, adds nothing to the contradiction; when no
    /// literal remains, only a path with no values at all could imply the
    /// clause, and a path is kept only while it has values.
    fn implies_clause(&self, path: &Path, clause: &[Comparison]) -> bool {
        let mut negated = Vec::new();
        for literal in clause {
            match constant_truth(literal) {
                Some(true) => return true,
                Some(false) => continue,
                None => {}
            }
            if path.comparisons.contains(literal) {
                return true;
            }
            // A literal's negation is its comparison complement; an operator
            // with no complement stands as itself, so a non-comparison
            // literal never gains a negation it does not have.
            let negation = compare(
                literal.operator.complement().unwrap_or(literal.operator),
                literal.left.clone(),
                literal.right.clone(),
            );
            if !path.comparisons.contains(&negation) {
                negated.push(negation);
            }
        }
        if negated.is_empty() {
            return false;
        }
        let mut comparisons = path.comparisons.clone();
        comparisons.extend(negated.iter().cloned());
        self.contradictory(&comparisons, &path.clauses, Some(&negated))
    }

    fn implies(&self, path: &Path, comparison: &Comparison) -> bool {
        self.implies_clause(path, std::slice::from_ref(comparison))
    }

    fn entails(&self, path: &Path, goal: &Boolean) -> bool {
        if self.impossible(path) {
            return true;
        }
        match goal {
            Boolean::Constant(value) => *value,
            Boolean::Comparison(comparison) => self.implies(path, comparison),
            Boolean::And(left, right) => self.entails(path, left) && self.entails(path, right),
            Boolean::Or(left, right) => self.entails(path, left) || self.entails(path, right),
            Boolean::Not(value) => self
                .assume_readonly(path, value, true)
                .is_some_and(|assumed| assumed.is_none_or(|path| self.impossible(&path))),
            Boolean::Call { .. } => false,
        }
    }

    /// `path` under `value == holds`: one path carrying the assumption's
    /// unit clauses as comparisons and its wider clauses deferred. `Some(None)`
    /// is an assumption that is false on its face; `None` is a value outside
    /// the comparison language, such as a predicate call.
    fn assume_readonly(&self, path: &Path, value: &Boolean, holds: bool) -> Option<Option<Path>> {
        let mut path = path.clone();
        for clause in clauses(value, holds)? {
            match clause.as_slice() {
                [] => return Some(None),
                [comparison] => path.comparisons.push(comparison.clone()),
                _ => path.clauses.push(clause),
            }
        }
        Some(Some(path))
    }

    fn assume(&mut self, path: Path, value: &Boolean, holds: bool) -> Option<Option<Path>> {
        self.spend()?;
        Some(
            self.assume_readonly(&path, value, holds)?
                .filter(|path| !self.impossible(path)),
        )
    }

    fn premise(
        &mut self,
        path: Path,
        value: &Boolean,
        holds: bool,
    ) -> Option<Vec<(Path, Vec<Boolean>)>> {
        self.spend()?;
        match value {
            Boolean::Call { .. } => Some(vec![(
                path,
                if holds {
                    vec![value.clone()]
                } else {
                    Vec::new()
                },
            )]),
            Boolean::Not(value) => self.premise(path, value, !holds),
            Boolean::And(left, right) | Boolean::Or(left, right) => {
                if matches!(value, Boolean::And(..)) == holds {
                    let mut result = Vec::new();
                    for (path, first) in self.premise(path, left, holds)? {
                        for (path, second) in self.premise(path, right, holds)? {
                            let mut combined = first.clone();
                            combined.extend(second);
                            result.push((path, combined));
                        }
                    }
                    Some(result)
                } else {
                    let mut result = self.premise(path.clone(), left, holds)?;
                    result.extend(self.premise(path, right, holds)?);
                    Some(result)
                }
            }
            _ => Some(
                self.assume(path, value, holds)?
                    .into_iter()
                    .map(|path| (path, Vec::new()))
                    .collect(),
            ),
        }
    }

    fn expression(
        &mut self,
        frame: &Frame,
        path: Path,
        expression: ExpressionHandle,
        depth: usize,
    ) -> Option<Vec<(Path, Value)>> {
        self.spend()?;
        if depth >= 64
            || !self
                .program
                .expression_table
                .expression_is_valid(expression)
        {
            return None;
        }
        let machine = &self.program.machines()[frame.machine];
        let state = &self.program.machine_states(machine)[frame.state];
        match self.program.expression_table.expression(expression) {
            ExpressionNode::Integer(literal) => Some(vec![(
                path,
                Value::Integer {
                    value: Integer::Integer(literal.value_bignum()?),
                    carrier: None,
                },
            )]),
            ExpressionNode::Boolean(value) => {
                Some(vec![(path, Value::Boolean(Boolean::Constant(*value)))])
            }
            ExpressionNode::Name(name) => {
                if self
                    .program
                    .expression_table
                    .name_path_members(name.members)
                    .len()
                    != 1
                    || name.symbol != name.head_symbol
                {
                    return None;
                }
                let value = frame
                    .bindings
                    .iter()
                    .rev()
                    .find(|(symbol, _)| *symbol == name.symbol)?
                    .1
                    .clone();
                Some(vec![(path, value)])
            }
            ExpressionNode::Borrow(borrow) if borrow.access.is_readable() => {
                self.expression(frame, path, borrow.target, depth + 1)
            }
            ExpressionNode::Member(member)
                if super::length_member(self.program, expression).is_some() =>
            {
                let values = self.expression(frame, path, member.receiver, depth + 1)?;
                values
                    .into_iter()
                    .map(|(path, value)| {
                        let Value::Collection { length, .. } = value else {
                            return None;
                        };
                        Some((
                            path,
                            Value::Integer {
                                value: length,
                                carrier: Some(self.u64_type()?),
                            },
                        ))
                    })
                    .collect()
            }
            ExpressionNode::Indexed(indexed) => {
                if !crate::value_custody::places::place_has_builtin_coordinates(
                    self.program,
                    machine,
                    Some(state),
                    expression,
                ) {
                    return None;
                }
                let mut result = Vec::new();
                for (path, collection) in
                    self.expression(frame, path, indexed.collection, depth + 1)?
                {
                    let Value::Collection {
                        identity,
                        length,
                        element,
                    } = collection
                    else {
                        return None;
                    };
                    for (path, selector) in
                        self.expression(frame, path, indexed.index, depth + 1)?
                    {
                        let Value::Integer {
                            value: selector, ..
                        } = selector
                        else {
                            return None;
                        };
                        if !self.implies(
                            &path,
                            &compare(BinaryOperator::GreaterOrEqual, selector.clone(), number(0)),
                        ) || !self.implies(
                            &path,
                            &compare(BinaryOperator::Less, selector.clone(), length.clone()),
                        ) {
                            return None;
                        }
                        result.extend(self.read(path, identity, selector, element, 0)?);
                    }
                }
                Some(result)
            }
            ExpressionNode::Unary(unary) => {
                let mut result = Vec::new();
                for (path, value) in self.expression(frame, path, unary.operand, depth + 1)? {
                    let value = match (unary.operator, value) {
                        (UnaryOperator::LogicalNot, Value::Boolean(value)) => {
                            Value::Boolean(Boolean::Not(Box::new(value)))
                        }
                        _ => return None,
                    };
                    result.push((path, value));
                }
                Some(result)
            }
            ExpressionNode::Binary(binary) => {
                if !has_builtin_binary_expression_meaning(
                    self.program,
                    machine,
                    Some(state),
                    expression,
                ) {
                    return None;
                }
                let mut result = Vec::new();
                for (path, left) in self.expression(frame, path, binary.left, depth + 1)? {
                    for (path, right) in self.expression(frame, path, binary.right, depth + 1)? {
                        let value = match (left.clone(), right) {
                            (
                                Value::Integer {
                                    value: left,
                                    carrier: left_carrier,
                                },
                                Value::Integer {
                                    value: right,
                                    carrier: right_carrier,
                                },
                            ) => match binary.operator {
                                BinaryOperator::Add
                                | BinaryOperator::Subtract
                                | BinaryOperator::Multiply => {
                                    let carrier = left_carrier.or(right_carrier)?;
                                    if self.program.arithmetic_domain_for_type_reference(carrier)
                                        != numerics::arithmetic::ArithmeticDomain::Exact
                                    {
                                        return None;
                                    }
                                    Value::Integer {
                                        value: Integer::IntegerOperation {
                                            operator: binary.operator,
                                            left: Box::new(left),
                                            right: Box::new(right),
                                        },
                                        carrier: Some(carrier),
                                    }
                                }
                                BinaryOperator::Equal
                                | BinaryOperator::NotEqual
                                | BinaryOperator::Less
                                | BinaryOperator::LessOrEqual
                                | BinaryOperator::Greater
                                | BinaryOperator::GreaterOrEqual => Value::Boolean(
                                    Boolean::Comparison(compare(binary.operator, left, right)),
                                ),
                                _ => return None,
                            },
                            (Value::Boolean(left), Value::Boolean(right)) => {
                                match binary.operator {
                                    BinaryOperator::And => Value::Boolean(Boolean::And(
                                        Box::new(left),
                                        Box::new(right),
                                    )),
                                    BinaryOperator::Or => {
                                        Value::Boolean(Boolean::Or(Box::new(left), Box::new(right)))
                                    }
                                    BinaryOperator::Equal | BinaryOperator::NotEqual => {
                                        let equal = Boolean::Or(
                                            Box::new(Boolean::And(
                                                Box::new(left.clone()),
                                                Box::new(right.clone()),
                                            )),
                                            Box::new(Boolean::And(
                                                Box::new(Boolean::Not(Box::new(left))),
                                                Box::new(Boolean::Not(Box::new(right))),
                                            )),
                                        );
                                        Value::Boolean(
                                            if binary.operator == BinaryOperator::Equal {
                                                equal
                                            } else {
                                                Boolean::Not(Box::new(equal))
                                            },
                                        )
                                    }
                                    _ => return None,
                                }
                            }
                            _ => return None,
                        };
                        result.push((path, value));
                    }
                }
                Some(result)
            }
            ExpressionNode::Call(call) if call.is_ordinary_positional_call() => {
                let machine = admitted_predicate_machine(self.program, call.target_symbol, true)?;
                let arguments = self
                    .program
                    .expression_table
                    .expression_handles(call.arguments);
                Some(
                    self.arguments(frame, path, arguments, depth + 1)?
                        .into_iter()
                        .map(|(path, arguments)| {
                            (path, Value::Boolean(Boolean::Call { machine, arguments }))
                        })
                        .collect(),
                )
            }
            _ => None,
        }
    }

    fn u64_type(&self) -> Option<TypeReferenceHandle> {
        self.program
            .type_reference_table
            .named_references()
            .find_map(|(reference, symbol, _)| {
                (self.program.symbols.builtin_type_atom(symbol)
                    == Some(symbols::BuiltinTypeAtom::U64))
                .then_some(reference)
            })
    }

    fn arguments(
        &mut self,
        frame: &Frame,
        path: Path,
        arguments: &[ExpressionHandle],
        depth: usize,
    ) -> Option<Vec<(Path, Vec<Value>)>> {
        let mut rows = vec![(path, Vec::new())];
        for argument in arguments {
            let mut next = Vec::new();
            for (path, values) in rows {
                for (path, value) in self.expression(frame, path, *argument, depth + 1)? {
                    let mut values = values.clone();
                    values.push(value);
                    next.push((path, values));
                }
            }
            rows = next;
        }
        Some(rows)
    }

    fn read(
        &mut self,
        path: Path,
        collection: usize,
        selector: Integer,
        element: TypeReferenceHandle,
        observation_position: usize,
    ) -> Option<Vec<(Path, Value)>> {
        self.spend()?;
        if let Some(update) = self
            .updates
            .iter()
            .find(|update| update.identity == collection)
        {
            let previous = update.previous;
            let stored_selector = update.selector.clone();
            let stored_value = update.value.clone();
            let equality = Boolean::Comparison(compare(
                BinaryOperator::Equal,
                selector.clone(),
                stored_selector,
            ));
            let mut result = self
                .assume(path.clone(), &equality, true)?
                .into_iter()
                .map(|path| Some((path, observed_carrier(stored_value.clone(), element)?)))
                .collect::<Option<Vec<_>>>()?;
            if let Some(path) = self.assume(path, &equality, false)? {
                result.extend(self.read(path, previous, selector.clone(), element, 0)?);
            }
            return Some(result);
        }
        let Some(observation) = path.observations.get(observation_position).cloned() else {
            let mut path = path;
            let value = self.integer(element, &mut path)?;
            path.observations.push(Observation {
                collection,
                selector,
                value: value.clone(),
            });
            return Some(vec![(path, value)]);
        };
        if observation.collection != collection {
            return self.read(
                path,
                collection,
                selector,
                element,
                observation_position + 1,
            );
        }
        let equality = compare(
            BinaryOperator::Equal,
            selector.clone(),
            observation.selector.clone(),
        );
        if self.implies(&path, &equality) {
            return Some(vec![(path, observed_carrier(observation.value, element)?)]);
        }
        if constant_truth(&equality) == Some(false)
            || self.implies(
                &path,
                &compare(
                    BinaryOperator::NotEqual,
                    selector.clone(),
                    observation.selector.clone(),
                ),
            )
        {
            return self.read(
                path,
                collection,
                selector,
                element,
                observation_position + 1,
            );
        }
        let mut equal_path = path.clone();
        equal_path.comparisons.push(equality);
        let mut different_path = path;
        different_path.comparisons.push(compare(
            BinaryOperator::NotEqual,
            selector.clone(),
            observation.selector,
        ));
        let mut result = vec![(equal_path, observed_carrier(observation.value, element)?)];
        result.extend(self.read(
            different_path,
            collection,
            selector,
            element,
            observation_position + 1,
        )?);
        Some(result)
    }

    fn activation(
        &mut self,
        machine: usize,
        state: usize,
        arguments: Vec<Value>,
        mut path: Path,
        goal: &Boolean,
        judgment: Judgment,
        depth: usize,
    ) -> Option<bool> {
        self.spend()?;
        if depth >= 64 {
            return None;
        }
        if judgment == Judgment::Consequence && self.entails(&path, goal) {
            return Some(true);
        }
        let owner = self.program.machines().get(machine)?;
        let target = self.program.machine_states(owner).get(state)?;
        let parameters = self.program.state_parameters(target);
        if parameters.len() != arguments.len()
            || parameters.iter().any(|parameter| {
                parameter.is_mutable
                    || value_shape(self.program, parameter.type_reference).is_none()
            })
        {
            return None;
        }
        let mut recurrent = false;
        for lemma_position in (0..self.lemmas.len()).rev() {
            let lemma = &self.lemmas[lemma_position];
            if lemma.machine != machine
                || lemma.state != state
                || lemma.judgment != judgment
                || lemma.goal != *goal
            {
                continue;
            }
            recurrent = true;
            let replacements = substitutions(self.program, &lemma.arguments, &arguments)?;
            if !lemma
                .premises
                .iter()
                .all(|premise| self.implies(&path, &substitute_comparison(premise, &replacements)))
                || !lemma.premise_clauses.iter().all(|premise| {
                    self.implies_clause(&path, &substitute_clause(premise, &replacements))
                })
            {
                continue;
            }
            let measures =
                lemma
                    .measures
                    .iter()
                    .copied()
                    .filter(|(position, direction)| {
                        let (
                            Value::Integer { value: before, .. },
                            Value::Integer { value: after, .. },
                        ) = (&lemma.arguments[*position], &arguments[*position])
                        else {
                            return false;
                        };
                        self.implies(&path, &compare(*direction, after.clone(), before.clone()))
                    })
                    .collect::<Vec<_>>();
            if measures.is_empty() {
                continue;
            }
            self.lemmas[lemma_position].measures = measures;
            return Some(true);
        }
        // A generic cyclic goal whose induction domain is not preserved needs
        // a stronger lemma, not repeated unrolling of a symbolic extent.
        if recurrent {
            return None;
        }
        // Keep a dependent transfer inside its enclosing induction scope.
        let dependent = arguments.iter().any(|argument| {
            let Value::Integer { value, .. } = argument else {
                return false;
            };
            self.lemmas.iter().any(|lemma| {
                lemma.arguments.iter().any(|formal| {
                    let Value::Integer { value: formal, .. } = formal else {
                        return false;
                    };
                    contains(value, formal)
                })
            })
        });
        let mut generalized = parameters
            .iter()
            .zip(&arguments)
            .map(|(parameter, argument)| {
                Some(
                    match (
                        value_shape(self.program, parameter.type_reference)?,
                        argument,
                    ) {
                        (ValueShape::Integer(carrier), Value::Integer { value, .. }) => {
                            Value::Integer {
                                value: value.clone(),
                                carrier: Some(carrier),
                            }
                        }
                        (
                            ValueShape::Collection { element_type, .. },
                            Value::Collection {
                                identity, length, ..
                            },
                        ) => Value::Collection {
                            identity: *identity,
                            length: length.clone(),
                            element: element_type,
                        },
                        (ValueShape::Boolean, Value::Boolean(value)) => {
                            Value::Boolean(value.clone())
                        }
                        _ => return None,
                    },
                )
            })
            .collect::<Option<Vec<_>>>()?;
        let mut replacements = Vec::new();
        let mut formal_bounds = Vec::new();
        if !dependent {
            for (position, (parameter, argument)) in parameters.iter().zip(&arguments).enumerate() {
                let Value::Integer { value, .. } = argument else {
                    continue;
                };
                let mut bounds = Path::default();
                let fresh = self.integer(parameter.type_reference, &mut bounds)?;
                let Value::Integer {
                    value: replacement, ..
                } = &fresh
                else {
                    return None;
                };
                replacements.push((value.clone(), replacement.clone()));
                generalized[position] = fresh;
                formal_bounds.extend(bounds.comparisons);
            }
            // Incoming observations retain their original coordinates. In
            // particular, a rank initially equal to a collection's extent is
            // not permission to rename that extent in unrelated store bounds.
            // Required results get only carrier bounds and the callee's
            // checked arrival requirements for fresh formals. A consequence
            // additionally needs the numeric domain covering its observations.
            if judgment == Judgment::Consequence {
                // A selected-cell consequence needs an induction domain that
                // still covers its selected coordinate. Keep the instantiated
                // numeric domain as well as the original observation facts.
                let mut support = Vec::new();
                boolean_atoms(goal, &mut support);
                for observation in &path.observations {
                    if let Value::Integer { value, .. } = &observation.value
                        && (matches!(goal, Boolean::Constant(false))
                            || support.iter().any(|atom| contains(value, atom)))
                    {
                        integer_atoms(&observation.selector, &mut support);
                    }
                }
                let mentions_support = |comparison: &Comparison| {
                    support.iter().any(|atom| {
                        contains(&comparison.left, atom) || contains(&comparison.right, atom)
                    })
                };
                // A premise the substitution leaves unchanged is already on
                // the path: a predicate with no integer formal would
                // otherwise double its selector facts at every state.
                let generalized_premises = path
                    .comparisons
                    .iter()
                    .filter(|comparison| mentions_support(comparison))
                    .map(|comparison| substitute_comparison(comparison, &replacements))
                    .filter(|comparison| !path.comparisons.contains(comparison))
                    .collect::<Vec<_>>();
                let generalized_clauses = path
                    .clauses
                    .iter()
                    .filter(|clause| clause.iter().any(mentions_support))
                    .map(|clause| substitute_clause(clause, &replacements))
                    .filter(|clause| !path.clauses.contains(clause))
                    .collect::<Vec<_>>();
                path.comparisons.extend(generalized_premises);
                path.clauses.extend(generalized_clauses);
            }
            path.comparisons.extend(formal_bounds);
        }
        let frame = Frame {
            machine,
            state,
            bindings: parameters
                .iter()
                .zip(&generalized)
                .map(|(parameter, value)| (parameter.symbol, value.clone()))
                .collect(),
        };
        let mut paths = vec![path];
        let machine_contracts = if state == 0 {
            self.program.machine_contracts(owner)
        } else {
            &[]
        };
        for requirement in self.program.contract_expressions(
            machine_contracts
                .iter()
                .chain(self.program.state_contracts(target).iter()),
            SignatureContractKind::Requires,
        ) {
            let mut next = Vec::new();
            for path in paths {
                for (path, value) in self.expression(&frame, path, requirement, 0)? {
                    let Value::Boolean(value) = value else {
                        return None;
                    };
                    next.extend(self.assume(path, &value, true)?);
                }
            }
            paths = next;
        }
        let mut result = true;
        for path in paths {
            if !dependent {
                let measures = generalized
                    .iter()
                    .enumerate()
                    .filter(|(_, value)| matches!(value, Value::Integer { .. }))
                    .flat_map(|(position, _)| {
                        [
                            (position, BinaryOperator::Less),
                            (position, BinaryOperator::Greater),
                        ]
                    })
                    .collect();
                self.lemmas.push(Lemma {
                    machine,
                    state,
                    arguments: generalized.clone(),
                    premises: path.comparisons.clone(),
                    premise_clauses: path.clauses.clone(),
                    measures,
                    judgment,
                    goal: goal.clone(),
                });
            }
            let proved = self.statements(&frame, path, goal, judgment, depth + 1);
            if !dependent {
                self.lemmas.pop();
            }
            result &= proved?;
            if !result {
                break;
            }
        }
        Some(result)
    }

    fn statements(
        &mut self,
        frame: &Frame,
        path: Path,
        goal: &Boolean,
        judgment: Judgment,
        depth: usize,
    ) -> Option<bool> {
        let machine = &self.program.machines()[frame.machine];
        let state = &self.program.machine_states(machine)[frame.state];
        let statements = self
            .program
            .statement_table
            .statements(state.statement_nodes);
        let mut pending = vec![(0usize, frame.clone(), path)];
        while let Some((statement_position, frame, path)) = pending.pop() {
            self.spend()?;
            if judgment == Judgment::Consequence && self.entails(&path, goal) {
                continue;
            }
            match statements.get(statement_position)? {
                StatementNode::LocalData(local)
                    if !local.is_mutable
                        && local.ordinary_value_initializer().is_some()
                        && local.relevance == language_core::BindingRelevance::Relevant =>
                {
                    for (path, value) in self.expression(&frame, path, local.initial_value, 0)? {
                        let mut frame = frame.clone();
                        frame.bindings.push((
                            local.symbol,
                            declared_value(self.program, value, local.type_reference)?,
                        ));
                        pending.push((statement_position + 1, frame, path));
                    }
                }
                StatementNode::Transition(transition)
                    if transition.exit == TransitionExit::Ordinary
                        && !transition.continuation.is_valid() =>
                {
                    let guards = match transition.guard {
                        TransitionGuardNode::Always => {
                            vec![(path, Value::Boolean(Boolean::Constant(true)))]
                        }
                        TransitionGuardNode::When(guard) => {
                            self.expression(&frame, path, guard, 0)?
                        }
                    };
                    for (path, guard) in guards {
                        let Value::Boolean(guard) = guard else {
                            return None;
                        };
                        // Explore both guard outcomes. A required false result
                        // owes a contradiction with the original predicate;
                        // eagerly proving every guard repeats whole induction
                        // searches even where both branches return true.
                        if let Some(path) = self.assume(path.clone(), &guard, false)? {
                            pending.push((statement_position + 1, frame.clone(), path));
                        }
                        if let Some(path) = self.assume(path, &guard, true)? {
                            match self
                                .program
                                .statement_table
                                .transition_target(transition.target)
                            {
                                TransitionTargetNode::Value(value) => {
                                    if !self.result(
                                        &frame,
                                        path,
                                        *value,
                                        goal,
                                        judgment,
                                        depth + 1,
                                    )? {
                                        return Some(false);
                                    }
                                }
                                TransitionTargetNode::Named {
                                    path: target,
                                    arguments,
                                    static_machine_parameter,
                                    evidence_arguments,
                                    ..
                                } if !static_machine_parameter.is_valid()
                                    && evidence_arguments.is_empty() =>
                                {
                                    let (machine, state) = if let Some(state) =
                                        typed_trees::state::state_position(
                                            self.program.machine_states(machine),
                                            target.symbol,
                                        )
                                        .filter(|position| *position != 0)
                                    {
                                        (frame.machine, state)
                                    } else {
                                        (
                                            admitted_predicate_machine(
                                                self.program,
                                                target.symbol,
                                                true,
                                            )?,
                                            0,
                                        )
                                    };
                                    for (path, arguments) in self.arguments(
                                        &frame,
                                        path,
                                        self.program.statement_table.expression_handles(*arguments),
                                        0,
                                    )? {
                                        if !self.activation(
                                            machine,
                                            state,
                                            arguments,
                                            path,
                                            goal,
                                            judgment,
                                            depth + 1,
                                        )? {
                                            return Some(false);
                                        }
                                    }
                                }
                                _ => return None,
                            }
                        }
                    }
                }
                StatementNode::Expression(value) if statement_position + 1 == statements.len() => {
                    if !self.result(&frame, path, *value, goal, judgment, depth + 1)? {
                        return Some(false);
                    }
                }
                _ => return None,
            }
        }
        Some(true)
    }

    fn result(
        &mut self,
        frame: &Frame,
        path: Path,
        expression: ExpressionHandle,
        goal: &Boolean,
        judgment: Judgment,
        depth: usize,
    ) -> Option<bool> {
        for (path, value) in self.expression(frame, path, expression, 0)? {
            let Value::Boolean(value) = value else {
                return None;
            };
            if judgment == Judgment::RequiredResult {
                if self.establish(&path, &value, depth + 1) != Some(true) {
                    return Some(false);
                }
                continue;
            }
            if let Boolean::Call { machine, arguments } = value {
                if !self.activation(machine, 0, arguments, path, goal, judgment, depth + 1)? {
                    return Some(false);
                }
            } else if let Some(path) = self.assume(path, &value, true)?
                && !self.entails(&path, goal)
            {
                return Some(false);
            }
        }
        Some(true)
    }
}

fn number(value: i128) -> Integer {
    Integer::Integer(BigInt::from_i128(value))
}

fn compare(operator: BinaryOperator, left: Integer, right: Integer) -> Comparison {
    Comparison {
        operator,
        left,
        right,
    }
}

fn contains(value: &Integer, sought: &Integer) -> bool {
    value == sought
        || matches!(value, Integer::IntegerOperation { left, right, .. } if contains(left, sought) || contains(right, sought))
}

fn integer_atoms(value: &Integer, atoms: &mut Vec<Integer>) {
    match value {
        Integer::Atom { .. } => {
            if !atoms.contains(value) {
                atoms.push(value.clone());
            }
        }
        Integer::IntegerOperation { left, right, .. } => {
            integer_atoms(left, atoms);
            integer_atoms(right, atoms);
        }
        _ => {}
    }
}

fn boolean_atoms(value: &Boolean, atoms: &mut Vec<Integer>) {
    match value {
        Boolean::Comparison(comparison) => {
            integer_atoms(&comparison.left, atoms);
            integer_atoms(&comparison.right, atoms);
        }
        Boolean::And(left, right) | Boolean::Or(left, right) => {
            boolean_atoms(left, atoms);
            boolean_atoms(right, atoms);
        }
        Boolean::Not(value) => boolean_atoms(value, atoms),
        _ => {}
    }
}

fn substitute(value: &Integer, replacements: &[(Integer, Integer)]) -> Integer {
    if let Some((_, replacement)) = replacements.iter().find(|(original, _)| original == value) {
        return replacement.clone();
    }
    match value {
        Integer::IntegerOperation {
            operator,
            left,
            right,
        } => Integer::IntegerOperation {
            operator: *operator,
            left: Box::new(substitute(left, replacements)),
            right: Box::new(substitute(right, replacements)),
        },
        _ => value.clone(),
    }
}

fn substitute_comparison(
    comparison: &Comparison,
    replacements: &[(Integer, Integer)],
) -> Comparison {
    compare(
        comparison.operator,
        substitute(&comparison.left, replacements),
        substitute(&comparison.right, replacements),
    )
}

fn substitute_clause(clause: &[Comparison], replacements: &[(Integer, Integer)]) -> Clause {
    clause
        .iter()
        .map(|comparison| substitute_comparison(comparison, replacements))
        .collect()
}

/// Solver consultations one contradiction decision may spend splitting
/// clauses before it declines to decide.
const SPLIT_CONSULTATIONS: usize = 256;
/// Clauses one assumption may expand to; a wider distribution is declined.
const MAXIMUM_CLAUSES: usize = 64;

/// `value == holds` in conjunctive normal form over comparisons. No clause
/// is `true`; an empty clause is `false`. A predicate call has no
/// comparison form.
fn clauses(value: &Boolean, holds: bool) -> Option<Vec<Clause>> {
    match value {
        Boolean::Constant(value) => Some(if *value == holds {
            Vec::new()
        } else {
            vec![Vec::new()]
        }),
        Boolean::Comparison(comparison) => Some(vec![vec![compare(
            if holds {
                comparison.operator
            } else {
                comparison
                    .operator
                    .complement()
                    .unwrap_or(comparison.operator)
            },
            comparison.left.clone(),
            comparison.right.clone(),
        )]]),
        Boolean::Not(value) => clauses(value, !holds),
        Boolean::And(left, right) | Boolean::Or(left, right) => {
            let mut left = clauses(left, holds)?;
            let right = clauses(right, holds)?;
            if matches!(value, Boolean::And(..)) == holds {
                left.extend(right);
                return Some(left);
            }
            if left.len().saturating_mul(right.len()) > MAXIMUM_CLAUSES {
                return None;
            }
            Some(
                left.iter()
                    .flat_map(|first| {
                        right.iter().map(|second| {
                            let mut clause = first.clone();
                            clause.extend(second.iter().cloned());
                            clause
                        })
                    })
                    .collect(),
            )
        }
        Boolean::Call { .. } => None,
    }
}

/// The truth of a comparison between two literal integers; `None` when a
/// side is symbolic.
fn constant_truth(comparison: &Comparison) -> Option<bool> {
    let (Integer::Integer(left), Integer::Integer(right)) = (&comparison.left, &comparison.right)
    else {
        return None;
    };
    Some(match comparison.operator {
        BinaryOperator::Equal => left == right,
        BinaryOperator::NotEqual => left != right,
        BinaryOperator::Less => left < right,
        BinaryOperator::LessOrEqual => left <= right,
        BinaryOperator::Greater => left > right,
        BinaryOperator::GreaterOrEqual => left >= right,
        _ => return None,
    })
}

fn comparison_atoms(comparison: &Comparison, atoms: &mut Vec<Integer>) {
    integer_atoms(&comparison.left, atoms);
    integer_atoms(&comparison.right, atoms);
}

/// The comparisons and clauses sharing at least one atom, transitively.
struct Component {
    atoms: Vec<Integer>,
    comparisons: Vec<Comparison>,
    clauses: Vec<Clause>,
}

/// Partition `comparisons` and `clauses` by shared atoms. A comparison over
/// constants alone joins no component; the whole conjunction decides it.
fn components(comparisons: &[Comparison], clauses: &[Clause]) -> Vec<Component> {
    let mut components: Vec<Component> = Vec::new();
    let mut place =
        |atoms: Vec<Integer>, comparison: Option<&Comparison>, clause: Option<&Clause>| {
            let mut joined = Component {
                atoms,
                comparisons: comparison.cloned().into_iter().collect(),
                clauses: clause.cloned().into_iter().collect(),
            };
            let mut position = 0;
            while position < components.len() {
                if components[position]
                    .atoms
                    .iter()
                    .any(|atom| joined.atoms.contains(atom))
                {
                    let absorbed = components.swap_remove(position);
                    for atom in absorbed.atoms {
                        if !joined.atoms.contains(&atom) {
                            joined.atoms.push(atom);
                        }
                    }
                    joined.comparisons.extend(absorbed.comparisons);
                    joined.clauses.extend(absorbed.clauses);
                } else {
                    position += 1;
                }
            }
            components.push(joined);
        };
    for comparison in comparisons {
        let mut atoms = Vec::new();
        comparison_atoms(comparison, &mut atoms);
        if !atoms.is_empty() {
            place(atoms, Some(comparison), None);
        }
    }
    for clause in clauses {
        let mut atoms = Vec::new();
        for comparison in clause {
            comparison_atoms(comparison, &mut atoms);
        }
        place(atoms, None, Some(clause));
    }
    components
}

fn declared_value(
    program: &dyn crate::CheckingInput,
    value: Value,
    reference: TypeReferenceHandle,
) -> Option<Value> {
    Some(match (readable_shape(program, reference)?, value) {
        (ValueShape::Integer(carrier), Value::Integer { value, .. }) => Value::Integer {
            value,
            carrier: Some(carrier),
        },
        (
            ValueShape::Collection { element_type, .. },
            Value::Collection {
                identity, length, ..
            },
        ) => Value::Collection {
            identity,
            length,
            element: element_type,
        },
        (ValueShape::Boolean, Value::Boolean(value)) => Value::Boolean(value),
        _ => return None,
    })
}

fn readable_shape(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<ValueShape> {
    let mut reference = reference;
    for _ in 0..8 {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference {
                referee, access, ..
            } if access.is_readable() => {
                reference = *referee;
            }
            _ => return value_shape(program, reference),
        }
    }
    None
}

fn observed_carrier(value: Value, carrier: TypeReferenceHandle) -> Option<Value> {
    let Value::Integer { value, .. } = value else {
        return None;
    };
    Some(Value::Integer {
        value,
        carrier: Some(carrier),
    })
}

fn substitutions(
    program: &dyn crate::CheckingInput,
    formals: &[Value],
    arguments: &[Value],
) -> Option<Vec<(Integer, Integer)>> {
    let mut replacements = Vec::new();
    for (formal, argument) in formals.iter().zip(arguments) {
        match (formal, argument) {
            (
                Value::Integer { value: formal, .. },
                Value::Integer {
                    value: argument, ..
                },
            ) => replacements.push((formal.clone(), argument.clone())),
            (
                Value::Collection {
                    identity: first,
                    length: first_length,
                    element: first_element,
                },
                Value::Collection {
                    identity: second,
                    length: second_length,
                    element: second_element,
                },
            ) if first == second
                && first_length == second_length
                && program.primitive_type_reference(*first_element)
                    == program.primitive_type_reference(*second_element) => {}
            _ => return None,
        }
    }
    Some(replacements)
}
