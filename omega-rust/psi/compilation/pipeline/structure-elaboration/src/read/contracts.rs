/*
The contract family of the reader: `requires`, `ensures` and `crashes` clauses
read into propositions over the formals, and the identity of the reserved
`result`. A data declaration's `requires` is read by the same clause reader,
with `self` and its fields as the names in scope (`ClauseScope::subject`).
*/

use semantic_vocabulary::{
    CanonicalStructuralPathSegment, MachineId, PlaceId, Proposition, ScalarTerm, ScalarType,
    StructuralCaseSubject, StructuralTypeId, ValueId,
};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};
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
    /// clause may not, yet.
    pub(super) formal_fields: bool,
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
            let case = self.data_type(id, sum)?.cases[case].id;
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
                // A record formal is a place, not a value: a clause can only
                // project it, which the member arm below refuses.
                let ValueType::Scalar(scalar_type) = scope.slots[slot].value_type else {
                    return unserved("record formal named whole in a clause");
                };
                // The `k`-th scalar formal is value `k + 1`
                // (`Signature::parameters`).
                let earlier = scope.slots[..slot]
                    .iter()
                    .filter(|formal| formal.value_type.scalar().is_some())
                    .count();
                let value = ValueId::new(earlier as u64 + 1).ok_or_else(|| {
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
            _ => unserved("term other than a formal or literal"),
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
            ValueType::Scalar(ScalarType::Integer(scalar_type)) => Ok((
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
                    let earlier = scope.slots[..slot]
                        .iter()
                        .filter(|formal| formal.value_type.scalar().is_none())
                        .count();
                    let root = PlaceId::new(earlier as u64 + 1).ok_or_else(|| {
                        refuse(
                            id,
                            Construct::Machine,
                            Unserved::Contract("formal place identity"),
                        )
                    })?;
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
                path.push(CanonicalStructuralPathSegment::Field(data.fields[field].id));
                data.fields[field].value_type
            }
            Member::Payload { case, field } => {
                let owner = &data.cases[case];
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
