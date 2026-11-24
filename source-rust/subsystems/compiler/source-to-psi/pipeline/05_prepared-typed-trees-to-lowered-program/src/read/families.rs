/*
Data indexed by values.

`data Vector<Length: u64, Total: u64> requires .. { case Nil; case
Cons(previous: u64, previous_total: u64, head: u64, tail [erased]:
Vector<previous, previous_total>); }` contains itself, so it has no layout
and no value of it is stored: a value is evidence that such a list exists.
Its parameters are values, and the type a case holds of itself is the
declaration at other values (`Vector<previous, previous_total>`), so the
declaration is a family of types, one per choice of its parameters
(wiki/spec/proofs/inductive_profile.md, "Derived indexed families").

It is read here for the mathematical signature: the parameters as the
family's indices, each case with its payload values and the index every
child sits at, and the declaration's `requires`, which say what a value of
each case needs of the indices and its payload. In the bodies the
mathematical reader reads, every application of the declaration is one
record type; which index a value is at is the signature's to track.

A theorem over such data is a machine that returns nothing and takes one
erased value of it:

    machine vector_bound<Length: u64, Total: u64>(evidence [erased]: Vector<Length, Total>)
    ensures embed(Total) <= 10 * embed(Length)
    {
        transition evidence {
            Vector::Nil { .. } -> done(),
            Vector::Cons { previous, previous_total, head, tail, .. } -> step(previous, previous_total, head, tail)
        }
        machine done() requires Length == 0 && Total == 0 {}
        machine step(previous: u64, .., tail [erased]: Vector<previous, previous_total>)
        requires .. { vector_bound::<previous, previous_total>(tail); }
    }

Its body is an induction over the evidence and is not lowered. It is read
here as, for each case of the data, what the step that case calls requires
and which children the step cites the theorem at. Only this shape is read:
one dispatch on the evidence with one arm per case, each arm ending or
calling a machine declared inside the theorem with the case's own payload,
and that machine's body a list of calls of the theorem at a child.
*/

use crate::source_state::data::{DataDefinition, DataMember, TypeParameterKind};
use crate::source_state::expression::{ExpressionHandle, ExpressionNode, MemberSelection};
use crate::source_state::machine::Machine;
use crate::source_state::signature::SignatureContractKind;
use crate::source_state::state::State;
use crate::source_state::statement::{StatementNode, TransitionExit, TransitionTargetNode};
use crate::source_state::types::{TypeReferenceHandle, TypeReferenceNode};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentPlaceVersion, IntegerType, MachineId, Proposition,
    ScalarTerm, ScalarType, StructuralCaseId, StructuralFieldId, StructuralTypeId, ValueId,
};
use source_text::SourceSpan;
use symbols::SymbolHandle;
use terminal_check_tables::{Checked, Construct};

use super::Program;
use super::contracts::{ClauseScope, DataSubject, ResultTerm};
use super::places::RecordSource;
use crate::lowering::body::{CaseType, RecordField, RecordType, StorageShape, ValueType};
use crate::lowering::{Unserved, refuse};

/// A data declaration indexed by values, as the signature needs it.
#[derive(Debug, Clone)]
pub(crate) struct FamilyRecord {
    pub(crate) record: StructuralTypeId,
    pub(crate) declaration: SymbolHandle,
    /// The declaration's value parameters, in declared order.
    pub(crate) indices: Vec<FamilyValue>,
    pub(crate) cases: Vec<FamilyCase>,
    /// The declaration's `requires`: the value itself is the data subject,
    /// and parameter `i` is the scalar value numbered `i + 1`.
    pub(crate) requires: Vec<Proposition>,
}

/// An index of a family or a payload value of one of its cases.
#[derive(Debug, Clone)]
pub(crate) struct FamilyValue {
    pub(crate) symbol: SymbolHandle,
    pub(crate) name: String,
    pub(crate) integer: IntegerType,
    /// The field of the case this value is, for a payload value.
    pub(crate) field: Option<StructuralFieldId>,
}

