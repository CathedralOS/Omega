use crate::read::refuse_at;
/*
The contract family of the reader: `requires`, `ensures` and `crashes` clauses
read into propositions over the formals, and the identity of the reserved
`result`. A data declaration's `requires` is read by the same clause reader,
with `self` and its fields as the names in scope (`ClauseScope::subject`).
*/

mod identity_type;

use crate::body::Qualification;
use crate::source_state::domain::ProofFact;
use crate::source_state::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableCallExpression, UnaryOperator,
};
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerMathTerm, MachineId, PlaceId, Proposition,
    ScalarReadProjection, ScalarTerm, ScalarType, StructuralCaseSubject, ValueId,
};
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};

use super::authority::Access;
use super::clause_terms::{ClauseTerm, ClauseUse};
use super::places::Member;
use super::types::TypeTruth;
use super::{Program, Slot};
use crate::StorageShape;
use crate::body::ValueType;
use crate::canonical::{self, Repeated};
use crate::{ClauseText, Piece, SourceOrigin, Unserved, reject};

/// Names a clause is being read through, each standing for an expression.
/// For a named proposition: its declaration, which owns what its formula
/// writes, and each parameter with the argument written for it where the
/// proposition was applied. For the body of a generic mathematical machine:
/// no declaration, and each `let` with its value.
pub(super) struct AppliedProposition {
    pub(super) declaration: Option<SymbolHandle>,
    pub(super) arguments: Vec<(SymbolHandle, ExpressionHandle)>,
}

/// What a contract clause may name: the formals, and the result pseudo-value
/// only in a normal-return guarantee of its owning callable.
#[derive(Clone, Copy)]
pub(super) struct ClauseScope<'s> {
    pub(super) slots: &'s [Slot],
    /// Each formal's immutable logical input identity, by slot
    /// (`body::formal_values`); `None` for a record or borrowed formal.
    /// Body assignments change working slots, never these contract inputs.
    /// The signature owner constructs this roster from declaration formals
    /// only. `old` excludes erased inputs; borrowed inputs use their exact
    /// storage observation instead of a scalar input identity.
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
    pub(super) subject: Option<DataSubject<'s>>,
    /// Whether a term may project a formal (`p.first`). Entry requirements
    /// and crash guards read the invocation input. Normal guarantees keep
    /// owned inline input fields, but read borrowed storage at completion.
    /// `old` explicitly selects the input observation.
    pub(super) formal_fields: bool,
    pub(super) logical: Option<super::logical::LogicalScope<'s>>,
}

/// The stored value a `requires` clause is about: the value of a data
/// declaration for its own clauses, or the field a named domain qualifies
/// for that domain's clauses (`predicates`).
#[derive(Clone, Copy)]
pub(super) struct DataSubject<'s> {
    /// The type of the value `self` is. In a domain's clause it is the
    /// carrier without the domain being defined (domains.md, "Operands").
    pub(super) value_type: ValueType,
    /// The symbol stage 02 gave the clause's `self`; invalid when the
    /// declaration never names it.
    pub(super) symbol: SymbolHandle,
    /// The root that stands for the owning data value in the clause
    /// (`body::self_place`).
    pub(super) root: PlaceId,
    /// From the root down to the value `self` is: empty for a data
    /// declaration's own clause.
    pub(super) path: &'s [CanonicalStructuralPathSegment],
    /// Whether a name written bare is a field of `self`: so in a data
    /// declaration (domains.md: "Existing unqualified data-field names in
    /// data predicates remain field references"), never in a domain's.
    pub(super) bare_fields: bool,
}

#[derive(Clone, Copy)]
pub(super) enum ResultTerm {
    /// The result's identity, scalar carrier and complete qualification.
    Named(ValueId, ScalarType, Qualification),
    /// The declared result root, shared with structural result lowering.
    Structural(PlaceId, ValueType),
    /// Why this clause cannot name `result`.
    Absent(&'static str),
}

/// A supplied value or checked literal already forms. More complex terms
/// need their formation obligations retained with the owning proposition.
pub(super) fn needs_call_argument_formation(term: &ScalarTerm) -> bool {
    !matches!(
        term,
        ScalarTerm::Value { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. }
            | ScalarTerm::IeeeFloatLiteral(_)
    )
}

impl Program<'_> {
    /// An applied body keeps its declaration's clause trees. Both a whole
    /// scalar result and a projected result must name that exact owner.
    pub(super) fn require_result_owner(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        owner: SymbolHandle,
    ) -> Checked<()> {
        if owner == scope.owner {
            return Ok(());
        }
        let declared = self
            .typed
            .executable_callable_template(scope.owner)
            .map_err(|what| refuse_at(id, Unserved::Contract(what)))?;
        if declared == Some(owner) {
            Ok(())
        } else {
            Err(refuse_at(
                id,
                Unserved::Contract("result of another callable"),
            ))
        }
    }
}

/// The place of the formal in `slot`: lowering mints the formals that are
/// places first, in position order (a record formal, and a borrowed formal
/// of any referent), so the `k`-th of them is place `k + 1`
/// (`Signature::is_place`).
pub(super) fn formal_place(id: MachineId, slots: &[Slot], slot: usize) -> Checked<PlaceId> {
    let earlier = slots[..slot]
        .iter()
        .filter(|formal| {
            formal.borrowed.is_some() || matches!(formal.value_type, ValueType::Record(_, _, _))
        })
        .count();
    PlaceId::new(earlier as u64 + 1)
        .ok_or_else(|| refuse_at(id, Unserved::Contract("formal place identity")))
}

