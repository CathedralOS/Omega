/*
The contract family of the reader: `requires`, `ensures` and `crashes` clauses
read into propositions over the formals, and the identity of the reserved
`result`. A data declaration's `requires` is read by the same clause reader,
with `self` and its fields as the names in scope (`ClauseScope::subject`).
*/

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerMathLiteral, IntegerMathTerm, MachineId, PlaceId,
    Proposition, ScalarTerm, ScalarType, StructuralCaseSubject, StructuralTypeId, ValueId,
};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
use typed_trees::closed_numeric;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

use super::places::Member;
use super::{Program, Slot};
use crate::body::ValueType;
use crate::canonical::{self, Repeated};
use crate::{ClauseText, Piece, Unserved, refuse};

/// What a contract clause may name: the formals, and the result pseudo-value
/// only in a normal-return guarantee of a scalar machine.
#[derive(Clone, Copy)]
pub(super) struct ClauseScope<'s> {
    pub(super) slots: &'s [Slot],
    /// Each formal's value identity, by slot (`body::formal_values`);
    /// `None` for a record formal.
    pub(super) values: &'s [Option<ValueId>],
    /// The value parameters this reading of the clause has bound, each to
    /// its literal: a requirement read at the static arguments of one call
    /// (`requirement_signature`). A name that resolves to one is that
    /// literal.
    pub(super) bound: &'s [(SymbolHandle, ScalarTerm)],
    /// The callable whose reserved `result` this clause may name (the
    /// machine's one state; see `TableNamePath::reserved_result_of`).
    pub(super) owner: SymbolHandle,
    pub(super) result: ResultTerm,
    /// Set for a clause of a data declaration's `requires`: the type whose
    /// value `self` is. Such a clause may also test a case and be a
    /// disjunction, because it never enters the module's contracts: the
    /// checker replaces its names by values before it is a goal.
    pub(super) subject: Option<DataSubject>,
    /// Whether a term may project a record formal (`p.first`): so for an
    /// entry `requires` clause, whose term is then the field of the
    /// formal's place, about what that place holds at entry
    /// (fact_validity.md: "Entry hypotheses bind exact invocation formals
    /// or structural entry paths"). A crash route guard and an `ensures`
    /// clause may not, yet. The slice that lets them must settle what a
    /// field of a `mut` record formal means there first: the body stores
    /// into that formal's own place, so the place no longer holds what the
    /// caller passed.
    pub(super) formal_fields: bool,
    /// Whether a comparison in this clause may be between mathematical
    /// integers (`embed(a) + embed(b) <= 99`).
    pub(super) mathematical: MathematicalTerms,
}

/// Whether a clause may compare mathematical integers. A machine's
/// `requires` and `ensures` may: the checker states either over a call's or
/// a return's values by the one substitution it has for every clause. A
/// crash route guard and a data `requires` may not yet, because each is
/// restated by a substitution of its own that knows only scalar comparisons
/// (`canonical::substitute`, and the checker's data-domain goal).
#[derive(Clone, Copy)]
pub(super) enum MathematicalTerms {
    Served,
    /// The refusal that names the clause kind.
    Refused(&'static str),
}

/// The value a data declaration's `requires` is about.
#[derive(Clone, Copy)]
pub(super) struct DataSubject {
    pub(super) record: StructuralTypeId,
    /// The symbol stage 02 gave the clause's `self`; invalid when the
    /// declaration never names it.
    pub(super) symbol: SymbolHandle,
    /// The root that stands for `self` in the clause (`body::self_place`).
    pub(super) root: PlaceId,
}

#[derive(Clone, Copy)]
pub(super) enum ResultTerm {
    /// The result pseudo-value's identity and type.
    Named(ValueId, ScalarType),
    /// Why this clause cannot name `result`.
    Absent(&'static str),
}

/// The mathematical integer `magnitude`, negated when `negative`; zero has
/// no sign (`IntegerMathLiteral::new` refuses a negative zero).
fn mathematical_literal(negative: bool, magnitude: u64) -> Option<IntegerMathLiteral> {
    IntegerMathLiteral::new(negative && magnitude != 0, u128::from(magnitude)).ok()
}

/// The place of the formal in `slot`: lowering mints the formals that are
/// places first, in position order (a record formal, and a borrowed formal
/// of any referent), so the `k`-th of them is place `k + 1`
/// (`Signature::is_place`).
fn formal_place(id: MachineId, slots: &[Slot], slot: usize) -> Checked<PlaceId> {
    let earlier = slots[..slot]
        .iter()
        .filter(|formal| {
            formal.borrowed.is_some() || matches!(formal.value_type, ValueType::Record(_))
        })
        .count();
    PlaceId::new(earlier as u64 + 1).ok_or_else(|| {
        refuse(
            id,
            Construct::Machine,
            Unserved::Contract("formal place identity"),
        )
    })
}

impl Program<'_> {
    // Contract clauses: a comparison of formals, literals and Booleans, or a
    // conjunction of those. Terms name formal `i` as value `i + 1`.

