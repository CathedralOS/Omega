use crate::read::refuse_at;
/*
`clause_term` reads every term a clause writes: an operand of a comparison
in a `requires` or `ensures`, a domain or data predicate, the body of a
transparent `let` definition, a logical invocation input, an argument of a
clause call and an operand of Identity. It is the only reader of each term
form, so a form it serves is served at every one of those positions.

The clause's `ClauseScope` says what each name denotes where the clause
stands: a formal's input value, a logical input, a value parameter bound to
its argument, `result`, or a field of the value a data clause is about. The
caller says which sort of term it needs (`ClauseUse`): a carrier value
(`Scalar`), a mathematical integer (`Integer`), a general operand of Identity
or of a clause application (`Operand`), a proposition or a proof-relevant
type. A refusal here states why the form has no term of that sort, or why
the name has no meaning in that scope, never which reader the clause reached.
*/

use crate::source_state::closed_numeric;
use crate::source_state::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator,
};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ClauseCarrier, ClauseDeclaration, ClauseOperand, ClauseValue,
    IntegerMathLiteral, IntegerMathTerm, MachineId, Proposition, ScalarTerm, ScalarType,
};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};

use super::Program;
use super::authority::Access;
use super::contracts::{ClauseScope, ProjectionUse, ResultTerm, formal_place, static_read_path};
use super::types::{OPEN_CARRIER_FAMILY, TypeTruth};
use crate::body::{Qualification, ValueType};
use crate::{SourceOrigin, Unserved, reject};

/// The sort of term the caller needs.
pub(super) enum ClauseUse<'a> {
    /// A value of the written carrier, recorded in the clause's value table.
    Operand(ValueType, &'a mut Vec<ClauseValue>),
    Proposition,
    ProofRelevantType,
    /// A carrier value. An anonymous literal lands in the given carrier.
    Scalar(Option<ScalarType>),
    /// The carrier value of a call: what the selected declaration returns,
    /// or what the called machine's body computes.
    Value,
    /// A mathematical integer, which no carrier bounds.
    Integer,
}

pub(super) enum ClauseTerm {
    Operand(u32),
    TypeParameter(u32),
    Proposition(Proposition),
    Scalar(ScalarTerm, ValueType),
    Integer(IntegerMathTerm),
}

impl ClauseTerm {
    pub(super) fn type_body(
        self,
        id: MachineId,
    ) -> Checked<terminal_psi::MathematicalPredicateBody> {
        match self {
            Self::Proposition(body) => {
                Ok(terminal_psi::MathematicalPredicateBody::Proposition(body))
            }
            Self::TypeParameter(slot) => {
                Ok(terminal_psi::MathematicalPredicateBody::TypeParameter(slot))
            }
            _ => Err(refuse_at(
                id,
                Unserved::Contract("expected a proof-relevant mathematical type"),
            )),
        }
    }

    fn from_type(body: terminal_psi::MathematicalPredicateBody) -> Self {
        match body {
            terminal_psi::MathematicalPredicateBody::Proposition(body) => Self::Proposition(body),
            terminal_psi::MathematicalPredicateBody::TypeParameter(slot) => {
                Self::TypeParameter(slot)
            }
        }
    }

    pub(super) fn operand(self, id: MachineId) -> Checked<u32> {
        match self {
            Self::Operand(position) => Ok(position),
            _ => Err(refuse_at(
                id,
                Unserved::Contract("expected a value operand of Identity"),
            )),
        }
    }

    pub(super) fn scalar(self, id: MachineId) -> Checked<(ScalarTerm, ValueType)> {
        match self {
            Self::Scalar(term, value_type) => Ok((term, value_type)),
            _ => Err(refuse_at(
                id,
                Unserved::Contract("expected a scalar operand"),
            )),
        }
    }

    pub(super) fn integer(self, id: MachineId) -> Checked<IntegerMathTerm> {
        match self {
            Self::Integer(term) => Ok(term),
            _ => Err(refuse_at(
                id,
                Unserved::Contract("expected a mathematical integer"),
            )),
        }
    }
}

/// The mathematical integer `magnitude`, negated when `negative`; zero has
/// no sign (`IntegerMathLiteral::new` refuses a negative zero).
fn mathematical_literal(negative: bool, magnitude: u64) -> Option<IntegerMathLiteral> {
    IntegerMathLiteral::new(negative && magnitude != 0, u128::from(magnitude)).ok()
}