#[derive(Debug, Clone)]
pub(crate) struct FamilyCase {
    pub(crate) symbol: SymbolHandle,
    pub(crate) name: String,
    pub(crate) case: StructuralCaseId,
    /// The payload fields that are values, in declared order.
    pub(crate) payload: Vec<FamilyValue>,
    /// The payload fields that are values of the family itself, in declared
    /// order.
    pub(crate) children: Vec<FamilyChild>,
}

/// A value of the family held by one of its cases.
#[derive(Debug, Clone)]
pub(crate) struct FamilyChild {
    pub(crate) symbol: SymbolHandle,
    /// The index the child sits at: one term per index of the family.
    pub(crate) at: Vec<FamilyTerm>,
}

/// What a type argument of the family is, where a case or a formal names
/// the family at arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FamilyTerm {
    /// The declaration's own parameter at this position.
    Index(u32),
    /// The payload value at this position of the case that holds the child.
    Payload(u32),
    Literal(i128),
    /// Any other name: a formal or a local of the machine that names the
    /// type, by its symbol.
    Name(SymbolHandle),
}

/// A theorem over a family, as the signature needs it.
#[derive(Debug, Clone)]
pub(crate) struct FamilyTheorem {
    pub(crate) record: StructuralTypeId,
    /// What the theorem guarantees of the indices: index `i` is the scalar
    /// value numbered `i + 1`, as in the family's `requires`.
    pub(crate) ensures: Vec<Proposition>,
    /// One per case of the family, in declared order.
    pub(crate) cases: Vec<FamilyTheoremCase>,
}

/// What a theorem's body does in one case of the family.
#[derive(Debug, Clone, Default)]
pub(crate) struct FamilyTheoremCase {
    /// What the step this case calls requires, over the indices and this
    /// case's payload, written as the family's `requires` write them.
    pub(crate) requires: Vec<Proposition>,
    /// For each child of the case, whether the step cites the theorem at it.
    pub(crate) cited: Vec<bool>,
    pub(crate) source: Option<SourceSpan>,
}