    /// A contract fact that is an expression; memberships and named
    /// propositions are outside the subset.
    pub(super) fn fact(&self, id: MachineId, fact: &ProofFact) -> Checked<ExpressionHandle> {
        match fact {
            ProofFact::Expression(expression) => Ok(*expression),
            ProofFact::Membership(_) => Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("domain membership"),
            )),
            ProofFact::Proposition(_) => Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("named proposition"),
            )),
        }
    }

    /// The disjuncts of an `||` chain, left to right.
    fn disjuncts(&self, expression: ExpressionHandle, out: &mut Vec<ExpressionHandle>) {
        if expression.is_valid()
            && let ExpressionNode::Binary(binary) =
                self.typed.expression_table.expression(expression)
            && binary.operator == BinaryOperator::Or
        {
            self.disjuncts(binary.left, out);
            self.disjuncts(binary.right, out);
        } else {
            out.push(expression);
        }
    }

    pub(super) fn clause(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<Proposition> {
        let typed = self.typed;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if !expression.is_valid() {
            return unserved("missing clause expression");
        }
        let ExpressionNode::Binary(binary) = typed.expression_table.expression(expression) else {
            return unserved("clause that is not a comparison or conjunction");
        };
        let operator = binary.operator;
        // A machine clause over a type equation the machine being read
        // decides: a conjunct decided true states nothing and the rest is
        // the clause, and a disjunct decided false contributes nothing and
        // is not read (`guarded_false`). What is left must still be a clause
        // this reader serves; a clause decided false outright is refused.
        if scope.subject.is_none() && operator == BinaryOperator::And {
            if self.guarded_false(expression) {
                return unserved("conjunction that type equations decide false");
            }
            if self.static_truth(binary.left) == Some(true) {
                return self.clause(id, scope, binary.right);
            }
            if self.static_truth(binary.right) == Some(true) {
                return self.clause(id, scope, binary.left);
            }
        }
        if scope.subject.is_none() && operator == BinaryOperator::Or {
            let mut disjuncts = Vec::new();
            self.disjuncts(expression, &mut disjuncts);
            if disjuncts
                .iter()
                .any(|disjunct| self.guarded_false(*disjunct))
            {
                let open: Vec<_> = disjuncts
                    .into_iter()
                    .filter(|disjunct| !self.guarded_false(*disjunct))
                    .collect();
                return match open[..] {
                    [only] => self.clause(id, scope, only),
                    [] => unserved("disjunction that type equations decide false"),
                    _ => unserved("disjunction"),
                };
            }
        }
        // `self in S::C`, in a data clause only. Stage 01 writes a case test
        // as `CaseMembership` or as `Equal` against a case name; in both the
        // right operand names a case, which is what makes it one.
        if matches!(
            operator,
            BinaryOperator::CaseMembership | BinaryOperator::Equal
        ) && let Some(case) = self.named_case(binary.right)
        {
            if scope.subject.is_none() {
                return unserved("case membership");
            }
            let Some((root, path, ValueType::Record(sum))) =
                self.projected(id, scope, binary.left)?
            else {
                return unserved("case test of something that is not a sum");
            };
            let case = self.case_position(id, sum, case)?;
            let case = self.data_type(id, sum)?.cases()[case].id;
            return Ok(Proposition::StructuralCaseMembership {
                subject: StructuralCaseSubject::new(root, path),
                case,
            });
        }
        let comparison = match operator {
            BinaryOperator::And => {
                let mut conjuncts = Vec::new();
                for side in [binary.left, binary.right] {
                    match self.clause(id, scope, side)? {
                        Proposition::Conjunction(inner) => conjuncts.extend(inner),
                        other => conjuncts.push(other),
                    }
                }
                // A data clause keeps its authored order: it is never
                // encoded, and a conjunct that tests the case guards the
                // conjuncts that project its payload.
                if scope.subject.is_some() {
                    return Ok(Proposition::Conjunction(conjuncts));
                }
                return canonical::clauses(conjuncts, Repeated::Refuse, id)
                    .map(Proposition::Conjunction);
            }
            BinaryOperator::Or if scope.subject.is_some() => {
                let mut disjuncts = Vec::new();
                for side in [binary.left, binary.right] {
                    match self.clause(id, scope, side)? {
                        Proposition::Disjunction(inner) => disjuncts.extend(inner),
                        other => disjuncts.push(other),
                    }
                }
                return Ok(Proposition::Disjunction(disjuncts));
            }
            BinaryOperator::Equal
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::NotEqual => operator,
            BinaryOperator::Or => return unserved("disjunction"),
            BinaryOperator::Add
            | BinaryOperator::Subtract
            | BinaryOperator::Multiply
            | BinaryOperator::Divide
            | BinaryOperator::Modulo
            | BinaryOperator::ShiftLeft
            | BinaryOperator::ShiftRight
            | BinaryOperator::BitwiseAnd
            | BinaryOperator::BitwiseOr
            | BinaryOperator::BitwiseXor => return unserved("arithmetic clause"),
            BinaryOperator::CaseMembership => return unserved("case membership"),
        };
        // A comparison with an `embed` on either side is between
        // mathematical integers, which no carrier bounds.
        if self.mathematical(binary.left) || self.mathematical(binary.right) {
            return self.mathematical_comparison(id, scope, comparison, binary.left, binary.right);
        }
        // A wholly anonymous operand lands in its partner's type.
        let ((left, left_type), (right, right_type)) = if self.wholly_anonymous(binary.left) {
            let right = self.term(id, scope, binary.right, None)?;
            (self.term(id, scope, binary.left, Some(right.1))?, right)
        } else {
            let left = self.term(id, scope, binary.left, None)?;
            let right = self.term(id, scope, binary.right, Some(left.1))?;
            (left, right)
        };
        if left_type != right_type {
            return Err(refuse(
                id,
                Construct::Machine,
                Unserved::Typing("clause operand types differ"),
            ));
        }
        let ordered = matches!(left_type, ScalarType::Integer(_));
        match comparison {
            BinaryOperator::Equal if scope.subject.is_some() => Ok(Proposition::Equal(left, right)),
            BinaryOperator::Equal => canonical::equal(left, right, id),
            _ if !ordered => unserved("ordering of Booleans"),
            BinaryOperator::Less => Ok(Proposition::LessThan(left, right)),
            BinaryOperator::LessOrEqual => Ok(Proposition::LessOrEqual(left, right)),
            BinaryOperator::Greater => Ok(Proposition::LessThan(right, left)),
            BinaryOperator::GreaterOrEqual => Ok(Proposition::LessOrEqual(right, left)),
            // There is no negated proposition. An integer `a != b` is the
            // disjunction the kernel denotes a false integer equality by
            // (`predicate_denotation`: `a < b or b < a`), so a requirement
            // and a guard of the same disequality state the same fact.
            BinaryOperator::NotEqual => canonical::clauses(
                vec![
                    Proposition::LessThan(left.clone(), right.clone()),
                    Proposition::LessThan(right, left),
                ],
                Repeated::Refuse,
                id,
            )
            .map(Proposition::Disjunction),
            _ => unserved("comparison"),
        }
    }

    /// A clause's authored text, each formal left open by position, for
    /// diagnostics (`names`). Rendered as `ExpressionTable::display_name`
    /// renders the clause, so a message reads as the current checker's.
    pub(super) fn clause_text(&self, slots: &[Slot], expression: ExpressionHandle) -> ClauseText {
        let mut text = ClauseText::default();
        self.write_clause_text(slots, expression, &mut text);
        text
    }

    fn write_clause_text(
        &self,
        slots: &[Slot],
        expression: ExpressionHandle,
        text: &mut ClauseText,
    ) {
        let table = &self.typed.expression_table;
        if !expression.is_valid() {
            return;
        }
        match table.expression(expression) {
            ExpressionNode::Binary(binary) => {
                self.write_clause_text(slots, binary.left, text);
                text.push_text(" ");
                text.push_text(binary.operator.display_name());
                text.push_text(" ");
                self.write_clause_text(slots, binary.right, text);
            }
            ExpressionNode::Unary(unary) => {
                text.push_text(unary.operator.display_name());
                self.write_clause_text(slots, unary.operand, text);
            }
            ExpressionNode::Member(member) => {
                self.write_clause_text(slots, member.receiver, text);
                text.push_text(&language_core::PLACE_MEMBER_SEPARATOR.to_string());
                text.push_text(member.member.as_str());
            }
            // `embed(rest)`: the formal inside stays open, so a call's
            // statement of the clause names its own argument there.
            ExpressionNode::Call(call) if self.embeds(call) => {
                text.push_text(call.target.as_str());
                text.push_text("(");
                for argument in table.expression_handles(call.arguments) {
                    self.write_clause_text(slots, *argument, text);
                }
                text.push_text(")");
            }
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && !path.reserved_result_of.is_valid()
                    && table.name_path_members(path.members).len() == 1 =>
            {
                match slots.iter().position(|slot| slot.symbol == path.symbol) {
                    Some(position) => text.0.push(Piece::Formal(position)),
                    None => text.push_text(&table.display_name(expression)),
                }
            }
            _ => text.push_text(&table.display_name(expression)),
        }
    }

    pub(super) fn term(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
        expected: Option<ScalarType>,
    ) -> Checked<(ScalarTerm, ScalarType)> {
        let typed = self.typed;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if !expression.is_valid() {
            return unserved("missing term");
        }
        match typed.expression_table.expression(expression) {
            ExpressionNode::Name(path) if path.static_application.is_valid() => {
                unserved("applied name used as a plain scalar term")
            }
            // The reserved result, by the owner identity stage 02 recorded,
            // never by its spelling. calls_and_outcomes.md: "The reserved
            // result occurrence belongs to its contract owner, not any local
            // or parameter spelled `result`."
            ExpressionNode::Name(path) if path.reserved_result_of.is_valid() => {
                // A machine stage 04 generated for a static application
                // keeps its declaration's clause trees, whose result is
                // recorded against that declaration.
                let declared = typed.symbols.get(scope.owner).generated_from;
                if path.reserved_result_of != scope.owner
                    && !(declared.is_valid() && path.reserved_result_of == declared)
                {
                    return unserved("result of another callable");
                }
                match scope.result {
                    ResultTerm::Named(id, scalar_type) => {
                        Ok((ScalarTerm::Value { id, scalar_type }, scalar_type))
                    }
                    ResultTerm::Absent(why) => unserved(why),
                }
            }
            ExpressionNode::Name(_) if scope.subject.is_some() => {
                self.field_term(id, scope, expression)
            }
            ExpressionNode::Name(path)
                if path.symbol.is_valid()
                    && scope.bound.iter().any(|(symbol, _)| *symbol == path.symbol) =>
            {
                if typed.expression_table.name_path_members(path.members).len() != 1 {
                    return unserved("projected value parameter");
                }
                let literal = scope
                    .bound
                    .iter()
                    .find(|(symbol, _)| *symbol == path.symbol)
                    .map(|(_, literal)| literal.clone())
                    .ok_or_else(|| {
                        refuse(
                            id,
                            Construct::Machine,
                            Unserved::Contract("value parameter binding"),
                        )
                    })?;
                let scalar_type = literal.scalar_type();
                Ok((literal, scalar_type))
            }
            ExpressionNode::Name(path) => {
                let slot = self.slot_of(id, scope.slots, path)?;
                // A borrowed scalar formal is a place too: an entry clause
                // names what its referent holds at entry, as it names a
                // field of a record formal. A guarantee would name what it
                // holds at the return, a version no slice serves yet.
                if scope.slots[slot].borrowed.is_some() {
                    if !scope.formal_fields {
                        return unserved("clause that names a borrowed formal outside requires");
                    }
                    let ValueType::Scalar(scalar_type) = scope.slots[slot].value_type else {
                        return unserved("record formal named whole in a clause");
                    };
                    let root = formal_place(id, scope.slots, slot)?;
                    return match scalar_type {
                        ScalarType::Integer(integer) => Ok((
                            ScalarTerm::IntegerField {
                                root,
                                path: Vec::new(),
                                scalar_type: integer,
                            },
                            scalar_type,
                        )),
                        ScalarType::Boolean => Ok((
                            ScalarTerm::BooleanField {
                                root,
                                path: Vec::new(),
                            },
                            scalar_type,
                        )),
                        ScalarType::IeeeFloat(_) => unserved("float field in a clause"),
                    };
                }
                // A record formal is a place, not a value: a clause can only
                // project it, which the member arm below refuses.
                // A policy-qualified formal is named as its stored value: a
                // comparison of it compares that value whatever the policy.
                let (ValueType::Scalar(scalar_type) | ValueType::Qualified(scalar_type, _)) =
                    scope.slots[slot].value_type
                else {
                    return unserved("record formal named whole in a clause");
                };
                // A `mut` formal names two things by the end of the body:
                // the argument the caller passed and whatever the body
                // last assigned. An entry `requires` can only mean the
                // first. Which one a guarantee or a crash route guard
                // means the specification does not say
                // (`OWNER_QUESTIONS.md`, decision
                // `mutable-parameter-contract-name`), so such a clause is
                // refused rather than read as either.
                if scope.slots[slot].assignable && !scope.formal_fields {
                    return unserved("clause that names a mutable parameter outside requires");
                }
                let value = scope.values.get(slot).copied().flatten().ok_or_else(|| {
                    refuse(
                        id,
                        Construct::Machine,
                        Unserved::Contract("formal identity"),
                    )
                })?;
                Ok((
                    ScalarTerm::Value {
                        id: value,
                        scalar_type,
                    },
                    scalar_type,
                ))
            }
            // `self.value`, `self.from.x`: a field of the value a data
            // clause is about.
            ExpressionNode::Member(_) if scope.subject.is_some() => {
                self.field_term(id, scope, expression)
            }
            // `p.first == 7` in an entry `requires`: the field of a record
            // formal's place.
            ExpressionNode::Member(_) if scope.formal_fields => {
                self.field_term(id, scope, expression)
            }
            // `result.count <= 5`, and a formal's field in any other
            // clause: a term over the result root or over entry content
            // that no slice serves yet.
            ExpressionNode::Member(_) => unserved("clause that projects a formal or result"),
            ExpressionNode::Integer(literal) => {
                let (value, integer) = self.literal(id, literal, expected)?;
                Ok((
                    ScalarTerm::Integer {
                        scalar_type: integer,
                        value,
                    },
                    ScalarType::Integer(integer),
                ))
            }
            ExpressionNode::Binary(_) if self.wholly_anonymous(expression) => {
                let (value, integer) = self.anonymous_value(id, expression, expected)?;
                Ok((
                    ScalarTerm::Integer {
                        scalar_type: integer,
                        value,
                    },
                    ScalarType::Integer(integer),
                ))
            }
            ExpressionNode::Boolean(value) => {
                Ok((ScalarTerm::Boolean(*value), ScalarType::Boolean))
            }
            // Carrier arithmetic computes, so it owes its formation
            // condition before the clause can use it (contracts.md, "Total
            // arithmetic": "Exact operations discharge representability
            // before formation", and a suffixed operator "whose selected
            // meaning can transfer control cannot form a proof term"). No
            // slice states that obligation yet.
            ExpressionNode::Binary(binary)
                if binary.policy == numerics::arithmetic::ArithmeticDomain::Exact =>
            {
                unserved("carrier arithmetic in a clause term")
            }
            ExpressionNode::Binary(_) => unserved("policy operator in a clause term"),
            ExpressionNode::Cast(_) => unserved("cast in a clause term"),
            // An `embed` is a mathematical integer, which only a
            // mathematical comparison holds (`mathematical_term`).
            ExpressionNode::Call(call) if self.embeds(call) => {
                unserved("embed outside a mathematical comparison")
            }
            ExpressionNode::Call(_) => unserved("call in a clause term"),
            _ => unserved("term other than a formal, a literal or an embedded integer"),
        }
    }

    /// Whether `call` is the builtin integer embedding applied to one value:
    /// decided by the symbol stage 02 resolved the callee to, never by its
    /// spelling, so a machine a program names `embed` is not one.
    fn embeds(&self, call: &typed_trees::expression::TableCallExpression) -> bool {
        !call.receiver.is_valid()
            && call.machine_arguments.is_empty()
            && call.evidence_arguments.is_empty()
            && call.target_symbol.is_valid()
            && self
                .typed
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                == Some(symbols::BuiltinFunction::IntegerEmbed)
    }

    /// Whether `expression` holds an `embed` beneath binary operators only:
    /// what makes the comparison it stands in one between mathematical
    /// integers. An `embed` inside a call's or a cast's operand does not; that
    /// operand is the call's or the cast's to read.
    fn mathematical(&self, expression: ExpressionHandle) -> bool {
        let table = &self.typed.expression_table;
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            if !expression.is_valid() {
                continue;
            }
            match table.expression(expression) {
                ExpressionNode::Call(call) if self.embeds(call) => return true,
                ExpressionNode::Binary(binary) => pending.extend([binary.left, binary.right]),
                _ => {}
            }
        }
        false
    }

    /// A comparison between mathematical integers, as the proposition
    /// Terminal Psi keeps for one (`IntegerMathEqual`, `IntegerMathLessThan`,
    /// `IntegerMathLessOrEqual`). `>` and `>=` are the same order read from
    /// the other side, and `!=` is `a < b or b < a`, as for carrier integers.
    fn mathematical_comparison(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        comparison: BinaryOperator,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> Checked<Proposition> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if let MathematicalTerms::Refused(why) = scope.mathematical {
            return unserved(why);
        }
        let left = self.mathematical_term(id, scope, left)?;
        let right = self.mathematical_term(id, scope, right)?;
        match comparison {
            // The codec holds an equality's operands in the vocabulary's
            // order (`validate_canonical_proposition`).
            BinaryOperator::Equal if left <= right => {
                Ok(Proposition::IntegerMathEqual(left, right))
            }
            BinaryOperator::Equal => Ok(Proposition::IntegerMathEqual(right, left)),
            BinaryOperator::Less => Ok(Proposition::IntegerMathLessThan(left, right)),
            BinaryOperator::LessOrEqual => Ok(Proposition::IntegerMathLessOrEqual(left, right)),
            BinaryOperator::Greater => Ok(Proposition::IntegerMathLessThan(right, left)),
            BinaryOperator::GreaterOrEqual => Ok(Proposition::IntegerMathLessOrEqual(right, left)),
            BinaryOperator::NotEqual => canonical::clauses(
                vec![
                    Proposition::IntegerMathLessThan(left.clone(), right.clone()),
                    Proposition::IntegerMathLessThan(right, left),
                ],
                Repeated::Refuse,
                id,
            )
            .map(Proposition::Disjunction),
            _ => unserved("comparison"),
        }
    }

    /// One side of a comparison between mathematical integers: `embed` of a
    /// term this reader serves, an anonymous integer, or `+`, `-` and `*` of
    /// those.
    ///
    /// contracts.md, "Mathematical values and quantification": "Runtime
    /// integer/address values enter mathematical integer arithmetic through
    /// the total `embed` projection, retaining exact carrier range."
    /// mathematical_values.md, "Integer terms": "These terms denote
    /// unbounded mathematical integers and cannot overflow." So nothing here
    /// computes in a carrier and no term owes a formation condition, which is
    /// what tells this from carrier arithmetic in a clause (`term`).
    ///
    /// What `embed` is applied to is read by `term`, so it is whatever a
    /// clause of this kind may name: a formal, a field of a record formal in
    /// an entry `requires`, `result` in an `ensures`, a bound value
    /// parameter or a typed literal. The embedding keeps that term's carrier
    /// (`IntegerMathTerm::embed`).
    ///
    /// Refused by name: `/` and `%`, whose mathematical meaning is rational
    /// division or a named quotient and remainder (contracts.md: "Its result
    /// is always `Rat`"); shifts and bitwise operators; a suffixed operator;
    /// a call other than `embed`; a cast; and a runtime value or typed
    /// literal that is not embedded, since no promotion of one into the
    /// mathematical integers is implied.
    fn mathematical_term(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<IntegerMathTerm> {
        let typed = self.typed;
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if !expression.is_valid() {
            return unserved("missing term");
        }
        match typed.expression_table.expression(expression) {
            ExpressionNode::Call(call) if self.embeds(call) => {
                let [argument] = typed.expression_table.expression_handles(call.arguments) else {
                    return unserved("embed of other than one value");
                };
                let (term, _) = self.term(id, scope, *argument, None)?;
                match IntegerMathTerm::embed(term) {
                    Some(embedded) => Ok(embedded),
                    None => unserved("embed of a value that is not an integer"),
                }
            }
            ExpressionNode::Call(_) => unserved("call in a mathematical term"),
            ExpressionNode::Integer(literal) if literal.landing().is_none() => {
                let magnitude = if literal.is_negative() {
                    literal.value_i64().map(i64::unsigned_abs)
                } else {
                    literal.value_u64()
                };
                let literal = magnitude
                    .and_then(|magnitude| mathematical_literal(literal.is_negative(), magnitude));
                match literal {
                    Some(literal) => Ok(IntegerMathTerm::IntegerLiteral(literal)),
                    None => unserved("mathematical literal beyond 64 bits"),
                }
            }
            ExpressionNode::Integer(_) => {
                unserved("typed literal in a mathematical term without embed")
            }
            // numeric_values.md: wholly anonymous arithmetic is exact, and
            // contracts.md: "An anonymous integral final value can land in
            // mathematical `Int` or `Nat` without fractional-history
            // acknowledgment". So `(7 / 2) * 2` is the integer 7 here, where
            // a carrier landing would need an `as`.
            ExpressionNode::Binary(_) if self.wholly_anonymous(expression) => {
                let evaluated = closed_numeric::evaluate_anonymous_value::<true>(
                    typed,
                    expression,
                    &[],
                    &mut |expression| {
                        closed_numeric::has_anonymous_operator_meaning(typed, expression)
                    },
                );
                let Some(evaluated) = evaluated else {
                    return unserved("anonymous operation without an exact meaning");
                };
                let Some(value) = evaluated.value.to_integer_exact() else {
                    return unserved("fractional anonymous value in a mathematical term");
                };
                let literal = match (value.to_u64(), value.to_i64()) {
                    (Some(magnitude), _) => mathematical_literal(false, magnitude),
                    (None, Some(negative)) => mathematical_literal(true, negative.unsigned_abs()),
                    (None, None) => None,
                };
                match literal {
                    Some(literal) => Ok(IntegerMathTerm::IntegerLiteral(literal)),
                    None => unserved("mathematical literal beyond 64 bits"),
                }
            }
            ExpressionNode::Binary(binary) => {
                if binary.policy != numerics::arithmetic::ArithmeticDomain::Exact {
                    return unserved("policy operator in a mathematical term");
                }
                let operation: fn(Box<IntegerMathTerm>, Box<IntegerMathTerm>) -> IntegerMathTerm =
                    match binary.operator {
                        BinaryOperator::Add => IntegerMathTerm::Add,
                        BinaryOperator::Subtract => IntegerMathTerm::Subtract,
                        BinaryOperator::Multiply => IntegerMathTerm::Multiply,
                        BinaryOperator::Divide | BinaryOperator::Modulo => {
                            return unserved("mathematical division in a clause");
                        }
                        _ => {
                            return unserved(
                                "mathematical operator other than add, subtract and multiply",
                            );
                        }
                    };
                let left = self.mathematical_term(id, scope, binary.left)?;
                let right = self.mathematical_term(id, scope, binary.right)?;
                Ok(operation(Box::new(left), Box::new(right)))
            }
            ExpressionNode::Cast(_) => unserved("cast in a mathematical term"),
            ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                unserved("runtime value in a mathematical term without embed")
            }
            _ => unserved("mathematical term other than embed, a literal and arithmetic"),
        }
    }

    /// A scalar field of the value a data clause is about, as the term
    /// Terminal Psi gives a field of storage: the root and the dense field
    /// and case identities down to the field.
    fn field_term(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<(ScalarTerm, ScalarType)> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        let Some((root, path, value_type)) = self.projected(id, scope, expression)? else {
            return unserved("term other than a field of the value or a literal");
        };
        match value_type {
            ValueType::Rigid(_) => unserved("field of a rigid type in a clause"),
            ValueType::Scalar(ScalarType::Integer(scalar_type))
            | ValueType::Qualified(ScalarType::Integer(scalar_type), _) => Ok((
                ScalarTerm::IntegerField {
                    root,
                    path,
                    scalar_type,
                },
                ScalarType::Integer(scalar_type),
            )),
            ValueType::Scalar(ScalarType::Boolean) => {
                Ok((ScalarTerm::BooleanField { root, path }, ScalarType::Boolean))
            }
            ValueType::Scalar(ScalarType::IeeeFloat(_)) => unserved("float field in a clause"),
            // A policy is carried by an integer only (`qualified_type`).
            ValueType::Qualified(ScalarType::Boolean | ScalarType::IeeeFloat(_), _) => {
                unserved("policy-qualified field that is not an integer")
            }
            ValueType::Record(_) => unserved("data value compared whole in a clause"),
        }
    }

    /// The storage a clause's expression names. In a data clause: `self`, a
    /// field named bare, or a member path beneath either. In a machine's
    /// entry clause: a record formal, whose root is its place, or a member
    /// path beneath one. `None` for an expression that names no storage.
    /// Every step is resolved from a symbol; a name or member the typed
    /// trees hold only as a spelling is refused.
    fn projected(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<Option<(PlaceId, Vec<CanonicalStructuralPathSegment>, ValueType)>> {
        if scope.subject.is_none() && !scope.formal_fields {
            return Ok(None);
        }
        let spelling = |what| refuse(id, Construct::Machine, Unserved::Spelling(what));
        if !expression.is_valid() {
            return Ok(None);
        }
        let (root, base, record, symbol) = match self.typed.expression_table.expression(expression)
        {
            ExpressionNode::Name(path) => {
                if path.static_application.is_valid() {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Contract("applied name used as a plain storage subject"),
                    ));
                }
                if !path.symbol.is_valid() {
                    return Err(spelling("clause name without a symbol"));
                }
                let Some(subject) = scope.subject else {
                    // A record formal is its root place: lowering mints the
                    // record formals' places first, so the `k`-th record
                    // formal is place `k + 1` (`Signature::parameters`).
                    if path.reserved_result_of.is_valid() {
                        return Ok(None);
                    }
                    let slot = self.slot_of(id, scope.slots, path)?;
                    let ValueType::Record(record) = scope.slots[slot].value_type else {
                        return Ok(None);
                    };
                    let root = formal_place(id, scope.slots, slot)?;
                    return Ok(Some((root, Vec::new(), ValueType::Record(record))));
                };
                if subject.symbol.is_valid() && path.symbol == subject.symbol {
                    return Ok(Some((
                        subject.root,
                        Vec::new(),
                        ValueType::Record(subject.record),
                    )));
                }
                // A field named bare is that field of `self`.
                (subject.root, Vec::new(), subject.record, path.symbol)
            }
            ExpressionNode::Member(member) => {
                if !member.member_symbol.is_valid() {
                    return Err(spelling("clause member without a field symbol"));
                }
                let Some((root, base, ValueType::Record(record))) =
                    self.projected(id, scope, member.receiver)?
                else {
                    return Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Contract("member of something that is not data"),
                    ));
                };
                (root, base, record, member.member_symbol)
            }
            ExpressionNode::Indexed(_)
            | ExpressionNode::TypeExpression(_)
            | ExpressionNode::Match(_)
            | ExpressionNode::SavedSubject(_)
            | ExpressionNode::ArrayLiteral(_)
            | ExpressionNode::Atomic(_)
            | ExpressionNode::Binary(_)
            | ExpressionNode::Boolean(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Call(_)
            | ExpressionNode::Float(_)
            | ExpressionNode::UndeterminedNaN(_)
            | ExpressionNode::Integer(_)
            | ExpressionNode::Borrow(_)
            | ExpressionNode::Range(_)
            | ExpressionNode::String(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::ZeroValue(_) => return Ok(None),
        };
        let mut path = base;
        let data = self.data_type(id, record)?;
        let value_type = match self.member_of(id, record, symbol)? {
            Member::Field(field) => {
                path.push(CanonicalStructuralPathSegment::Field(
                    data.fields()[field].id,
                ));
                data.fields()[field].value_type
            }
            Member::Payload { case, field } => {
                let owner = &data.cases()[case];
                path.push(CanonicalStructuralPathSegment::Case(owner.id));
                path.push(CanonicalStructuralPathSegment::Field(
                    owner.fields[field].id,
                ));
                owner.fields[field].value_type
            }
        };
        Ok(Some((root, path, value_type)))
    }
}