impl Program<'_> {
    /// The machine a direct call with value arguments only names, when it has
    /// a result and is over data that contains itself. A call names the
    /// machine's entry state.
    fn mathematical_function(
        &self,
        call: &crate::source_state::expression::TableCallExpression,
    ) -> Option<SymbolHandle> {
        let typed = self.typed;
        let target = call.target_symbol;
        if call.receiver.is_valid()
            || !call.evidence_arguments.is_empty()
            || !typed.symbols.symbols().nodes().is_valid(target)
        {
            return None;
        }
        let machine = match typed.symbols.get(target).kind {
            SymbolKind::Machine => target,
            SymbolKind::State => typed.symbols.get(target).parent,
            _ => return None,
        };
        typed
            .machine_by_symbol(machine)
            .filter(|declared| {
                // Machine arguments are for a machine generic over machines.
                call.machine_arguments.is_empty() != self.mathematical_template(declared)
                    && self.proof_machine(declared)
                    && typed
                        .machine_states(declared)
                        .first()
                        .is_some_and(|entry| entry.return_type.is_valid())
            })
            .map(|declared| declared.symbol)
    }

    /// Bind the observation to the actual storage selected in this clause.
    /// Borrowed scalar formals name the lent place directly, just as in a body.
    fn location_alignment_clause(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        operand: ExpressionHandle,
    ) -> Checked<(ScalarTerm, ValueType)> {
        use semantic_vocabulary::ScalarReadProjection;
        let invalid = || {
            refuse_at(
                id,
                Unserved::Contract("location alignment requires accessible storage in this clause"),
            )
        };
        let (root, mut path, value_type) = self
            .read_projection(id, scope, operand, ProjectionUse::Location)?
            .ok_or_else(invalid)?;
        if let ValueType::Record(record, _, _) = value_type {
            if let Some((_, access)) = self.reference_referent(id, value_type)? {
                if access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
                    return Err(invalid());
                }
                path.push(ScalarReadProjection::Referent);
            } else if self.alignment_view_element(id, record)?.is_some() {
                path.push(CanonicalStructuralPathSegment::FixedIndex(0).into());
            }
        }
        path.push(ScalarReadProjection::Alignment);
        let scalar = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64 is valid"),
        );
        let term = ScalarTerm::structural_read(root, path, scalar).map_err(|_| invalid())?;
        Ok((term, ValueType::plain(scalar)))
    }

    /// Read `expression` as a term of the sort `usage` asks for, with its
    /// names denoting what `scope` gives them.
    pub(super) fn clause_term(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: ExpressionHandle,
        mut usage: ClauseUse<'_>,
    ) -> Checked<ClauseTerm> {
        let typed = self.typed;
        let table = &typed.expression_table;
        let unavailable = |what| refuse_at(id, Unserved::Contract(what));
        if !expression.is_valid() {
            return Err(unavailable("missing term"));
        }

        for test in table.executable_domain_tests(expression) {
            let definition = if test.application.is_valid() {
                Some(self.domain_theory(
                    id,
                    crate::body::DomainInstance {
                        declaration: test.domain,
                        identity: test.application,
                    },
                )?)
            } else {
                typed
                    .domain_definition(test.domain)
                    .filter(|domain| domain.alias.is_some() && domain.type_parameters.is_empty())
            };
            let Some(definition) = definition else {
                continue;
            };
            let carrier = self.value_type(id, definition.target_type)?;
            let actual = if let Some((_, _, actual)) = self.projected(id, scope, test.subject)? {
                actual
            } else {
                self.clause_term(id, scope, test.subject, ClauseUse::Scalar(carrier.scalar()))?
                    .scalar(id)?
                    .1
            };
            let at = SourceOrigin::of(table.source_span(test.subject));
            self.checked_membership_carrier(id, definition, carrier, Some(actual), at)?;
            if test.application.is_valid() {
                self.form_domain(
                    id,
                    crate::body::DomainInstance {
                        declaration: test.domain,
                        identity: test.application,
                    },
                    actual,
                    at,
                    super::types::DomainUse::test(definition.requires_result_membership()),
                )?;
            }
        }

        // A mathematical integer: `embed` of a carrier value, an anonymous
        // integer, the address bound, or `+`, `-` and `*` of those.
        //
        // contracts.md, "Mathematical values and quantification": "Runtime
        // integer/address values enter mathematical integer arithmetic
        // through the total `embed` projection, retaining exact carrier
        // range." mathematical_values.md, "Integer terms": "These terms
        // denote unbounded mathematical integers and cannot overflow." So
        // nothing here computes in a carrier and no term owes a formation
        // condition.
        //
        // Refused by name: `/` and `%`, whose mathematical meaning is
        // rational division or a named quotient and remainder (contracts.md:
        // "Its result is always `Rat`"); shifts and bitwise operators; a
        // suffixed operator, since a mathematical integer has no carrier to
        // wrap or clamp at; a call other than `embed`, whose value is a
        // carrier value; a cast; and a runtime value or typed literal that is
        // not embedded, since no promotion of one into the mathematical
        // integers is implied.
        if matches!(usage, ClauseUse::Integer) {
            let read = match table.expression(expression) {
                ExpressionNode::Call(call)
                    if typed
                        .symbols
                        .builtin_function_for_symbol(call.target_symbol)
                        == Some(symbols::BuiltinFunction::IntegerEmbed) =>
                {
                    if call.receiver.is_valid()
                        || call.runtime_call.is_some()
                        || call.requirement_application.is_valid()
                        || call.exact_coercion_target.is_valid()
                        || call.build_selection.is_valid()
                        || call.operational_acknowledgement.acknowledges_suspend
                        || call.operational_acknowledgement.acknowledges_block
                        || !call.evidence_arguments.is_empty()
                        || call.static_machine_parameter.is_valid()
                        || call.static_requirement_dispatch.is_some()
                        || call.quotient_operation.is_some()
                        || call.private_layout_operation.is_some()
                        || call.installed_interrupt_operation.is_some()
                        || call.layout_query.is_some()
                        || call.native_code_layout.is_some()
                    {
                        return Err(unavailable("integer embedding has an unformed application"));
                    }
                    self.applies_nothing(
                        id,
                        call.target_symbol,
                        call.authored_machine_application,
                        &call.machine_arguments,
                    )?;
                    let [argument] = table.expression_handles(call.arguments) else {
                        return Err(unavailable("embed of other than one value"));
                    };
                    // Embedding reads an integer's value without invoking a
                    // carrier operator. Domain qualifications do not change
                    // that value. The checker still proves the operand forms.
                    let (term, value_type) = self
                        .clause_term(id, scope, *argument, ClauseUse::Scalar(None))?
                        .scalar(id)?;
                    let ValueType::Scalar(ScalarType::Integer(carrier), _) = value_type else {
                        return Err(unavailable("embed of a value that is not an integer"));
                    };
                    if term.scalar_type() != ScalarType::Integer(carrier) {
                        return Err(unavailable(
                            "mathematical embedding changed its source carrier",
                        ));
                    }
                    IntegerMathTerm::embed(term)
                        .ok_or_else(|| unavailable("embed of a value that is not an integer"))?
                }
                ExpressionNode::Call(_) => return Err(unavailable("call in a mathematical term")),
                // A proof integer has no runtime carrier. A literal with a
                // carrier suffix enters the proof only through `embed`.
                ExpressionNode::Integer(literal) => {
                    if literal.landing().is_some() {
                        return Err(unavailable(
                            "typed literal in a mathematical term without embed",
                        ));
                    }
                    let magnitude = literal.magnitude_u128().ok_or_else(|| {
                        unavailable("mathematical literal magnitude exceeds 128 bits")
                    })?;
                    IntegerMathLiteral::new(literal.is_negative() && magnitude != 0, magnitude)
                        .map(IntegerMathTerm::IntegerLiteral)
                        .map_err(|_| unavailable("invalid mathematical literal"))?
                }
                // numeric_values.md: wholly anonymous arithmetic is exact, and
                // contracts.md: "An anonymous integral final value can land
                // in mathematical `Int` or `Nat` without fractional-history
                // acknowledgment". So `(7 / 2) * 2` is the integer 7 here,
                // where a carrier landing would need an `as`.
                ExpressionNode::Binary(_) if self.wholly_anonymous(expression) => {
                    let evaluated = closed_numeric::evaluate_anonymous_value::<true>(
                        typed,
                        expression,
                        &[],
                        &mut |expression| {
                            closed_numeric::has_anonymous_operator_meaning(typed, expression)
                        },
                    )
                    .ok_or_else(|| unavailable("anonymous operation without an exact meaning"))?;
                    let value = evaluated.value.to_integer_exact().ok_or_else(|| {
                        unavailable("fractional anonymous value in a mathematical term")
                    })?;
                    let literal = match (value.to_u64(), value.to_i64()) {
                        (Some(magnitude), _) => mathematical_literal(false, magnitude),
                        (None, Some(negative)) => {
                            mathematical_literal(true, negative.unsigned_abs())
                        }
                        (None, None) => None,
                    };
                    IntegerMathTerm::IntegerLiteral(
                        literal
                            .ok_or_else(|| unavailable("mathematical literal beyond 64 bits"))?,
                    )
                }
                ExpressionNode::Binary(binary) => {
                    // `embed(a +% b)` is the wrapped sum, embedded:
                    // "embedding a wrapped sum does not turn it into
                    // unbounded addition".
                    if !matches!(
                        crate::policy::suffix(binary.policy),
                        crate::policy::Suffix::Unwritten
                    ) {
                        return Err(unavailable("policy operator on mathematical integers"));
                    }
                    let operation: fn(
                        Box<IntegerMathTerm>,
                        Box<IntegerMathTerm>,
                    ) -> IntegerMathTerm = match binary.operator {
                        BinaryOperator::Add => IntegerMathTerm::Add,
                        BinaryOperator::Subtract => IntegerMathTerm::Subtract,
                        BinaryOperator::Multiply => IntegerMathTerm::Multiply,
                        BinaryOperator::Divide | BinaryOperator::Modulo => {
                            return Err(unavailable("mathematical division in a clause"));
                        }
                        _ => {
                            return Err(unavailable(
                                "mathematical operator other than add, subtract and multiply",
                            ));
                        }
                    };
                    let left = self
                        .clause_term(id, scope, binary.left, ClauseUse::Integer)?
                        .integer(id)?;
                    let right = self
                        .clause_term(id, scope, binary.right, ClauseUse::Integer)?
                        .integer(id)?;
                    operation(Box::new(left), Box::new(right))
                }
                ExpressionNode::Cast(_) => return Err(unavailable("cast in a mathematical term")),
                // counts_and_addresses.md: valid geometry proves
                // `embed(base) + embed(length) <= address::Bound` "in unbounded
                // proof integers". The bound is one past the largest
                // address. The scope says which address carrier: the
                // ordinary lowered one, or the exact retained model of a
                // bound callable.
                ExpressionNode::Name(_) | ExpressionNode::Member(_)
                    if self.address_bound(expression) =>
                {
                    let address =
                        super::logical::MathematicalAddressContext::of(scope).carrier(id)?;
                    let bound = 1_u128
                        .checked_shl(u32::from(address.bits()))
                        .ok_or_else(|| {
                            unavailable("address bound outside mathematical literal support")
                        })?;
                    IntegerMathTerm::literal(semantic_vocabulary::IntegerValue::Unsigned(bound))
                }
                ExpressionNode::Name(_) | ExpressionNode::Member(_) => {
                    return Err(unavailable(
                        "runtime value in a mathematical term without embed",
                    ));
                }
                _ => {
                    return Err(unavailable(
                        "mathematical term other than embed, a literal and arithmetic",
                    ));
                }
            };
            self.complete_contract_operator(
                id,
                scope,
                expression,
                self.mathematical_operator_operands(),
            )?;
            return Ok(ClauseTerm::Integer(read));
        }

        // A carrier value. Keep the qualification with the value until its
        // consuming operation: a cast can remove a named domain; merely
        // naming the value cannot.
        if let ClauseUse::Scalar(expected) = usage {
            let mut operands = [None, None];
            // A clause such as `requires value < LIMIT` reads the value
            // `LIMIT`'s declaration stores, as the body's value reader does.
            let read = self.constant_value(id, expression)?;
            let (term, value_type) = 'read: {
                if let ExpressionNode::Call(call) = table.expression(read)
                    && let Some((predicate, argument)) =
                        self.byte_observation_clause_call(id, call)?
                {
                    let (root, path, _) = self.byte_predicate_subject(id, scope, argument)?;
                    break 'read (
                        ScalarTerm::byte_sequence_predicate(predicate, root, path),
                        ValueType::plain(ScalarType::Boolean),
                    );
                }
                if let Some(logical) = &scope.logical
                    && let Some((value, scalar)) = logical.input(typed, read, false)?
                {
                    break 'read (
                        ScalarTerm::value(value, scalar),
                        ValueType::Scalar(scalar, Qualification::NONE),
                    );
                }
                // A Boolean static type propositions decide, such as a field
                // of the integer carrier query, is that Boolean
                // (`clause_truth`).
                if matches!(
                    table.expression(read),
                    ExpressionNode::Member(_)
                        | ExpressionNode::Call(_)
                        | ExpressionNode::Unary(_)
                        | ExpressionNode::Binary(_)
                ) {
                    let owner = self.contract_operator_owner(id, scope)?;
                    match self.read_static_truth(id, owner, read, super::types::Reading::Clause)? {
                        Some(TypeTruth::Known(truth)) => {
                            break 'read (
                                ScalarTerm::Boolean(truth),
                                ValueType::plain(ScalarType::Boolean),
                            );
                        }
                        Some(TypeTruth::Open) => return Err(unavailable(OPEN_CARRIER_FAMILY)),
                        None => {}
                    }
                }
                match table.expression(read) {
                    ExpressionNode::Name(path) if path.static_application.is_valid() => {
                        return Err(unavailable("applied name used as a plain scalar term"));
                    }
                    ExpressionNode::Name(path)
                        if self
                            .typed
                            .named_const_declaration(path)
                            .is_some_and(|constant| {
                                constant.pending_target_observation
                                    && constant.evaluated_value.is_none()
                            }) =>
                    {
                        let constant = self
                            .typed
                            .named_const_declaration(path)
                            .expect("checked deferred constant");
                        let (application, scalar_type) =
                            self.deferred_scalar_constant(id, constant)?;
                        (
                            ScalarTerm::Application {
                                application,
                                scalar_type,
                            },
                            ValueType::plain(scalar_type),
                        )
                    }
                    // The reserved result, by the owner identity stage 02
                    // recorded, never by its spelling. calls_and_outcomes.md:
                    // "The reserved result occurrence belongs to its contract
                    // owner, not any local or parameter spelled `result`."
                    ExpressionNode::Name(path) if path.reserved_result_of.is_valid() => {
                        self.require_result_owner(id, scope, path.reserved_result_of)?;
                        match scope.result {
                            ResultTerm::Named(value, scalar_type, qualification) => (
                                ScalarTerm::Value {
                                    id: value,
                                    scalar_type,
                                },
                                ValueType::Scalar(scalar_type, qualification),
                            ),
                            ResultTerm::Structural(..) => {
                                return Err(unavailable(
                                    "whole structural result in a scalar clause",
                                ));
                            }
                            ResultTerm::Absent(why) => return Err(unavailable(why)),
                        }
                    }
                    // A value parameter bound to its argument, including one
                    // of an applied data declaration whose `requires` also
                    // names its fields.
                    ExpressionNode::Name(path)
                        if path.symbol.is_valid()
                            && scope.bound.iter().any(|(symbol, _)| *symbol == path.symbol) =>
                    {
                        if table.name_path_members(path.members).len() != 1 {
                            return Err(unavailable("projected value parameter"));
                        }
                        let (position, literal) = scope
                            .bound
                            .iter()
                            .enumerate()
                            .find(|(_, (symbol, _))| *symbol == path.symbol)
                            .map(|(position, (_, literal))| (position, literal.clone()))
                            .ok_or_else(|| unavailable("value parameter binding"))?;
                        self.use_unfolded_argument(id, scope.owner, position)?;
                        let scalar_type = literal.scalar_type();
                        (literal, ValueType::plain(scalar_type))
                    }
                    ExpressionNode::Member(member)
                        if matches!(
                            member.selection,
                            crate::source_state::expression::MemberSelection::CollectionMeasure(
                                crate::source_state::expression::CollectionMeasure::Length
                            )
                        ) =>
                    {
                        if !scope.formal_fields
                            && scope.subject.is_none()
                            && !matches!(scope.result, ResultTerm::Structural(..))
                        {
                            return Err(unavailable("extent without a structural subject"));
                        }
                        let Some((root, path, receiver)) =
                            self.projected(id, scope, member.receiver)?
                        else {
                            return Err(unavailable("extent without an exact formal descriptor"));
                        };
                        let Some(record) = self
                            .opened(id, Some(receiver), Access::Read)?
                            .map(super::authority::Opened::record)
                        else {
                            return Err(unavailable("extent of non-collection storage"));
                        };
                        let scalar_type = semantic_vocabulary::IntegerType::new(
                            semantic_vocabulary::IntegerSign::Unsigned,
                            64,
                        )
                        .expect("u64 is valid");
                        let shape = &self.data_type(id, record)?.shape;
                        let length = if let Some(length) = shape.fixed_array_extent() {
                            // Taking a type's constant length must not erase
                            // the obligation to establish a payload before
                            // selecting it.
                            if path.iter().any(|segment| {
                                matches!(segment, CanonicalStructuralPathSegment::Case(_))
                            }) {
                                return Err(unavailable(
                                    "fixed array length through an unestablished case payload",
                                ));
                            }
                            match length {
                                terminal_psi::ArrayExtent::Literal(length) => ScalarTerm::Integer {
                                    scalar_type,
                                    value: semantic_vocabulary::IntegerValue::Unsigned(
                                        length.into(),
                                    ),
                                },
                                terminal_psi::ArrayExtent::Parameter { owner, value } => {
                                    let binding = self.extent_input(id, owner, value)?;
                                    let ScalarType::Integer(input_type) = binding.scalar_type
                                    else {
                                        return Err(unavailable(
                                            "fixed-array length input is not an integer",
                                        ));
                                    };
                                    let term = ScalarTerm::Value {
                                        id: value,
                                        scalar_type: binding.scalar_type,
                                    };
                                    if input_type == scalar_type {
                                        term
                                    } else {
                                        ScalarTerm::integer_widen(input_type, scalar_type, term)
                                            .map_err(|_| {
                                                unavailable(
                                                    "fixed-array length has no count conversion",
                                                )
                                            })?
                                    }
                                }
                                terminal_psi::ArrayExtent::Application(application) => {
                                    let input_scalar = self
                                        .scalar_applications
                                        .scalar_type(application)
                                        .ok_or_else(|| {
                                            unavailable(
                                                "fixed-array length has no declared scalar application",
                                            )
                                        })?;
                                    let ScalarType::Integer(input_type) = input_scalar else {
                                        return Err(unavailable(
                                            "fixed-array length application is not an integer",
                                        ));
                                    };
                                    let term = ScalarTerm::Application {
                                        application,
                                        scalar_type: input_scalar,
                                    };
                                    if input_type == scalar_type {
                                        term
                                    } else {
                                        ScalarTerm::integer_widen(input_type, scalar_type, term)
                                            .map_err(|_| {
                                                unavailable(
                                                    "fixed-array length has no count conversion",
                                                )
                                            })?
                                    }
                                }
                            }
                        } else if shape.length_operation().is_some() {
                            ScalarTerm::ViewExtent {
                                root,
                                path,
                                scalar_type,
                            }
                        } else {
                            return Err(unavailable("extent without its descriptor operation"));
                        };
                        (length, ValueType::plain(ScalarType::Integer(scalar_type)))
                    }
                    // A field of storage the clause names: `self.value` or a
                    // field named bare in a data clause, `p.first` of a
                    // record formal where the clause reads inputs, and a
                    // field of a structural `result`. Its term is the root
                    // and the dense field and case identities down to it.
                    // `$held.slot` continues across the stored reference's
                    // `Referent` crossing to the scalar it refers to.
                    node @ (ExpressionNode::Name(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Unary(_))
                        if super::contracts::stored_referent_read(table, node)
                            && (scope.subject.is_some()
                                || (!matches!(node, ExpressionNode::Name(_))
                                    && (scope.formal_fields
                                        || matches!(
                                            scope.result,
                                            ResultTerm::Structural(..)
                                        )))) =>
                    {
                        let Some((root, path, value_type)) =
                            self.read_projection(id, scope, read, ProjectionUse::Contents)?
                        else {
                            return Err(unavailable(
                                "name other than the clause's subject, a field of it or a literal",
                            ));
                        };
                        let Some(path) = static_read_path(&path) else {
                            let ValueType::Scalar(scalar_type, _) = value_type else {
                                return Err(unavailable(
                                    "indexed observation does not select a scalar",
                                ));
                            };
                            if matches!(scalar_type, ScalarType::Callable(_)) {
                                return Err(unavailable("callable field in a clause"));
                            }
                            break 'read (
                                ScalarTerm::StructuralRead {
                                    root,
                                    path,
                                    scalar_type,
                                },
                                value_type,
                            );
                        };
                        let term = match value_type {
                            ValueType::Rigid(_) => {
                                return Err(unavailable("field of a rigid type in a clause"));
                            }
                            ValueType::Scalar(ScalarType::Integer(scalar_type), _) => {
                                ScalarTerm::IntegerField {
                                    version: semantic_vocabulary::ContentPlaceVersion::Current,
                                    root,
                                    path,
                                    scalar_type,
                                }
                            }
                            ValueType::Scalar(ScalarType::Boolean, _) => ScalarTerm::BooleanField {
                                version: semantic_vocabulary::ContentPlaceVersion::Current,
                                root,
                                path,
                            },
                            ValueType::Scalar(ScalarType::IeeeFloat(format), _) => {
                                ScalarTerm::ieee_float_field_path(root, path, format)
                            }
                            ValueType::Scalar(ScalarType::Callable(_), _) => {
                                return Err(unavailable("callable field in a clause"));
                            }
                            ValueType::Record(_, _, _) => {
                                return Err(unavailable("data value compared whole in a clause"));
                            }
                        };
                        (term, value_type)
                    }
                    // `$value` reads the scalar a borrowed formal refers to.
                    // That referent is storage, so the clause's consumer
                    // observes it at entry or completion through the same
                    // exact input place as a projected record field.
                    ExpressionNode::Unary(unary)
                        if unary.operator == UnaryOperator::Dereference =>
                    {
                        let ExpressionNode::Name(path) = table.expression(unary.operand) else {
                            return Err(unavailable(
                                "dereference of other than a borrowed formal in a clause",
                            ));
                        };
                        if path.reserved_result_of.is_valid() || path.static_application.is_valid()
                        {
                            return Err(unavailable(
                                "dereference of other than a borrowed formal in a clause",
                            ));
                        }
                        let slot = self.slot_of(id, scope.slots, path)?;
                        if scope.slots[slot].borrowed.is_none() {
                            return Err(unavailable(
                                "dereference of other than a borrowed formal in a clause",
                            ));
                        }
                        if !scope.formal_fields {
                            return Err(unavailable(
                                "borrowed formal outside an admitted storage clause",
                            ));
                        }
                        let ValueType::Scalar(scalar_type, qualification) =
                            scope.slots[slot].value_type
                        else {
                            return Err(unavailable("record formal named whole in a clause"));
                        };
                        let root = formal_place(id, scope.slots, slot)?;
                        let version = semantic_vocabulary::ContentPlaceVersion::Current;
                        let term = match scalar_type {
                            ScalarType::Integer(integer) => ScalarTerm::IntegerField {
                                version,
                                root,
                                path: Vec::new(),
                                scalar_type: integer,
                            },
                            ScalarType::Boolean => ScalarTerm::BooleanField {
                                version,
                                root,
                                path: Vec::new(),
                            },
                            ScalarType::IeeeFloat(format) => {
                                ScalarTerm::ieee_float_field_path(root, Vec::new(), format)
                            }
                            ScalarType::Callable(_) => {
                                return Err(unavailable("callable field in a clause"));
                            }
                        };
                        (term, ValueType::Scalar(scalar_type, qualification))
                    }
                    ExpressionNode::Name(path) => {
                        let slot = self.slot_of(id, scope.slots, path)?;
                        // A borrowed scalar named bare is the reference, not
                        // the value it refers to. expressions.md, "Explicit
                        // dereference": "Omega never inserts a dereference to
                        // satisfy an expected value type", operator operands
                        // included, so a clause reads it as a body does.
                        if scope.slots[slot].borrowed.is_some() {
                            let ValueType::Scalar(referent, _) = scope.slots[slot].value_type
                            else {
                                return Err(unavailable("record formal named whole in a clause"));
                            };
                            return Err(super::rejections::implicit_dereference(
                                id,
                                SourceOrigin::of(table.source_span(read)),
                                referent,
                            ));
                        }
                        // A record formal is a place, not a scalar value: a
                        // clause names it through the field projection above.
                        // A policy-qualified formal is named as its stored
                        // value: a comparison of it compares that value
                        // whatever the policy.
                        let ValueType::Scalar(scalar_type, qualification) =
                            scope.slots[slot].value_type
                        else {
                            return Err(unavailable("record formal named whole in a clause"));
                        };
                        if matches!(scalar_type, ScalarType::Callable(_)) {
                            return Err(unavailable("callable value in a clause"));
                        }
                        // Contracts name the supplied scalar value in every
                        // clause. Lowering initializes the body's working
                        // slot from this formal ValueId; assignments and
                        // writable loans can change that slot without
                        // rebinding the logical input. Returns and calls
                        // substitute these same immutable identities, so no
                        // runtime snapshot or additional custody is needed.
                        let value = scope
                            .values
                            .get(slot)
                            .copied()
                            .flatten()
                            .ok_or_else(|| unavailable("formal identity"))?;
                        (
                            ScalarTerm::Value {
                                id: value,
                                scalar_type,
                            },
                            ValueType::Scalar(scalar_type, qualification),
                        )
                    }
                    // Other formal projections need their own input or
                    // completion storage correspondence; a readable spelling
                    // does not supply it.
                    ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
                        return Err(unavailable("clause that projects a formal or result"));
                    }
                    ExpressionNode::Integer(literal)
                        if literal.landing().is_none()
                            && matches!(expected, Some(ScalarType::IeeeFloat(_))) =>
                    {
                        let Some(ScalarType::IeeeFloat(format)) = expected else {
                            unreachable!("float landing selected this arm")
                        };
                        let (value, _) = self.anonymous_float(id, read, format)?;
                        (
                            ScalarTerm::IeeeFloatLiteral(value),
                            ValueType::plain(ScalarType::IeeeFloat(format)),
                        )
                    }
                    ExpressionNode::Float(literal) => {
                        let value = self.float_literal(id, literal, expected)?;
                        (
                            ScalarTerm::IeeeFloatLiteral(value),
                            ValueType::plain(ScalarType::IeeeFloat(value.format())),
                        )
                    }
                    ExpressionNode::Integer(literal) => {
                        let (value, integer) = self.literal(id, literal, expected)?;
                        (
                            ScalarTerm::Integer {
                                scalar_type: integer,
                                value,
                            },
                            ValueType::plain(ScalarType::Integer(integer)),
                        )
                    }
                    ExpressionNode::Binary(_)
                        if self.wholly_anonymous(read)
                            && matches!(expected, Some(ScalarType::IeeeFloat(_))) =>
                    {
                        let Some(ScalarType::IeeeFloat(format)) = expected else {
                            unreachable!("float landing selected this arm")
                        };
                        let (value, operators) = self.anonymous_float(id, read, format)?;
                        if !operators.is_empty() {
                            let owner = self.contract_operator_owner(id, scope)?;
                            self.complete_anonymous_operators(id, owner, &operators)?;
                        }
                        (
                            ScalarTerm::IeeeFloatLiteral(value),
                            ValueType::plain(ScalarType::IeeeFloat(format)),
                        )
                    }
                    ExpressionNode::Binary(_) if self.wholly_anonymous(read) => {
                        let evaluated = self.anonymous_value(
                            id,
                            read,
                            expected,
                            super::types::AnonymousLanding::Implicit,
                        )?;
                        if !evaluated.operators.is_empty() {
                            let owner = self.contract_operator_owner(id, scope)?;
                            self.complete_anonymous_operators(id, owner, &evaluated.operators)?;
                        }
                        (
                            ScalarTerm::Integer {
                                scalar_type: evaluated.integer,
                                value: evaluated.value,
                            },
                            ValueType::plain(ScalarType::Integer(evaluated.integer)),
                        )
                    }
                    ExpressionNode::Boolean(value) => (
                        ScalarTerm::Boolean(*value),
                        ValueType::plain(ScalarType::Boolean),
                    ),
                    // `left <op><suffix> right`: the value the operator
                    // computes under its policy. The operator means here
                    // what it means in a body (`crate::policy`):
                    // both operands are unqualified integers of one carrier,
                    // an anonymous operand lands in its partner's carrier, a
                    // shift's count keeps its own type, and the result is
                    // the bare carrier. contracts.md, "Total arithmetic":
                    // "Wrapping and Saturating discharge primitive
                    // definedness not resolved by their overflow policy, such
                    // as a nonzero divisor." A divide or remainder therefore
                    // owes a condition before it forms a term. Declaration
                    // formation checks that condition before using the
                    // clause.
                    // A case test is a proposition, not a carrier value: it
                    // is an operand only of `==` or `!=` in a whole clause
                    // (`read_clause`), never of arithmetic or a call.
                    ExpressionNode::Binary(binary)
                        if binary.operator == BinaryOperator::CaseMembership =>
                    {
                        return Err(unavailable(
                            "case test as a value outside a Boolean comparison",
                        ));
                    }
                    ExpressionNode::Binary(binary) => {
                        let source_left = SourceOrigin::of(table.source_span(binary.left));
                        // A comparison computes a Boolean from operands of
                        // their own type, so what the position expects of
                        // the result says nothing of them.
                        let comparison = matches!(
                            binary.operator,
                            BinaryOperator::Equal
                                | BinaryOperator::NotEqual
                                | BinaryOperator::Less
                                | BinaryOperator::LessOrEqual
                                | BinaryOperator::Greater
                                | BinaryOperator::GreaterOrEqual
                        );
                        let expected = if comparison { None } else { expected };
                        let suffix = match crate::policy::suffix(binary.policy) {
                            crate::policy::Suffix::Unwritten => None,
                            // "An arithmetic operation whose selected meaning
                            // can transfer control cannot form a proof term.
                            // It is not silently read as Exact or
                            // mathematical arithmetic."
                            crate::policy::Suffix::Trapping => {
                                return Err(reject(
                                    id,
                                    SourceOrigin::of(table.source_span(read)),
                                    "a trapping operator (`+!`, `-!`, `*!`) in a contract clause: \
                                     an operation that can crash forms no proof term, and it is \
                                     not read as exact or mathematical arithmetic",
                                ));
                            }
                            crate::policy::Suffix::Total(policy) => Some(policy),
                        };
                        let anonymous = (
                            self.wholly_anonymous(binary.left),
                            self.wholly_anonymous(binary.right),
                        );
                        if suffix.is_some() && anonymous == (true, true) {
                            return Err(reject(id, source_left, crate::policy::WHOLLY_ANONYMOUS));
                        }
                        let ((left, left_type), (right, right_type)) = if anonymous.0 {
                            let right = self
                                .clause_term(id, scope, binary.right, ClauseUse::Scalar(expected))?
                                .scalar(id)?;
                            let left = self
                                .clause_term(
                                    id,
                                    scope,
                                    binary.left,
                                    ClauseUse::Scalar(right.1.scalar()),
                                )?
                                .scalar(id)?;
                            (left, right)
                        } else {
                            let left = self
                                .clause_term(id, scope, binary.left, ClauseUse::Scalar(expected))?
                                .scalar(id)?;
                            let right = self
                                .clause_term(
                                    id,
                                    scope,
                                    binary.right,
                                    ClauseUse::Scalar(left.1.scalar()),
                                )?
                                .scalar(id)?;
                            (left, right)
                        };
                        let (operation, policy, selected) = super::expressions::binary_selection(
                            id,
                            binary.operator,
                            Some(left_type),
                            Some(right_type),
                            anonymous,
                            suffix,
                            (
                                source_left,
                                SourceOrigin::of(table.source_span(binary.right)),
                            ),
                        )?;
                        let operation = operation.ok_or_else(|| {
                            unavailable("short-circuit expression is not a scalar arithmetic term")
                        })?;
                        // `a < b` is the Boolean value it computes wherever a
                        // clause writes one, as an operand (`flag == (a < b)`)
                        // as well as a whole clause. `>` and `>=` are the
                        // same relations with the operands exchanged, and
                        // `!=` is the negation of `==`.
                        let term = if comparison {
                            if suffix.is_some() || policy.is_some() {
                                return Err(unavailable(
                                    "comparison under an arithmetic policy as a Boolean value",
                                ));
                            }
                            let unrelated = |_| {
                                unavailable(
                                    "comparison as a Boolean value of operands it does not relate",
                                )
                            };
                            match (left_type.scalar(), right_type.scalar()) {
                                (Some(ScalarType::Boolean), Some(ScalarType::Boolean)) => {
                                    let equal = ScalarTerm::boolean_equal(left, right)
                                        .map_err(unrelated)?;
                                    match binary.operator {
                                        BinaryOperator::Equal => equal,
                                        BinaryOperator::NotEqual => {
                                            ScalarTerm::boolean_not(equal).map_err(unrelated)?
                                        }
                                        _ => {
                                            return Err(unavailable("order of two Boolean values"));
                                        }
                                    }
                                }
                                (
                                    Some(ScalarType::Integer(carrier)),
                                    Some(ScalarType::Integer(other)),
                                ) => {
                                    let (carrier, left, right) = if matches!(
                                        binary.operator,
                                        BinaryOperator::Greater | BinaryOperator::GreaterOrEqual
                                    ) {
                                        (other, right, left)
                                    } else {
                                        (carrier, left, right)
                                    };
                                    match binary.operator {
                                        BinaryOperator::Equal => {
                                            ScalarTerm::integer_equal(carrier, left, right)
                                                .map_err(unrelated)?
                                        }
                                        BinaryOperator::NotEqual => ScalarTerm::boolean_not(
                                            ScalarTerm::integer_equal(carrier, left, right)
                                                .map_err(unrelated)?,
                                        )
                                        .map_err(unrelated)?,
                                        BinaryOperator::Less | BinaryOperator::Greater => {
                                            ScalarTerm::integer_less_than(carrier, left, right)
                                                .map_err(unrelated)?
                                        }
                                        _ => {
                                            ScalarTerm::integer_less_or_equal(carrier, left, right)
                                                .map_err(unrelated)?
                                        }
                                    }
                                }
                                _ => {
                                    return Err(unavailable(
                                        "comparison as a Boolean value of operands that are neither integers nor bool",
                                    ));
                                }
                            }
                        } else {
                            let Some(ScalarType::Integer(carrier)) = selected.scalar() else {
                                return Err(unavailable(
                                    "arithmetic contract term has no selected integer carrier",
                                ));
                            };
                            crate::policy::term(operation, policy, carrier, left, right)
                                .ok_or_else(|| {
                                unavailable(
                                    "selected arithmetic operator has no canonical contract term",
                                )
                            })?
                        };
                        // Source selection must see the operands this reader
                        // actually checked; unknown operands would leave
                        // unrelated overloads eligible.
                        operands = [left_type, right_type]
                            .map(|value| self.operator_operand_reference(value));
                        (term, selected)
                    }
                    // `!b` is a Boolean value wherever a clause writes one,
                    // as an operand (`a == !b`) as well as a whole clause.
                    ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::LogicalNot => {
                        let (operand, value_type) = self
                            .clause_term(
                                id,
                                scope,
                                unary.operand,
                                ClauseUse::Scalar(Some(ScalarType::Boolean)),
                            )?
                            .scalar(id)?;
                        if value_type != ValueType::plain(ScalarType::Boolean) {
                            return Err(unavailable(
                                "logical negation of a value that is not bool",
                            ));
                        }
                        let term = ScalarTerm::boolean_not(operand).map_err(|_| {
                            unavailable("logical negation of a value that is not bool")
                        })?;
                        (term, value_type)
                    }
                    ExpressionNode::Unary(unary) if unary.operator == UnaryOperator::BitwiseNot => {
                        let (operand, value_type) = self
                            .clause_term(id, scope, unary.operand, ClauseUse::Scalar(expected))?
                            .scalar(id)?;
                        let Some(result @ ValueType::Scalar(ScalarType::Integer(carrier), _)) =
                            crate::policy::bitwise_not_type(value_type)
                        else {
                            return Err(unavailable(
                                "bitwise complement on an operand without a core word signature",
                            ));
                        };
                        let term = ScalarTerm::integer_bitwise_not(carrier, operand)
                            .map_err(|_| unavailable("bitwise complement term"))?;
                        (term, result)
                    }
                    ExpressionNode::Cast(cast)
                        if cast.form == language_core::cast_form::CastForm::Value
                            && cast.result_type.is_valid() =>
                    {
                        let after = self.qualified_scalar_type(id, cast.result_type)?;
                        let landing = after.scalar().filter(|_| self.wholly_anonymous(cast.value));
                        let (term, before) = match landing {
                            // `as` lands the complete anonymous operand here
                            // as in a body (`Program::cast`), fractional
                            // intermediates included.
                            Some(integer @ ScalarType::Integer(_))
                                if !matches!(
                                    table.expression(cast.value),
                                    ExpressionNode::Integer(_)
                                ) =>
                            {
                                let evaluated = self.anonymous_value(
                                    id,
                                    cast.value,
                                    Some(integer),
                                    super::types::AnonymousLanding::ExplicitAs,
                                )?;
                                if !evaluated.operators.is_empty() {
                                    let owner = self.contract_operator_owner(id, scope)?;
                                    self.complete_anonymous_operators(
                                        id,
                                        owner,
                                        &evaluated.operators,
                                    )?;
                                }
                                (
                                    ScalarTerm::Integer {
                                        scalar_type: evaluated.integer,
                                        value: evaluated.value,
                                    },
                                    ValueType::plain(ScalarType::Integer(evaluated.integer)),
                                )
                            }
                            _ => self
                                .clause_term(id, scope, cast.value, ClauseUse::Scalar(landing))?
                                .scalar(id)?,
                        };
                        if before == after {
                            break 'read (term, after);
                        }
                        if let (
                            ValueType::Scalar(ScalarType::Integer(from), Qualification::NONE),
                            ValueType::Scalar(ScalarType::Integer(to), Qualification::NONE),
                        ) = (before, after)
                            && !from.is_address()
                            && !to.is_address()
                        {
                            // Widening is total. A narrowing or sign change
                            // must form its representability obligation
                            // before this clause can be used.
                            let converted = if from.can_widen_to(to) {
                                ScalarTerm::integer_widen(from, to, term)
                            } else {
                                ScalarTerm::integer_exact_cast(from, to, term)
                            };
                            break 'read (
                                converted.map_err(|_| {
                                    unavailable("integer cast with no exact carrier conversion")
                                })?,
                                after,
                            );
                        }
                        self.scalar_domain_cast_type(id, cast.result_type, before)?;
                        return Err(unavailable(
                            "value cast in a clause changes its qualification",
                        ));
                    }
                    ExpressionNode::Cast(cast) => {
                        let entry = cast
                            .form
                            .entry()
                            .ok_or_else(|| unavailable("cast in a clause term"))?;
                        let run = self.read_domain_cast(
                            id,
                            super::casts::CastContext::Clause,
                            expression,
                            cast,
                            entry,
                        )?;
                        let (term, actual) = self
                            .clause_term(id, scope, run.value, ClauseUse::Scalar(expected))?
                            .scalar(id)?;
                        if actual != run.input {
                            return Err(unavailable(
                                "domain cast does not account for the clause operand's qualifications",
                            ));
                        }
                        for (domain, source) in run.removed {
                            self.domain_removal(id, domain, actual, source)?;
                        }
                        // All entries refer to this one immutable scalar term.
                        (term, run.result)
                    }
                    // `old` reads the invocation input, not the current
                    // working storage. A scalar input already has that
                    // identity; a projected field retains its exact
                    // root/path and asks the existing storage binding for
                    // the entry observation. Results and computed terms have
                    // no such input.
                    ExpressionNode::Call(call)
                        if self.builtin_call(call, symbols::BuiltinFunction::ContentOld) =>
                    {
                        if scope.subject.is_some() {
                            return Err(unavailable(
                                "old in a data or domain predicate, which has no invocation entry",
                            ));
                        }
                        let [argument] = table.expression_handles(call.arguments) else {
                            return Err(unavailable("old of other than one subject"));
                        };
                        let argument = *argument;
                        if let Some(logical) = scope.logical
                            && let Some((value, scalar)) = logical.input(typed, argument, true)?
                        {
                            break 'read (
                                ScalarTerm::value(value, scalar),
                                ValueType::plain(scalar),
                            );
                        }
                        let inputs = ClauseScope {
                            result: ResultTerm::Absent("result has no callable-entry value"),
                            formal_fields: true,
                            // Static substitutions are not invocation inputs.
                            // Resolve the authored subject against the actual
                            // formal roster, so a bound term cannot
                            // impersonate an input by reusing its numeric IDs.
                            bound: &[],
                            ..*scope
                        };
                        let (mut term, value_type) = self
                            .clause_term(id, &inputs, argument, ClauseUse::Scalar(expected))?
                            .scalar(id)?;
                        let is_input = match &mut term {
                            ScalarTerm::IntegerField { version, .. }
                            | ScalarTerm::BooleanField { version, .. }
                            | ScalarTerm::IeeeFloatField { version, .. } => {
                                *version = semantic_vocabulary::ContentPlaceVersion::Entry;
                                true
                            }
                            ScalarTerm::Value {
                                id: value,
                                scalar_type,
                            } => scope.slots.iter().zip(scope.values).any(|(slot, input)| {
                                !slot.erased
                                    && slot.borrowed.is_none()
                                    && *input == Some(*value)
                                    && slot.value_type == value_type
                                    && matches!(slot.value_type, ValueType::Scalar(declared, _)
                                            if declared == *scalar_type)
                            }),
                            _ => false,
                        };
                        if !is_input {
                            return Err(unavailable(
                                "old of a subject without an admitted invocation input",
                            ));
                        }
                        (term, value_type)
                    }
                    // `embed` makes a mathematical integer, which is no
                    // carrier value; a comparison with an `embed` on either
                    // side compares mathematical integers.
                    ExpressionNode::Call(call)
                        if self.builtin_call(call, symbols::BuiltinFunction::IntegerEmbed) =>
                    {
                        return Err(unavailable(
                            "embed where a carrier value is needed: it is a mathematical integer",
                        ));
                    }
                    ExpressionNode::Call(call) if call.layout_query.is_some() => {
                        let query = call.layout_query.as_ref().expect("matched a layout query");
                        if matches!(query, crate::source_state::expression::DataLayoutQueryRequest::LocationAlignment) {
                            let [operand] = table.expression_handles(call.arguments) else {
                                return Err(unavailable("location alignment needs one operand"));
                            };
                            break 'read self.location_alignment_clause(id, scope, *operand)?;
                        }
                        let source = SourceOrigin::of(table.source_span(read));
                        let measured = self.layout_query(id, call, query, source)?;
                        let (
                            crate::body::ExpressionKind::ScalarApplication { application },
                            Some(ValueType::Scalar(scalar_type, qualification)),
                        ) = (measured.kind, measured.result)
                        else {
                            return Err(unavailable("layout query has no scalar application"));
                        };
                        (
                            ScalarTerm::Application {
                                application,
                                scalar_type,
                            },
                            ValueType::Scalar(scalar_type, qualification),
                        )
                    }
                    ExpressionNode::Call(_) => self
                        .clause_term(id, scope, read, ClauseUse::Value)?
                        .scalar(id)?,
                    _ => return Err(unavailable("expression with no carrier value term")),
                }
            };
            self.complete_contract_operator(id, scope, expression, operands)?;
            return Ok(ClauseTerm::Scalar(term, value_type));
        }

        if let ExpressionNode::Call(call) = table.expression(expression)
            && typed
                .symbols
                .fixed_core_declaration_for_symbol(call.target_symbol)
                == Some(symbols::FixedCoreDeclaration::Identity)
        {
            return self
                .identity_clause(
                    id,
                    scope,
                    expression,
                    call,
                    matches!(usage, ClauseUse::ProofRelevantType),
                )
                .map(ClauseTerm::Proposition);
        }
        if matches!(usage, ClauseUse::ProofRelevantType | ClauseUse::Proposition)
            && let ExpressionNode::Call(call) = table.expression(expression)
            && let Some(sort) = self.mathematical_definition_sort(call.target_symbol)
            && (matches!(usage, ClauseUse::Proposition)
                || sort == symbols::FixedCoreDeclaration::Type)
        {
            return self
                .applied_definition(id, scope, call)
                .map(ClauseTerm::from_type);
        }
        if matches!(usage, ClauseUse::ProofRelevantType) {
            if let ExpressionNode::Call(call) = table.expression(expression)
                && typed
                    .proposition_definition_by_symbol(call.target_symbol)
                    .is_some()
            {
                return self
                    .named_proposition_call(id, scope, call)
                    .map(ClauseTerm::Proposition);
            }
            if let ExpressionNode::Name(name) = table.expression(expression)
                && !name.reserved_result_of.is_valid()
                && !name.static_application.is_valid()
                && table.name_path_members(name.members).len() == 1
                && let Some(body) = scope
                    .logical
                    .and_then(|scope| scope.type_argument(name.symbol))
            {
                return Ok(ClauseTerm::from_type(body.clone()));
            }
            return Err(unavailable("expected a proof-relevant mathematical type"));
        }
        let bodyless = match table.expression(expression) {
            ExpressionNode::Call(call) if !call.receiver.is_valid() => typed
                .bodyless_machine_signature(call.target_symbol)
                .map_err(unavailable)?,
            _ => None,
        };
        let abstract_application = bodyless.is_some()
            || matches!(table.expression(expression), ExpressionNode::Call(call)
            if call.receiver.is_valid() || (typed.symbols.symbols().nodes().is_valid(call.target_symbol)
                && (typed.closed_requirement_application(call.target_symbol).is_some()
                    || typed.trait_definition_by_symbol(typed.symbols.get(call.target_symbol).parent).is_some())));
        // A mathematical machine with a `bool` result is a definition of
        // the signature, so its proposition is that its result is true
        // (mathematical_bindings.md), never an unfolding of its body.
        let boolean_proposition = matches!(usage, ClauseUse::Proposition)
            && (abstract_application
                || match table.expression(expression) {
                    ExpressionNode::Call(call) if self.mathematical => {
                        match self.mathematical_function(call) {
                            Some(target) => {
                                let (_, entry) = self.declaration(id, target)?;
                                self.value_type(id, entry.return_type)?
                                    == ValueType::plain(ScalarType::Boolean)
                            }
                            None => false,
                        }
                    }
                    _ => false,
                });
        if boolean_proposition {
            // A proposition-valued use of a Boolean operation requires its
            // actual result to be true under the same declaration and inputs.
            let mut values = Vec::new();
            let boolean = ValueType::plain(ScalarType::Boolean);
            let left = self
                .clause_term(
                    id,
                    scope,
                    expression,
                    ClauseUse::Operand(boolean, &mut values),
                )?
                .operand(id)?;
            let right = u32::try_from(values.len())
                .map_err(|_| unavailable("clause term table overflow"))?;
            values.push(ClauseValue {
                carrier: ClauseCarrier::Scalar(ScalarType::Boolean),
                operand: ClauseOperand::Scalar(ScalarTerm::Boolean(true)),
            });
            return Ok(ClauseTerm::Proposition(Proposition::DeclarationIdentity(
                semantic_vocabulary::ClauseIdentity {
                    values,
                    left,
                    right,
                },
            )));
        }
        let mut inferred = Vec::new();
        let reading_scalar = abstract_application && matches!(usage, ClauseUse::Value);
        let operands = match &mut usage {
            ClauseUse::Operand(expected, values) => Some((Some(*expected), &mut **values)),
            ClauseUse::Value if reading_scalar => Some((None, &mut inferred)),
            _ => None,
        };
        if let Some((mut expected, values)) = operands {
            // A parameter of a named proposition being read is the argument
            // written for it where the proposition was applied.
            if let ExpressionNode::Name(path) = table.expression(expression)
                && let Some(expected) = expected
                && let Some(actual) = self.proposition_argument(path.symbol)
            {
                return self.clause_term(id, scope, actual, ClauseUse::Operand(expected, values));
            }
            let operand = match (table.expression(expression), expected) {
                // A machine defined by cases over data that contains itself is
                // a function of the signature. The clause applies it by its
                // declaration; the kernel unfolds it, never this reader.
                (ExpressionNode::Call(call), _)
                    if self.mathematical && self.mathematical_function(call).is_some() =>
                {
                    let target = self
                        .mathematical_function(call)
                        .expect("the guard found the machine");
                    // The signature needs the applied machine's definition.
                    self.machine(target)?;
                    let (_, entry) = self.declaration(id, target)?;
                    let result = self.value_type(id, entry.return_type)?;
                    if expected.is_some_and(|expected| expected != result) {
                        return Err(unavailable(
                            "clause application result differs from the written carrier",
                        ));
                    }
                    expected = Some(result);
                    let parameters = typed
                        .state_parameters
                        .span(entry.parameters)
                        .ok_or_else(|| unavailable("clause application lost its parameters"))?;
                    let actuals = table.expression_handles(call.arguments);
                    if parameters.len() != actuals.len() {
                        return Err(unavailable(
                            "clause application arity differs from its signature",
                        ));
                    }
                    let mut arguments = Vec::with_capacity(actuals.len());
                    // A machine argument is a function passed whole: a
                    // machine parameter or a concrete mathematical machine
                    // (`machine_argument`). Machine arguments come before
                    // the value arguments, as the generic machine's
                    // definition takes them.
                    if !call.machine_arguments.is_empty() {
                        let callee = typed
                            .machine_by_symbol(target)
                            .ok_or_else(|| unavailable("clause application lost its machine"))?;
                        let required = self.required_callables(id, callee)?;
                        if required.len() != call.machine_arguments.len() {
                            return Err(unavailable(
                                "generic mathematical machine applied to the wrong number of machines",
                            ));
                        }
                        for (argument, required) in call.machine_arguments.iter().zip(&required) {
                            arguments.push(self.machine_argument(id, argument, required, values)?);
                        }
                    }
                    for (formal, &actual) in parameters.iter().zip(actuals) {
                        let ty = self.value_type(id, formal.type_reference)?;
                        arguments.push(
                            self.clause_term(id, scope, actual, ClauseUse::Operand(ty, values))?
                                .operand(id)?,
                        );
                    }
                    ClauseOperand::Application {
                        declaration: ClauseDeclaration {
                            member: (target.arena_index(), target.generation()),
                            supplier: None,
                        },
                        arguments,
                    }
                }
                // A case of such data is a declaration too: the clause applies
                // it to its fields in declared order.
                (ExpressionNode::StructLiteral(literal), _) if self.mathematical => {
                    let symbol = literal
                        .case_symbol
                        .filter(|symbol| symbol.is_valid() && literal.type_symbol.is_valid())
                        .ok_or_else(|| unavailable("clause constructor without a case"))?;
                    // A case of generic data is read at the application the
                    // literal constructs (`Seq::Empty` as a `Seq<u64>`), not
                    // at the declaration it names.
                    let application = [
                        literal.application_type,
                        table.constructed_value_type_reference(expression),
                    ]
                    .into_iter()
                    .find(|reference| reference.is_valid());
                    let constructed = match application {
                        Some(reference) => self.value_type(id, reference)?,
                        None => self.record(id, literal.type_symbol)?,
                    };
                    let ValueType::Record(record, ..) = constructed else {
                        return Err(unavailable("clause constructor of a scalar type"));
                    };
                    if self.self_containing.binary_search(&record).is_err() {
                        return Err(unavailable("clause constructor of data with storage"));
                    }
                    if expected.is_some_and(|expected| expected != constructed) {
                        return Err(unavailable(
                            "clause constructor differs from the written carrier",
                        ));
                    }
                    expected = Some(constructed);
                    let opened = super::authority::Opened::constructing(record);
                    let case = self.case_position(id, opened, symbol)?;
                    let declared: Vec<ValueType> = self
                        .data_type(id, record)?
                        .cases()
                        .get(case)
                        .ok_or_else(|| unavailable("clause constructor lost its case"))?
                        .fields
                        .iter()
                        .map(|field| field.value_type)
                        .collect();
                    let mut fields = vec![None; declared.len()];
                    for authored in typed.expression_table.struct_fields(literal.fields) {
                        let position = match self.member_of(id, opened, authored.field_symbol)? {
                            super::places::Member::Payload { case: owner, field }
                                if owner == case =>
                            {
                                field
                            }
                            _ => {
                                return Err(unavailable(
                                    "clause constructor names a field of another case",
                                ));
                            }
                        };
                        let term = self
                            .clause_term(
                                id,
                                scope,
                                authored.value,
                                ClauseUse::Operand(declared[position], values),
                            )?
                            .operand(id)?;
                        if fields[position].replace(term).is_some() {
                            return Err(unavailable("clause constructor names a field twice"));
                        }
                    }
                    ClauseOperand::Application {
                        declaration: ClauseDeclaration {
                            member: (symbol.arena_index(), symbol.generation()),
                            supplier: None,
                        },
                        arguments: fields
                            .into_iter()
                            .collect::<Option<_>>()
                            .ok_or_else(|| unavailable("clause constructor misses a field"))?,
                    }
                }
                (ExpressionNode::Call(call), carrier)
                    if abstract_application || carrier.is_none_or(|ty| ty.scalar().is_none()) =>
                {
                    let target = call.target_symbol;
                    if !typed.symbols.symbols().nodes().is_valid(target) {
                        return Err(unavailable(
                            "clause application lost its selected declaration",
                        ));
                    }
                    let owner = self.contract_operator_owner(id, scope)?;
                    let invocation = self.static_invocation(
                        crate::source_state::references::StaticArgumentCall::Expression(
                            expression, call,
                        ),
                    );
                    if invocation.unsupported
                        || call.operational_acknowledgement.acknowledges_suspend
                        || call.operational_acknowledgement.acknowledges_block
                    {
                        return Err(unavailable(
                            "clause application has another dispatch or effect",
                        ));
                    }
                    self.direct_selection(
                        id,
                        target,
                        call.authored_call_selection,
                        call.authored_call_binding,
                    )?;
                    let receiver = table.name_path_node(call.receiver).map(|name| name.symbol);
                    if call.receiver.is_valid() && receiver.is_none() {
                        return Err(unavailable(
                            "clause application has no selected static receiver",
                        ));
                    }
                    let parameter = self.callable_parameter(target);
                    let supplier = receiver
                        .or(self.clause_supplier)
                        .filter(|_| bodyless.is_none() && parameter.is_none());
                    let member = bodyless
                        .as_ref()
                        .map(|(machine, _)| *machine)
                        .unwrap_or_else(|| {
                            typed
                                .closed_requirement_application(target)
                                .map_or(target, |row| row.template)
                        });
                    let realization = supplier
                        .and_then(|supplier| self.closed_conformance_requirement(supplier, target))
                        .transpose()
                        .map_err(unavailable)?;
                    let signature = if let Some(parameter) = parameter {
                        Some(parameter)
                    } else if let Some((_, signature)) = &bodyless {
                        Some(signature)
                    } else if let Some(realization) = &realization {
                        Some(realization)
                    } else if let Some(supplier) = supplier {
                        let lexical = typed.symbols.get(supplier).parent;
                        if receiver.is_some()
                            && typed
                                .static_record_parameter_schema(owner, supplier)
                                .map_err(|_| {
                                    unavailable("clause application uses a foreign record")
                                })?
                                .is_none()
                        {
                            return Err(unavailable("clause application has no record in scope"));
                        }
                        typed
                            .static_record_bound_requirement(lexical, supplier, target)
                            .map_err(|_| {
                                unavailable("clause application lost its applied requirement")
                            })?
                            .map(|required| required.signature)
                    } else {
                        let owner = typed
                            .closed_requirement_application(owner)
                            .map_or(owner, |row| row.template);
                        let schema = typed.symbols.get(member).parent;
                        if typed.symbols.get(owner).parent != schema {
                            return Err(unavailable(
                                "clause operation has no supplying declaration",
                            ));
                        }
                        typed.trait_definition_by_symbol(schema).and_then(|schema| {
                            typed.trait_machine_signature_by_symbol(schema, target)
                        })
                    }
                    .ok_or_else(|| {
                        unavailable("clause application has no requirement signature")
                    })?;
                    let envelope = self.static_envelope(id, signature, false)?;
                    // A machine parameter of a mathematical machine takes
                    // and returns data that has no layout, so no machine
                    // that fits it runs anywhere: it reaches no service and
                    // has no crash route to declare. What it must promise is
                    // that it terminates, since a clause gives its call a
                    // value.
                    let runs = parameter.is_none();
                    if (runs
                        && (!matches!(envelope.service_reach, crate::callables::static_contracts::SourceServiceCeiling::Closed(ref row) if row.is_empty())
                            || !envelope.invocations.is_empty()
                            || !signature.declares_no_crash_route))
                        || !matches!(envelope.termination, terminal_psi::TerminalCallableTermination::Terminates { ref premises } if premises.is_empty())
                        || envelope.suspends
                        || envelope.blocks
                        || !signature.contracts.is_empty()
                        || !call.machine_arguments.is_empty()
                        || !call.evidence_arguments.is_empty()
                        || call.runtime_call.is_some()
                        || call.build_selection.is_valid()
                    {
                        return Err(unavailable(
                            "clause application needs an unconditional pure and total operation",
                        ));
                    }
                    let result = self.value_type(id, signature.return_type)?;
                    if expected.is_some_and(|expected| expected != result) {
                        return Err(unavailable(
                            "clause application result differs from the written carrier",
                        ));
                    }
                    expected = Some(result);
                    let parameters = typed
                        .state_parameters
                        .span(signature.parameters)
                        .ok_or_else(|| unavailable("clause application lost its parameters"))?;
                    let actuals = table.expression_handles(call.arguments);
                    if parameters.len() != actuals.len() {
                        return Err(unavailable(
                            "clause application arity differs from its signature",
                        ));
                    }
                    let mut arguments = Vec::with_capacity(actuals.len());
                    for (formal, &actual) in parameters.iter().zip(actuals) {
                        // A shared borrowed formal (`left: @T`) observes the
                        // lent value, so its operand is that value at `T`.
                        let observed = match typed
                            .type_reference_table
                            .type_reference(formal.type_reference)
                        {
                            crate::source_state::types::TypeReferenceNode::Reference {
                                referee,
                                access: language_core::ReferenceAccess::Shared,
                                ..
                            } if !formal.is_mutable => *referee,
                            crate::source_state::types::TypeReferenceNode::Reference { .. } => {
                                return Err(unavailable(
                                    "clause application lends a value for change",
                                ));
                            }
                            _ => formal.type_reference,
                        };
                        let ty = self.value_type(id, observed)?;
                        arguments.push(
                            self.clause_term(id, scope, actual, ClauseUse::Operand(ty, values))?
                                .operand(id)?,
                        );
                    }
                    let declaration = ClauseDeclaration {
                        member: (member.arena_index(), member.generation()),
                        supplier: supplier
                            .map(|symbol| (symbol.arena_index(), symbol.generation())),
                    };
                    if let Some(realization) = &realization {
                        let carriers = arguments
                            .iter()
                            .map(|&argument| {
                                values.get(argument as usize).map(|value| value.carrier)
                            })
                            .collect::<Option<Vec<_>>>()
                            .ok_or_else(|| unavailable("clause application lost an operand"))?;
                        self.record_applied_operation(
                            id,
                            realization.symbol,
                            declaration,
                            &carriers,
                            result,
                        )?;
                    }
                    ClauseOperand::Application {
                        declaration,
                        arguments,
                    }
                }
                (_, Some(expected)) if expected.scalar().is_some() => {
                    let (term, actual) = self
                        .clause_term(id, scope, expression, ClauseUse::Scalar(expected.scalar()))?
                        .scalar(id)?;
                    if expected != actual {
                        return Err(unavailable(
                            "Identity operand differs from the written carrier",
                        ));
                    }
                    ClauseOperand::Scalar(term)
                }
                (ExpressionNode::Name(path), Some(expected))
                    if path.reserved_result_of.is_valid()
                        || scope
                            .subject
                            .is_some_and(|subject| subject.symbol == path.symbol) =>
                {
                    let (root, path, ty) = self
                        .projected(id, scope, expression)?
                        .ok_or_else(|| unavailable("Identity operand has no bound subject"))?;
                    if ty != expected {
                        return Err(unavailable(
                            "Identity subject differs from the written carrier",
                        ));
                    }
                    ClauseOperand::Input(semantic_vocabulary::StructuralCaseSubject::new(
                        root, path,
                    ))
                }
                (ExpressionNode::Name(path), Some(expected))
                    if !path.static_application.is_valid()
                        && !path.reserved_result_of.is_valid()
                        && table.name_path_members(path.members).len() == 1 =>
                {
                    let (position, slot) = scope
                        .slots
                        .iter()
                        .enumerate()
                        .find(|(_, slot)| slot.symbol == path.symbol)
                        .ok_or_else(|| unavailable("Identity operand has no bound formal"))?;
                    if slot.value_type != expected || slot.borrowed.is_some() {
                        return Err(unavailable(
                            "Identity operand has no exact input observation",
                        ));
                    }
                    if matches!(expected, ValueType::Rigid(_)) {
                        ClauseOperand::Formal(
                            u32::try_from(position)
                                .map_err(|_| unavailable("formal ordinal overflow"))?,
                        )
                    } else {
                        ClauseOperand::Input(semantic_vocabulary::StructuralCaseSubject::new(
                            formal_place(id, scope.slots, position)?,
                            Vec::new(),
                        ))
                    }
                }
                // A shared borrow handed to an operation that observes it,
                // as `a == b` lends both operands to the selected `equals`,
                // names the value it observes: a by-value formal itself, or
                // the referent at a borrowed formal's place, read at the
                // clause's point like `$a`.
                (ExpressionNode::Borrow(borrow), Some(expected))
                    if !self.mathematical
                        && borrow.access == language_core::ReferenceAccess::Shared =>
                {
                    let target = borrow.target;
                    if let ExpressionNode::Name(path) = table.expression(target)
                        && !path.static_application.is_valid()
                        && !path.reserved_result_of.is_valid()
                        && table.name_path_members(path.members).len() == 1
                        && let Some((position, slot)) = scope
                            .slots
                            .iter()
                            .enumerate()
                            .find(|(_, slot)| slot.symbol == path.symbol)
                        && slot.borrowed.is_some()
                    {
                        // As for `$a` below: only a shared loan keeps the
                        // referent the clause names.
                        if slot.value_type != expected
                            || slot.borrowed.as_ref().is_some_and(|borrowed| {
                                *borrowed != language_core::ReferenceAccess::Shared
                            })
                            || slot.erased
                            || matches!(expected, ValueType::Rigid(_))
                        {
                            return Err(unavailable(
                                "Identity operand has no exact input observation",
                            ));
                        }
                        ClauseOperand::Input(semantic_vocabulary::StructuralCaseSubject::new(
                            formal_place(id, scope.slots, position)?,
                            Vec::new(),
                        ))
                    } else {
                        return self.clause_term(
                            id,
                            scope,
                            target,
                            ClauseUse::Operand(expected, values),
                        );
                    }
                }
                // Explicit dereference (`$a`) of a borrowed structural formal
                // names the referent's value at that formal's place.
                (ExpressionNode::Unary(unary), Some(expected))
                    if !self.mathematical
                        && matches!(unary.operator, UnaryOperator::Dereference)
                        && let ExpressionNode::Name(path) = table.expression(unary.operand)
                        && !path.static_application.is_valid()
                        && !path.reserved_result_of.is_valid()
                        && table.name_path_members(path.members).len() == 1 =>
                {
                    let (position, slot) = scope
                        .slots
                        .iter()
                        .enumerate()
                        .find(|(_, slot)| slot.symbol == path.symbol)
                        .ok_or_else(|| unavailable("Identity operand has no bound formal"))?;
                    // The Input is the referent at entry. Only a shared loan
                    // keeps it there: the machine may write through a
                    // `@mutable` formal, after which a fact about the entry
                    // value says nothing of the referent a call observes.
                    if slot.value_type != expected
                        || slot.borrowed.as_ref().is_none_or(|borrowed| {
                            *borrowed != language_core::ReferenceAccess::Shared
                        })
                    {
                        return Err(unavailable(
                            "Identity operand has no exact input observation",
                        ));
                    }
                    let owner = self.contract_operator_owner(id, scope)?;
                    self.complete_anonymous_operators(id, owner, &[expression])?;
                    ClauseOperand::Input(semantic_vocabulary::StructuralCaseSubject::new(
                        formal_place(id, scope.slots, position)?,
                        Vec::new(),
                    ))
                }
                // A value enters a quotient by `as`: its class is the
                // quotient's declaration applied to it.
                (ExpressionNode::Cast(cast), Some(expected))
                    if self.mathematical && self.quotient_of(expected).is_some() =>
                {
                    let quotient = self.quotient_of(expected).expect("the guard found it");
                    let (declaration, carrier) =
                        (quotient.declaration, ValueType::record(quotient.carrier));
                    if self.value_type(id, cast.target_type)? != expected {
                        return Err(unavailable(
                            "Identity operand differs from the written carrier",
                        ));
                    }
                    let value = self
                        .clause_term(id, scope, cast.value, ClauseUse::Operand(carrier, values))?
                        .operand(id)?;
                    ClauseOperand::Application {
                        declaration: ClauseDeclaration {
                            member: (declaration.arena_index(), declaration.generation()),
                            supplier: None,
                        },
                        arguments: vec![value],
                    }
                }
                // A field of data that has storage is the input's value at
                // that field path; the checker projects it from the record.
                (ExpressionNode::Member(_), Some(expected)) if !self.mathematical => {
                    let (root, path, ty) =
                        self.projected(id, scope, expression)?.ok_or_else(|| {
                            unavailable("Identity operand has no term at its written carrier")
                        })?;
                    if ty != expected {
                        return Err(unavailable(
                            "Identity operand differs from the written carrier",
                        ));
                    }
                    ClauseOperand::Input(semantic_vocabulary::StructuralCaseSubject::new(
                        root, path,
                    ))
                }
                // A field of a value is the field's declaration applied to
                // that value, whatever the value is: a formal, a call or
                // another field.
                (ExpressionNode::Member(member), Some(expected)) if self.mathematical => {
                    let selected = member.selection.field_symbol();
                    // The field's declaration says whose field it is. A
                    // field of a call of a machine parameter has no such
                    // selection: the call's declared result is the record,
                    // and the field is the one of that name.
                    let receiver = if selected.is_valid() {
                        self.record(id, typed.symbols.get(selected).parent)?
                    } else {
                        match table.expression(member.receiver) {
                            ExpressionNode::Call(call) => {
                                let signature =
                                    self.callable_parameter(call.target_symbol).ok_or_else(
                                        || unavailable("clause member without a field symbol"),
                                    )?;
                                self.value_type(id, signature.return_type)?
                            }
                            _ => return Err(unavailable("clause member without a field symbol")),
                        }
                    };
                    let opened = self
                        .opened(id, Some(receiver), Access::Read)?
                        .ok_or_else(|| unavailable("member of something that is not data"))?;
                    let (field, position) = if selected.is_valid() {
                        let super::places::Member::Field(position) =
                            self.member_of(id, opened, selected)?
                        else {
                            return Err(unavailable("clause member of one case of its data"));
                        };
                        (selected, position)
                    } else {
                        let position = self
                            .data_type(id, opened.record())?
                            .fields()
                            .iter()
                            .position(|field| field.identity == member.member.as_str())
                            .ok_or_else(|| {
                                unavailable("clause member names no field of its record")
                            })?;
                        let field = terminal_check_tables::row(
                            &self.record_sources,
                            opened.record(),
                            id,
                            Construct::Machine,
                        )?
                        .fields
                        .get(position)
                        .copied()
                        .ok_or_else(|| unavailable("clause member names no field of its record"))?;
                        (field, position)
                    };
                    if self.data_type(id, opened.record())?.fields()[position].value_type
                        != expected
                    {
                        return Err(unavailable(
                            "Identity operand differs from the written carrier",
                        ));
                    }
                    let value = self
                        .clause_term(
                            id,
                            scope,
                            member.receiver,
                            ClauseUse::Operand(receiver, values),
                        )?
                        .operand(id)?;
                    ClauseOperand::Application {
                        declaration: ClauseDeclaration {
                            member: (field.arena_index(), field.generation()),
                            supplier: None,
                        },
                        arguments: vec![value],
                    }
                }
                // The class of a value of data that has storage is the
                // quotient's declaration applied to it: an opaque value of
                // the quotient, equal to another class only by what the
                // checker proves (`QuotientSoundness`).
                (ExpressionNode::Cast(cast), Some(expected))
                    if !self.mathematical && self.executable_quotient_of(expected).is_some() =>
                {
                    let quotient = self
                        .executable_quotient_of(expected)
                        .expect("the guard found it");
                    let (declaration, carrier) = (quotient.declaration, quotient.carrier);
                    if self.value_type(id, cast.target_type)? != expected {
                        return Err(unavailable(
                            "Identity operand differs from the written carrier",
                        ));
                    }
                    let value = self
                        .clause_term(id, scope, cast.value, ClauseUse::Operand(carrier, values))?
                        .operand(id)?;
                    ClauseOperand::Application {
                        declaration: ClauseDeclaration {
                            member: (declaration.arena_index(), declaration.generation()),
                            supplier: None,
                        },
                        arguments: vec![value],
                    }
                }
                _ => {
                    return Err(unavailable(
                        "Identity operand has no term at its written carrier",
                    ));
                }
            };
            let expected = expected.ok_or_else(|| unavailable("clause operand lost its type"))?;
            let carrier = match expected {
                ValueType::Rigid(parameter) => ClauseCarrier::Parameter(parameter.0),
                ValueType::Record(record, _, _) => {
                    match self.records.get(record).map(|record| &record.shape) {
                        Some(crate::StorageShape::TypeParameter(parameter)) => {
                            ClauseCarrier::Parameter(parameter.0)
                        }
                        Some(_) => ClauseCarrier::Structural(record),
                        None => {
                            return Err(unavailable(
                                "Identity carrier has no structural declaration",
                            ));
                        }
                    }
                }
                ValueType::Scalar(scalar, _) => ClauseCarrier::Scalar(scalar),
            };
            let position = u32::try_from(values.len())
                .map_err(|_| unavailable("clause term table overflow"))?;
            // An operation of a const record parameter is the same function
            // wherever a clause applies it, so its record keeps it once for
            // the Identity table of a generic check form.
            if let ClauseOperand::Application {
                declaration,
                arguments,
            } = &operand
                && let Some((index, generation)) = declaration.supplier
                && let Some(binding) = self
                    .static_contracts
                    .record_binding_mut(symbols::SymbolHandle::from_parts(index, generation))
            {
                let applied = terminal_check_tables::mathematics::IdentityOperation {
                    declaration: *declaration,
                    domains: arguments
                        .iter()
                        .filter_map(|&argument| values.get(argument as usize))
                        .map(|argument| argument.carrier)
                        .collect(),
                    result: carrier,
                };
                if !binding.account.applied.contains(&applied) {
                    binding.account.applied.push(applied);
                }
            }
            values.push(ClauseValue { carrier, operand });
            if reading_scalar {
                let term = crate::body::scalar_clause_value(inferred).ok_or_else(|| {
                    unavailable("clause application needs a scalar result and operands")
                })?;
                return Ok(ClauseTerm::Scalar(term, expected));
            }
            return Ok(ClauseTerm::Operand(position));
        }
        let ExpressionNode::Call(call) = table.expression(expression) else {
            return Err(unavailable("clause application lost its selected call"));
        };
        let unfolded = self.unfold(id, scope, call)?;
        let (term, used) =
            self.within_unfolding(id, unfolded.machine, unfolded.bound.len(), |program| {
                let callee = ClauseScope {
                    slots: &unfolded.callee_slots,
                    values: &[],
                    bound: &unfolded.bound,
                    owner: unfolded.machine,
                    result: ResultTerm::Absent("result in a called machine's body"),
                    subject: None,
                    formal_fields: unfolded.formal_fields,
                    logical: None,
                };
                for &req_expr in &unfolded.requires {
                    let req_prop = program.clause(id, &callee, req_expr)?;
                    if !program.enclosing_requires.contains(&req_prop) {
                        return Err(refuse_at(
                            id,
                            Unserved::Contract(
                                "called machine in a clause has an unproved precondition",
                            ),
                        ));
                    }
                }
                match usage {
                    ClauseUse::Operand(..)
                    | ClauseUse::ProofRelevantType
                    | ClauseUse::Scalar(_)
                    | ClauseUse::Integer => {
                        unreachable!("every use but a proposition and a call value is read above")
                    }
                    ClauseUse::Proposition if unfolded.result == ScalarType::Boolean => program
                        .clause(id, &callee, unfolded.body)
                        .map(ClauseTerm::Proposition),
                    ClauseUse::Proposition => {
                        Err(unavailable("called predicate whose result is not Boolean"))
                    }
                    ClauseUse::Value => {
                        let (value, ty) = program
                            .clause_term(
                                id,
                                &callee,
                                unfolded.body,
                                ClauseUse::Scalar(Some(unfolded.result)),
                            )?
                            .scalar(id)?;
                        if ty.scalar() != Some(unfolded.result) {
                            return Err(unavailable(
                                "called machine's body differs from its result type",
                            ));
                        }
                        Ok(ClauseTerm::Scalar(value, ValueType::plain(unfolded.result)))
                    }
                }
            })?;
        // A scalar term still contains each used argument. Reading a
        // predicate may discard a branch, so all its arguments must form.
        self.retain_call_arguments(
            id,
            unfolded
                .bound
                .into_iter()
                .zip(used)
                .filter_map(|(bound, used)| {
                    (matches!(usage, ClauseUse::Proposition) || !used).then_some(bound)
                }),
        )?;
        Ok(term)
    }
}