impl Program<'_> {
    // Contract clauses: a comparison of formals, literals and Booleans, or a
    // conjunction or disjunction of those. Terms name formal `i` as value `i + 1`.

    /// Source-free models have no authored operator to complete. An authored
    /// data clause keeps its record declaration; callable clauses keep the
    /// exact contract owner, independently of the reserved result identity.
    pub(super) fn complete_contract_operator(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
        operands: [Option<TypeReferenceHandle>; 2],
    ) -> Checked<()> {
        if self
            .typed
            .expression_table
            .authored_selection_occurrences(expression)
            .next()
            .is_none()
        {
            return Ok(());
        }
        let owner = self.contract_operator_owner(id, scope)?;
        self.complete_builtin_operator(id, owner, expression, &operands)
    }

    /// A width-parameterized integer (`u<N>`) has no builtin carrier to name,
    /// so the operand keeps the declared type of the formal or result it
    /// reads. Every other operand returns `None` here.
    fn width_parameter_operand_reference(
        &self,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
        operand: ValueType,
    ) -> Option<TypeReferenceHandle> {
        let ValueType::Scalar(ScalarType::Integer(integer), qualification) = operand else {
            return None;
        };
        if integer.bits().is_some() || !qualification.domains.is_empty() {
            return None;
        }
        let typed = self.typed;
        let ExpressionNode::Name(path) = typed.expression_table.expression(expression) else {
            return None;
        };
        let state = typed.state_by_symbol(scope.owner)?;
        let reference = if path.reserved_result_of.is_valid() {
            state.return_type
        } else {
            typed
                .state_parameter_by_symbol(state, path.resolved_symbol()?)?
                .type_reference
        };
        let TypeReferenceNode::Generic { base_symbol, .. } =
            typed.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        typed
            .symbols
            .integer_family(*base_symbol)
            .map(|_| reference)
    }

    pub(super) fn contract_operator_owner(
        &self,
        id: MachineId,
        scope: &ClauseScope<'_>,
    ) -> Checked<SymbolHandle> {
        Ok(
            if let Some(subject) = scope.subject
                && subject.bare_fields
            {
                let ValueType::Record(record, _, _) = subject.value_type else {
                    return Err(refuse_at(
                        id,
                        Unserved::Contract("data predicate subject without its record declaration"),
                    ));
                };
                terminal_check_tables::row(&self.record_sources, record, id, Construct::Machine)?
                    .declaration
            } else {
                scope.owner
            },
        )
    }

    /// A named proposition applied in a clause of a mathematical machine is
    /// the formula it is declared as, with each parameter standing for the
    /// argument written for it. Nothing of the name reaches the signature:
    /// `int_pair_equivalent(a, b)` and the Identity it abbreviates are one
    /// clause. A proposition declared without a formula, or with a binder,
    /// has no such reading and is refused.
    pub(super) fn named_proposition_call(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &crate::source_state::expression::TableCallExpression,
    ) -> Checked<Proposition> {
        let application = crate::source_state::proposition::PropositionApplication {
            name: call.target.clone(),
            proposition: call.target_symbol,
            binder_arguments: Default::default(),
            arguments: call.arguments,
        };
        self.named_proposition(id, scope, &application)
            .map(|(proposition, _)| proposition)
    }

    fn named_proposition(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        application: &crate::source_state::proposition::PropositionApplication,
    ) -> Checked<(Proposition, ClauseText)> {
        use crate::source_state::proposition::{PropositionBody, PropositionFormula};
        let unserved = |what| refuse_at(id, Unserved::Contract(what));
        let typed = self.typed;
        let mut proposition_symbol = application.proposition;
        if typed
            .proposition_definition_by_symbol(proposition_symbol)
            .is_none()
        {
            let mut current = Some(&*self.source_scope);
            while let Some(scope) = current {
                if let Some((_, actual)) = scope
                    .substitutions
                    .iter()
                    .find(|(symbol, _)| *symbol == proposition_symbol)
                {
                    if let crate::source_state::types::TypeReferenceNode::Named { symbol, .. } =
                        typed.type_reference_table.type_reference(*actual)
                    {
                        proposition_symbol = *symbol;
                    }
                    break;
                }
                current = scope.parent.as_deref();
            }
        }
        let definition = typed
            .proposition_definition_by_symbol(proposition_symbol)
            .ok_or_else(|| unserved("named proposition without its declaration"))?;
        let PropositionBody::Transparent { proposition } = &definition.body else {
            return Err(unserved("named proposition declared without a formula"));
        };
        if !definition.binders.is_empty() || !application.binder_arguments.is_empty() {
            return Err(unserved("named proposition with a binder"));
        }
        let parameters = typed
            .state_parameters
            .span(definition.parameters)
            .ok_or_else(|| unserved("named proposition lost its parameters"))?;
        let arguments = typed
            .expression_table
            .expression_handles(application.arguments);
        if parameters.len() != arguments.len() {
            return Err(unserved(
                "named proposition applied to the wrong number of arguments",
            ));
        }
        // A proposition's formula may apply another, never itself.
        if self
            .applied_propositions
            .iter()
            .any(|applied| applied.declaration == Some(definition.symbol))
        {
            return Err(unserved("recursive named proposition"));
        }
        self.applied_propositions.push(AppliedProposition {
            declaration: Some(definition.symbol),
            arguments: parameters
                .iter()
                .zip(arguments)
                .map(|(parameter, &argument)| (parameter.symbol, argument))
                .collect(),
        });
        let read = match proposition {
            PropositionFormula::BooleanExpression(formula) => self.clause(id, scope, *formula),
            PropositionFormula::Application(inner) => self
                .named_proposition(id, scope, inner)
                .map(|(clause, _)| clause),
        };
        self.applied_propositions.pop();
        let mut text = ClauseText {
            source: arguments
                .first()
                .map(|&argument| typed.expression_table.source_span(argument)),
            ..Default::default()
        };
        text.push_text(application.name.as_str());
        text.push_text("(");
        for (position, &argument) in arguments.iter().enumerate() {
            if position != 0 {
                text.push_text(", ");
            }
            self.write_clause_text(scope.slots, argument, &mut text);
        }
        text.push_text(")");
        Ok((read?, text))
    }

    /// The argument a clause being read through a named proposition has for
    /// one of that proposition's parameters.
    pub(super) fn proposition_argument(&self, parameter: SymbolHandle) -> Option<ExpressionHandle> {
        self.applied_propositions
            .iter()
            .flat_map(|applied| &applied.arguments)
            .find(|(bound, _)| *bound == parameter)
            .map(|(_, argument)| *argument)
    }

    /// A contract fact that is an expression. Memberships are outside the
    /// subset, and so is a named proposition outside a mathematical machine
    /// (`named_proposition` reads it there).
    pub(super) fn fact(&self, id: MachineId, fact: &ProofFact) -> Checked<ExpressionHandle> {
        match fact {
            ProofFact::Expression(expression) => Ok(*expression),
            ProofFact::Membership(_) => Err(refuse_at(id, Unserved::Contract("domain membership"))),
            ProofFact::Proposition(_) => {
                Err(refuse_at(id, Unserved::Contract("named proposition")))
            }
        }
    }

    /// The operands of one connective chain, in authored order.
    fn connective_parts(
        &self,
        expression: ExpressionHandle,
        connective: BinaryOperator,
        out: &mut Vec<ExpressionHandle>,
    ) {
        if expression.is_valid()
            && let ExpressionNode::Binary(binary) =
                self.typed.expression_table.expression(expression)
            && binary.operator == connective
        {
            self.connective_parts(binary.left, connective, out);
            self.connective_parts(binary.right, connective, out);
        } else {
            out.push(expression);
        }
    }

    /// Read every remaining alternative before simplifying. Otherwise `true`
    /// could hide arithmetic that still needs a safety proof. Reuse the
    /// existing representation for `!=`.
    fn disjunctive_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        operands: &[ExpressionHandle],
    ) -> Checked<Proposition> {
        let mut disjuncts = Vec::new();
        for &operand in operands {
            match self.clause(id, scope, operand)? {
                Proposition::Disjunction(inner) => disjuncts.extend(inner),
                other => disjuncts.push(other),
            }
        }
        canonical::clauses(disjuncts, Repeated::Refuse, id).map(Proposition::Disjunction)
    }

    /// Read one authored callable predicate at the observation its contract
    /// chooses. Both ordinary and projected refinement signatures use this
    /// owner; naming a logical membership never introduces its own evidence.
    pub(super) fn contract_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        fact: &ProofFact,
        requirement: bool,
    ) -> Checked<Option<(Proposition, ClauseText)>> {
        if let ProofFact::Membership(membership) = fact {
            return match scope.logical {
                Some(logical) => {
                    let ((clause, text), arguments) = self
                        .with_clause_arguments(id, |program| {
                            logical.membership_clause(program, scope, membership)
                        })?;
                    Ok(Some((
                        self.form_call_arguments(id, clause, arguments)?,
                        text,
                    )))
                }
                None => self.runtime_membership_clause(id, scope, membership),
            };
        }
        if let ProofFact::Proposition(application) = fact {
            return self.named_proposition(id, scope, application).map(Some);
        }
        let expression = self.fact(id, fact)?;
        if self.states_nothing(id, expression, requirement)? {
            let owner = self.contract_operator_owner(id, scope)?;
            let _ = self.read_static_truth(id, owner, expression, super::types::Reading::Clause)?;
            return Ok(None);
        }
        Ok(Some((
            self.clause(id, scope, expression)?,
            self.clause_text(scope.slots, expression),
        )))
    }

    pub(super) fn clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<Proposition> {
        let (clause, arguments) = self.with_clause_arguments(id, |program| {
            let mut operands = [None, None];
            let clause = program.read_clause(id, scope, expression, &mut operands)?;
            program.complete_contract_operator(id, scope, expression, operands)?;
            Ok(clause)
        })?;
        self.form_call_arguments(id, clause, arguments)
    }

    /// Reading a helper can discard an argument's value, but must still form
    /// that argument. Keep this work in the enclosing clause's own scope.
    pub(super) fn with_clause_arguments<T>(
        &mut self,
        id: MachineId,
        read: impl FnOnce(&mut Self) -> Checked<T>,
    ) -> Checked<(T, Vec<ScalarTerm>)> {
        self.clause_arguments.push(Vec::new());
        let read = read(self);
        let arguments = self.clause_arguments.pop().ok_or_else(|| {
            refuse_at(
                id,
                Unserved::Contract("clause lost its argument formation scope"),
            )
        })?;
        read.map(|result| (result, arguments))
    }

    pub(super) fn retain_call_arguments(
        &mut self,
        id: MachineId,
        bound: impl IntoIterator<Item = (SymbolHandle, ScalarTerm)>,
    ) -> Checked<()> {
        // Declared scalar inputs and validated literals are already formed.
        // Projections and computations still need their own formation check.
        let mut pending = bound
            .into_iter()
            .map(|(_, term)| term)
            .filter(needs_call_argument_formation)
            .peekable();
        if pending.peek().is_none() {
            return Ok(());
        }
        let arguments = self.clause_arguments.last_mut().ok_or_else(|| {
            refuse_at(
                id,
                Unserved::Contract("called machine has no owning clause for argument formation"),
            )
        })?;
        arguments.extend(pending);
        Ok(())
    }

    /// Reflexivity keeps each argument in ordinary formation. Keep the result
    /// in the same clause: a false result must not become an independent
    /// premise that excuses its own argument's partial computations.
    pub(super) fn form_call_arguments(
        &self,
        id: MachineId,
        clause: Proposition,
        arguments: Vec<ScalarTerm>,
    ) -> Checked<Proposition> {
        if arguments.is_empty() {
            return Ok(clause);
        }
        let mut formed = Vec::with_capacity(arguments.len());
        for argument in arguments {
            formed.push(canonical::equal(argument.clone(), argument, id)?);
        }
        // Once its terms form, the reflexive premise is true, so this retains
        // exactly the helper's result. Formation visits the whole implication
        // before publishing any consequence of the result.
        Ok(Proposition::Implication {
            premise: Box::new(canonical::conjunction(formed, id)?),
            conclusion: Box::new(clause),
        })
    }

    /// Whether `expression` is a case test, `x in S::C` or `x == S::C`.
    fn tests_a_case(&self, expression: ExpressionHandle) -> bool {
        expression.is_valid()
            && matches!(
                self.typed.expression_table.expression(expression),
                ExpressionNode::Binary(binary)
                    if binary.operator == BinaryOperator::CaseMembership
                        || (binary.operator == BinaryOperator::Equal
                            && self.named_case(binary.right).is_some())
            )
    }

    fn read_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
        operands: &mut [Option<TypeReferenceHandle>; 2],
    ) -> Checked<Proposition> {
        let typed = self.typed;
        let unserved = |what| Err(refuse_at(id, Unserved::Contract(what)));
        if !expression.is_valid() {
            return unserved("missing clause expression");
        }
        if let ExpressionNode::Call(call) = typed.expression_table.expression(expression)
            && let Some((predicate, argument)) = self.byte_observation_clause_call(id, call)?
        {
            return self.byte_predicate_clause(id, scope, predicate, argument);
        }
        match typed.expression_table.expression(expression) {
            ExpressionNode::Boolean(value) => {
                return Ok(if *value {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                });
            }
            ExpressionNode::Call(call) => {
                if let Some(predicate) = self.numeric_comparison_clause(id, scope, call)? {
                    return Ok(predicate);
                }
                // A call of a declared machine states what its body computes
                // for the arguments (`clause_calls`).
                return match self.clause_term(id, scope, expression, ClauseUse::Proposition)? {
                    ClauseTerm::Proposition(proposition) => Ok(proposition),
                    ClauseTerm::Scalar(..)
                    | ClauseTerm::Integer(_)
                    | ClauseTerm::Operand(_)
                    | ClauseTerm::TypeParameter(_) => {
                        unserved("value application used as a proposition")
                    }
                };
            }
            ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                // A callable predicate is Boolean, including a named input
                // and its negation. Keep its actual contract input identity;
                // a caller substitutes that identity, never its spelling.
                return Ok(Proposition::Implication {
                    premise: Box::new(self.clause(id, scope, unary.operand)?),
                    conclusion: Box::new(Proposition::Falsehood),
                });
            }
            // A clause that is one Boolean value reads it as an operand is
            // read: `requires $flag` over `flag: @bool` reads the referent,
            // and `clause_term` refuses a bare `flag`.
            node @ (ExpressionNode::Name(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Unary(_))
                if match node {
                    ExpressionNode::Unary(unary) => unary.operator == UnaryOperator::Dereference,
                    _ => true,
                } =>
            {
                let (term, value_type) = self
                    .clause_term(
                        id,
                        scope,
                        expression,
                        ClauseUse::Scalar(Some(ScalarType::Boolean)),
                    )?
                    .scalar(id)?;
                if self.clause_operand_type(id, value_type)? != ScalarType::Boolean {
                    return Err(reject(
                        id,
                        SourceOrigin::of(typed.expression_table.source_span(expression)),
                        "a contract predicate must have type bool",
                    ));
                }
                return canonical::equal(term, ScalarTerm::Boolean(true), id);
            }
            _ => {}
        }
        let ExpressionNode::Binary(binary) = typed.expression_table.expression(expression) else {
            return unserved("clause that is not a comparison or conjunction");
        };
        let operator = binary.operator;
        if operator == BinaryOperator::Equal
            && let (ExpressionNode::TypeExpression(left), ExpressionNode::TypeExpression(right)) = (
                typed.expression_table.expression(binary.left),
                typed.expression_table.expression(binary.right),
            )
            && let Some(equation) = self.structural_type_equation(id, scope, *left, *right)?
        {
            let owner = self.contract_operator_owner(id, scope)?;
            self.complete_type_equation(id, owner, expression)?;
            return Ok(equation);
        }
        // `T == i32` in `data Value<T> requires ...` is decided by each
        // application. The template is formed for every `T`, so there the
        // equation is a Boolean input whose value nothing states.
        if operator == BinaryOperator::Equal
            && let (ExpressionNode::TypeExpression(_), ExpressionNode::TypeExpression(_)) = (
                typed.expression_table.expression(binary.left),
                typed.expression_table.expression(binary.right),
            )
            && let Some(next) = self.open_equation_inputs
            && !matches!(self.clause_truth(expression), Some(TypeTruth::Known(_)))
        {
            let input = semantic_vocabulary::ValueId::new(next)
                .ok_or_else(|| refuse_at(id, Unserved::Contract("input identity")))?;
            self.open_equation_inputs = Some(next + 1);
            return canonical::equal(
                ScalarTerm::value(input, ScalarType::Boolean),
                ScalarTerm::Boolean(true),
                id,
            );
        }
        if matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
            *operands = [self.operator_operand_reference(ValueType::plain(ScalarType::Boolean)); 2];
        }
        // A machine clause over a type equation the machine being read
        // decides: a conjunct decided true states nothing and the rest is
        // the clause, and a disjunct decided false contributes nothing and
        // is not read (`guarded_false`). What is left must still be a clause
        // this reader serves; a clause decided false outright is refused.
        if scope.subject.is_none() && operator == BinaryOperator::And {
            if self.guarded_false(expression) {
                return unserved("conjunction that type equations decide false");
            }
            let owner = self.contract_operator_owner(id, scope)?;
            // A folded type guard still owns the operator answers produced
            // while evaluating it. Keep them before retaining its sibling.
            if self.read_static_truth(id, owner, binary.left, super::types::Reading::Clause)?
                == Some(TypeTruth::Known(true))
            {
                return self.clause(id, scope, binary.right);
            }
            if self.read_static_truth(id, owner, binary.right, super::types::Reading::Clause)?
                == Some(TypeTruth::Known(true))
            {
                return self.clause(id, scope, binary.left);
            }
        }
        if scope.subject.is_none() && operator == BinaryOperator::Or {
            let mut disjuncts = Vec::new();
            self.connective_parts(expression, BinaryOperator::Or, &mut disjuncts);
            if disjuncts
                .iter()
                .any(|disjunct| self.guarded_false(*disjunct))
            {
                let (vacuous, open): (Vec<_>, Vec<_>) = disjuncts
                    .into_iter()
                    .partition(|disjunct| self.guarded_false(*disjunct));
                let owner = self.contract_operator_owner(id, scope)?;
                self.skip_vacuous_disjuncts(id, owner, expression, &vacuous)?;
                return match open[..] {
                    [only] => self.clause(id, scope, only),
                    [] => unserved("disjunction that type equations decide false"),
                    _ => self.disjunctive_clause(id, scope, &open),
                };
            }
        }
        // `x in S::C`: in a data clause `x` is the value the clause is about
        // or a part of it, and in a machine clause a formal or the result,
        // wherever `projected` can name its place. `self == S::C` is the
        // same test only on a payload-free sum
        // (`require_intrinsic_tag_equality`).
        if matches!(
            operator,
            BinaryOperator::CaseMembership | BinaryOperator::Equal
        ) && let Some(case) = self.named_case(binary.right)
        {
            let Some((root, path, tested)) = self.projected(id, scope, binary.left)? else {
                return unserved("case test of something that is not a sum");
            };
            let Some(sum) = self.opened(id, Some(tested), Access::Read)? else {
                return unserved("case test of something that is not a sum");
            };
            if operator == BinaryOperator::Equal {
                self.require_intrinsic_tag_equality(id, sum)?;
            }
            let case = self.case_position(id, sum, case)?;
            let case = self.data_type(id, sum.record())?.cases()[case].id;
            return Ok(Proposition::StructuralCaseMembership {
                subject: StructuralCaseSubject::new(root, path),
                case,
            });
        }
        // `flag == (x in S::C)`: a case test has no carrier value, but as an
        // operand of `==` or `!=` it is the proposition it states (domains.md:
        // "`x in A | B` states union membership", a proposition like any
        // other). `L == R` is that both sides hold or neither does, and
        // `L != R` that exactly one does, so the published clause vocabulary
        // is unchanged and a proof splits on the two alternatives as for any
        // `||`.
        if matches!(operator, BinaryOperator::Equal | BinaryOperator::NotEqual)
            && (self.tests_a_case(binary.left) || self.tests_a_case(binary.right))
        {
            *operands = [self.operator_operand_reference(ValueType::plain(ScalarType::Boolean)); 2];
            let left = self.clause(id, scope, binary.left)?;
            let right = self.clause(id, scope, binary.right)?;
            let not = |proposition: &Proposition| Proposition::Implication {
                premise: Box::new(proposition.clone()),
                conclusion: Box::new(Proposition::Falsehood),
            };
            let (first, second) = if operator == BinaryOperator::Equal {
                ([left.clone(), right.clone()], [not(&left), not(&right)])
            } else {
                ([left.clone(), not(&right)], [not(&left), right.clone()])
            };
            let mut alternatives = Vec::with_capacity(2);
            for parts in [first, second] {
                let mut conjuncts = Vec::new();
                for part in parts {
                    match part {
                        Proposition::Conjunction(inner) => conjuncts.extend(inner),
                        other => conjuncts.push(other),
                    }
                }
                alternatives.push(Proposition::Conjunction(canonical::clauses(
                    conjuncts,
                    Repeated::Refuse,
                    id,
                )?));
            }
            return canonical::clauses(alternatives, Repeated::Refuse, id)
                .map(Proposition::Disjunction);
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
                // A data clause is published with its type, so it takes the
                // canonical order too. A conjunct that tests the case still
                // guards the conjuncts that project its payload wherever it
                // stands: substitution decides a connective by any one part
                // (`substitute_clause`).
                return canonical::clauses(conjuncts, Repeated::Refuse, id)
                    .map(Proposition::Conjunction);
            }
            BinaryOperator::Or => {
                return self.disjunctive_clause(id, scope, &[binary.left, binary.right]);
            }
            BinaryOperator::Equal
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::NotEqual => operator,
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
            *operands = self.mathematical_operator_operands();
            return self.mathematical_comparison(id, scope, comparison, binary.left, binary.right);
        }
        // A wholly anonymous operand lands in its partner's type; an
        // operand that already has a type keeps it (numeric_values.md,
        // "Integer equality and order": "Already-typed operands retain their
        // types").
        let ((left, left_type), (right, right_type)) = if self.wholly_anonymous(binary.left) {
            let right = self
                .clause_term(id, scope, binary.right, ClauseUse::Scalar(None))?
                .scalar(id)?;
            (
                self.clause_term(id, scope, binary.left, ClauseUse::Scalar(right.1.scalar()))?
                    .scalar(id)?,
                right,
            )
        } else {
            let left = self
                .clause_term(id, scope, binary.left, ClauseUse::Scalar(None))?
                .scalar(id)?;
            let expected = if self.wholly_anonymous(binary.right) {
                left.1.scalar()
            } else {
                None
            };
            let right = self
                .clause_term(id, scope, binary.right, ClauseUse::Scalar(expected))?
                .scalar(id)?;
            (left, right)
        };
        *operands =
            [(binary.left, left_type), (binary.right, right_type)].map(|(expression, operand)| {
                self.operator_operand_reference(operand)
                    .or_else(|| self.width_parameter_operand_reference(scope, expression, operand))
            });
        // Core policy comparisons have the same exact declared signatures
        // as executable comparisons. Plain cross-carrier comparisons keep
        // their separately specified mathematical-value relation below.
        let policy_qualified = |value_type| {
            matches!(value_type,
            ValueType::Scalar(_, qualification) if qualification.policy.is_some())
        };
        if policy_qualified(left_type) || policy_qualified(right_type) {
            super::expressions::binary_selection(
                id,
                comparison,
                Some(left_type),
                Some(right_type),
                (
                    self.wholly_anonymous(binary.left),
                    self.wholly_anonymous(binary.right),
                ),
                None,
                (
                    SourceOrigin::of(typed.expression_table.source_span(binary.left)),
                    SourceOrigin::of(typed.expression_table.source_span(binary.right)),
                ),
            )?;
        }
        let left_type = self.clause_operand_type(id, left_type)?;
        let right_type = self.clause_operand_type(id, right_type)?;
        // Integers of different types compare by their exact values ("The
        // operands may differ in width, signedness, or both"), which is the
        // comparison of their embeddings: `characters <= 4096u64` with an
        // `i32` formal is `embed(characters) <= 4096`.
        if left_type != right_type
            && let (Some(_), Some(_)) = (left_type.as_integer(), right_type.as_integer())
        {
            let embedded = |term| {
                IntegerMathTerm::embed(term).ok_or_else(|| {
                    refuse_at(
                        id,
                        Unserved::Contract("embed of a value that is not an integer"),
                    )
                })
            };
            return mathematical_relation(comparison, embedded(left)?, embedded(right)?, id);
        }
        if left_type != right_type {
            return Err(refuse_at(
                id,
                Unserved::Typing("clause operand types differ"),
            ));
        }
        if matches!(left_type, ScalarType::IeeeFloat(_)) {
            return unserved("IEEE comparison without its selected declaration");
        }
        let ordered = matches!(left_type, ScalarType::Integer(_));
        match comparison {
            BinaryOperator::Equal => canonical::equal(left, right, id),
            // Booleans have no order to split a disequality on. `a != b` is
            // the clause `!(a == b)`, read as the negation above reads it.
            BinaryOperator::NotEqual if !ordered => Ok(Proposition::Implication {
                premise: Box::new(canonical::equal(left, right, id)?),
                conclusion: Box::new(Proposition::Falsehood),
            }),
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

    /// Bind the fixed declaration, written type, and both subjects before
    /// selecting the corresponding identity in the proof vocabulary.
    pub(super) fn identity_inputs(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
        call: &TableCallExpression,
    ) -> Checked<(SymbolHandle, [ExpressionHandle; 2], SourceOrigin)> {
        use crate::declaration_selection::AuthoredDeclarationSelectionKind::{
            Call, StaticArgument,
        };

        let unavailable = |what| refuse_at(id, Unserved::Contract(what));
        if call.receiver.is_valid()
            || call.runtime_call.is_some()
            || call.build_selection.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.requirement_application.is_valid()
            || call.static_machine_parameter.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.installed_interrupt_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
            || !call.evidence_arguments.is_empty()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return Err(unavailable("Identity has a special invocation form"));
        }
        let typed = self.typed;
        let expressions = &typed.expression_table;
        let [argument] = call.machine_arguments.as_ref() else {
            return Err(unavailable("Identity requires one explicit type argument"));
        };
        if !typed.contains_type_reference(argument.type_reference) {
            return Err(unavailable("Identity lost its written type reference"));
        }
        let (subject, applied) = match typed
            .type_reference_table
            .type_reference(argument.type_reference)
        {
            TypeReferenceNode::Named { symbol, .. }
                if !argument.path.is_empty() && *symbol == argument.symbol =>
            {
                (*symbol, false)
            }
            TypeReferenceNode::Generic { base_symbol, .. }
                if argument.path.is_empty() && !argument.symbol.is_valid() =>
            {
                (*base_symbol, true)
            }
            _ => return Err(unavailable("Identity lost its exact written type")),
        };
        if !typed.symbols.symbols().nodes().is_valid(subject)
            || argument.application.is_some()
            || argument.const_literal.is_some()
            || argument.evidence_projection.is_some()
            || argument.authored_machine_application.is_valid()
            || argument.authored_selection.is_some()
        {
            return Err(unavailable("Identity lost its exact written type"));
        }
        let application_handle = call.authored_machine_application;
        if !expressions.static_machine_application_is_valid(application_handle) {
            return Err(unavailable("Identity lost its authored type application"));
        }
        let application = expressions.static_machine_application(application_handle);
        // A concrete body's result belongs to its entry state, while its
        // static applications belong to the machine. A bodyless requirement
        // owns its own result and its written applications directly.
        let owner = self.contract_operator_owner(id, scope)?;
        let owner = self.entered_machine(owner).unwrap_or(owner);
        // An Identity in a named proposition's formula was written there.
        let owner = self
            .applied_propositions
            .iter()
            .rev()
            .find_map(|applied| applied.declaration)
            .unwrap_or(owner);
        if !typed.symbols.symbols().nodes().is_valid(owner) || application.lexical_owner != owner {
            return Err(unavailable(
                "Identity application has a different lexical declaration",
            ));
        }
        if application.registration_operation != call.target_symbol
            || application.callable_contract_source.is_valid()
            || application.source_record_receiver.is_some()
            || application.lifetime_binding_declaration.is_valid()
            || application.binding_declaration.is_valid()
            || application.conformance_selection.is_some()
            || !application.input_selections.is_empty()
            || !application.selected_machine_arguments.is_empty()
            || !application.bindings.is_empty()
            || !application.lifetime_bindings.is_empty()
            || !matches!(
                expressions.static_machine_application_arguments(application_handle),
                [applied] if applied == argument || {
                    // A proposition generated at closed types copies the
                    // call's argument and its application's apart: the same
                    // written type in two rows.
                    let table = &typed.type_reference_table;
                    let mut applied = applied.clone();
                    applied.type_reference = argument.type_reference;
                    applied == *argument
                        && table.type_reference(applied_type(expressions, application_handle))
                            == table.type_reference(argument.type_reference)
                }
            )
        {
            return Err(unavailable(
                "Identity changed its authored type application",
            ));
        }
        let source = expressions.source_span(expression);
        let within_call = |selected: source_text::SourceSpan| {
            selected.source_id == source.source_id
                && source.span.start <= selected.span.start
                && selected.span.start < selected.span.end
                && selected.span.end <= source.span.end
        };
        let ledger = typed.authored_declaration_selections();
        let selection = call
            .authored_call_selection
            .ok_or_else(|| unavailable("Identity lost its authored declaration"))?;
        let call_row = ledger
            .get(selection)
            .filter(|row| row.kind() == Call && within_call(row.source_span()))
            .ok_or_else(|| unavailable("Identity changed its source call occurrence"))?;
        if call_row
            .target()
            .as_resolved()
            .map(|row| row.selected_symbol())
            != Some(call.target_symbol)
            || call.authored_call_binding.is_none()
        {
            return Err(unavailable("Identity changed its selected declaration"));
        }
        let call_source = call_row.source_span();
        self.direct_selection(
            id,
            call.target_symbol,
            Some(selection),
            call.authored_call_binding,
        )?;

        let mut attached_call = false;
        let mut written_type = None;
        let mut nested_types = Vec::new();
        for occurrence in expressions.authored_selection_occurrences(expression) {
            let row = ledger
                .get(occurrence)
                .ok_or_else(|| unavailable("Identity lost an attached source selection"))?;
            if occurrence == selection {
                if std::mem::replace(&mut attached_call, true) {
                    return Err(unavailable("Identity repeats its source call occurrence"));
                }
            } else if row.kind() == StaticArgument
                && row.top_level_static_argument_position() == Some(0)
                && written_type.replace(row).is_some()
            {
                return Err(unavailable("Identity repeats its written type selection"));
            } else if applied
                && row.kind() == StaticArgument
                && row.top_level_static_argument_position().is_none()
            {
                let symbol = row
                    .target()
                    .as_resolved()
                    .map(|selected| selected.selected_symbol())
                    .ok_or_else(|| {
                        unavailable("Identity has an unresolved nested type selection")
                    })?;
                nested_types.push((row.source_span(), symbol));
            }
        }
        // Name resolution records selections for declarations. A lexical type
        // parameter instead has its identity in the current type telescope.
        let parameter_in_scope = typed.symbols.get(subject).kind
            == symbols::SymbolKind::TypeParameter
            && self.scope.iter().any(|(binder, _)| *binder == subject);
        let supplied_type = if written_type.is_none()
            && !parameter_in_scope
            && let Some(supplier) = self.clause_supplier
        {
            // The requirement was written over a binder and its prepared copy
            // names the supplied type. Match the same retained call occurrence
            // in the requirement signature before applying its binder.
            let lexical = typed.symbols.get(supplier).parent;
            let required = typed
                .static_record_bound_requirement(lexical, supplier, owner)
                .map_err(|_| unavailable("Identity lost its supplying requirement scope"))?;
            required.is_some_and(|required| {
                let template = typed
                    .closed_requirement_application(owner)
                    .map_or(owner, |closed| closed.template);
                let Some(original) = typed
                    .trait_machine_signature_by_symbol(required.application.declaration, template)
                else {
                    return false;
                };
                let Some(contracts) = typed.signature_contracts.span(original.contracts) else {
                    return false;
                };
                let mut pending = Vec::new();
                for contract in contracts {
                    let Some(facts) = typed.proof_facts.span(contract.facts) else {
                        return false;
                    };
                    pending.extend(
                        facts
                            .iter()
                            .filter_map(|fact| fact.as_expression().copied()),
                    );
                }
                while let Some(handle) = pending.pop() {
                    if !expressions.expression_is_valid(handle) {
                        return false;
                    }
                    let node = expressions.expression(handle);
                    if let ExpressionNode::Call(written) = node
                        && written.authored_call_selection == Some(selection)
                        && written.target_symbol == call.target_symbol
                        && let [binder] = written.machine_arguments.as_ref()
                        && typed.symbols.symbols().nodes().is_valid(binder.symbol)
                        && typed.symbols.get(binder.symbol).kind
                            == symbols::SymbolKind::TypeParameter
                    {
                        let supplied = required
                            .application
                            .parameter_argument(typed, binder.symbol);
                        return typed.contains_type_reference(supplied)
                            && matches!(typed.type_reference_table.type_reference(supplied),
                                TypeReferenceNode::Named { symbol, .. } if *symbol == subject);
                    }
                    typed.expression_children(node, |child| pending.push(child));
                }
                false
            })
        } else {
            false
        };
        if !attached_call
            || written_type.map_or(!(parameter_in_scope || supplied_type), |row| {
                let typed_source = row.source_span();
                !within_call(typed_source)
                    || typed_source.span.start < call_source.span.end
                    || row
                        .target()
                        .as_resolved()
                        .map(|selected| selected.selected_symbol())
                        != Some(subject)
            })
        {
            return Err(unavailable(
                "Identity lost the source selection for its written type",
            ));
        }

        if applied {
            let head = written_type
                .ok_or_else(|| unavailable("Identity lost its applied type selection"))?;
            self.check_identity_type_application(
                id,
                argument.type_reference,
                source,
                head.source_span(),
                nested_types,
            )?;
        }

        let [left, right] = expressions.expression_handles(call.arguments) else {
            return Err(reject(
                id,
                SourceOrigin::Authored(call_source),
                "Identity requires two subjects",
            ));
        };
        Ok((
            subject,
            [*left, *right],
            SourceOrigin::Authored(call_source),
        ))
    }

    pub(super) fn identity_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
        call: &TableCallExpression,
        preserve_type: bool,
    ) -> Checked<Proposition> {
        let (subject_type, [left, right], origin) =
            self.identity_inputs(id, scope, expression, call)?;
        if self.typed.core_vocabulary(subject_type)
            == Some(language_core::core_vocabulary::CoreVocabulary::FloatMeaning)
        {
            return self.float_meaning_identity(id, scope, left, right);
        }
        // The mathematical signature owns recursive data and records containing it.
        // A runtime contract can state an Identity over that data, but its terms
        // need a mathematical interpretation before the checker can judge them.
        if !self.mathematical
            && self
                .data_declaration(subject_type)
                .is_some_and(|data| self.is_mathematical_data(data))
        {
            return Err(refuse_at(
                id,
                Unserved::Contract("Identity over proof-only data in an executable contract"),
            )
            .with_source_span(origin.span()));
        }
        let carrier = self.value_type(id, call.machine_arguments[0].type_reference)?;
        let (left_expression, right_expression) = (left, right);
        let mut values = Vec::new();
        let left = self
            .clause_term(id, scope, left, ClauseUse::Operand(carrier, &mut values))?
            .operand(id)?;
        let right = self
            .clause_term(id, scope, right, ClauseUse::Operand(carrier, &mut values))?
            .operand(id)?;
        if !self.mathematical
            && let Some(relation) = self
                .executable_quotient_of(carrier)
                .map(|quotient| quotient.relation)
        {
            let classes = semantic_vocabulary::ClauseIdentity {
                values,
                left,
                right,
            };
            self.quotient_soundness(
                id,
                scope,
                relation,
                [left_expression, right_expression],
                &classes,
            )
            .map_err(|refusal| refusal.with_source_span(origin.span()))?;
            return Ok(Proposition::DeclarationIdentity(classes));
        }
        // Keep the arithmetic prover's integer relation when both operands
        // already have scalar denotations. An abstract operation stays applied.
        if !preserve_type
            && matches!(carrier.scalar(), Some(ScalarType::Integer(_)))
            && let (
                semantic_vocabulary::ClauseOperand::Scalar(a),
                semantic_vocabulary::ClauseOperand::Scalar(b),
            ) = (
                &values[left as usize].operand,
                &values[right as usize].operand,
            )
        {
            return canonical::equal(a.clone(), b.clone(), id);
        }
        Ok(Proposition::DeclarationIdentity(
            semantic_vocabulary::ClauseIdentity {
                values,
                left,
                right,
            },
        ))
    }

    /// Records what proves an Identity of two classes of an executable
    /// quotient: the quotient's relation read at the two representatives,
    /// as one Identity. Only two classes written `value as Quotient` have
    /// representatives to relate; any other Identity of the quotient's
    /// values is left opaque, so nothing proves it but an equal premise.
    fn quotient_soundness(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        relation: SymbolHandle,
        sides: [ExpressionHandle; 2],
        classes: &semantic_vocabulary::ClauseIdentity,
    ) -> Checked<()> {
        use crate::source_state::proposition::{PropositionBody, PropositionFormula};
        let unserved = |what| refuse_at(id, Unserved::Contract(what));
        let typed = self.typed;
        let mut representatives = Vec::with_capacity(2);
        for side in sides {
            match typed.expression_table.expression(side) {
                ExpressionNode::Cast(cast) => representatives.push(cast.value),
                _ => return Ok(()),
            }
        }
        let definition = typed
            .proposition_definition_by_symbol(relation)
            .ok_or_else(|| unserved("quotient relation that is no declared proposition"))?;
        let PropositionBody::Transparent {
            proposition: PropositionFormula::BooleanExpression(formula),
        } = &definition.body
        else {
            return Err(unserved("quotient relation declared without a formula"));
        };
        let parameters = typed
            .state_parameters
            .span(definition.parameters)
            .ok_or_else(|| unserved("quotient relation lost its parameters"))?;
        if parameters.len() != 2 {
            return Err(unserved(
                "quotient relation over something other than two values of its carrier",
            ));
        }
        self.applied_propositions.push(AppliedProposition {
            declaration: Some(definition.symbol),
            arguments: parameters
                .iter()
                .zip(&representatives)
                .map(|(parameter, &argument)| (parameter.symbol, argument))
                .collect(),
        });
        let read = self.clause(id, scope, *formula);
        self.applied_propositions.pop();
        // The relation is one Identity, or one equality of two scalars,
        // which is their Identity at the scalar's type.
        let related = match read? {
            Proposition::DeclarationIdentity(related) => related,
            Proposition::Equal(left, right) if left.scalar_type() == right.scalar_type() => {
                let carrier = semantic_vocabulary::ClauseCarrier::Scalar(left.scalar_type());
                semantic_vocabulary::ClauseIdentity {
                    values: vec![
                        semantic_vocabulary::ClauseValue {
                            carrier,
                            operand: semantic_vocabulary::ClauseOperand::Scalar(left),
                        },
                        semantic_vocabulary::ClauseValue {
                            carrier,
                            operand: semantic_vocabulary::ClauseOperand::Scalar(right),
                        },
                    ],
                    left: 0,
                    right: 1,
                }
            }
            _ => {
                return Err(unserved(
                    "Identity of two classes of a quotient whose relation is not one Identity \
                     or one equality",
                ));
            }
        };
        let row = terminal_check_tables::QuotientSoundness {
            classes: classes.clone(),
            related,
        };
        if !self
            .quotient_soundness
            .iter()
            .any(|(machine, known)| *machine == id && *known == row)
        {
            self.quotient_soundness.push((id, row));
        }
        Ok(())
    }

    /// A primitive byte predicate of a byte view (`no_nul(self)`,
    /// `valid_utf8(text)`) is decided by the bytes the view shows, so a
    /// clause states the term the checker reconstructs for those bytes:
    /// `ByteSequencePredicate { root, path } == true` over the storage the
    /// argument names (byte_views.md: "An ordinary or boundary shared-byte
    /// argument may satisfy its formal's primitive byte domains from these
    /// reconstructed predicates"). A domain `[u8]::Path requires
    /// no_nul(self)` on a formal `path: @[u8] in Path` is then an entry
    /// requirement over that formal's bytes, which every caller proves under
    /// the exact argument substitution. An argument that names no byte
    /// view's storage is refused.
    fn byte_predicate_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        predicate: crate::source_state::byte_predicates::ByteSequencePredicate,
        argument: ExpressionHandle,
    ) -> Checked<Proposition> {
        let unserved = |what| refuse_at(id, Unserved::Contract(what));
        let (root, path, value_type) = self.byte_predicate_subject(id, scope, argument)?;
        let ValueType::Record(row, _, _) = value_type else {
            unreachable!("validated byte carrier")
        };
        match self.data_type(id, row)?.shape {
            StorageShape::ByteSequence(_) => {}
            // A fixed array's length is in its type, so the predicate is a
            // statement about its bytes (`fixed_bytes`).
            StorageShape::FixedArray { .. } => {
                return self.fixed_byte_predicate_clause(id, root, path, value_type, predicate);
            }
            _ => {
                return Err(unserved(
                    "byte predicate of something that is not a byte view or a byte array",
                ));
            }
        }
        canonical::equal(
            ScalarTerm::byte_sequence_predicate(predicate, root, path),
            ScalarTerm::Boolean(true),
            id,
        )
    }

    pub(super) fn byte_predicate_subject(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        argument: ExpressionHandle,
    ) -> Checked<(PlaceId, Vec<CanonicalStructuralPathSegment>, ValueType)> {
        let unserved = |what| refuse_at(id, Unserved::Contract(what));
        let subject = crate::source_state::byte_predicates::whole_byte_observation_subject(
            self.typed, argument,
        )
        .ok_or_else(|| unserved("logical byte observation requires a shared exact whole view"))?;
        let Some((root, path, value_type)) = self.projected(id, scope, subject)? else {
            return Err(unserved("byte predicate of something that is not storage"));
        };
        let ValueType::Record(row, _, _) = value_type else {
            return Err(unserved(
                "byte predicate of something that is not a byte carrier",
            ));
        };
        let explicit_loan = matches!(
            self.typed.expression_table.expression(argument),
            ExpressionNode::Borrow(_)
        );
        let valid = match self.data_type(id, row)?.shape {
            StorageShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }) => true,
            StorageShape::ByteSequence(_) if explicit_loan => true,
            StorageShape::FixedArray { .. } if explicit_loan => {
                let Some(array) = self.opened(id, Some(value_type), Access::Read)? else {
                    return Err(unserved("byte array is not readable"));
                };
                let (element, qualification, _) = self.array_element(id, array)?;
                matches!(self.storage_value_type(id, element, qualification)?,
                    ValueType::Scalar(ScalarType::Integer(integer), _) if integer.bits() == Some(8)
                    && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned && !integer.is_address())
            }
            _ => false,
        };
        if !valid {
            return Err(unserved(
                "byte predicate requires a byte view or fixed u8 array",
            ));
        }
        let collection = if let ExpressionNode::Borrow(borrow) =
            self.typed.expression_table.expression(argument)
            && let ExpressionNode::Indexed(indexed) =
                self.typed.expression_table.expression(borrow.target)
        {
            // A domain's `self` names the declared logical carrier, which
            // may be unsized. Its actual source type is retained by that
            // declaration; reading it is not executable slice materialization.
            if let Some(subject) = scope.subject
                && let Some(definition) = self.typed.domain_definition(scope.owner)
                && subject.symbol == definition.subject_symbol
                && self
                    .typed
                    .type_reference_table
                    .contains_type_reference(definition.target_type)
                && matches!(
                    self.typed
                        .type_reference_table
                        .type_reference(definition.target_type),
                    crate::source_state::types::TypeReferenceNode::Slice { .. }
                )
                && crate::source_state::domain::domain_subject_reference(
                    self.typed,
                    definition,
                    indexed.collection,
                )
            {
                Some(definition.target_type)
            } else {
                self.structural_operator_operand_reference(id, indexed.collection, value_type)?
            }
        } else {
            None
        };
        let owner = self.contract_operator_owner(id, scope)?;
        self.complete_byte_observation_range(id, owner, argument, collection)?;
        Ok((root, path, value_type))
    }

    /// Form the source range of a logical byte observation. The declaration
    /// reader supplies its exact logical carrier; instantiated clauses supply
    /// the actual checked structural operand. Neither call grants membership.
    pub(super) fn complete_byte_observation_range(
        &mut self,
        id: MachineId,
        owner: SymbolHandle,
        argument: ExpressionHandle,
        collection: Option<TypeReferenceHandle>,
    ) -> Checked<()> {
        if let ExpressionNode::Borrow(borrow) = self.typed.expression_table.expression(argument)
            && let ExpressionNode::Indexed(indexed) =
                self.typed.expression_table.expression(borrow.target)
            && matches!(
                self.typed.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            )
        {
            return self.complete_collection_range_operator(id, owner, borrow.target, collection);
        }
        Ok(())
    }

    /// A selected pure comparison denotes its canonical IEEE predicate. Its
    /// full result contract states equivalence, so this is the same Boolean
    /// meaning rather than a consequence guessed from an operator token.
    fn numeric_comparison_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &crate::source_state::expression::TableCallExpression,
    ) -> Checked<Option<Proposition>> {
        if self.typed.symbols.sealed_intrinsic(call.target_symbol)
            != Some(language_core::sealed_intrinsics::SealedIntrinsic::FloatOperationRequirement)
        {
            return Ok(None);
        }
        let unavailable = |what| refuse_at(id, Unserved::Contract(what));
        self.direct_selection(
            id,
            call.target_symbol,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        self.applies_nothing(
            id,
            call.target_symbol,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        if call.receiver.is_valid()
            || !call.evidence_arguments.is_empty()
            || call.requirement_application.is_valid()
            || call.exact_coercion_target.is_valid()
            || call.build_selection.is_valid()
            || call.static_machine_parameter.is_valid()
            || call.static_requirement_dispatch.is_some()
            || call.quotient_operation.is_some()
            || call.private_layout_operation.is_some()
            || call.installed_interrupt_operation.is_some()
            || call.layout_query.is_some()
            || call.native_code_layout.is_some()
            || call.operational_acknowledgement.acknowledges_suspend
            || call.operational_acknowledgement.acknowledges_block
        {
            return Err(unavailable(
                "float comparison in a contract cannot use a receiver, evidence arguments or another call selection",
            ));
        }
        let meaning = self
            .typed
            .numeric_intrinsic_execution(call.target_symbol)
            .ok_or_else(|| unavailable("numeric predicate lost its exact declaration meaning"))?;
        let signature = meaning
            .signature()
            .ok_or_else(|| unavailable("numeric predicate has no complete scalar signature"))?;
        let operand = ScalarType::from_numeric_type(signature.operand)
            .filter(|operand| matches!(operand, ScalarType::IeeeFloat(_)))
            .ok_or_else(|| unavailable("numeric predicate has no permanent float format"))?;
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        let [left, right] = arguments else {
            return Err(unavailable("float comparison needs exactly two arguments"));
        };
        let (left, left_type) = self
            .clause_term(id, scope, *left, ClauseUse::Scalar(Some(operand)))?
            .scalar(id)?;
        let (right, right_type) = self
            .clause_term(id, scope, *right, ClauseUse::Scalar(Some(operand)))?
            .scalar(id)?;
        if left_type != ValueType::plain(operand) || right_type != ValueType::plain(operand) {
            return Err(unavailable(
                "numeric predicate changes its selected carrier or qualification",
            ));
        }
        terminal_semantics::numeric_primitives::comparison_predicate(meaning, left, right)
            .map(Some)
            .map_err(unavailable)
    }

    /// A clause's authored text, each formal left open by position, for
    /// diagnostics. Rendering preserves the authored operation and permits
    /// substitution of the caller's actuals without re-reading source text.
    pub(super) fn clause_text(&self, slots: &[Slot], expression: ExpressionHandle) -> ClauseText {
        let mut text = ClauseText {
            source: expression
                .is_valid()
                .then(|| self.typed.expression_table.source_span(expression)),
            ..Default::default()
        };
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
                let grouped = matches!(
                    table.expression(unary.operand),
                    ExpressionNode::Binary(_)
                );
                if grouped { text.push_text("("); }
                self.write_clause_text(slots, unary.operand, text);
                if grouped { text.push_text(")"); }
            }
            ExpressionNode::Member(member) => {
                self.write_clause_text(slots, member.receiver, text);
                text.push_text(&language_core::PLACE_MEMBER_SEPARATOR.to_string());
                text.push_text(member.member.as_str());
            }
            // Retain the type argument and static receiver in the diagnostic.
            // The callee display shared with runtime calls prints receiver
            // methods with a dot and package-qualified targets with "::".
            ExpressionNode::Call(call) if self.typed.symbols.fixed_core_declaration_for_symbol(call.target_symbol)
                    == Some(symbols::FixedCoreDeclaration::Identity)
                || table.name_path_node(call.receiver).is_some_and(|name|
                    self.typed.symbols.symbols().nodes().is_valid(name.symbol)
                    && self.typed.symbols.get(name.symbol).kind == symbols::SymbolKind::TypeParameter) => {
                if call.receiver.is_valid() {
                    self.write_clause_text(slots, call.receiver, text);
                    text.push_text("::");
                }
                text.push_text(call.target.as_str());
                if !call.machine_arguments.is_empty() {
                    text.push_text("::<");
                    for (position, argument) in call.machine_arguments.iter().enumerate() {
                        if position != 0 { text.push_text(", "); }
                        text.push_text(&argument.display_name_in(table));
                    }
                    text.push_text(">");
                }
                text.push_text("(");
                for (position, &argument) in table.expression_handles(call.arguments).iter().enumerate() {
                    if position != 0 { text.push_text(", "); }
                    self.write_clause_text(slots, argument, text);
                }
                text.push_text(")");
            }
            // Proof projections keep the formal inside open, so a call's
            // statement of the clause names its own argument there.
            ExpressionNode::Call(call)
                if self.embeds(call)
                    || self.builtin_call(call, symbols::BuiltinFunction::ContentOld)
                    || self.typed.symbols.sealed_intrinsic(call.target_symbol)
                        == Some(language_core::sealed_intrinsics::SealedIntrinsic::FloatOperationRequirement) =>
            {
                text.push_text(call.target.as_str());
                text.push_text("(");
                for (position, argument) in table.expression_handles(call.arguments).iter().enumerate() {
                    if position != 0 { text.push_text(", "); }
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
                    Some(position) => text.pieces.push(Piece::Formal(position)),
                    None => text.push_text(&table.display_name(expression)),
                }
            }
            // A case of data as written: `Nat::Succ { prev: n }`.
            ExpressionNode::StructLiteral(literal) => {
                text.push_text(literal.type_name.as_str());
                if let Some(case) = &literal.case_name {
                    text.push_text("::");
                    text.push_text(case.as_str());
                }
                let fields = table.struct_fields(literal.fields);
                if !fields.is_empty() {
                    text.push_text(" { ");
                    for (position, field) in fields.iter().enumerate() {
                        if position != 0 {
                            text.push_text(", ");
                        }
                        text.push_text(field.name.as_str());
                        text.push_text(": ");
                        self.write_clause_text(slots, field.value, text);
                    }
                    text.push_text(" }");
                }
            }
            _ => text.push_text(&table.display_name(expression)),
        }
    }

    fn clause_operand_type(&self, id: MachineId, value_type: ValueType) -> Checked<ScalarType> {
        match value_type {
            ValueType::Scalar(scalar, qualification) if qualification.domains.is_empty() => {
                Ok(scalar)
            }
            ValueType::Scalar(..) => Err(refuse_at(
                id,
                Unserved::Contract(
                    "builtin operator on a value with a declared domain in a clause",
                ),
            )),
            _ => Err(refuse_at(
                id,
                Unserved::Contract("non-scalar clause operand"),
            )),
        }
    }

    /// Whether `call` is the builtin integer embedding applied to one value:
    /// decided by the symbol stage 02 resolved the callee to, never by its
    /// spelling, so a machine a program names `embed` is not one.
    pub(super) fn embeds(
        &self,
        call: &crate::source_state::expression::TableCallExpression,
    ) -> bool {
        self.builtin_call(call, symbols::BuiltinFunction::IntegerEmbed)
    }

    pub(super) fn builtin_call(
        &self,
        call: &crate::source_state::expression::TableCallExpression,
        builtin: symbols::BuiltinFunction,
    ) -> bool {
        !call.receiver.is_valid()
            && call.machine_arguments.is_empty()
            && call.evidence_arguments.is_empty()
            && call.target_symbol.is_valid()
            && self
                .typed
                .symbols
                .builtin_function_for_symbol(call.target_symbol)
                == Some(builtin)
    }

    /// Whether `expression` holds an `embed` beneath binary operators only:
    /// what makes the comparison it stands in one between mathematical
    /// integers. An `embed` inside a call's or a cast's operand does not; that
    /// operand is the call's or the cast's to read.
    fn mathematical(&self, expression: ExpressionHandle) -> bool {
        if self.is_mathematical_term(expression) {
            return true;
        }
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
    /// Domain and data clauses retain the same terms; the lowered-form checker
    /// still proves their formation and establishment in each subject scope.
    fn mathematical_comparison(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        comparison: BinaryOperator,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> Checked<Proposition> {
        let left = self
            .clause_term(id, scope, left, ClauseUse::Integer)?
            .integer(id)?;
        let right = self
            .clause_term(id, scope, right, ClauseUse::Integer)?
            .integer(id)?;
        mathematical_relation(comparison, left, right, id)
    }

    /// The storage a clause's expression names. In a data clause: `self`, a
    /// field named bare, or a member path beneath either. In an admitted
    /// machine clause: a formal, whose root is its place, or a member path
    /// beneath one. `None` for an expression that names no storage.
    /// Every step is resolved from a symbol; a name or member the typed
    /// trees hold only as a spelling is refused.
    pub(super) fn projected(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<Option<(PlaceId, Vec<CanonicalStructuralPathSegment>, ValueType)>> {
        let Some((root, path, ty)) = self.read_projection(id, scope, expression)? else {
            return Ok(None);
        };
        let path = static_read_path(&path).ok_or_else(|| {
            refuse_at(
                id,
                Unserved::Contract("dynamic selection used as a static structural subject"),
            )
        })?;
        Ok(Some((root, path, ty)))
    }

    pub(super) fn read_projection(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
    ) -> Checked<Option<(PlaceId, Vec<ScalarReadProjection>, ValueType)>> {
        if scope.subject.is_none()
            && !scope.formal_fields
            && !matches!(scope.result, ResultTerm::Structural(..))
        {
            return Ok(None);
        }
        let spelling = |what| refuse_at(id, Unserved::Spelling(what));
        if !expression.is_valid() {
            return Ok(None);
        }
        let (root, base, record, symbol) = match self.typed.expression_table.expression(expression)
        {
            ExpressionNode::Name(path) => {
                if path.static_application.is_valid() {
                    return Err(refuse_at(
                        id,
                        Unserved::Contract("applied name used as a plain storage subject"),
                    ));
                }
                if path.reserved_result_of.is_valid() {
                    self.require_result_owner(id, scope, path.reserved_result_of)?;
                    return Ok(match scope.result {
                        ResultTerm::Structural(root, value_type) => {
                            Some((root, Vec::new(), value_type))
                        }
                        _ => None,
                    });
                }
                if !path.symbol.is_valid() {
                    return Err(spelling("clause name without a symbol"));
                }
                // A parameter of a named proposition being read is the
                // argument written for it where it was applied, so its
                // fields are that argument's.
                if let Some(actual) = self.proposition_argument(path.symbol) {
                    return self.read_projection(id, scope, actual);
                }
                let Some(subject) = scope.subject else {
                    // A scope that reads no formal storage names none here,
                    // as the early return above says, whether or not its
                    // result is structural.
                    if !scope.formal_fields {
                        return Ok(None);
                    }
                    // A record formal is its root place: lowering mints the
                    // record formals' places first, so the `k`-th record
                    // formal is place `k + 1` (`Signature::parameters`).
                    let slot = self.slot_of(id, scope.slots, path)?;
                    if scope.slots[slot].erased {
                        return Err(refuse_at(
                            id,
                            Unserved::Contract("erased input used as structural storage"),
                        ));
                    }
                    // The formal itself can be a membership subject. Keep its
                    // qualifications when authorizing a read of its storage.
                    let value_type = scope.slots[slot].value_type;
                    if self.opened(id, Some(value_type), Access::Read)?.is_none() {
                        return Ok(None);
                    }
                    let root = formal_place(id, scope.slots, slot)?;
                    return Ok(Some((root, Vec::new(), value_type)));
                };
                if subject.symbol.is_valid() && path.symbol == subject.symbol {
                    return Ok(Some((
                        subject.root,
                        subject.path.iter().copied().map(Into::into).collect(),
                        subject.value_type,
                    )));
                }
                // A field named bare is that field of `self`, read inside
                // `self`'s value as a member of it is.
                if !subject.bare_fields {
                    return Ok(None);
                }
                let Some(record) = self.opened(id, Some(subject.value_type), Access::Read)? else {
                    return Ok(None);
                };
                (
                    subject.root,
                    subject.path.iter().copied().map(Into::into).collect(),
                    record,
                    path.symbol,
                )
            }
            ExpressionNode::Member(member) => {
                if !member.selection.field_symbol().is_valid() {
                    return Err(spelling("clause member without a field symbol"));
                }
                let not_data = || {
                    refuse_at(
                        id,
                        Unserved::Contract("member of something that is not data"),
                    )
                };
                let Some((root, mut base, receiver)) =
                    self.read_projection(id, scope, member.receiver)?
                else {
                    return Err(not_data());
                };
                let Some(record) = self.opened(id, Some(receiver), Access::Read)? else {
                    return Err(not_data());
                };
                let record = self.through_shared_referent(id, &mut base, record)?;
                (root, base, record, member.selection.field_symbol())
            }
            ExpressionNode::Indexed(indexed) => {
                let unavailable = |what| refuse_at(id, Unserved::Contract(what));
                let Some((root, mut path, receiver)) =
                    self.read_projection(id, scope, indexed.collection)?
                else {
                    return Err(unavailable("index of something that is not storage"));
                };
                let Some(array) = self.opened(id, Some(receiver), Access::Read)? else {
                    return Err(unavailable("index of something that is not an array"));
                };
                let array = self.through_shared_referent(id, &mut path, array)?;
                let (element, qualification, extent) = self.array_element(id, array)?;
                let count = ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Unsigned,
                        64,
                    )
                    .expect("u64 is a carrier"),
                );
                let folded = self
                    .typed
                    .closed_integer_expression_value_in(indexed.index, scope.owner);
                let selector = match (extent, folded) {
                    (terminal_semantics::CollectionExtent::Fixed(length), Some(selected)) => {
                        if matches!(
                            self.typed.expression_table.expression(indexed.index),
                            ExpressionNode::Binary(_)
                        ) {
                            self.clause_term(
                                id,
                                scope,
                                indexed.index,
                                ClauseUse::Scalar(Some(count)),
                            )?
                            .scalar(id)?;
                        }
                        let position = selected.to_u64().filter(|position| *position < length).ok_or_else(|| reject(
                            id,
                            SourceOrigin::of(self.typed.expression_table.source_span(indexed.index)),
                            format!("contract array index {selected} is outside its fixed extent {length}"),
                        ))?;
                        CanonicalStructuralPathSegment::FixedIndex(position).into()
                    }
                    (
                        terminal_semantics::CollectionExtent::Fixed(_)
                        | terminal_semantics::CollectionExtent::Parameter { .. }
                        | terminal_semantics::CollectionExtent::Application(_)
                        | terminal_semantics::CollectionExtent::StoredLength,
                        _,
                    ) => {
                        let (index, _) = self
                            .clause_term(id, scope, indexed.index, ClauseUse::Scalar(Some(count)))?
                            .scalar(id)?;
                        ScalarReadProjection::Index(Box::new(index))
                    }
                    _ => {
                        return Err(unavailable(
                            "contract index needs the actual collection extent",
                        ));
                    }
                };
                // A contract's checked index is the same primitive collection
                // operation as a body read. Retain its actual collection type
                // before publishing the source selection; bounds alone cannot
                // resolve a competing declared operator.
                let operands = self.collection_index_operand_types(id, array.record());
                self.complete_contract_operator(id, scope, expression, operands)?;
                // Keep the checked index in the same typed path as fields
                // and cases. Storage substitution must follow this exact leaf.
                path.push(selector);
                let value_type = self.storage_value_type(id, element, qualification)?;
                return Ok(Some((root, path, value_type)));
            }
            ExpressionNode::TypeExpression(_)
            | ExpressionNode::Match(_)
            | ExpressionNode::Block(_)
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
            | ExpressionNode::MachineValue(_)
            | ExpressionNode::Borrow(_)
            | ExpressionNode::Range(_)
            | ExpressionNode::String(_)
            | ExpressionNode::StructLiteral(_)
            | ExpressionNode::Unary(_)
            | ExpressionNode::ZeroValue(_) => return Ok(None),
        };
        let mut path = base;
        let data = self.data_type(id, record.record())?;
        let value_type = if let Some(position) = self.erased_member(id, record.record(), symbol)? {
            let erased = data
                .erased
                .get(position)
                .ok_or_else(|| refuse_at(id, Unserved::Typing("erased field position")))?;
            if erased.evidence || erased.value_type.scalar().is_none() {
                return Err(refuse_at(
                    id,
                    Unserved::Expression("read of an erased field of data"),
                ));
            }
            if let Some(case) = erased.case {
                path.push(CanonicalStructuralPathSegment::Case(data.cases()[case].id).into());
            }
            path.push(CanonicalStructuralPathSegment::Field(erased.id).into());
            erased.value_type
        } else {
            match self.member_of(id, record, symbol)? {
                Member::Field(field) => {
                    path.push(
                        CanonicalStructuralPathSegment::Field(data.fields()[field].id).into(),
                    );
                    data.fields()[field].value_type
                }
                Member::Payload { case, field } => {
                    let owner = &data.cases()[case];
                    path.push(CanonicalStructuralPathSegment::Case(owner.id).into());
                    path.push(CanonicalStructuralPathSegment::Field(owner.fields[field].id).into());
                    owner.fields[field].value_type
                }
            }
        };
        Ok(Some((root, path, value_type)))
    }

    /// A clause names storage beneath a stored shared reference, such as
    /// `self.buffer[4095]` for `buffer: @'s [u16; 4096]`, as the referent the
    /// reference lends: one `Referent` step, then the referent's own fields
    /// or elements. A shared referent cannot change while it is lent, so a
    /// fact about it holds for as long as the reference does. Storage behind
    /// an exclusive reference can be written through it, so a clause there is
    /// refused by name. Anything that is not a reference is `record` itself.
    fn through_shared_referent(
        &self,
        id: MachineId,
        path: &mut Vec<ScalarReadProjection>,
        record: super::authority::Opened,
    ) -> Checked<super::authority::Opened> {
        match self.data_type(id, record.record())?.shape {
            StorageShape::Reference {
                referent,
                access: terminal_psi::StructuralAccess::SharedBorrow,
            } => {
                path.push(ScalarReadProjection::Referent);
                self.opened(id, Some(ValueType::record(referent)), Access::Read)?
                    .ok_or_else(|| refuse_at(id, Unserved::Contract("referent that is not data")))
            }
            StorageShape::Reference { .. } => Err(refuse_at(
                id,
                Unserved::Contract("clause through an exclusive stored reference"),
            )),
            _ => Ok(record),
        }
    }
}

