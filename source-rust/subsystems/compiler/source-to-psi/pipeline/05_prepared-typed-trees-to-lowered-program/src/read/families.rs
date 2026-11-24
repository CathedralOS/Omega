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
    /// The declaration's type parameters, each with its position among all
    /// the declared parameters. No clause of the family can say anything of
    /// a value of such a type and no index depends on one, so the
    /// signature's family leaves them out: forgetting the types and the
    /// values of them maps every value of the declared data to a value of
    /// that family at the same index, and a theorem about the indices holds
    /// whatever the types are. That the types agree where evidence is built
    /// or passed is typed preparation's check, as for any generic data.
    pub(crate) types: Vec<(u32, SymbolHandle)>,
    pub(crate) cases: Vec<FamilyCase>,
    /// The declaration's `requires`: the value itself is the data subject,
    /// and parameter `i` is the scalar value numbered `i + 1`.
    pub(crate) requires: Vec<Proposition>,
}

impl FamilyRecord {
    /// How many parameters the declaration has, values and types.
    fn arity(&self) -> usize {
        self.indices.len() + self.types.len()
    }

    /// The type parameter declared at `position`, when that parameter is a
    /// type.
    fn type_at(&self, position: usize) -> Option<SymbolHandle> {
        self.types
            .iter()
            .find(|(declared, _)| *declared as usize == position)
            .map(|(_, symbol)| *symbol)
    }
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
    /// The payload fields whose type is one of the family's type
    /// parameters. A literal writes them; the signature's family has none.
    pub(crate) carried: Vec<SymbolHandle>,
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
    /// The index the theorem is ranked on (`terminates by Length`), by its
    /// position among the family's indices. A theorem that cites itself has
    /// one, and its type is unsigned.
    pub(crate) rank: Option<u32>,
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

/// A value a theorem is cited at, in the citing machine's own terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CitedValue {
    /// A type argument. No guarantee of a theorem names one.
    Type,
    Literal(i128),
    /// One of the citing machine's value parameters.
    Parameter(SymbolHandle),
}

/// What a machine that runs holds in place of an erased value of indexed
/// data: an erased Boolean nothing reads. The value itself has no layout
/// and exists only for the checker, and what it is evidence of is the
/// mathematical signature's to check.
pub(super) fn evidence_witness() -> ValueType {
    ValueType::Scalar(
        ScalarType::Boolean,
        crate::lowering::body::Qualification::NONE,
    )
}

