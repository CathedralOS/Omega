/*
A clause that calls a pure, total declaration can name the result even when
the declaration has no body. The term records the selected operation, its
supplying record, and each typed argument. A cited law can then prove an
identity about those results for an arbitrary carrier.

For a closed scalar helper such as `domain bool::Held requires premise(self);`,
the reader can also compute the term by reading the helper's body with its
formal parameters bound to the caller's arguments.

`wiki/spec/language/domains.md`: "Eligible total, pure machine calls may
occur as denotational value terms inside propositions", and "Predicate calls
retain their ordinary formation and evaluation obligations."
`wiki/spec/terminal-psi/fact_validity.md`: "Pure call congruence requires the
same admitted operation and unchanged argument values". Unfolding meets that
requirement by construction: two calls of one machine on the same argument
terms unfold to the same proposition, and no fact about the call outlives its
argument terms.

Only a machine whose meaning is exactly one body expression the clause reader
itself accepts is unfolded, so the call adds no operation a clause could not
already state. The callee must be closed (no type parameters or static
arguments), take plain by-value scalar parameters and end in a single value
expression. Scalar helpers declare no contracts. Proof-only IEEE views may
declare result guarantees that their checked body establishes for every
input (`float_meaning`). A callee with `requires`
would owe them at the call, which this reader does not yet discharge, so it
is refused, as is any recursion through unfolding.
*/

use crate::source_state::expression::TableCallExpression;
use crate::source_state::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};
use semantic_vocabulary::{MachineId, Proposition, ScalarTerm, ScalarType, ValueId};
use symbols::{SymbolHandle, SymbolKind};
use terminal_check_tables::{Checked, Construct};

use super::Program;
use super::contracts::{ClauseScope, ResultTerm};
use crate::lowering::body::{Qualification, ValueType};
use crate::lowering::{Unserved, refuse};

/// A called machine prepared for unfolding: its parameter bindings, its body
/// expression and its declared result.
pub(super) struct Unfolded<R> {
    pub(super) machine: SymbolHandle,
    pub(super) bound: Vec<(SymbolHandle, ScalarTerm)>,
    pub(super) body: crate::source_state::expression::ExpressionHandle,
    pub(super) result: R,
}

/// A scalar term forms all its operands. Track substitutions during its
/// expansion so only discarded arguments need a separate formation clause.
pub(super) struct Unfolding {
    machine: SymbolHandle,
    used: Vec<bool>,
}

pub(super) enum ClauseUse<'a> {
    Operand(ValueType, &'a mut Vec<semantic_vocabulary::ClauseValue>),
    Proposition,
    ProofRelevantType,
    Value,
}