pub(super) fn static_read_path(
    path: &[ScalarReadProjection],
) -> Option<Vec<CanonicalStructuralPathSegment>> {
    path.iter()
        .map(|step| match step {
            ScalarReadProjection::Referent => Some(CanonicalStructuralPathSegment::Referent),
            ScalarReadProjection::Static(segment) => Some(*segment),
            ScalarReadProjection::Index(_) | ScalarReadProjection::Extent => None,
        })
        .collect()
}

/// Inspect only evaluated source bodies. Signature and type metadata may
/// describe other callable families without invoking their classifiers.
#[derive(Default)]
struct PartitionCalls {
    calls: Vec<(
        SymbolHandle,
        crate::source_state::static_applications::StaticMachineApplicationHandle,
    )>,
}

impl Extend<SymbolHandle> for PartitionCalls {
    fn extend<T: IntoIterator<Item = SymbolHandle>>(&mut self, _: T) {}
}

impl crate::source_state::references::TypedReferences for PartitionCalls {
    fn derived_declaration_references(&self) -> bool {
        false
    }
    fn callable_signature_contents(
        &mut self,
        _: &crate::source_state::signature::StateSignature,
    ) -> bool {
        false
    }
    fn type_reference(&mut self, _: TypeReferenceHandle) -> bool {
        false
    }
    fn proof_fact(&mut self, _: arena::Handle<ProofFact>) -> bool {
        false
    }
    fn call_arguments(
        &mut self,
        _: &crate::source_state::TypedTrees,
        call: crate::source_state::references::StaticArgumentCall<'_>,
    ) -> bool {
        let direct = match call {
            crate::source_state::references::StaticArgumentCall::Expression(_, call) => {
                call.runtime_call.is_none()
                    && !call.static_machine_parameter.is_valid()
                    && call.static_requirement_dispatch.is_none()
            }
            crate::source_state::references::StaticArgumentCall::Statement(call) => {
                call.runtime_call.is_none()
                    && !call.static_machine_parameter.is_valid()
                    && call.static_requirement_dispatch.is_none()
            }
        };
        if direct {
            let target = match call {
                crate::source_state::references::StaticArgumentCall::Expression(_, call) => {
                    call.target_symbol
                }
                crate::source_state::references::StaticArgumentCall::Statement(call) => {
                    call.target_symbol
                }
            };
            if call.application().is_valid() {
                self.calls.push((target, call.application()));
            }
        }
        false
    }
}