impl Program<'_> {
    /// The number of the scalar value that stands for index `position` of
    /// a family in its clauses.
    fn index_identity(position: usize) -> Option<ValueId> {
        ValueId::new(position as u64 + 1)
    }

    /// Reads the body of a theorem over indexed data.
    pub(super) fn family_theorem(
        &mut self,
        id: MachineId,
        machine: &Machine,
        state: &State,
    ) -> Checked<FamilyTheorem> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let formals = typed
            .state_parameters
            .span(state.parameters)
            .ok_or_else(|| unserved("theorem over indexed data lost its parameters"))?;
        let evidence = formals
            .iter()
            .next()
            .ok_or_else(|| unserved("theorem over indexed data without its evidence"))?;
        let own = self.value_type(id, evidence.type_reference)?;
        let family = self
            .family_of(own)
            .ok_or_else(|| unserved("theorem over data that is not indexed"))?
            .clone();
        // The evidence is at the theorem's own value parameters, in order.
        let parameters = typed.machine_type_parameters(machine);
        let at = self.family_arguments(id, &family, &[], evidence.type_reference)?;
        if parameters.len() != family.indices.len()
            || !parameters.iter().zip(&at).all(|(parameter, term)| {
                matches!(parameter.kind, TypeParameterKind::Value { .. })
                    && *term == FamilyTerm::Name(parameter.symbol)
            })
        {
            return Err(unserved(
                "theorem over indexed data at other than its own value parameters in order",
            ));
        }
        let mut indices = Vec::with_capacity(parameters.len());
        for (position, (parameter, index)) in parameters.iter().zip(&family.indices).enumerate() {
            indices.push((
                parameter.symbol,
                ScalarTerm::Value {
                    id: Self::index_identity(position)
                        .ok_or_else(|| unserved("index identity overflow"))?,
                    scalar_type: ScalarType::Integer(index.integer),
                },
            ));
        }
        let clauses = |program: &mut Self,
                       contracts,
                       bound: &[(SymbolHandle, ScalarTerm)],
                       want: SignatureContractKind,
                       other: &'static str|
         -> Checked<Vec<Proposition>> {
            let scope = ClauseScope {
                slots: &[],
                values: &[],
                bound,
                // The clause's operators are selected under the state that
                // declares it.
                owner: state.symbol,
                result: ResultTerm::Absent("result in a theorem over indexed data"),
                formal_fields: false,
                logical: None,
                subject: None,
            };
            let mut read = Vec::new();
            for contract in super::span(
                |span| typed.signature_contracts.span(span),
                contracts,
                id,
                "contract span",
            )?
            .iter()
            {
                if contract.kind != want {
                    return Err(unserved(other));
                }
                for fact in super::span(
                    |span| typed.proof_facts.span(span),
                    contract.facts,
                    id,
                    "contract fact span",
                )?
                .iter()
                {
                    let fact = program.fact(id, fact)?;
                    read.push(program.clause(id, &scope, fact)?);
                }
            }
            Ok(read)
        };
        let ensures = clauses(
            self,
            machine.contracts,
            &indices,
            SignatureContractKind::Ensures,
            "theorem over indexed data with a clause other than `ensures`",
        )?;
        if ensures.is_empty() {
            return Err(unserved(
                "theorem over indexed data that guarantees nothing",
            ));
        }
        // The body: the evidence held as the dispatch's subject, then one
        // arm per case.
        let statements = typed.statement_table.statements(state.statement_nodes);
        let Some((StatementNode::LocalData(subject), arms)) = statements.split_first() else {
            return Err(unserved(
                "theorem over indexed data whose body is not one dispatch on its evidence",
            ));
        };
        let names = |expression: ExpressionHandle, symbol: SymbolHandle| {
            expression.is_valid()
                && matches!(typed.expression_table.expression(expression),
                    ExpressionNode::Name(path) if path.symbol == symbol)
        };
        if !names(subject.initial_value, evidence.symbol) {
            return Err(unserved(
                "theorem over indexed data whose body is not one dispatch on its evidence",
            ));
        }
        let mut cases: Vec<Option<FamilyTheoremCase>> = vec![None; family.cases.len()];
        for arm in arms {
            let StatementNode::Transition(transition) = arm else {
                return Err(unserved(
                    "theorem over indexed data with a statement beside its dispatch",
                ));
            };
            let [pattern] = typed
                .statement_table
                .transition_patterns(transition.patterns)
            else {
                return Err(unserved("theorem dispatch arm without one case pattern"));
            };
            if !names(pattern.subject, subject.symbol)
                || transition.exit != TransitionExit::Ordinary
                || transition.continuation.is_valid()
                || !transition.proof_selectors.is_empty()
                || pattern.pattern.guard.is_valid()
                || !pattern.pattern.head.is_valid()
            {
                return Err(unserved("theorem dispatch arm that is no plain case arm"));
            }
            let ExpressionNode::Name(head) =
                typed.expression_table.expression(pattern.pattern.head)
            else {
                return Err(unserved("theorem dispatch arm that names no case"));
            };
            let position = family
                .cases
                .iter()
                .position(|case| case.symbol == head.symbol)
                .ok_or_else(|| unserved("theorem dispatch arm that names no case"))?;
            if cases[position].is_some() {
                return Err(unserved("theorem dispatch with two arms for one case"));
            }
            let case = &family.cases[position];
            let mut read = FamilyTheoremCase {
                requires: Vec::new(),
                cited: vec![false; case.children.len()],
                source: Some(transition.source_span),
            };
            if !transition.target.is_valid() {
                return Err(unserved("theorem dispatch arm without a target"));
            }
            match typed.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Terminal => {}
                TransitionTargetNode::Named {
                    path,
                    static_machine_parameter,
                    arguments,
                    evidence_arguments,
                    ..
                } if !static_machine_parameter.is_valid() && evidence_arguments.is_empty() => {
                    let arguments = typed.statement_table.expression_handles(*arguments);
                    self.family_step(
                        id,
                        machine,
                        &family,
                        position,
                        &indices,
                        subject.symbol,
                        path.symbol,
                        arguments,
                        &mut read,
                    )?;
                }
                _ => {
                    return Err(unserved(
                        "theorem dispatch arm that neither ends nor calls a step",
                    ));
                }
            }
            let _ = case;
            cases[position] = Some(read);
        }
        let cases = cases
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| unserved("theorem dispatch without an arm for every case"))?;
        Ok(FamilyTheorem {
            record: family.record,
            ensures,
            cases,
        })
    }

    /// Reads the machine one arm of a theorem's dispatch calls: what it
    /// requires, and which children it cites the theorem at.
    #[allow(clippy::too_many_arguments)]
    fn family_step(
        &mut self,
        id: MachineId,
        theorem: &Machine,
        family: &FamilyRecord,
        position: usize,
        indices: &[(SymbolHandle, ScalarTerm)],
        subject: SymbolHandle,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        read: &mut FamilyTheoremCase,
    ) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let case = &family.cases[position];
        let step = self
            .entered_machine(target)
            .and_then(|machine| typed.machine_by_symbol(machine))
            .filter(|step| self.enclosing_machine(step.symbol) == Some(theorem.symbol))
            .ok_or_else(|| {
                unserved("theorem dispatch arm that calls a machine declared outside the theorem")
            })?;
        let entry = typed
            .machine_states(step)
            .first()
            .filter(|entry| !entry.return_type.is_valid())
            .ok_or_else(|| unserved("step of a theorem that returns a value"))?;
        let formals = typed
            .state_parameters
            .span(entry.parameters)
            .ok_or_else(|| unserved("step of a theorem lost its parameters"))?;
        if formals.len() != arguments.len() {
            return Err(unserved(
                "step of a theorem called with the wrong number of arguments",
            ));
        }
        // Each formal of the step receives a payload value or a child of
        // the case, written as a member of the dispatch's subject.
        enum Received {
            Payload(usize),
            Child(usize),
        }
        let mut received = Vec::with_capacity(formals.len());
        for (formal, &argument) in formals.iter().zip(arguments) {
            let field = match typed.expression_table.expression(argument) {
                ExpressionNode::Member(member)
                    if matches!(typed.expression_table.expression(member.receiver),
                        ExpressionNode::Name(path) if path.symbol == subject) =>
                {
                    match member.selection {
                        MemberSelection::Field(field) => Some(field),
                        _ => None,
                    }
                }
                _ => None,
            }
            .ok_or_else(|| {
                unserved("step of a theorem called with other than the case's own payload")
            })?;
            let what = if let Some(payload) =
                case.payload.iter().position(|value| value.symbol == field)
            {
                let ScalarType::Integer(integer) = self.scalar_type(id, formal.type_reference)?
                else {
                    return Err(unserved("step of a theorem with a formal of another type"));
                };
                if integer != case.payload[payload].integer {
                    return Err(unserved("step of a theorem with a formal of another type"));
                }
                Received::Payload(payload)
            } else if let Some(child) = case.children.iter().position(|child| child.symbol == field)
            {
                if formal.relevance != language_core::BindingRelevance::Erased {
                    return Err(unserved(
                        "step of a theorem that takes a child in a formal that is not erased",
                    ));
                }
                Received::Child(child)
            } else {
                return Err(unserved(
                    "step of a theorem called with other than the case's own payload",
                ));
            };
            received.push((formal.symbol, what));
        }
        // An argument a type or a call applies the family at, in the terms
        // the case's own declaration uses.
        let resolve = |terms: Vec<FamilyTerm>| -> Checked<Vec<FamilyTerm>> {
            terms
                .into_iter()
                .map(|term| match term {
                    FamilyTerm::Name(symbol) => {
                        if let Some(index) =
                            indices.iter().position(|(parameter, _)| *parameter == symbol)
                        {
                            Ok(FamilyTerm::Index(index as u32))
                        } else if let Some((_, Received::Payload(payload))) =
                            received.iter().find(|(formal, _)| *formal == symbol)
                        {
                            Ok(FamilyTerm::Payload(*payload as u32))
                        } else {
                            Err(unserved(
                                "step of a theorem that names a child at something other than an index or a payload value",
                            ))
                        }
                    }
                    other => Ok(other),
                })
                .collect()
        };
        for (formal, (_, what)) in formals.iter().zip(&received) {
            if let Received::Child(child) = what {
                let at = resolve(self.family_arguments(id, family, &[], formal.type_reference)?)?;
                if at != case.children[*child].at {
                    return Err(unserved(
                        "step of a theorem that takes a child at another index than the case declares",
                    ));
                }
            }
        }
        // What the step requires, with each scalar formal standing for the
        // payload value it receives.
        let mut bound = indices.to_vec();
        for (symbol, what) in &received {
            if let Received::Payload(payload) = what {
                let value = &case.payload[*payload];
                let field = value
                    .field
                    .ok_or_else(|| unserved("payload value lost its field"))?;
                bound.push((
                    *symbol,
                    ScalarTerm::IntegerField {
                        version: ContentPlaceVersion::Current,
                        root: crate::lowering::body::self_place()
                            .ok_or_else(|| unserved("self place identity"))?,
                        path: vec![
                            CanonicalStructuralPathSegment::Case(case.case),
                            CanonicalStructuralPathSegment::Field(field),
                        ],
                        scalar_type: value.integer,
                    },
                ));
            }
        }
        let scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &bound,
            owner: entry.symbol,
            result: ResultTerm::Absent("result in a step of a theorem"),
            formal_fields: false,
            logical: None,
            subject: None,
        };
        for contract in super::span(
            |span| typed.signature_contracts.span(span),
            step.contracts,
            id,
            "contract span",
        )?
        .iter()
        {
            if contract.kind != SignatureContractKind::Requires {
                return Err(unserved(
                    "step of a theorem with a clause other than `requires`",
                ));
            }
            for fact in super::span(
                |span| typed.proof_facts.span(span),
                contract.facts,
                id,
                "contract fact span",
            )?
            .iter()
            {
                let fact = self.fact(id, fact)?;
                read.requires.push(self.clause(id, &scope, fact)?);
            }
        }
        // The step's body: calls of the theorem at a child, each at the
        // index the case declares for that child.
        for statement in typed.statement_table.statements(entry.statement_nodes) {
            let StatementNode::Call(call) = statement else {
                return Err(unserved(
                    "step of a theorem that does other than cite the theorem at a child",
                ));
            };
            let [argument] = typed.statement_table.expression_handles(call.arguments) else {
                return Err(unserved(
                    "step of a theorem that does other than cite the theorem at a child",
                ));
            };
            if self.entered_machine(call.target_symbol) != Some(theorem.symbol)
                || !call.receiver.is_empty()
                || !call.evidence_arguments.is_empty()
            {
                return Err(unserved(
                    "step of a theorem that does other than cite the theorem at a child",
                ));
            }
            let child = match typed.expression_table.expression(*argument) {
                ExpressionNode::Name(path) => {
                    received.iter().find_map(|(formal, what)| match what {
                        Received::Child(child) if *formal == path.symbol => Some(*child),
                        _ => None,
                    })
                }
                _ => None,
            }
            .ok_or_else(|| unserved("step of a theorem that cites it at other than a child"))?;
            let mut at = Vec::with_capacity(call.machine_arguments.len());
            for argument in &call.machine_arguments {
                if argument.application.is_some() || argument.evidence_projection.is_some() {
                    return Err(unserved("theorem cited at an index that is no value"));
                }
                at.push(match &argument.const_literal {
                    Some(literal) => {
                        FamilyTerm::Literal(literal.to_string().parse().map_err(|_| {
                            unserved("theorem cited at an index that is no integer")
                        })?)
                    }
                    None if argument.symbol.is_valid() => FamilyTerm::Name(argument.symbol),
                    None => return Err(unserved("theorem cited at an index that is no value")),
                });
            }
            if resolve(at)? != case.children[child].at {
                return Err(unserved(
                    "theorem cited at another index than the case declares for that child",
                ));
            }
            read.cited[child] = true;
        }
        Ok(())
    }

    /// Whether the declaration is a family this module reads: data that
    /// contains itself and whose every parameter is a value.
    pub(crate) fn is_family(&self, data: &DataDefinition) -> bool {
        let parameters = self.typed.data_type_parameters(data);
        !parameters.is_empty()
            && parameters
                .iter()
                .all(|parameter| matches!(parameter.kind, TypeParameterKind::Value { .. }))
            && data.quotient.is_none()
            && self.contains_itself(data)
    }

    /// Whether the machine is a theorem over indexed data: it returns
    /// nothing, and its one formal is erased evidence of such data.
    pub(crate) fn is_family_theorem(&self, machine: &Machine) -> bool {
        let typed = self.typed;
        typed.machine_states(machine).first().is_some_and(|entry| {
            !entry.return_type.is_valid()
                && typed
                    .state_parameters
                    .span(entry.parameters)
                    .is_some_and(|formals| {
                        let mut formals = formals.iter();
                        matches!((formals.next(), formals.next()), (Some(formal), None)
                            if formal.relevance == language_core::BindingRelevance::Erased
                                && self
                                    .names_family(formal.type_reference)
                                    .and_then(|symbol| typed.unique_data_definition_by_symbol(symbol))
                                    .is_some_and(|data| self.is_family(data)))
                    })
        })
    }

    /// The family a value type is, if it is one read here.
    pub(crate) fn family_of(&self, value_type: ValueType) -> Option<&FamilyRecord> {
        let ValueType::Record(record, ..) = value_type else {
            return None;
        };
        self.families.iter().find(|family| family.record == record)
    }

    /// The arguments a type reference applies a family at, one per index.
    /// `payload` are the payload values in scope, when the reference is the
    /// type of a child in one of the family's own cases.
    pub(crate) fn family_arguments(
        &self,
        id: MachineId,
        family: &FamilyRecord,
        payload: &[FamilyValue],
        reference: TypeReferenceHandle,
    ) -> Checked<Vec<FamilyTerm>> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Type(what));
        let TypeReferenceNode::Generic {
            base_symbol,
            lifetime_arguments,
            arguments,
            ..
        } = typed.type_reference_table.type_reference(reference)
        else {
            return Err(unserved("indexed data named without its indices"));
        };
        if *base_symbol != family.declaration || !lifetime_arguments.is_empty() {
            return Err(unserved("indexed data named through another declaration"));
        }
        let arguments = typed
            .type_reference_table
            .type_reference_handles(*arguments);
        if arguments.len() != family.indices.len() {
            return Err(unserved(
                "indexed data applied at the wrong number of indices",
            ));
        }
        let named = |symbol: SymbolHandle| {
            if let Some(position) = family
                .indices
                .iter()
                .position(|index| index.symbol == symbol)
            {
                FamilyTerm::Index(position as u32)
            } else if let Some(position) = payload.iter().position(|value| value.symbol == symbol) {
                FamilyTerm::Payload(position as u32)
            } else {
                FamilyTerm::Name(symbol)
            }
        };
        let literal = |text: &str| {
            text.parse::<i128>()
                .map(FamilyTerm::Literal)
                .map_err(|_| unserved("index of indexed data that is no integer"))
        };
        arguments
            .iter()
            .map(
                |argument| match typed.type_reference_table.type_reference(*argument) {
                    TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                        Ok(named(*symbol))
                    }
                    TypeReferenceNode::Named { name, .. } => literal(name.as_str()),
                    TypeReferenceNode::ConstExpression(expression) => {
                        match typed.expression_table.expression(*expression) {
                            ExpressionNode::Name(path) if path.symbol.is_valid() => {
                                Ok(named(path.symbol))
                            }
                            ExpressionNode::Integer(value) => literal(&value.to_string()),
                            _ => Err(unserved(
                                "index of indexed data other than a name or an integer",
                            )),
                        }
                    }
                    _ => Err(unserved(
                        "index of indexed data other than a name or an integer",
                    )),
                },
            )
            .collect()
    }

    /// Reads a data declaration indexed by values as one type of the
    /// mathematical signature.
    pub(super) fn family_record(
        &mut self,
        id: MachineId,
        data: &DataDefinition,
    ) -> Checked<ValueType> {
        let symbol = data.symbol;
        if let Some(known) = self
            .families
            .iter()
            .find(|family| family.declaration == symbol)
        {
            return Ok(ValueType::record(known.record));
        }
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Type(what));
        if !data.lifetime_parameters.is_empty()
            || !data.conformance_bounds.is_empty()
            || data.cleanup_selection.is_some()
            || data.supply_mode == language_core::DataSupplyMode::BoundaryOpaque
            || !data.retired_identities.is_empty()
        {
            return Err(unserved(
                "indexed data with lifetimes, bounds, cleanup or opaque storage",
            ));
        }
        let identity = typed
            .normalized_nominal_type_identity(symbol)
            .and_then(|identity| typed.type_identity_text(identity))
            .filter(|identity| !identity.is_empty())
            .ok_or_else(|| unserved("data without a complete application identity"))?;
        let integer = |program: &Self, reference: TypeReferenceHandle| match program
            .scalar_type(id, reference)?
        {
            ScalarType::Integer(integer) => Ok(integer),
            _ => Err(unserved("indexed data over a value that is no integer")),
        };
        let mut indices = Vec::new();
        for parameter in typed.data_type_parameters(data) {
            let TypeParameterKind::Value { type_reference } = parameter.kind else {
                return Err(unserved("indexed data with a parameter that is no value"));
            };
            if parameter.bounds != crate::source_state::data::DataProperties::default() {
                return Err(unserved("indexed data with a constrained parameter"));
            }
            indices.push(FamilyValue {
                symbol: parameter.symbol,
                name: parameter.name.as_str().to_owned(),
                integer: integer(self, type_reference)?,
                field: None,
            });
        }
        // The record is reserved before the cases are read, so that a child
        // of the family's own type resolves to it.
        let record = self.records.push(RecordType {
            multiplicity: crate::lowering::body::source_multiplicity(data.properties.multiplicity),
            cleanup: None,
            identity,
            shape: StorageShape::Sum(Vec::new()),
            requires: Vec::new(),
            authored_layout: false,
        });
        self.record_sources.push(RecordSource {
            constant_input_owner: None,
            declaration: symbol,
            cleanup: None,
            application: None,
            region_owner: SymbolHandle::invalid(),
            fields: Vec::new(),
            cases: Vec::new(),
        });
        if let Err(position) = self.self_containing.binary_search(&record) {
            self.self_containing.insert(position, record);
        }
        if let Err(position) = self.value_records.binary_search(&record) {
            self.value_records.insert(position, record);
        }
        self.families.push(FamilyRecord {
            record,
            declaration: symbol,
            indices,
            cases: Vec::new(),
            requires: Vec::new(),
        });
        let own = ValueType::record(record);
        let mut cases = Vec::new();
        let mut case_types = Vec::new();
        let mut case_symbols = Vec::new();
        for member in typed.data_members(data) {
            let DataMember::Variant(variant) = member else {
                return Err(unserved("indexed data with a field outside a case"));
            };
            if !variant.symbol.is_valid()
                || !variant.requires_facts.is_empty()
                || !variant.retired_payload_identities.is_empty()
            {
                return Err(unserved(
                    "case of indexed data with requirements of its own",
                ));
            }
            self.cases_minted += 1;
            let case = StructuralCaseId::new(self.cases_minted)
                .ok_or_else(|| unserved("case identity overflow"))?;
            let mut payload = Vec::new();
            let mut children = Vec::new();
            let mut fields = Vec::new();
            let mut symbols = Vec::new();
            for field in typed.data_payload_fields(variant) {
                if !field.symbol.is_valid() {
                    return Err(unserved("field without a symbol"));
                }
                self.fields_minted += 1;
                let minted = StructuralFieldId::new(self.fields_minted)
                    .ok_or_else(|| unserved("field identity overflow"))?;
                let name = field.name.as_str().to_owned();
                let value_type = if self.names_family(field.type_reference) == Some(symbol) {
                    if field.relevance == language_core::BindingRelevance::Relevant {
                        return Err(unserved(
                            "indexed data that holds itself in a field that is not erased",
                        ));
                    }
                    let family = self
                        .families
                        .last()
                        .expect("the family was pushed above")
                        .clone();
                    children.push(FamilyChild {
                        symbol: field.symbol,
                        at: self.family_arguments(id, &family, &payload, field.type_reference)?,
                    });
                    own
                } else {
                    let integer = integer(self, field.type_reference)?;
                    payload.push(FamilyValue {
                        symbol: field.symbol,
                        name,
                        integer,
                        field: Some(minted),
                    });
                    self.value_type(id, field.type_reference)?
                };
                fields.push(RecordField {
                    id: minted,
                    identity: field.member_identity().spelling(),
                    value_type,
                    byte_predicates: self.view_predicates(id, value_type)?,
                });
                symbols.push(field.symbol);
            }
            case_types.push(CaseType {
                id: case,
                identity: variant.member_identity().spelling(),
                fields,
            });
            case_symbols.push((variant.symbol, symbols));
            cases.push(FamilyCase {
                symbol: variant.symbol,
                name: variant.name.as_str().to_owned(),
                case,
                payload,
                children,
            });
        }
        if cases.is_empty() {
            return Err(unserved("indexed data without a case"));
        }
        self.records
            .get_mut(record)
            .ok_or_else(|| unserved("record identity"))?
            .shape = StorageShape::Sum(case_types);
        self.record_sources
            .get_mut(record)
            .ok_or_else(|| unserved("record identity"))?
            .cases = case_symbols;
        self.admitted_declarations.include(symbol);
        // The declaration's `requires`, read as a record's are: the value
        // itself is the data subject, and each parameter stands for a value
        // of its own, numbered in declared order from one.
        let mut bound = Vec::new();
        for (position, index) in self
            .families
            .last()
            .expect("the family was pushed above")
            .indices
            .iter()
            .enumerate()
        {
            bound.push((
                index.symbol,
                ScalarTerm::Value {
                    id: Self::index_identity(position)
                        .ok_or_else(|| unserved("index identity overflow"))?,
                    scalar_type: ScalarType::Integer(index.integer),
                },
            ));
        }
        let root =
            crate::lowering::body::self_place().ok_or_else(|| unserved("self place identity"))?;
        let scope = ClauseScope {
            slots: &[],
            values: &[],
            bound: &bound,
            owner: SymbolHandle::invalid(),
            result: ResultTerm::Absent("result in a data clause"),
            formal_fields: false,
            logical: None,
            subject: Some(DataSubject {
                value_type: own,
                symbol: data.predicate_subject_symbol,
                root,
                path: &[],
                bare_fields: true,
            }),
        };
        let facts = super::span(
            |span| typed.proof_facts.span(span),
            data.requires_facts,
            id,
            "data requires span",
        )?;
        let requires = facts
            .iter()
            .map(|fact| {
                let fact = self.fact(id, fact)?;
                self.clause(id, &scope, fact)
            })
            .collect::<Checked<Vec<_>>>()?;
        let family = self
            .families
            .iter_mut()
            .find(|family| family.record == record)
            .expect("the family was pushed above");
        family.cases = cases;
        family.requires = requires;
        Ok(own)
    }

    /// The declaration a type reference applies at arguments, when it is
    /// data.
    fn names_family(&self, reference: TypeReferenceHandle) -> Option<SymbolHandle> {
        match self.typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Generic { base_symbol, .. } if base_symbol.is_valid() => {
                Some(*base_symbol)
            }
            _ => None,
        }
    }
}