pub(super) enum ClauseTerm {
    Operand(u32),
    TypeParameter(u32),
    Proposition(Proposition),
    Scalar(ScalarTerm, ValueType),
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
            _ => Err(refuse(
                id,
                Construct::Machine,
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
            _ => Err(refuse(
                id,
                Construct::Machine,
                Unserved::Contract("expected a value operand of Identity"),
            )),
        }
    }
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
            || !call.machine_arguments.is_empty()
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
                self.proof_machine(declared)
                    && typed
                        .machine_states(declared)
                        .first()
                        .is_some_and(|entry| entry.return_type.is_valid())
            })
            .map(|declared| declared.symbol)
    }

    /// The scope binds names. The requested use gives calls and both operands
    /// of Identity the same interpretation in each enclosing source clause.
    pub(super) fn clause_term(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        expression: crate::source_state::expression::ExpressionHandle,
        mut usage: ClauseUse<'_>,
    ) -> Checked<ClauseTerm> {
        use crate::source_state::expression::ExpressionNode;
        use semantic_vocabulary::{ClauseCarrier, ClauseDeclaration, ClauseOperand, ClauseValue};
        let typed = self.typed;
        let table = &typed.expression_table;
        let unavailable = |what| refuse(id, Construct::Machine, Unserved::Contract(what));
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
        if abstract_application && matches!(usage, ClauseUse::Proposition) {
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
                    let constructed = self.record(id, literal.type_symbol)?;
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
                    let supplier = receiver
                        .or(self.clause_supplier)
                        .filter(|_| bodyless.is_none());
                    let member = bodyless
                        .as_ref()
                        .map(|(machine, _)| *machine)
                        .unwrap_or_else(|| {
                            typed
                                .closed_requirement_application(target)
                                .map_or(target, |row| row.template)
                        });
                    let signature = if let Some((_, signature)) = &bodyless {
                        Some(signature)
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
                    if !matches!(envelope.service_reach, crate::lowering::static_contracts::SourceServiceCeiling::Closed(ref row) if row.is_empty())
                        || !matches!(envelope.termination, terminal_psi::TerminalCallableTermination::Terminates { ref premises } if premises.is_empty())
                        || !envelope.invocations.is_empty()
                        || envelope.suspends
                        || envelope.blocks
                        || !signature.declares_no_crash_route
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
                        let ty = self.value_type(id, formal.type_reference)?;
                        arguments.push(
                            self.clause_term(id, scope, actual, ClauseUse::Operand(ty, values))?
                                .operand(id)?,
                        );
                    }
                    ClauseOperand::Application {
                        declaration: ClauseDeclaration {
                            member: (member.arena_index(), member.generation()),
                            supplier: supplier
                                .map(|symbol| (symbol.arena_index(), symbol.generation())),
                        },
                        arguments,
                    }
                }
                (_, Some(expected)) if expected.scalar().is_some() => {
                    let (term, actual) = self.term(id, scope, expression, expected.scalar())?;
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
                            super::contracts::formal_place(id, scope.slots, position)?,
                            Vec::new(),
                        ))
                    }
                }
                // A field of a formal, or of a field of one: the formal's
                // place and the path beneath it.
                (ExpressionNode::Member(_), Some(expected)) if self.mathematical => {
                    let fields = ClauseScope {
                        formal_fields: true,
                        ..*scope
                    };
                    let (root, path, ty) = self
                        .projected(id, &fields, expression)?
                        .ok_or_else(|| unavailable("Identity operand has no bound formal"))?;
                    if ty != expected {
                        return Err(unavailable(
                            "Identity operand differs from the written carrier",
                        ));
                    }
                    ClauseOperand::Input(semantic_vocabulary::StructuralCaseSubject::new(
                        root, path,
                    ))
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
                        Some(crate::lowering::StorageShape::TypeParameter(parameter)) => {
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
            values.push(ClauseValue { carrier, operand });
            if reading_scalar {
                let term =
                    crate::lowering::body::scalar_clause_value(inferred).ok_or_else(|| {
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
                let callee = callee_scope(&unfolded.bound, unfolded.machine);
                match usage {
                    ClauseUse::Operand(..) | ClauseUse::ProofRelevantType => {
                        unreachable!("operands and proof-relevant types are read above")
                    }
                    ClauseUse::Proposition if unfolded.result == ScalarType::Boolean => program
                        .clause(id, &callee, unfolded.body)
                        .map(ClauseTerm::Proposition),
                    ClauseUse::Proposition => Err(refuse(
                        id,
                        Construct::Machine,
                        Unserved::Contract("called predicate whose result is not Boolean"),
                    )),
                    ClauseUse::Value => {
                        let (value, ty) =
                            program.term(id, &callee, unfolded.body, Some(unfolded.result))?;
                        if ty.scalar() != Some(unfolded.result) {
                            return Err(refuse(
                                id,
                                Construct::Machine,
                                Unserved::Contract(
                                    "called machine's body differs from its result type",
                                ),
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

    /// Read `body` with `machine` marked as being unfolded, refusing a
    /// machine that is already being unfolded: a recursive predicate has no
    /// finite unfolding.
    pub(super) fn within_unfolding<T>(
        &mut self,
        id: MachineId,
        machine: SymbolHandle,
        arguments: usize,
        body: impl FnOnce(&mut Self) -> Checked<T>,
    ) -> Checked<(T, Vec<bool>)> {
        self.unfolding_calls.push(Unfolding {
            machine,
            used: vec![false; arguments],
        });
        let read = body(self);
        let unfolding = self.unfolding_calls.pop().ok_or_else(|| {
            refuse(
                id,
                Construct::Machine,
                Unserved::Contract("called machine lost its unfolding scope"),
            )
        })?;
        if read.is_ok() {
            self.admitted_declarations.include(machine);
            self.admitted_bodies.include(machine);
        }
        read.map(|result| (result, unfolding.used))
    }

    pub(super) fn use_unfolded_argument(
        &mut self,
        id: MachineId,
        owner: SymbolHandle,
        position: usize,
    ) -> Checked<()> {
        if let Some(unfolding) = self
            .unfolding_calls
            .last_mut()
            .filter(|frame| frame.machine == owner)
        {
            let used = unfolding.used.get_mut(position).ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Contract("called machine argument is outside its unfolding scope"),
                )
            })?;
            *used = true;
        }
        Ok(())
    }

    fn unfold(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
    ) -> Checked<Unfolded<ScalarType>> {
        let machine = self.clause_call_machine(id, call)?;
        let mut unfolded = self.unfold_definition(
            id,
            call.target_symbol,
            machine,
            |program, declaration, state| {
                let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
                if !declaration.contracts.is_empty() || !state.contracts.is_empty() {
                    return unserved(
                        "called machine in a clause is not a closed, contract-free body",
                    );
                }
                let Some(result) = program
                    .result_type(id, state.return_type)?
                    .filter(|result| result.qualification() == Qualification::NONE)
                    .and_then(|result| result.scalar())
                else {
                    return unserved("called machine in a clause has no plain scalar result");
                };
                Ok(result)
            },
        )?;
        self.bind_call_arguments(id, scope, call, &mut unfolded.bound)?;
        Ok(unfolded)
    }

    pub(super) fn clause_call_machine(
        &mut self,
        id: MachineId,
        call: &TableCallExpression,
    ) -> Checked<SymbolHandle> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        if call.receiver.is_valid()
            || call.runtime_call.is_some()
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
            return unserved("call in a clause with a receiver, evidence or another selection");
        }
        let target = call.target_symbol;
        if !target.is_valid() || !self.typed.symbols.symbols().nodes().is_valid(target) {
            return unserved("call in a clause without a resolved target");
        }
        let machine = match self.typed.symbols.get(target).kind {
            SymbolKind::Machine => target,
            SymbolKind::State => self.typed.symbols.get(target).parent,
            _ => return unserved("call in a clause of something that is not a machine"),
        };
        self.direct_selection(
            id,
            target,
            call.authored_call_selection,
            call.authored_call_binding,
        )?;
        self.applies_nothing(
            id,
            target,
            call.authored_machine_application,
            &call.machine_arguments,
        )?;
        Ok(machine)
    }

    pub(super) fn unfold_definition<R>(
        &mut self,
        id: MachineId,
        target: SymbolHandle,
        machine: SymbolHandle,
        result: impl FnOnce(
            &mut Self,
            &crate::source_state::machine::Machine,
            &crate::source_state::state::State,
        ) -> Checked<R>,
    ) -> Checked<Unfolded<R>> {
        let unserved = |what| Err(refuse(id, Construct::Machine, Unserved::Contract(what)));
        let typed = self.typed;
        let (declaration, state) = self.declaration(id, machine)?;
        self.served(id, machine)?;
        if target != machine && target != state.symbol {
            return unserved("call in a clause of a state other than the entry");
        }
        if self
            .unfolding_calls
            .iter()
            .any(|frame| frame.machine == machine)
        {
            return unserved("recursive call in a clause");
        }
        if !declaration.body_is_present
            || declaration.suspends
            || declaration.blocks
            || !declaration.type_parameters.is_empty()
        {
            return unserved("called machine in a clause is not a closed, contract-free body");
        }
        let result = result(self, declaration, state)?;
        // The body's one value: a trailing expression, or the unconditional
        // value transition stage 02 writes for a body that is only a value.
        let body = match typed.statement_table.statements(state.statement_nodes) {
            [StatementNode::Expression(body)] => *body,
            [StatementNode::Transition(transition)]
                if transition.patterns.is_empty()
                    && transition.guard == TransitionGuardNode::Always
                    && transition.exit == TransitionExit::Ordinary
                    && !transition.continuation.is_valid()
                    && transition.proof_selectors.is_empty()
                    && transition.target.is_valid() =>
            {
                match typed.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Value(body) => *body,
                    _ => return unserved("called machine in a clause is not one value expression"),
                }
            }
            _ => return unserved("called machine in a clause is not one value expression"),
        };
        let parameters = typed.state_parameters(state);
        let mut bound = Vec::with_capacity(parameters.len());
        for (position, parameter) in parameters.iter().enumerate() {
            let (value_type, erased, borrowed) = self.formal(id, parameter)?;
            if erased || borrowed.is_some() || parameter.is_mutable {
                return unserved("called machine in a clause takes a non-value parameter");
            }
            let ValueType::Scalar(scalar_type, qualification) = value_type else {
                return unserved("called machine in a clause takes a non-scalar parameter");
            };
            if qualification != Qualification::NONE {
                return unserved("called machine in a clause takes a qualified parameter");
            }
            let formal = ValueId::new(position as u64 + 1).ok_or_else(|| {
                refuse(
                    id,
                    Construct::Machine,
                    Unserved::Contract("too many clause parameters"),
                )
            })?;
            bound.push((parameter.symbol, ScalarTerm::value(formal, scalar_type)));
        }
        Ok(Unfolded {
            machine,
            bound,
            body,
            result,
        })
    }

    /// Substitute actuals simultaneously. Each argument is read in the
    /// caller's scope, independently of the other formals being replaced.
    pub(super) fn bind_call_arguments(
        &mut self,
        id: MachineId,
        scope: &ClauseScope<'_>,
        call: &TableCallExpression,
        bound: &mut [(SymbolHandle, ScalarTerm)],
    ) -> Checked<()> {
        let arguments = self
            .typed
            .expression_table
            .expression_handles(call.arguments);
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Contract(what));
        if arguments.len() != bound.len() {
            return Err(unserved(
                "call in a clause with the wrong number of arguments",
            ));
        }
        for ((_, formal), &argument) in bound.iter_mut().zip(arguments) {
            let scalar = formal.scalar_type();
            let (actual, supplied) = self.term(id, scope, argument, Some(scalar))?;
            if supplied.scalar() != Some(scalar) {
                return Err(unserved(
                    "argument in a clause call differs from its parameter type",
                ));
            }
            *formal = actual;
        }
        Ok(())
    }
}

/// The scope the callee's body is read in: its parameters, each bound to the
/// caller's argument term, and nothing of the caller's own names.
pub(super) fn callee_scope<'b>(
    bound: &'b [(SymbolHandle, ScalarTerm)],
    owner: SymbolHandle,
) -> ClauseScope<'b> {
    ClauseScope {
        slots: &[],
        values: &[],
        bound,
        owner,
        result: ResultTerm::Absent("result in a called machine's body"),
        subject: None,
        formal_fields: false,
        logical: None,
    }
}