impl Program<'_> {
    /// Complete alternatives for one type binder. A finite family names
    /// them in a requirement such as `requires T == u32 || T == u64`.
    /// generics.md, "Static type equality": "Checking
    /// the body once per listed alternative, each with `T` fixed to that
    /// type, satisfies this rule for a finite family; the list is closed".
    /// Other binders remain opaque under their own bounds. An open binder
    /// instead checks every closed type named by its static guards and one
    /// opaque remainder that excludes those types; this remains a linear
    /// partition rather than a Cartesian product of independent families.
    pub fn family_alternatives(
        &self,
        id: MachineId,
        symbol: SymbolHandle,
    ) -> Checked<Vec<super::TypeAlternative>> {
        let unserved = |what| Err(refuse_at(id, Unserved::Machine(what)));
        let (machine, _) = self.declaration(id, symbol)?;
        let parameters = self.typed.machine_type_parameters(machine);
        if parameters.is_empty()
            || parameters.iter().any(|parameter| {
                !parameter.symbol.is_valid()
                    || !matches!(
                        parameter.kind,
                        crate::source_state::data::TypeParameterKind::Type
                            | crate::source_state::data::TypeParameterKind::Const { .. }
                            | crate::source_state::data::TypeParameterKind::Value { .. }
                            | crate::source_state::data::TypeParameterKind::Machine { .. }
                    )
            })
        {
            return unserved("finite family needs an exact type-parameter roster");
        }
        let table = &self.typed.expression_table;
        let types = &self.typed.type_reference_table;
        let operand =
            |handle: ExpressionHandle| match handle.is_valid().then(|| table.expression(handle)) {
                Some(ExpressionNode::TypeExpression(reference))
                    if types.contains_type_reference(*reference) =>
                {
                    Some(*reference)
                }
                _ => None,
            };
        // One alternative per disjunct `T == X`, either way round, with `X`
        // a closed scalar or record type. Keep its source reference here;
        // a record identity belongs to the reader that materializes it.
        let alternative = |parameter: &crate::source_state::data::TypeParameter,
                           disjunct: ExpressionHandle|
         -> Option<TypeReferenceHandle> {
            let ExpressionNode::Binary(binary) = table.expression(disjunct) else {
                return None;
            };
            if binary.operator != BinaryOperator::Equal {
                return None;
            }
            let (left, right) = (operand(binary.left)?, operand(binary.right)?);
            let binder = |reference| matches!(types.type_reference(reference), crate::source_state::types::TypeReferenceNode::Named {symbol, ..} if *symbol == parameter.symbol);
            let other = match (binder(left), binder(right)) {
                (true, false) => right,
                (false, true) => left,
                _ => return None,
            };
            if self.scalar_type(id, other).is_ok() {
                return Some(other);
            }
            let data = self.typed.closed_data_definition(other)?;
            super::source_records::source_application(self.typed, data, other, id).ok()?;
            Some(other)
        };
        let mut found: Option<Vec<Vec<Option<TypeReferenceHandle>>>> = None;
        let contracts = super::span(
            |span| self.typed.signature_contracts.span(span),
            machine.contracts,
            id,
            "contract span",
        )?;
        for contract in contracts {
            if contract.kind != crate::source_state::signature::SignatureContractKind::Requires {
                continue;
            }
            for fact in super::span(
                |span| self.typed.proof_facts.span(span),
                contract.facts,
                id,
                "fact span",
            )? {
                let expression = self.fact(id, fact)?;
                let mut conjuncts = Vec::new();
                self.connective_parts(expression, BinaryOperator::And, &mut conjuncts);
                // A finite family remains complete when another input
                // requirement shares its clause. Every alternative still
                // reads that whole clause, including its runtime conditions.
                for expression in conjuncts {
                    let mut disjuncts = Vec::new();
                    self.connective_parts(expression, BinaryOperator::Or, &mut disjuncts);
                    let listed = parameters
                        .iter()
                        .enumerate()
                        .filter(|(_, parameter)| {
                            parameter.kind == crate::source_state::data::TypeParameterKind::Type
                        })
                        .find_map(|(position, parameter)| {
                            disjuncts
                                .iter()
                                .map(|disjunct| {
                                    disjunct
                                        .is_valid()
                                        .then(|| alternative(parameter, *disjunct))?
                                })
                                .collect::<Option<Vec<_>>>()
                                .map(|listed| (position, listed))
                        });
                    let Some((position, listed)) = listed else {
                        continue;
                    };
                    if found.is_some() {
                        return unserved("finite family listed by two clauses");
                    }
                    let mut alternatives: Vec<Vec<Option<TypeReferenceHandle>>> = Vec::new();
                    for reference in listed {
                        let identity = self.typed.normalized_type_identity(reference);
                        if !alternatives.iter().any(|known| {
                            known[position].is_some_and(|reference| {
                                self.typed.normalized_type_identity(reference) == identity
                            })
                        }) {
                            let mut arguments = vec![None; parameters.len()];
                            arguments[position] = Some(reference);
                            alternatives.push(arguments);
                        }
                    }
                    found = Some(alternatives);
                }
            }
        }
        match found {
            Some(alternatives) => Ok(alternatives
                .into_iter()
                .map(|arguments| super::TypeAlternative {
                    arguments,
                    remainder: None,
                })
                .collect()),
            None => {
                // Follow exact plain type forwarding as well as local guards.
                // The mapping names root binders, not spelling or the callee's
                // declaration ordinal. A closed argument has no root binder.
                let mut instances = vec![(
                    symbol,
                    (0..parameters.len()).map(Some).collect::<Vec<_>>(),
                    None::<usize>,
                )];
                let mut visited = arena::HandleKeyMap::<SymbolHandle, Vec<usize>>::default();
                visited.insert(symbol, vec![0]);
                let mut active = arena::HandleKeyMap::<SymbolHandle, usize>::default();
                let mut pending_instances = vec![(0, true)];
                let mut selected = None;
                let mut listed = Vec::new();
                while let Some((instance, entering)) = pending_instances.pop() {
                    let (owner, mapping, parent) = &instances[instance];
                    let owner = *owner;
                    if !entering {
                        active.remove(&owner);
                        continue;
                    }
                    if parent.is_some_and(|parent| {
                        parent >= instance || active.get(&instances[parent].0) != Some(&parent)
                    }) {
                        return unserved("static partition has an invalid introducing caller");
                    }
                    let mapping = mapping.clone();
                    active.insert(owner, instance);
                    pending_instances.push((instance, false));
                    let (current, state) = self.declaration(id, owner)?;
                    let current_parameters = self.typed.machine_type_parameters(current);
                    if current_parameters.len() != mapping.len() {
                        return unserved("static partition lost its forwarded binder roster");
                    }
                    let statements = self.typed.statement_table.statements(state.statement_nodes);
                    let mut pending = statements
                        .iter()
                        .filter_map(|statement| {
                            let crate::source_state::statement::StatementNode::Transition(
                                transition,
                            ) = statement
                            else {
                                return None;
                            };
                            match transition.guard {
                                crate::source_state::statement::TransitionGuardNode::When(
                                    guard,
                                ) => Some(guard),
                                _ => None,
                            }
                        })
                        .collect::<Vec<_>>();
                    while let Some(expression) = pending.pop() {
                        match table.expression(expression) {
                            ExpressionNode::Unary(unary)
                                if unary.operator == UnaryOperator::LogicalNot =>
                            {
                                pending.push(unary.operand)
                            }
                            ExpressionNode::Binary(binary)
                                if matches!(
                                    binary.operator,
                                    BinaryOperator::And | BinaryOperator::Or
                                ) =>
                            {
                                pending.push(binary.left);
                                pending.push(binary.right);
                            }
                            ExpressionNode::Binary(binary)
                                if binary.operator == BinaryOperator::Equal
                                    && operand(binary.left).is_some()
                                    && operand(binary.right).is_some() =>
                            {
                                let Some((position, reference)) = current_parameters
                                    .iter()
                                    .enumerate()
                                    .find_map(|(position, parameter)| {
                                        alternative(parameter, expression)
                                            .map(|reference| (position, reference))
                                    })
                                else {
                                    return unserved(
                                        "static partition needs a binder and a closed type",
                                    );
                                };
                                let Some(position) = mapping[position] else {
                                    continue;
                                };
                                if selected.is_some_and(|selected| selected != position) {
                                    return unserved(
                                        "static partition over independent type binders",
                                    );
                                }
                                selected = Some(position);
                                let identity = self.typed.normalized_type_identity(reference);
                                if !listed.iter().any(|known| {
                                    self.typed.normalized_type_identity(*known) == identity
                                }) {
                                    listed.push(reference);
                                }
                            }
                            _ => {}
                        }
                    }
                    let mut calls = PartitionCalls::default();
                    crate::source_state::references::visit_machine(self.typed, &mut calls, current);
                    for (target, application) in calls.calls {
                        let Some(callee) = self.entered_machine(target) else {
                            continue;
                        };
                        if !application.is_valid() {
                            continue;
                        }
                        let (declaration, _) = self.declaration(id, callee)?;
                        let parameters = self.typed.machine_type_parameters(declaration);
                        if parameters.is_empty()
                            || parameters.iter().any(|parameter| {
                                parameter.kind != crate::source_state::data::TypeParameterKind::Type
                            })
                        {
                            continue;
                        }
                        let prepared =
                            self.application_bindings(id, owner, callee, application, &[])?;
                        if prepared.inherited {
                            continue;
                        }
                        let arguments = prepared.parameters.iter().map(|parameter| {
                            let binding = prepared.bindings.get(&parameter.symbol).ok_or_else(|| {
                                refuse_at(id, Unserved::Call("static partition lost a declared binder"))
                            })?;
                            let crate::source_state::static_applications::StaticMachineBindingValue::Type(reference) = binding.value else {
                                return Err(refuse_at(id, Unserved::Call("static partition type has another argument kind")));
                            };
                            if !types.contains_type_reference(reference) {
                                return Err(refuse_at(id, Unserved::Call("static partition has a stale type argument")));
                            }
                            Ok(reference)
                        }).collect::<Checked<Vec<_>>>()?;
                        let mut forwarded = Vec::with_capacity(arguments.len());
                        for argument in arguments {
                            if let crate::source_state::types::TypeReferenceNode::Named {
                                symbol,
                                ..
                            } = types.type_reference(argument)
                                && let Some(position) = current_parameters
                                    .iter()
                                    .position(|parameter| parameter.symbol == *symbol)
                            {
                                forwarded.push(mapping[position]);
                                continue;
                            }
                            let inputs = crate::source_state::references::type_reference_inputs(
                                self.typed, argument,
                            );
                            if inputs.symbols.iter().any(|symbol| {
                                current_parameters
                                    .iter()
                                    .any(|parameter| parameter.symbol == *symbol)
                            }) {
                                return unserved(
                                    "static partition cannot invert a constructed type argument",
                                );
                            }
                            // Use the same closed-carrier admission as guard
                            // alternatives; no physical layout stands for a type.
                            if self.scalar_type(id, argument).is_err() {
                                let Some(data) = self.typed.closed_data_definition(argument) else {
                                    return unserved(
                                        "static partition forwards neither a binder nor a closed type",
                                    );
                                };
                                super::source_records::source_application(
                                    self.typed, data, argument, id,
                                )?;
                            }
                            forwarded.push(None);
                        }
                        if forwarded.iter().all(Option::is_none) {
                            continue;
                        }
                        if visited.get(&callee).is_some_and(|known| {
                            known
                                .iter()
                                .any(|instance| instances[*instance].1 == forwarded)
                        }) {
                            continue;
                        }
                        // Keep the body's existing expansion boundary: exact
                        // recurrence reuses its tuple; a new mapping cannot
                        // re-enter a template on its introducing ancestry.
                        // The scoped index makes this check independent of
                        // the number of unrelated declarations already seen.
                        if active.contains_key(&callee) {
                            return unserved(
                                "recursive static partition forwarding changes its arguments",
                            );
                        }
                        let next = instances.len();
                        instances.push((callee, forwarded, Some(instance)));
                        visited.entry(callee).or_default().push(next);
                        pending_instances.push((next, true));
                    }
                }
                let Some(position) = selected else {
                    return unserved("type parameter that has no complete static partition");
                };
                let mut alternatives = listed
                    .iter()
                    .map(|reference| {
                        let mut arguments = vec![None; parameters.len()];
                        arguments[position] = Some(*reference);
                        super::TypeAlternative {
                            arguments,
                            remainder: None,
                        }
                    })
                    .collect::<Vec<_>>();
                alternatives.push(super::TypeAlternative {
                    arguments: vec![None; parameters.len()],
                    remainder: Some((position, listed)),
                });
                Ok(alternatives)
            }
        }
    }
}

/// `comparison` between two mathematical integers, as the kernel states it:
/// `>` and `>=` are `<` and `<=` swapped, `!=` is the disjunction of the two
/// strict orders, and an equality holds its operands in the vocabulary's
/// order (`validate_canonical_proposition`).
fn mathematical_relation(
    comparison: BinaryOperator,
    left: IntegerMathTerm,
    right: IntegerMathTerm,
    id: MachineId,
) -> Checked<Proposition> {
    match comparison {
        BinaryOperator::Equal if left <= right => Ok(Proposition::IntegerMathEqual(left, right)),
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
        _ => Err(refuse_at(id, Unserved::Contract("comparison"))),
    }
}

/// The written type of the one argument of an `Identity` application.
fn applied_type(
    expressions: &crate::source_state::expression::ExpressionTable,
    application: crate::source_state::static_applications::StaticMachineApplicationHandle,
) -> crate::source_state::types::TypeReferenceHandle {
    expressions.static_machine_application_arguments(application)[0].type_reference
}