impl Program<'_> {
    /// The alternatives of a finite type family, each as the type its one
    /// type parameter is fixed to: the machine `symbol` has exactly one type
    /// parameter and one `requires` clause that is a disjunction of type
    /// equations between that parameter and a builtin scalar type, `requires
    /// T == u32 || T == u64`. generics.md, "Static type equality": "Checking
    /// the body once per listed alternative, each with `T` fixed to that
    /// type, satisfies this rule for a finite family; the list is closed".
    /// Refused for anything else, an open `T` included: that is the opaque
    /// check form's.
    pub fn family_alternatives(
        &self,
        id: MachineId,
        symbol: SymbolHandle,
    ) -> Checked<Vec<Vec<ValueType>>> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Machine(what)));
        let (machine, _) = self.declaration(id, symbol)?;
        let [parameter] = self.typed.machine_type_parameters(machine) else {
            return unserved("finite family over other than one type parameter");
        };
        if parameter.kind != typed_trees::data::TypeParameterKind::Type
            || !parameter.symbol.is_valid()
        {
            return unserved("finite family over other than a type parameter");
        }
        let table = &self.typed.expression_table;
        let types = &self.typed.type_reference_table;
        let named =
            |handle: ExpressionHandle| match handle.is_valid().then(|| table.expression(handle)) {
                Some(ExpressionNode::TypeExpression(reference)) if reference.is_valid() => {
                    match types.type_reference(*reference) {
                        typed_trees::types::TypeReferenceNode::Named { symbol, .. }
                            if symbol.is_valid() =>
                        {
                            Some(*symbol)
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
        // One alternative per disjunct `T == X`, either way round, with `X`
        // a builtin scalar type.
        let alternative = |disjunct: ExpressionHandle| -> Option<ScalarType> {
            let ExpressionNode::Binary(binary) = table.expression(disjunct) else {
                return None;
            };
            if binary.operator != BinaryOperator::Equal {
                return None;
            }
            let (left, right) = (named(binary.left)?, named(binary.right)?);
            let other = match (left == parameter.symbol, right == parameter.symbol) {
                (true, false) => right,
                (false, true) => left,
                _ => return None,
            };
            let atom = self.typed.symbols.builtin_type_atom(other)?;
            super::types::atom_type(atom, id).ok()
        };
        let mut found: Option<Vec<Vec<ValueType>>> = None;
        let contracts = super::span(
            &self.typed.signature_contracts,
            machine.contracts,
            id,
            "contract span",
        )?;
        for contract in contracts {
            if contract.kind != typed_trees::signature::SignatureContractKind::Requires {
                continue;
            }
            for fact in super::span(&self.typed.proof_facts, contract.facts, id, "fact span")? {
                let expression = self.fact(id, fact)?;
                let mut disjuncts = Vec::new();
                self.disjuncts(expression, &mut disjuncts);
                let listed: Option<Vec<ScalarType>> = disjuncts
                    .iter()
                    .map(|disjunct| disjunct.is_valid().then(|| alternative(*disjunct))?)
                    .collect();
                let Some(listed) = listed else {
                    continue;
                };
                if found.is_some() {
                    return unserved("finite family listed by two clauses");
                }
                let mut alternatives: Vec<Vec<ValueType>> = Vec::new();
                for scalar_type in listed {
                    let alternative = vec![ValueType::Scalar(scalar_type)];
                    if !alternatives.contains(&alternative) {
                        alternatives.push(alternative);
                    }
                }
                found = Some(alternatives);
            }
        }
        match found {
            Some(alternatives) => Ok(alternatives),
            None => unserved("type parameter that is not a finite family"),
        }
    }
}