impl Program<'_> {
    /// Whether a type reference applies data indexed by values.
    pub(super) fn names_indexed_data(&self, reference: TypeReferenceHandle) -> bool {
        self.names_family(reference)
            .and_then(|symbol| self.typed.unique_data_definition_by_symbol(symbol))
            .is_some_and(|data| self.is_family(data))
    }

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
        // The evidence is at the theorem's own parameters, in order: a
        // value for each index and a type for each type of the family.
        let parameters = typed.machine_type_parameters(machine);
        let at = self.family_arguments(id, &family, &[], evidence.type_reference)?;
        let mut values = Vec::new();
        let mut own = parameters.len() == family.arity();
        for (position, parameter) in parameters.iter().enumerate() {
            match (family.type_at(position), &parameter.kind) {
                (None, TypeParameterKind::Value { .. }) => {
                    own &= at.get(values.len()) == Some(&FamilyTerm::Name(parameter.symbol));
                    values.push(parameter.symbol);
                }
                (Some(_), TypeParameterKind::Type) => {}
                _ => own = false,
            }
        }
        if !own {
            return Err(unserved(
                "theorem over indexed data at other than its own value parameters in order",
            ));
        }
        let mut indices = Vec::with_capacity(values.len());
        for (position, (symbol, index)) in values.iter().zip(&family.indices).enumerate() {
            indices.push((
                *symbol,
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
        // A citation of the theorem at a child rests on the theorem's rank
        // being smaller there: one of its indices, which counts down to
        // zero only if it cannot be negative.
        let rank =
            match crate::source_state::ranking::resolve_machine_witness_subjects(typed, machine)
                .as_deref()
            {
                None | Some([]) => None,
                Some([subject]) => {
                    let position = match typed.expression_table.expression(*subject) {
                        ExpressionNode::Name(path) => {
                            values.iter().position(|value| *value == path.symbol)
                        }
                        _ => None,
                    }
                    .ok_or_else(|| {
                        unserved(
                            "theorem over indexed data ranked on other than one of its indices",
                        )
                    })?;
                    if family.indices[position].integer.sign()
                        != semantic_vocabulary::IntegerSign::Unsigned
                    {
                        return Err(unserved(
                            "theorem over indexed data ranked on an index that may be negative",
                        ));
                    }
                    Some(position as u32)
                }
                Some(_) => {
                    return Err(unserved(
                        "theorem over indexed data ranked on other than one of its indices",
                    ));
                }
            };
        if rank.is_none() && cases.iter().any(|case| case.cited.contains(&true)) {
            return Err(unserved(
                "theorem over indexed data that cites itself without a rank",
            ));
        }
        Ok(FamilyTheorem {
            record: family.record,
            ensures,
            cases,
            rank,
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
            if call.machine_arguments.len() != family.arity() {
                return Err(unserved("theorem cited without each of its parameters"));
            }
            let mut at = Vec::with_capacity(family.indices.len());
            for (position, argument) in call.machine_arguments.iter().enumerate() {
                if family.type_at(position).is_some() {
                    continue;
                }
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

    /// The theorem over indexed data a call's target is, when it is one.
    pub(super) fn cited_theorem(&self, target: SymbolHandle) -> Option<&Machine> {
        self.entered_machine(target)
            .and_then(|machine| self.typed.machine_by_symbol(machine))
            .filter(|machine| self.is_family_theorem(machine))
    }

    /// Every call a machine's states make, at any depth: the target, the
    /// value arguments written for it and the application typed
    /// preparation recorded for them.
    fn calls_of(
        &self,
        machine: &Machine,
    ) -> Vec<(
        SymbolHandle,
        Vec<crate::source_state::expression::StaticMachineArgument>,
        crate::source_state::static_applications::StaticMachineApplicationHandle,
    )> {
        let typed = self.typed;
        let mut calls = Vec::new();
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                if let StatementNode::Call(call) = statement {
                    calls.push((
                        call.target_symbol,
                        call.machine_arguments.to_vec(),
                        call.authored_machine_application,
                    ));
                }
                for (_, root) in typed.expression_roots(statement) {
                    typed.subtree_for_each(
                        root,
                        crate::source_state::SubtreeEdges::VALUES,
                        |_, node| {
                            if let ExpressionNode::Call(call) = node {
                                calls.push((
                                    call.target_symbol,
                                    call.machine_arguments.to_vec(),
                                    call.authored_machine_application,
                                ));
                            }
                        },
                    );
                }
            }
        }
        calls
    }

    /// Whether a machine takes indexed data in one of its formals.
    fn takes_evidence(&self, machine: &Machine) -> bool {
        let typed = self.typed;
        typed
            .machine_states(machine)
            .first()
            .and_then(|entry| typed.state_parameters.span(entry.parameters))
            .is_some_and(|formals| {
                formals
                    .iter()
                    .any(|formal| self.names_indexed_data(formal.type_reference))
            })
    }

    /// The theorems a generic machine cites, each with the values it is
    /// cited at in the machine's own terms: one of its value parameters or
    /// a number. A machine it passes evidence on to counts, at the values
    /// written for that call.
    fn citations_of(
        &self,
        id: MachineId,
        machine: &Machine,
        visiting: &mut Vec<SymbolHandle>,
    ) -> Checked<Vec<(SymbolHandle, Vec<CitedValue>)>> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let mut cited = Vec::new();
        for (target, written, _) in self.calls_of(machine) {
            let Some(callee) = self
                .entered_machine(target)
                .and_then(|callee| typed.machine_by_symbol(callee))
            else {
                continue;
            };
            let theorem = self.is_family_theorem(callee);
            if !theorem && (!self.takes_evidence(callee) || visiting.contains(&callee.symbol)) {
                continue;
            }
            let parameters = typed.machine_type_parameters(callee);
            if parameters.is_empty() {
                // A copy made for closed values: nothing of this machine's
                // reaches it, and what it requires is closed.
                continue;
            }
            let values = written
                .iter()
                .zip(parameters.iter())
                .map(|(argument, parameter)| {
                    if argument.application.is_some() || argument.evidence_projection.is_some() {
                        return None;
                    }
                    if !matches!(parameter.kind, TypeParameterKind::Value { .. }) {
                        return Some(CitedValue::Type);
                    }
                    if let Some(literal) = &argument.const_literal {
                        return literal.to_string().parse().ok().map(CitedValue::Literal);
                    }
                    if argument.symbol.is_valid() {
                        return Some(CitedValue::Parameter(argument.symbol));
                    }
                    match argument.path.as_ref() {
                        [word] => word.as_str().parse().ok().map(CitedValue::Literal),
                        _ => None,
                    }
                })
                .collect::<Option<Vec<_>>>()
                .filter(|values| {
                    values.len() == parameters.len() && written.len() == parameters.len()
                })
                .ok_or_else(|| {
                    unserved("theorem over indexed data reached at a value that is neither a number nor a value parameter")
                })?;
            if theorem {
                cited.push((callee.symbol, values));
                continue;
            }
            visiting.push(callee.symbol);
            let inner = self.citations_of(id, callee, visiting);
            visiting.pop();
            for (theorem, at) in inner? {
                let at = at
                    .into_iter()
                    .map(|value| match value {
                        CitedValue::Parameter(symbol) => parameters
                            .iter()
                            .position(|parameter| parameter.symbol == symbol)
                            .and_then(|position| values.get(position).copied()),
                        literal => Some(literal),
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| {
                        unserved(
                            "theorem over indexed data reached at a value its caller does not fix",
                        )
                    })?;
                cited.push((theorem, at));
            }
        }
        Ok(cited)
    }

    /// What the theorems over indexed data that a running machine cites
    /// guarantee, at each citation's own value arguments.
    ///
    /// A machine that runs does not enter a theorem. `vector_bound::<Length,
    /// Total>(evidence);` in its body is read as one more requirement of
    /// the machine itself: `embed(Total) <= 10 * embed(Length)` holds at
    /// entry. The body may then use it, and each caller owes it. A caller
    /// that passes numbers proves it by arithmetic. A caller that passes
    /// its own value parameters cannot, so what the machines it passes
    /// evidence on to require this way is its requirement too, at the
    /// values it passes. The theorem itself is judged once, for every
    /// index, by the mathematical signature.
    pub(super) fn cited_guarantees(
        &mut self,
        id: MachineId,
        owner: SymbolHandle,
    ) -> Checked<Vec<(Proposition, crate::lowering::ClauseText)>> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let Some(machine) = typed.machine_by_own_or_state_symbol(owner) else {
            return Ok(Vec::new());
        };
        let mut guarantees = Vec::new();
        for (theorem, at) in self.citations_of(id, machine, &mut vec![machine.symbol])? {
            let theorem = typed
                .machine_by_symbol(theorem)
                .ok_or_else(|| unserved("cited theorem lost its declaration"))?;
            let entry = typed
                .machine_states(theorem)
                .first()
                .ok_or_else(|| unserved("cited theorem lost its state"))?;
            // The theorem's value parameters stand for the values it is
            // cited at: this machine's own value parameters, as the inputs
            // its clauses name, or numbers.
            let mut bound = Vec::new();
            for (parameter, value) in typed.machine_type_parameters(theorem).iter().zip(at) {
                if value == CitedValue::Type {
                    continue;
                }
                let TypeParameterKind::Value { type_reference } = parameter.kind else {
                    return Err(unserved("cited theorem with a parameter that is no value"));
                };
                let ScalarType::Integer(integer) = self.scalar_type(id, type_reference)? else {
                    return Err(unserved("cited theorem over a value that is no integer"));
                };
                let term = match value {
                    CitedValue::Type => continue,
                    CitedValue::Literal(value) => ScalarTerm::integer(
                        integer,
                        match integer.sign() {
                            semantic_vocabulary::IntegerSign::Unsigned => {
                                semantic_vocabulary::IntegerValue::Unsigned(
                                    u128::try_from(value).map_err(|_| {
                                        unserved("theorem cited at a number outside its parameter's type")
                                    })?,
                                )
                            }
                            semantic_vocabulary::IntegerSign::Signed => {
                                semantic_vocabulary::IntegerValue::Signed(value)
                            }
                        },
                    )
                    .map_err(|_| unserved("theorem cited at a number outside its parameter's type"))?,
                    CitedValue::Parameter(symbol) => self
                        .const_scope
                        .iter()
                        .find(|binding| binding.symbol == symbol && binding.owner == id)
                        .map(|binding| ScalarTerm::Value {
                            id: binding.value,
                            scalar_type: binding.scalar_type,
                        })
                        .ok_or_else(|| {
                            unserved("theorem cited at a value that is no value parameter of the citing machine")
                        })?,
                };
                bound.push((parameter.symbol, term));
            }
            let scope = ClauseScope {
                slots: &[],
                values: &[],
                bound: &bound,
                owner: entry.symbol,
                result: ResultTerm::Absent("result in a theorem over indexed data"),
                formal_fields: false,
                logical: None,
                subject: None,
            };
            for contract in super::span(
                |span| typed.signature_contracts.span(span),
                theorem.contracts,
                id,
                "contract span",
            )?
            .iter()
            {
                if contract.kind != SignatureContractKind::Ensures {
                    return Err(unserved("cited theorem with a clause other than `ensures`"));
                }
                for fact in super::span(
                    |span| typed.proof_facts.span(span),
                    contract.facts,
                    id,
                    "contract fact span",
                )?
                .iter()
                {
                    if let Some(guarantee) = self.contract_clause(id, &scope, fact, true)? {
                        guarantees.push(guarantee);
                    }
                }
            }
        }
        Ok(guarantees)
    }

    /// Whether the declaration is a family this module reads: data that
    /// contains itself and has a value among its parameters, the others
    /// being types.
    pub(crate) fn is_family(&self, data: &DataDefinition) -> bool {
        let parameters = self.typed.data_type_parameters(data);
        parameters
            .iter()
            .any(|parameter| matches!(parameter.kind, TypeParameterKind::Value { .. }))
            && parameters.iter().all(|parameter| {
                matches!(
                    parameter.kind,
                    TypeParameterKind::Value { .. } | TypeParameterKind::Type
                )
            })
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
        self.family_arguments_within(id, family, payload, &[], reference)
    }

    /// `family_arguments` for a type written in a mathematical definition.
    /// The typed trees hold a use of the definition's own binder in such a
    /// type only as it is spelled, so it is resolved here against
    /// `binders`, the one scope the type has.
    fn family_arguments_within(
        &self,
        id: MachineId,
        family: &FamilyRecord,
        payload: &[FamilyValue],
        binders: &[(String, SymbolHandle)],
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
        if arguments.len() != family.arity() {
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
            .enumerate()
            .filter(|(position, _)| family.type_at(*position).is_none())
            .map(
                |(_, argument)| match typed.type_reference_table.type_reference(*argument) {
                    TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                        Ok(named(*symbol))
                    }
                    TypeReferenceNode::Named { name, .. } => binders
                        .iter()
                        .find(|(binder, _)| binder == name.as_str())
                        .map_or_else(|| literal(name.as_str()), |(_, symbol)| Ok(named(*symbol))),
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

    /// The declaration a type reference names when it is a bare name.
    fn named_type(&self, reference: TypeReferenceHandle) -> Option<SymbolHandle> {
        if !reference.is_valid() {
            return None;
        }
        match self.typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => Some(*symbol),
            _ => None,
        }
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
        let mut types = Vec::new();
        for (position, parameter) in typed.data_type_parameters(data).iter().enumerate() {
            match parameter.kind {
                TypeParameterKind::Value { type_reference } => {
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
                TypeParameterKind::Type => types.push((position as u32, parameter.symbol)),
                _ => {
                    return Err(unserved(
                        "indexed data with a parameter that is neither a value nor a type",
                    ));
                }
            }
        }
        let own_types: Vec<_> = types.iter().map(|(_, symbol)| *symbol).collect();
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
            types,
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
            let mut carried = Vec::new();
            let mut children = Vec::new();
            let mut fields = Vec::new();
            let mut symbols = Vec::new();
            for field in typed.data_payload_fields(variant) {
                if !field.symbol.is_valid() {
                    return Err(unserved("field without a symbol"));
                }
                // A payload value of one of the family's type parameters.
                // No clause can name it and no index depends on it.
                if self
                    .named_type(field.type_reference)
                    .is_some_and(|named| own_types.contains(&named))
                {
                    carried.push(field.symbol);
                    continue;
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
                carried,
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

/// What a machine that runs does with evidence of indexed data: each value
/// it builds, checked to be passed and held only at the index it has.
#[derive(Debug, Clone)]
pub(crate) struct EvidenceUse {
    pub(crate) machine: SymbolHandle,
    pub(crate) name: String,
    /// The machine's value parameters, in declared order.
    pub(crate) parameters: Vec<FamilyValue>,
    /// What the machine requires of its value parameters: parameter `i` is
    /// the scalar value numbered `i + 1`. A requirement about anything else
    /// is left out; it is no hypothesis here.
    pub(crate) requires: Vec<Proposition>,
    pub(crate) constructions: Vec<Construction>,
}

/// One value of indexed data a machine builds with a case literal.
#[derive(Debug, Clone)]
pub(crate) struct Construction {
    pub(crate) record: StructuralTypeId,
    /// The case's position in the family.
    pub(crate) case: usize,
    /// The index the value is declared at.
    pub(crate) at: Vec<EvidenceTerm>,
    /// The payload values, in declared order.
    pub(crate) payload: Vec<EvidenceTerm>,
    pub(crate) source: Option<SourceSpan>,
}

/// An index or payload value where a running machine names evidence: one
/// of the machine's own value parameters, or a number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvidenceTerm {
    Parameter(u32),
    Literal(i128),
}

/// Indexed data as a running machine names it: the family and the index.
/// The types it is at are not kept: typed preparation has already refused
/// evidence built or passed at another type.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Applied {
    record: StructuralTypeId,
    at: Vec<EvidenceTerm>,
}

/// The evidence a machine holds so far: each erased formal or local, with
/// what it is evidence of.
type Held = Vec<(SymbolHandle, Applied)>;

/// What a call writes for one parameter of what it calls.
#[derive(Debug, Clone, Copy)]
enum Written {
    Value(EvidenceTerm),
    Type,
}

/// A definition that hands evidence on: its binders, each with whether it
/// is a value, and the index it takes and returns evidence at.
struct HandedOn {
    kinds: Vec<(SymbolHandle, bool)>,
    terms: Vec<FamilyTerm>,
}

impl Program<'_> {
    /// Reads what every machine of the subject that runs does with evidence
    /// of indexed data.
    ///
    /// The ordinary reader lowers such evidence as a witness nothing reads
    /// (`evidence_witness`) and does not look at how it was built. This is
    /// where that is checked: a value is built only by a case literal in an
    /// erased local, each child of the literal is evidence at the index the
    /// case declares, and evidence is passed only to a formal that takes it
    /// at the index it has. What each literal's case requires of its values
    /// becomes a law of the signature for stage 06 to prove.
    pub(crate) fn evidence_uses(
        &mut self,
        id: MachineId,
        subjects: &[Option<semantic_vocabulary::PackageKeyIdentity>],
        purpose: source_provenance::DependencyScope,
    ) -> Checked<()> {
        let typed = self.typed;
        for definition in typed.mathematical_definitions() {
            if self.returns_indexed_data(definition)
                && self.owned_by_subject(id, definition.symbol, subjects, purpose)?
            {
                self.evidence_definition(id, definition.symbol)
                    .map_err(|refusal| {
                        self.read_failure_at(refusal, definition.symbol, "the evidence")
                    })?;
            }
        }
        for machine in typed.machines() {
            // A theorem and its steps are read with the theorem. A copy
            // made for closed arguments does what its original does, which
            // is read here for every argument.
            if self.proof_machine(machine)
                || typed
                    .machine_instantiation(machine.symbol)
                    .map_err(|reason| refuse(id, Construct::Machine, Unserved::Machine(reason)))?
                    .is_some()
                || !self.owned_by_subject(id, machine.symbol, subjects, purpose)?
            {
                continue;
            }
            let read = self
                .evidence_use(id, machine)
                .map_err(|refusal| self.read_failure_at(refusal, machine.symbol, "the evidence"))?;
            if let Some(used) = read {
                self.evidence_uses.push(used);
            }
        }
        Ok(())
    }

    fn evidence_use(&mut self, id: MachineId, machine: &Machine) -> Checked<Option<EvidenceUse>> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let mut parameters = Vec::new();
        let mut plain = true;
        for parameter in typed.machine_type_parameters(machine) {
            match parameter.kind {
                TypeParameterKind::Value { type_reference } => {
                    match self.scalar_type(id, type_reference) {
                        Ok(ScalarType::Integer(integer)) => parameters.push(FamilyValue {
                            symbol: parameter.symbol,
                            name: parameter.name.as_str().to_owned(),
                            integer,
                            field: None,
                        }),
                        _ => plain = false,
                    }
                }
                TypeParameterKind::Type => {}
                _ => plain = false,
            }
        }
        let symbols: Vec<_> = parameters
            .iter()
            .map(|parameter| parameter.symbol)
            .collect();
        let own = symbols.as_slice();
        let mut held: Held = Vec::new();
        let mut constructions = Vec::new();
        let mut found = false;
        for state in typed.machine_states(machine) {
            let formals = typed
                .state_parameters
                .span(state.parameters)
                .ok_or_else(|| unserved("machine lost its parameters"))?;
            for formal in formals.iter() {
                if !self.names_indexed_data(formal.type_reference) {
                    continue;
                }
                if formal.relevance != language_core::BindingRelevance::Erased {
                    return Err(unserved("indexed data in a formal that is not erased"));
                }
                found = true;
                let applied = self.evidence_type(id, own, formal.type_reference)?;
                held.push((formal.symbol, applied));
            }
            for statement in typed.statement_table.statements(state.statement_nodes) {
                found |=
                    self.evidence_statement(id, own, statement, &mut held, &mut constructions)?;
            }
        }
        if !found {
            return Ok(None);
        }
        if !plain {
            return Err(unserved(
                "machine that holds indexed data and has a parameter that is neither an integer value nor a type",
            ));
        }
        // The machine's own requirements over its value parameters are what
        // a literal's case may rest on.
        let mut bound = Vec::with_capacity(parameters.len());
        for (position, parameter) in parameters.iter().enumerate() {
            bound.push((
                parameter.symbol,
                ScalarTerm::Value {
                    id: Self::index_identity(position)
                        .ok_or_else(|| unserved("index identity overflow"))?,
                    scalar_type: ScalarType::Integer(parameter.integer),
                },
            ));
        }
        let mut requires = Vec::new();
        if let Some(state) = typed.machine_states(machine).first() {
            let scope = ClauseScope {
                slots: &[],
                values: &[],
                bound: &bound,
                owner: state.symbol,
                result: ResultTerm::Absent("result in a requirement"),
                formal_fields: false,
                logical: None,
                subject: None,
            };
            for contract in super::span(
                |span| typed.signature_contracts.span(span),
                machine.contracts,
                id,
                "contract span",
            )?
            .iter()
            {
                if contract.kind != SignatureContractKind::Requires {
                    continue;
                }
                for fact in super::span(
                    |span| typed.proof_facts.span(span),
                    contract.facts,
                    id,
                    "contract fact span",
                )?
                .iter()
                {
                    // A requirement that names a formal or anything else
                    // this scope does not bind is not about the indices.
                    if let Ok(clause) = self
                        .fact(id, fact)
                        .and_then(|fact| self.clause(id, &scope, fact))
                    {
                        requires.push(clause);
                    }
                }
            }
        }
        Ok(Some(EvidenceUse {
            machine: machine.symbol,
            name: typed.symbols.display_path(machine.symbol, "::"),
            parameters,
            requires,
            constructions,
        }))
    }

    /// The family and index a type reference names, in the terms of the
    /// machine whose value parameters are `own`.
    fn evidence_type(
        &mut self,
        id: MachineId,
        own: &[SymbolHandle],
        reference: TypeReferenceHandle,
    ) -> Checked<Applied> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let value_type = self.value_type(id, reference)?;
        let family = self
            .family_of(value_type)
            .ok_or_else(|| unserved("evidence of data that is not indexed"))?
            .clone();
        let at = self
            .family_arguments(id, &family, &[], reference)?
            .into_iter()
            .map(|term| match term {
                FamilyTerm::Literal(value) => Ok(EvidenceTerm::Literal(value)),
                FamilyTerm::Name(symbol) => own
                    .iter()
                    .position(|parameter| *parameter == symbol)
                    .map(|position| EvidenceTerm::Parameter(position as u32))
                    .ok_or_else(|| {
                        unserved(
                            "indexed data at an index that is neither a number nor one of the machine's value parameters",
                        )
                    }),
                FamilyTerm::Index(_) | FamilyTerm::Payload(_) => {
                    Err(unserved("indexed data at one of its own indices outside its declaration"))
                }
            })
            .collect::<Checked<_>>()?;
        Ok(Applied {
            record: family.record,
            at,
        })
    }

    /// Whether a mathematical definition's result is indexed data.
    pub(super) fn returns_indexed_data(
        &self,
        definition: &crate::source_state::mathematical::MathematicalDefinition,
    ) -> bool {
        use crate::source_state::mathematical::MathematicalType;
        let typed = self.typed;
        typed.mathematical_types.is_valid(definition.result)
            && matches!(typed.mathematical_type(definition.result),
                MathematicalType::Ordinary(reference) if self.names_indexed_data(*reference))
    }

    /// The type a definition that hands evidence on takes and returns, when
    /// it is one: `let retain<Length: u64, Total: u64>(value: Vector<Length,
    /// Total>): Vector<Length, Total> = value;`. Its one parameter and its
    /// result are at the same index and its body is that parameter, so
    /// applying it changes neither the evidence nor its index.
    fn evidence_definition(
        &mut self,
        id: MachineId,
        symbol: SymbolHandle,
    ) -> Checked<Option<HandedOn>> {
        use crate::source_state::mathematical::{MathematicalBody, MathematicalType};
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let Some(definition) = typed
            .mathematical_definitions()
            .iter()
            .find(|definition| definition.symbol == symbol)
            .filter(|definition| self.returns_indexed_data(definition))
        else {
            return Ok(None);
        };
        let refused =
            || unserved("definition that returns indexed data other than the evidence it takes");
        let MathematicalType::Ordinary(result) = typed.mathematical_type(definition.result) else {
            return Err(refused());
        };
        let parameters = typed.mathematical_parameters(definition.parameters);
        let mut parameters = parameters.iter();
        let (Some(parameter), None) = (parameters.next(), parameters.next()) else {
            return Err(refused());
        };
        let MathematicalType::Ordinary(taken) = typed.mathematical_type(parameter.ty) else {
            return Err(refused());
        };
        let MathematicalBody::Definition(body) = definition.body else {
            return Err(refused());
        };
        if !matches!(typed.expression_table.expression(body),
            ExpressionNode::Name(path) if path.symbol == parameter.symbol)
        {
            return Err(refused());
        }
        let own = self.value_type(id, *result)?;
        let family = self
            .family_of(own)
            .ok_or_else(|| unserved("evidence of data that is not indexed"))?
            .clone();
        let declared = typed.data_type_parameters.span_or_empty(definition.binders);
        let binders: Vec<_> = declared
            .iter()
            .map(|binder| (binder.name.as_str().to_owned(), binder.symbol))
            .collect();
        let kinds: Vec<_> = declared
            .iter()
            .map(|binder| {
                (
                    binder.symbol,
                    matches!(binder.kind, TypeParameterKind::Value { .. }),
                )
            })
            .collect();
        let terms = self.family_arguments_within(id, &family, &[], &binders, *taken)?;
        if self.family_arguments_within(id, &family, &[], &binders, *result)? != terms {
            return Err(refused());
        }
        Ok(Some(HandedOn { kinds, terms }))
    }

    /// Reads one statement of a running machine for evidence. Returns
    /// whether it held, built or passed any.
    fn evidence_statement(
        &mut self,
        id: MachineId,
        own: &[SymbolHandle],
        statement: &StatementNode,
        held: &mut Held,
        constructions: &mut Vec<Construction>,
    ) -> Checked<bool> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        if let StatementNode::LocalData(local) = statement
            && local.type_reference.is_valid()
            && self.names_indexed_data(local.type_reference)
        {
            if local.relevance != language_core::BindingRelevance::Erased {
                return Err(unserved("indexed data in a local that is not erased"));
            }
            let applied = self.evidence_type(id, own, local.type_reference)?;
            self.evidence_initializer(id, own, local.initial_value, &applied, held, constructions)?;
            held.push((local.symbol, applied));
            return Ok(true);
        }
        // Every call the statement makes, at any depth, and every literal
        // of indexed data written anywhere but in an erased local above.
        let mut calls = Vec::new();
        let mut stray = false;
        match statement {
            StatementNode::Call(call) => calls.push((
                call.target_symbol,
                call.machine_arguments.to_vec(),
                typed
                    .statement_table
                    .expression_handles(call.arguments)
                    .to_vec(),
            )),
            StatementNode::Transition(transition) => {
                for target in [transition.target, transition.continuation] {
                    if typed.statement_table.transition_target_is_valid(target)
                        && let TransitionTargetNode::Named {
                            path, arguments, ..
                        } = typed.statement_table.transition_target(target)
                    {
                        calls.push((
                            path.symbol,
                            Vec::new(),
                            typed
                                .statement_table
                                .expression_handles(*arguments)
                                .to_vec(),
                        ));
                    }
                }
            }
            _ => {}
        }
        for (_, root) in typed.expression_roots(statement) {
            typed.subtree_for_each(
                root,
                crate::source_state::SubtreeEdges::VALUES,
                |_, node| match node {
                    ExpressionNode::Call(call) => calls.push((
                        call.target_symbol,
                        call.machine_arguments.to_vec(),
                        typed
                            .expression_table
                            .expression_handles(call.arguments)
                            .to_vec(),
                    )),
                    ExpressionNode::StructLiteral(literal) => {
                        stray |= typed
                            .unique_data_definition_by_symbol(literal.type_symbol)
                            .is_some_and(|data| self.is_family(data));
                    }
                    _ => {}
                },
            );
        }
        if stray {
            return Err(unserved(
                "indexed data built anywhere but in an erased local of the machine's own statements",
            ));
        }
        let mut found = false;
        for (target, written, arguments) in calls {
            found |= self.evidence_call(id, own, target, &written, &arguments, held)?;
        }
        Ok(found)
    }

    /// What a call writes for each parameter of what it calls: for a value,
    /// a number or one of the calling machine's own value parameters.
    /// `kinds` are the parameters called, each with whether it is a value.
    fn evidence_values(
        &self,
        id: MachineId,
        own: &[SymbolHandle],
        kinds: &[(SymbolHandle, bool)],
        written: &[crate::source_state::expression::StaticMachineArgument],
    ) -> Checked<Vec<Written>> {
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        if kinds.len() != written.len() {
            return Err(unserved(
                "evidence passed without an argument for each parameter",
            ));
        }
        let mut read = Vec::with_capacity(written.len());
        for (argument, (_, value)) in written.iter().zip(kinds) {
            if !*value {
                read.push(Written::Type);
                continue;
            }
            if argument.application.is_some() || argument.evidence_projection.is_some() {
                return Err(unserved(
                    "evidence at a value that is neither a number nor one of the machine's value parameters",
                ));
            }
            let term = if let Some(literal) = &argument.const_literal {
                literal.to_string().parse().ok().map(EvidenceTerm::Literal)
            } else if argument.symbol.is_valid() {
                own.iter()
                    .position(|parameter| *parameter == argument.symbol)
                    .map(|position| EvidenceTerm::Parameter(position as u32))
            } else {
                match argument.path.as_ref() {
                    [word] => word.as_str().parse().ok().map(EvidenceTerm::Literal),
                    _ => None,
                }
            };
            read.push(Written::Value(term.ok_or_else(|| {
                unserved(
                    "evidence at a value that is neither a number nor one of the machine's value parameters",
                )
            })?));
        }
        Ok(read)
    }

    /// The index a type of indexed data is at once each parameter of
    /// whatever declares it stands for what a call writes for it. `terms`
    /// are the type's indices as its declaration writes them, and `kinds`
    /// that declaration's parameters. `None` when the call does not fix one.
    fn applied_at_call(
        family: &FamilyRecord,
        terms: &[FamilyTerm],
        kinds: &[(SymbolHandle, bool)],
        written: &[Written],
    ) -> Option<Applied> {
        let at = terms
            .iter()
            .map(|term| match *term {
                FamilyTerm::Literal(value) => Some(EvidenceTerm::Literal(value)),
                FamilyTerm::Name(symbol) => {
                    let position = kinds.iter().position(|(kind, _)| *kind == symbol)?;
                    match written.get(position) {
                        Some(Written::Value(term)) => Some(*term),
                        _ => None,
                    }
                }
                FamilyTerm::Index(_) | FamilyTerm::Payload(_) => None,
            })
            .collect::<Option<_>>()?;
        Some(Applied {
            record: family.record,
            at,
        })
    }

    /// Checks one call: each argument for a formal that takes indexed data
    /// is evidence the machine holds, at the index the formal takes.
    fn evidence_call(
        &mut self,
        id: MachineId,
        own: &[SymbolHandle],
        target: SymbolHandle,
        written: &[crate::source_state::expression::StaticMachineArgument],
        arguments: &[ExpressionHandle],
        held: &Held,
    ) -> Checked<bool> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        let names_held = |expression: ExpressionHandle| {
            matches!(typed.expression_table.expression(expression),
                ExpressionNode::Name(path) if held.iter().any(|(symbol, ..)| *symbol == path.symbol))
        };
        let Some(callee) = self
            .entered_machine(target)
            .and_then(|machine| typed.machine_by_symbol(machine))
        else {
            // Not a machine: nothing here may take evidence.
            if arguments.iter().any(|argument| names_held(*argument)) {
                return Err(unserved(
                    "evidence passed to something other than a machine",
                ));
            }
            return Ok(false);
        };
        let Some(formals) = typed
            .machine_states(callee)
            .first()
            .and_then(|entry| typed.state_parameters.span(entry.parameters))
        else {
            return Ok(false);
        };
        let takes = formals
            .iter()
            .any(|formal| self.names_indexed_data(formal.type_reference));
        if formals.len() != arguments.len() {
            if takes || arguments.iter().any(|argument| names_held(*argument)) {
                return Err(unserved(
                    "call that takes evidence with another number of arguments",
                ));
            }
            return Ok(false);
        }
        if !takes {
            if arguments.iter().any(|argument| names_held(*argument)) {
                return Err(unserved("evidence passed to a formal that takes none"));
            }
            return Ok(false);
        }
        // The callee's parameters stand for what is written for them at
        // this call. A copy made for closed arguments has none left.
        let kinds: Vec<_> = typed
            .machine_type_parameters(callee)
            .into_iter()
            .map(|parameter| {
                (
                    parameter.symbol,
                    matches!(parameter.kind, TypeParameterKind::Value { .. }),
                )
            })
            .collect();
        if !kinds.is_empty() && kinds.len() != written.len() {
            return Err(unserved(
                "call that takes evidence without all its value arguments",
            ));
        }
        let written = if kinds.is_empty() {
            Vec::new()
        } else {
            self.evidence_values(id, own, &kinds, written)?
        };
        for (formal, &argument) in formals.iter().zip(arguments) {
            if !self.names_indexed_data(formal.type_reference) {
                if names_held(argument) {
                    return Err(unserved("evidence passed to a formal that takes none"));
                }
                continue;
            }
            let value_type = self.value_type(id, formal.type_reference)?;
            let family = self
                .family_of(value_type)
                .ok_or_else(|| unserved("evidence of data that is not indexed"))?
                .clone();
            let terms = self.family_arguments(id, &family, &[], formal.type_reference)?;
            let expected =
                Self::applied_at_call(&family, &terms, &kinds, &written).ok_or_else(|| {
                    unserved("formal that takes indexed data at an index this call does not fix")
                })?;
            let passed = match typed.expression_table.expression(argument) {
                ExpressionNode::Name(path) => {
                    held.iter().find(|(symbol, ..)| *symbol == path.symbol)
                }
                _ => None,
            }
            .ok_or_else(|| {
                unserved("evidence passed that is no erased local or formal of the machine")
            })?;
            if passed.1 != expected {
                return Err(unserved(
                    "evidence passed at another index than the formal takes",
                ));
            }
        }
        Ok(true)
    }

    /// Checks what an erased local of indexed data is initialised with:
    /// evidence the machine already holds of the same thing, or a case
    /// literal, whose children must be evidence at the indices the case
    /// declares for them.
    fn evidence_initializer(
        &mut self,
        id: MachineId,
        own: &[SymbolHandle],
        initializer: ExpressionHandle,
        applied: &Applied,
        held: &Held,
        constructions: &mut Vec<Construction>,
    ) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| refuse(id, Construct::Machine, Unserved::Machine(what));
        if !initializer.is_valid() {
            return Err(unserved(
                "erased local of indexed data without an initializer",
            ));
        }
        let record = applied.record;
        let at = &applied.at;
        let family = self
            .families
            .iter()
            .find(|family| family.record == record)
            .ok_or_else(|| unserved("evidence of data that is not indexed"))?
            .clone();
        let source = Some(typed.expression_table.source_span(initializer));
        let (case, fields) = match typed.expression_table.expression(initializer) {
            ExpressionNode::Name(path) => {
                if let Some((_, known)) = held.iter().find(|(symbol, ..)| *symbol == path.symbol) {
                    if known != applied {
                        return Err(unserved(
                            "evidence held at another index than the local declares",
                        ));
                    }
                    return Ok(());
                }
                // A case with nothing in it is written as its name.
                let case = family
                    .cases
                    .iter()
                    .position(|case| case.symbol == path.symbol)
                    .ok_or_else(|| {
                        unserved("erased local of indexed data initialised by another name")
                    })?;
                (case, Vec::new())
            }
            ExpressionNode::StructLiteral(literal)
                if literal.type_symbol == family.declaration && !literal.zero_omitted_fields =>
            {
                let case = literal
                    .case_symbol
                    .and_then(|symbol| family.cases.iter().position(|case| case.symbol == symbol))
                    .ok_or_else(|| unserved("literal of indexed data that names no case"))?;
                (
                    case,
                    typed
                        .expression_table
                        .struct_fields(literal.fields)
                        .iter()
                        .map(|field| (field.field_symbol, field.value))
                        .collect(),
                )
            }
            ExpressionNode::Call(call) => {
                let Some(handed) = self.evidence_definition(id, call.target_symbol)? else {
                    return Err(unserved(
                        "erased local of indexed data initialised by a call of other than a definition that hands evidence on",
                    ));
                };
                // The definition's binders stand for what is written for
                // them; its one argument is evidence of that, and so is its
                // result.
                if call.machine_arguments.len() != handed.kinds.len() {
                    return Err(unserved(
                        "definition that hands evidence on applied without all its value arguments",
                    ));
                }
                let written =
                    self.evidence_values(id, own, &handed.kinds, &call.machine_arguments)?;
                let expected = Self::applied_at_call(
                    &family,
                    &handed.terms,
                    &handed.kinds,
                    &written,
                )
                .ok_or_else(|| {
                    unserved("definition that hands evidence on at an index its call does not fix")
                })?;
                let [argument] = typed.expression_table.expression_handles(call.arguments) else {
                    return Err(unserved(
                        "definition that hands evidence on applied to other than one value",
                    ));
                };
                let passed = match typed.expression_table.expression(*argument) {
                    ExpressionNode::Name(path) => {
                        held.iter().find(|(symbol, ..)| *symbol == path.symbol)
                    }
                    _ => None,
                }
                .ok_or_else(|| {
                    unserved("evidence passed that is no erased local or formal of the machine")
                })?;
                if passed.1 != expected || expected != *applied {
                    return Err(unserved(
                        "evidence handed on at another index than the local declares",
                    ));
                }
                return Ok(());
            }
            _ => {
                return Err(unserved(
                    "erased local of indexed data initialised by other than a case literal or evidence the machine holds",
                ));
            }
        };
        let declared = &family.cases[case];
        if fields.len() != declared.payload.len() + declared.children.len() + declared.carried.len()
        {
            return Err(unserved("literal of indexed data without each field once"));
        }
        let once = |symbol: SymbolHandle| {
            let mut written = fields.iter().filter(|(field, _)| *field == symbol);
            match (written.next(), written.next()) {
                (Some((_, expression)), None) => Ok(*expression),
                _ => Err(unserved("literal of indexed data without each field once")),
            }
        };
        let mut payload = Vec::with_capacity(declared.payload.len());
        for value in &declared.payload {
            let expression = once(value.symbol)?;
            payload.push(match typed.expression_table.expression(expression) {
                ExpressionNode::Integer(literal) => literal
                    .to_string()
                    .parse()
                    .ok()
                    .map(EvidenceTerm::Literal),
                ExpressionNode::Name(path) => own
                    .iter()
                    .position(|parameter| *parameter == path.symbol)
                    .map(|position| EvidenceTerm::Parameter(position as u32)),
                _ => None,
            }
            .ok_or_else(|| {
                unserved(
                    "payload of indexed data that is neither a number nor one of the machine's value parameters",
                )
            })?);
        }
        // A payload value of a type parameter's type is erased with the
        // literal and nothing is proved of it. A name or a literal computes
        // nothing, so leaving it unread drops no obligation.
        for carried in &declared.carried {
            let expression = once(*carried)?;
            if !matches!(
                typed.expression_table.expression(expression),
                ExpressionNode::Name(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_)
            ) {
                return Err(unserved(
                    "payload of a type parameter's type that is neither a name nor a literal",
                ));
            }
        }
        for child in &declared.children {
            let expression = once(child.symbol)?;
            let expected: Vec<_> = child
                .at
                .iter()
                .map(|term| match *term {
                    FamilyTerm::Index(index) => at.get(index as usize).copied(),
                    FamilyTerm::Payload(position) => payload.get(position as usize).copied(),
                    FamilyTerm::Literal(value) => Some(EvidenceTerm::Literal(value)),
                    FamilyTerm::Name(_) => None,
                })
                .collect::<Option<_>>()
                .ok_or_else(|| {
                    unserved("child of indexed data at an index the literal does not fix")
                })?;
            let expected = Applied {
                record,
                at: expected,
            };
            let passed = match typed.expression_table.expression(expression) {
                ExpressionNode::Name(path) => {
                    held.iter().find(|(symbol, ..)| *symbol == path.symbol)
                }
                _ => None,
            }
            .ok_or_else(|| {
                unserved("child of a literal that is no erased local or formal of the machine")
            })?;
            if passed.1 != expected {
                return Err(unserved(
                    "child of a literal at another index than the case declares for it",
                ));
            }
        }
        constructions.push(Construction {
            record,
            case,
            at: at.clone(),
            payload,
            source,
        });
        Ok(())
    }
}