impl Program<'_> {
    /// The operation a clause names when it calls a row of a closed named
    /// implementation, such as `CoinByValue::equals` or `a == b` over data
    /// whose selected comparison is that row: the requirement's applied
    /// header with the selected member's own written envelope. A requirement
    /// that omits `reaches`, `crashes` or `terminates` defers to the selected
    /// member (generics.md), so `Equatable`'s bare `equals` admits a member
    /// written `reaches none crashes none terminates` and no other. `None`
    /// when `supplier` is not such an implementation, so the caller tries its
    /// other suppliers. The term keeps the implementation as its supplier,
    /// so two implementations of one requirement are different operations
    /// (contracts.md, "Equality bridges and comparison facts": "Recognize
    /// the checked law's form and exact applied equality row, not its name
    /// or another visible implementation for the same `T`").
    fn closed_conformance_requirement(
        &self,
        supplier: SymbolHandle,
        requirement: SymbolHandle,
    ) -> Option<Result<crate::source_state::signature::StateSignature, &'static str>> {
        let typed = self.typed;
        let conformance = typed.conformance_by_symbol(supplier)?;
        Some((|| {
            if !conformance.type_parameters.is_empty()
                || !conformance.lifetime_parameters.is_empty()
            {
                return Err("clause application of a generic implementation");
            }
            let row = conformance
                .rows
                .iter()
                .find(|row| row.requirement == requirement)
                .ok_or("clause application names no row of its implementation")?;
            let scope = crate::source_state::trait_definition::AppliedTraitScope::from_conformance(
                typed,
                conformance,
            )
            .and_then(|root| root.declaring_scope(typed, row.declaring_trait))
            .ok_or("clause application lost its implementation's trait application")?;
            let header = typed
                .applied_callable_signature(&scope, requirement)
                .ok_or("clause application has no applied requirement header")?;
            let member = typed
                .machine_by_symbol(row.realization_machine)
                .ok_or("clause application lost its selected member")?;
            Ok(crate::source_state::signature::StateSignature {
                invokes: member.invokes,
                service_reach_row: member.service_reach_row,
                service_reach_is_installation_bound: member.service_reach_is_installation_bound,
                suspends: member.suspends,
                blocks: member.blocks,
                contracts: member.contracts,
                termination_guarantee: match &member.termination_plan.interface {
                    source_declarations::TerminationInterface::Published(guarantee) => {
                        guarantee.clone()
                    }
                    _ => source_declarations::TerminationGuarantee::NoGuarantee,
                },
                declares_no_crash_route: member.declares_no_crash_route,
                // The written ceiling belongs to the member.
                symbol: member.symbol,
                ..header.clone()
            })
        })())
    }

    /// `X == true`, `X == false`, `X != true` or `X != false` (either side)
    /// where `X` calls a row of a closed named implementation: the
    /// proposition that `X`'s result is that Boolean. Stage 03 expands
    /// `a != b` over data with a user comparison to `equals(@a, @b) ==
    /// false`, and an applied comparison over records has no scalar term to
    /// compare. `None` for any other comparison, which the scalar reader
    /// takes.
    pub(super) fn applied_boolean_comparison(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        comparison: BinaryOperator,
        left: ExpressionHandle,
        right: ExpressionHandle,
    ) -> Checked<Option<Proposition>> {
        let typed = self.typed;
        let table = &typed.expression_table;
        // The other side is a literal, or, for `==`, a truth value the
        // operation's result is stated to equal: `result == (a == b)`.
        let (applied, other) = match (table.expression(left), table.expression(right)) {
            (ExpressionNode::Call(_), ExpressionNode::Boolean(literal)) => (left, Ok(*literal)),
            (ExpressionNode::Boolean(literal), ExpressionNode::Call(_)) => (right, Ok(*literal)),
            (ExpressionNode::Call(_), _) if comparison == BinaryOperator::Equal => {
                (left, Err(right))
            }
            (_, ExpressionNode::Call(_)) if comparison == BinaryOperator::Equal => {
                (right, Err(left))
            }
            _ => return Ok(None),
        };
        let ExpressionNode::Call(call) = table.expression(applied) else {
            return Ok(None);
        };
        let implementation = table
            .name_path_node(call.receiver)
            .map(|name| name.symbol)
            .filter(|&receiver| typed.conformance_by_symbol(receiver).is_some());
        if !call.receiver.is_valid() || implementation.is_none() {
            return Ok(None);
        }
        let ClauseTerm::Proposition(Proposition::DeclarationIdentity(mut identity)) =
            self.clause_term(id, scope, applied, ClauseUse::Proposition)?
        else {
            return Err(refuse_at(
                id,
                Unserved::Contract("applied comparison is not a Boolean result"),
            ));
        };
        let stated_term = match other {
            Ok(literal) => ScalarTerm::Boolean(literal == (comparison == BinaryOperator::Equal)),
            Err(other) => match self.clause_term(
                id,
                scope,
                other,
                ClauseUse::Scalar(Some(ScalarType::Boolean)),
            )? {
                ClauseTerm::Scalar(term, _) if term.scalar_type() == ScalarType::Boolean => term,
                _ => {
                    return Err(refuse_at(
                        id,
                        Unserved::Contract("applied comparison equals no truth value"),
                    ));
                }
            },
        };
        let stated = identity
            .values
            .get_mut(identity.right as usize)
            .ok_or_else(|| {
                refuse_at(id, Unserved::Contract("applied comparison lost its result"))
            })?;
        stated.operand = ClauseOperand::Scalar(stated_term);
        Ok(Some(Proposition::DeclarationIdentity(identity)))
    }

    /// Record, for the machine `id` whose clause names a selected row's
    /// operation, what a call of the row's member returns: the operation
    /// applied to the member's own formals. A member whose formals are not
    /// all shared loans records nothing, so its calls stay opaque.
    fn record_applied_operation(
        &mut self,
        id: MachineId,
        member: SymbolHandle,
        declaration: ClauseDeclaration,
        carriers: &[ClauseCarrier],
        result: ValueType,
    ) -> Checked<()> {
        let unavailable = |what| refuse_at(id, Unserved::Contract(what));
        let typed = self.typed;
        let Some(ScalarType::Boolean) = result.scalar() else {
            return Ok(());
        };
        let entry = typed
            .machine_by_symbol(member)
            .and_then(|machine| typed.machine_states(machine).first())
            .ok_or_else(|| unavailable("clause application lost its member's entry"))?;
        let parameters = typed.state_parameters(entry);
        if parameters.len() != carriers.len() {
            return Err(unavailable(
                "clause application arity differs from its member",
            ));
        }
        let mut values = Vec::with_capacity(carriers.len() + 2);
        for (position, (parameter, &carrier)) in parameters.iter().zip(carriers).enumerate() {
            let (_, erased, borrowed) = self.formal(id, parameter)?;
            if erased
                || parameter.is_mutable
                || borrowed.is_none_or(|borrowed| {
                    borrowed.access != language_core::ReferenceAccess::Shared
                })
            {
                return Ok(());
            }
            // Every formal is a shared loan, so the member's places are
            // its formals in order (`contracts::formal_place`).
            let place = semantic_vocabulary::PlaceId::new(position as u64 + 1)
                .ok_or_else(|| unavailable("formal place identity"))?;
            values.push(ClauseValue {
                carrier,
                operand: ClauseOperand::Input(semantic_vocabulary::StructuralCaseSubject::new(
                    place,
                    Vec::new(),
                )),
            });
        }
        let applied =
            u32::try_from(values.len()).map_err(|_| unavailable("clause term table overflow"))?;
        values.push(ClauseValue {
            carrier: ClauseCarrier::Scalar(ScalarType::Boolean),
            operand: ClauseOperand::Application {
                declaration,
                arguments: (0..applied).collect(),
            },
        });
        // With no scalar formal, the result is the member's first value
        // (`machine::signature`'s `result_value`).
        let result_value =
            semantic_vocabulary::ValueId::new(1).ok_or_else(|| unavailable("result identity"))?;
        values.push(ClauseValue {
            carrier: ClauseCarrier::Scalar(ScalarType::Boolean),
            operand: ClauseOperand::Scalar(ScalarTerm::value(result_value, ScalarType::Boolean)),
        });
        let statement = Proposition::DeclarationIdentity(semantic_vocabulary::ClauseIdentity {
            values,
            left: applied,
            right: applied + 1,
        });
        if !self
            .applied_operations
            .iter()
            .any(|(machine, callee, known)| {
                *machine == id && *callee == member && *known == statement
            })
        {
            self.applied_operations.push((id, member, statement));
        }
        Ok(())
    }
}
