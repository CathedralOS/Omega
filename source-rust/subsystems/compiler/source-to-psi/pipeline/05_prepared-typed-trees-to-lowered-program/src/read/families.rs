use crate::read::refuse_at;
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
    ScalarTerm, ScalarType, StructuralCaseId, StructuralCaseSubject, StructuralFieldId,
    StructuralTypeId, ValueId,
};
use source_text::SourceSpan;
use symbols::SymbolHandle;
use terminal_check_tables::Checked;

use super::Program;
use super::contracts::{ClauseScope, DataSubject, ResultTerm};
use super::places::RecordSource;
use crate::Unserved;
use crate::body::{CaseType, RecordField, RecordType, StorageShape, ValueType};

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
    pub(crate) domain: FamilyDomain,
    /// The field of the case this value is, for a payload value.
    pub(crate) field: Option<StructuralFieldId>,
}

/// What an index or a payload value of a family ranges over: an integer,
/// or mathematical data (`data Lookup<C: Ctx, T: Ty>`, a derivation indexed
/// by a context and a type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FamilyDomain {
    Integer(IntegerType),
    Data(StructuralTypeId),
}

impl FamilyValue {
    /// The integer type the value holds, when it is an integer.
    pub(crate) fn integer(&self) -> Option<IntegerType> {
        match self.domain {
            FamilyDomain::Integer(integer) => Some(integer),
            FamilyDomain::Data(_) => None,
        }
    }

    /// The value as a clause names it: an integer is the scalar `scalar`
    /// says it is, and data is the input at `subject`.
    fn binding(
        &self,
        scalar: impl FnOnce(IntegerType) -> ScalarTerm,
        subject: impl FnOnce() -> StructuralCaseSubject,
    ) -> IndexBinding {
        match self.domain {
            FamilyDomain::Integer(integer) => IndexBinding::Scalar(self.symbol, scalar(integer)),
            FamilyDomain::Data(record) => {
                IndexBinding::Data(self.symbol, ValueType::record(record), subject())
            }
        }
    }
}

/// How a clause over a family names one of its values.
enum IndexBinding {
    Scalar(SymbolHandle, ScalarTerm),
    Data(SymbolHandle, ValueType, StructuralCaseSubject),
}

/// The place that stands for index `position` of a family when it is
/// data, in a clause over the family: after `self`, which is place 1
/// (`body::self_place`).
pub(crate) fn index_place(position: usize) -> Option<semantic_vocabulary::PlaceId> {
    semantic_vocabulary::PlaceId::new(position as u64 + 2)
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

/// An integer operator in a computed index (`Length + 1`, `Length - 1`, `2 * N`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndexOp {
    Add,
    Sub,
    Mul,
}

/// What a type argument of the family is, where a case or a formal names
/// the family at arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FamilyTerm {
    /// The declaration's own parameter at this position.
    Index(u32),
    /// The payload value at this position of the case that holds the child.
    Payload(u32),
    Literal(i128),
    /// Any other name: a formal or a local of the machine that names the
    /// type, by its symbol.
    Name(SymbolHandle),
    Binary(IndexOp, Box<FamilyTerm>, Box<FamilyTerm>),
}

impl FamilyTerm {
    fn map_names<E>(
        &self,
        f: &mut impl FnMut(SymbolHandle) -> Result<FamilyTerm, E>,
    ) -> Result<FamilyTerm, E> {
        Ok(match self {
            Self::Name(symbol) => f(*symbol)?,
            Self::Binary(op, left, right) => Self::Binary(
                *op,
                Box::new(left.map_names(f)?),
                Box::new(right.map_names(f)?),
            ),
            other => other.clone(),
        })
    }

    fn try_to_evidence<E>(
        &self,
        f: &mut impl FnMut(&FamilyTerm) -> Result<EvidenceTerm, E>,
    ) -> Result<EvidenceTerm, E> {
        match self {
            Self::Binary(op, left, right) => Ok(EvidenceTerm::Binary(
                *op,
                Box::new(left.try_to_evidence(f)?),
                Box::new(right.try_to_evidence(f)?),
            )),
            leaf => f(leaf),
        }
    }
}

/// A theorem over a family, as the signature needs it.
#[derive(Debug, Clone)]
pub(crate) struct FamilyTheorem {
    pub(crate) record: StructuralTypeId,
    /// What the theorem guarantees of the indices: index `i` is the scalar
    /// value numbered `i + 1`, as in the family's `requires`.
    pub(crate) ensures: Vec<Proposition>,
    /// Each `ensures` clause as authored. A theorem over a family indexed
    /// by data states no contract of its own signature, so the signature's
    /// names do not hold these.
    pub(crate) ensures_text: Vec<crate::ClauseText>,
    /// One per case of the family, in declared order.
    pub(crate) cases: Vec<FamilyTheoremCase>,
    /// What the theorem is ranked on. A theorem that cites itself has a rank.
    pub(crate) rank: Option<FamilyRank>,
}

/// What a theorem over indexed data is ranked on (`terminates by ..`).
/// termination.md, "Ranking": a ranking view may be "a proper subtree",
/// and contracts.md, "Citation and induction": "Proof/compile-time
/// recursion may rank on mathematical structure".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FamilyRank {
    /// One of the family's indices, by position, of an unsigned type
    /// (`terminates by Length`). Each cited child's value of it is shown
    /// to be smaller.
    Index(u32),
    /// The evidence itself (`terminates by evidence`). The theorem is cited
    /// only at a child of the case it dispatched on, which is a proper part
    /// of the evidence, so nothing more is owed.
    Evidence,
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

/// What a machine that runs holds in place of an erased value of indexed
/// data: an erased Boolean nothing reads. The value itself has no layout
/// and exists only for the checker, and what it is evidence of is the
/// mathematical signature's to check.
pub(super) fn evidence_witness() -> ValueType {
    ValueType::Scalar(ScalarType::Boolean, crate::body::Qualification::NONE)
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
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
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
        // Each index as its clauses name it: an integer index is the
        // scalar value numbered from one, a data index the input at its
        // index place.
        let mut bindings = Vec::with_capacity(values.len());
        for (position, (symbol, index)) in values.iter().zip(&family.indices).enumerate() {
            let id = Self::index_identity(position)
                .ok_or_else(|| unserved("index identity overflow"))?;
            let place = index_place(position).ok_or_else(|| unserved("index place identity"))?;
            bindings.push(
                FamilyValue {
                    symbol: *symbol,
                    ..index.clone()
                }
                .binding(
                    |integer| ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Integer(integer),
                    },
                    || StructuralCaseSubject::new(place, Vec::new()),
                ),
            );
        }
        let saved = std::mem::take(&mut self.data_indices);
        let mut indices = Vec::new();
        self.bind(bindings, &mut indices);
        let read = self.family_theorem_body(id, machine, state, &family, &values, &indices);
        self.data_indices = saved;
        read
    }

    /// Splits the bindings of a family's values: the integers, added to a
    /// clause scope's `bound`, and the data, which the clause reader finds
    /// in `data_indices`.
    fn bind(&mut self, bindings: Vec<IndexBinding>, bound: &mut Vec<(SymbolHandle, ScalarTerm)>) {
        for binding in bindings {
            match binding {
                IndexBinding::Scalar(symbol, term) => bound.push((symbol, term)),
                IndexBinding::Data(symbol, ty, subject) => {
                    self.data_indices.push((symbol, ty, subject));
                }
            }
        }
    }

    /// The body of a theorem over indexed data, read with the family's
    /// indices bound: `values` are the theorem's own value parameters, in
    /// order, and `indices` the integer ones as their clauses name them.
    fn family_theorem_body(
        &mut self,
        id: MachineId,
        machine: &Machine,
        state: &State,
        family: &FamilyRecord,
        values: &[SymbolHandle],
        indices: &[(SymbolHandle, ScalarTerm)],
    ) -> Checked<FamilyTheorem> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        let formals = typed
            .state_parameters
            .span(state.parameters)
            .ok_or_else(|| unserved("theorem over indexed data lost its parameters"))?;
        let evidence = formals
            .iter()
            .next()
            .ok_or_else(|| unserved("theorem over indexed data without its evidence"))?;
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
            indices,
            SignatureContractKind::Ensures,
            "theorem over indexed data with a clause other than `ensures`",
        )?;
        if ensures.is_empty() {
            return Err(unserved(
                "theorem over indexed data that guarantees nothing",
            ));
        }
        let mut ensures_text = Vec::with_capacity(ensures.len());
        for contract in super::span(
            |span| typed.signature_contracts.span(span),
            machine.contracts,
            id,
            "contract span",
        )? {
            for fact in super::span(
                |span| typed.proof_facts.span(span),
                contract.facts,
                id,
                "contract fact span",
            )? {
                if let crate::source_state::domain::ProofFact::Expression(expression) = fact {
                    ensures_text.push(self.clause_text(&[], *expression));
                }
            }
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
                        family,
                        position,
                        values,
                        indices,
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
        // being smaller there: the evidence, of which the child is a part,
        // or one of its indices, which counts down to zero only if it
        // cannot be negative.
        let rank = match crate::source_state::ranking::resolve_machine_witness_subjects(
            typed, machine,
        )
        .as_deref()
        {
            None | Some([]) => None,
            Some([subject])
                if matches!(typed.expression_table.expression(*subject),
                        ExpressionNode::Name(path) if path.symbol == evidence.symbol) =>
            {
                Some(FamilyRank::Evidence)
            }
            Some([subject]) => {
                let position = match typed.expression_table.expression(*subject) {
                        ExpressionNode::Name(path) => {
                            values.iter().position(|value| *value == path.symbol)
                        }
                        _ => None,
                    }
                    .ok_or_else(|| {
                        unserved(
                            "theorem over indexed data ranked on other than its evidence or one of its indices",
                        )
                    })?;
                if family.indices[position].integer().is_none_or(|integer| {
                    integer.sign() != semantic_vocabulary::IntegerSign::Unsigned
                }) {
                    return Err(unserved(
                        "theorem over indexed data ranked on an index that is not an unsigned integer",
                    ));
                }
                Some(FamilyRank::Index(position as u32))
            }
            Some(_) => {
                return Err(unserved(
                    "theorem over indexed data ranked on other than its evidence or one of its indices",
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
            ensures_text,
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
        values: &[SymbolHandle],
        indices: &[(SymbolHandle, ScalarTerm)],
        subject: SymbolHandle,
        target: SymbolHandle,
        arguments: &[ExpressionHandle],
        read: &mut FamilyTheoremCase,
    ) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
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
                if self.family_domain(id, formal.type_reference)? != case.payload[payload].domain {
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
                .map(|term| {
                    term.map_names(&mut |symbol| {
                        if let Some(index) = values.iter().position(|parameter| *parameter == symbol)
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
                    })
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
        let mut bindings = Vec::new();
        for (symbol, what) in &received {
            if let Received::Payload(payload) = what {
                let value = &case.payload[*payload];
                let field = value
                    .field
                    .ok_or_else(|| unserved("payload value lost its field"))?;
                let root =
                    crate::body::self_place().ok_or_else(|| unserved("self place identity"))?;
                let path = vec![
                    CanonicalStructuralPathSegment::Case(case.case),
                    CanonicalStructuralPathSegment::Field(field),
                ];
                bindings.push(
                    FamilyValue {
                        symbol: *symbol,
                        ..value.clone()
                    }
                    .binding(
                        |scalar_type| ScalarTerm::IntegerField {
                            version: ContentPlaceVersion::Current,
                            root,
                            path: path.clone(),
                            scalar_type,
                        },
                        || StructuralCaseSubject::new(root, path.clone()),
                    ),
                );
            }
        }
        let held = self.data_indices.len();
        self.bind(bindings, &mut bound);
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
        self.data_indices.truncate(held);
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
            && self.contains_itself_directly(data)
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
        let unserved = |what| refuse_at(id, Unserved::Type(what));
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
        // Each value argument, read by the one index reader.
        arguments
            .iter()
            .enumerate()
            .filter(|(position, _)| family.type_at(*position).is_none())
            .map(|(_, argument)| {
                Ok(self
                    .written_index(id, binders, *argument)?
                    .to_family_term(&named))
            })
            .collect()
    }

    /// One value argument as a type reference writes it: a name, resolved
    /// to its symbol (against `binders` when the typed trees kept only its
    /// spelling), a number, or an integer arithmetic expression built from
    /// them (`Length + 1`). This is the one reader of an index, for a
    /// family and for data that holds evidence of one in an erased field.
    fn written_index(
        &self,
        id: MachineId,
        binders: &[(String, SymbolHandle)],
        argument: TypeReferenceHandle,
    ) -> Checked<WrittenIndex> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Type(what));
        let literal = |text: &str| {
            text.parse::<i128>()
                .map(WrittenIndex::Literal)
                .map_err(|_| unserved("index of indexed data that is no integer"))
        };
        match typed.type_reference_table.type_reference(argument) {
            TypeReferenceNode::Named { symbol, .. } if symbol.is_valid() => {
                Ok(WrittenIndex::Name(*symbol))
            }
            TypeReferenceNode::Named { name, .. } => binders
                .iter()
                .find(|(binder, _)| binder == name.as_str())
                .map_or_else(
                    || literal(name.as_str()),
                    |(_, symbol)| Ok(WrittenIndex::Name(*symbol)),
                ),
            TypeReferenceNode::ConstExpression(expression) => {
                self.written_index_expression(id, binders, *expression)
            }
            _ => Err(unserved(
                "index of indexed data other than a name, an integer, or integer arithmetic",
            )),
        }
    }

    fn written_index_expression(
        &self,
        id: MachineId,
        binders: &[(String, SymbolHandle)],
        expression: ExpressionHandle,
    ) -> Checked<WrittenIndex> {
        use crate::source_state::expression::BinaryOperator;
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Type(what));
        match typed.expression_table.expression(expression) {
            ExpressionNode::Name(path) if path.symbol.is_valid() => {
                Ok(WrittenIndex::Name(path.symbol))
            }
            ExpressionNode::Name(path) => {
                match typed.expression_table.name_path_members(path.members) {
                    [word] => binders
                        .iter()
                        .find(|(binder, _)| binder == word.as_str())
                        .map(|(_, symbol)| WrittenIndex::Name(*symbol))
                        .ok_or_else(|| {
                            unserved("index of indexed data that names an unresolved binder")
                        }),
                    _ => Err(unserved(
                        "index of indexed data other than a name, an integer, or integer arithmetic",
                    )),
                }
            }
            ExpressionNode::Integer(value) => value
                .to_string()
                .parse::<i128>()
                .map(WrittenIndex::Literal)
                .map_err(|_| unserved("index of indexed data that is no integer")),
            ExpressionNode::Binary(binary) => {
                let op = match binary.operator {
                    BinaryOperator::Add => IndexOp::Add,
                    BinaryOperator::Subtract => IndexOp::Sub,
                    BinaryOperator::Multiply => IndexOp::Mul,
                    _ => {
                        return Err(unserved(
                            "index of indexed data with an operator other than +, -, or *",
                        ));
                    }
                };
                let left = self.written_index_expression(id, binders, binary.left)?;
                let right = self.written_index_expression(id, binders, binary.right)?;
                Ok(WrittenIndex::Binary(op, Box::new(left), Box::new(right)))
            }
            _ => Err(unserved(
                "index of indexed data other than a name, an integer, or integer arithmetic",
            )),
        }
    }

    /// Checks the type of an erased field of evidence in a generic data
    /// template: each index it writes is built from the template's own value
    /// binders and numbers (`Measured<Length, Total>` holding
    /// `Vector<Length, Total>` or `Vector<Length + 1, Total>`).
    pub(super) fn evidence_field_indices(
        &self,
        id: MachineId,
        parameters: arena::RootRows<'_, crate::source_state::data::TypeParameter>,
        reference: TypeReferenceHandle,
    ) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Type(what));
        let TypeReferenceNode::Generic {
            arguments,
            lifetime_arguments,
            ..
        } = typed.type_reference_table.type_reference(reference)
        else {
            return Err(unserved("indexed data named without its indices"));
        };
        if !lifetime_arguments.is_empty() {
            return Err(unserved("indexed data named through another declaration"));
        }
        for argument in typed
            .type_reference_table
            .type_reference_handles(*arguments)
        {
            let index = self.written_index(id, &[], *argument)?;
            if !index.every_name(&mut |symbol| {
                parameters.iter().any(|parameter| {
                    parameter.symbol == symbol
                        && matches!(
                            parameter.kind,
                            TypeParameterKind::Value { .. } | TypeParameterKind::Const { .. }
                        )
                })
            }) {
                return Err(unserved(
                    "erased field of evidence at an index other than a value parameter of its data or a number",
                ));
            }
        }
        Ok(())
    }

    /// Whether a type reference names data, not itself indexed, with an
    /// erased field of indexed data: data that holds evidence.
    pub(super) fn holds_evidence(&self, reference: TypeReferenceHandle) -> bool {
        self.held_data(reference)
            .is_some_and(|data| self.evidence_fields(data).next().is_some())
    }

    /// The declaration a type reference applies, when it is data that is
    /// not itself indexed.
    fn held_data(&self, reference: TypeReferenceHandle) -> Option<&DataDefinition> {
        if !reference.is_valid() {
            return None;
        }
        let symbol = match self.typed.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Named { symbol, .. } => *symbol,
            TypeReferenceNode::Generic { base_symbol, .. } => *base_symbol,
            _ => return None,
        };
        self.typed
            .unique_data_definition_by_symbol(symbol)
            .filter(|data| !self.is_family(data))
    }

    /// The erased fields of a declaration that hold evidence of indexed
    /// data. Only a record's own fields do: a case's payload is not read.
    fn evidence_fields<'d>(
        &'d self,
        data: &'d DataDefinition,
    ) -> impl Iterator<Item = &'d crate::source_state::data::DataField> + 'd {
        self.typed
            .data_members(data)
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field) => Some(field),
                DataMember::Variant(_) => None,
            })
            .filter(|field| {
                field.relevance == language_core::BindingRelevance::Erased
                    && self.names_indexed_data(field.type_reference)
            })
    }

    /// The evidence the erased field `field` of data typed `holder` holds:
    /// the family its declared type names, at the indices that type writes
    /// once each binder of the data stands for what `holder` applies it at.
    /// `Measured<Length, Total>` with `evidence [erased]: Vector<Length,
    /// Total>`, held as `Measured<0, 7>`, holds `Vector` at `0, 7`.
    fn field_evidence(
        &mut self,
        id: MachineId,
        own: &[SymbolHandle],
        holder: TypeReferenceHandle,
        field: SymbolHandle,
    ) -> Checked<Option<Applied>> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        let Some(data) = self.held_data(holder) else {
            return Ok(None);
        };
        let Some(declared) = self
            .evidence_fields(data)
            .find(|candidate| field.is_valid() && candidate.symbol == field)
            .map(|declared| declared.type_reference)
        else {
            return Ok(None);
        };
        // What the holder applies each value binder of the data at.
        let mut applied = Vec::new();
        if let TypeReferenceNode::Generic { arguments, .. } =
            typed.type_reference_table.type_reference(holder)
        {
            let arguments = typed
                .type_reference_table
                .type_reference_handles(*arguments);
            for (parameter, argument) in typed.data_type_parameters(data).iter().zip(arguments) {
                if let TypeParameterKind::Value { .. } | TypeParameterKind::Const { .. } =
                    parameter.kind
                {
                    applied.push((parameter.symbol, self.written_index(id, &[], *argument)?));
                }
            }
        }
        let value_type = self.value_type(id, declared)?;
        let family = self
            .family_of(value_type)
            .ok_or_else(|| unserved("evidence of data that is not indexed"))?
            .clone();
        let at = self
            .family_arguments(id, &family, &[], declared)?
            .into_iter()
            .map(|term| {
                term.try_to_evidence(&mut |leaf| {
                    let written = match leaf {
                        FamilyTerm::Literal(value) => WrittenIndex::Literal(*value),
                        FamilyTerm::Name(binder) => applied
                            .iter()
                            .find(|(parameter, _)| *parameter == *binder)
                            .map(|(_, written)| written.clone())
                            .ok_or_else(|| {
                                unserved("evidence in a field at an index its data does not bind")
                            })?,
                        FamilyTerm::Index(_) | FamilyTerm::Payload(_) => {
                            return Err(unserved(
                                "evidence in a field at an index its data does not bind",
                            ));
                        }
                        FamilyTerm::Binary(..) => unreachable!(),
                    };
                    written.to_evidence_term(own, || {
                        unserved(
                            "data holding evidence at a value that is neither a number nor one of the machine's value parameters",
                        )
                    })
                })
            })
            .collect::<Checked<_>>()?;
        Ok(Some(Applied {
            record: family.record,
            at,
        }))
    }

    /// The evidence an expression names in a running machine: an erased
    /// local or formal it holds, or an erased field of evidence of data it
    /// holds in a formal or a local (`holder.evidence`). `None` when the
    /// expression names no evidence.
    fn held_evidence(
        &mut self,
        id: MachineId,
        own: &[SymbolHandle],
        held: &Held,
        holders: &Holders,
        expression: ExpressionHandle,
    ) -> Checked<Option<Applied>> {
        let typed = self.typed;
        match typed.expression_table.expression(expression) {
            ExpressionNode::Name(path) => Ok(held
                .iter()
                .find(|(symbol, ..)| *symbol == path.symbol)
                .map(|(_, applied)| applied.clone())),
            ExpressionNode::Member(member) => {
                let ExpressionNode::Name(path) = typed.expression_table.expression(member.receiver)
                else {
                    return Ok(None);
                };
                let Some(&(_, holder)) = holders.iter().find(|(symbol, _)| *symbol == path.symbol)
                else {
                    return Ok(None);
                };
                self.field_evidence(id, own, holder, member.selection.field_symbol())
            }
            _ => Ok(None),
        }
    }

    /// Checks a literal of data that holds evidence: each erased field of
    /// evidence is supplied evidence the machine holds, at the index the
    /// literal's type gives that field.
    fn evidence_in_literal(
        &mut self,
        id: MachineId,
        own: &[SymbolHandle],
        literal: ExpressionHandle,
        held: &Held,
        holders: &Holders,
        index_equalities: &mut Vec<IndexEquality>,
    ) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        let source = Some(typed.expression_table.source_span(literal));
        let ExpressionNode::StructLiteral(literal) = typed.expression_table.expression(literal)
        else {
            return Ok(());
        };
        if !literal.application_type.is_valid() {
            return Err(unserved(
                "literal of data holding evidence without its type",
            ));
        }
        for field in typed.expression_table.struct_fields(literal.fields) {
            let Some(expected) =
                self.field_evidence(id, own, literal.application_type, field.field_symbol)?
            else {
                continue;
            };
            let passed = self
                .held_evidence(id, own, held, holders, field.value)?
                .ok_or_else(|| {
                    unserved("evidence in a field that is no evidence the machine holds")
                })?;
            Self::require_same_indices(
                id,
                &passed,
                &expected,
                source,
                "evidence in a field at another index than the field's type takes",
                index_equalities,
            )?;
        }
        Ok(())
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
        let unserved = |what| refuse_at(id, Unserved::Type(what));
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
                        domain: self.family_domain(id, type_reference)?,
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
            multiplicity: crate::body::source_multiplicity(data.properties.multiplicity),
            cleanup: None,
            identity,
            shape: StorageShape::Sum(Vec::new()),
            requires: Vec::new(),
            authored_layout: false,
            erased: Vec::new(),
        });
        self.record_sources.push(RecordSource {
            constant_input_owner: None,
            declaration: symbol,
            cleanup: None,
            application: None,
            region_owner: SymbolHandle::invalid(),
            fields: Vec::new(),
            cases: Vec::new(),
            erased: Vec::new(),
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
                    let domain = self.family_domain(id, field.type_reference)?;
                    payload.push(FamilyValue {
                        symbol: field.symbol,
                        name,
                        domain,
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
        let mut bindings = Vec::new();
        for (position, index) in self
            .families
            .last()
            .expect("the family was pushed above")
            .indices
            .iter()
            .enumerate()
        {
            let value = Self::index_identity(position)
                .ok_or_else(|| unserved("index identity overflow"))?;
            let place = index_place(position).ok_or_else(|| unserved("index place identity"))?;
            bindings.push(index.binding(
                |integer| ScalarTerm::Value {
                    id: value,
                    scalar_type: ScalarType::Integer(integer),
                },
                || StructuralCaseSubject::new(place, Vec::new()),
            ));
        }
        let saved = std::mem::take(&mut self.data_indices);
        let mut bound = Vec::new();
        self.bind(bindings, &mut bound);
        let root = crate::body::self_place().ok_or_else(|| unserved("self place identity"))?;
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
            .collect::<Checked<Vec<_>>>();
        self.data_indices = saved;
        let requires = requires?;
        let family = self
            .families
            .iter_mut()
            .find(|family| family.record == record)
            .expect("the family was pushed above");
        family.cases = cases;
        family.requires = requires;
        Ok(own)
    }

    /// What an index or payload value of a family written with this type
    /// ranges over: an integer, or mathematical data that is not itself a
    /// family.
    fn family_domain(
        &mut self,
        id: MachineId,
        reference: TypeReferenceHandle,
    ) -> Checked<FamilyDomain> {
        match self.value_type(id, reference)? {
            ValueType::Scalar(ScalarType::Integer(integer), _) => {
                Ok(FamilyDomain::Integer(integer))
            }
            ValueType::Record(record, ..)
                if self.self_containing.binary_search(&record).is_ok()
                    && self.families.iter().all(|family| family.record != record) =>
            {
                Ok(FamilyDomain::Data(record))
            }
            _ => Err(refuse_at(
                id,
                Unserved::Type(
                    "indexed data over a value that is neither an integer nor mathematical data",
                ),
            )),
        }
    }

    /// Whether a machine has a value parameter of mathematical data.
    pub(super) fn has_data_values(&mut self, id: MachineId, machine: &Machine) -> bool {
        self.typed
            .machine_type_parameters(machine)
            .iter()
            .any(|parameter| match parameter.kind {
                TypeParameterKind::Value { type_reference } => {
                    self.names_mathematical_data(id, type_reference)
                }
                _ => false,
            })
    }

    /// Whether the type names mathematical data that is not itself a
    /// family: a value a family may be indexed by.
    pub(super) fn names_mathematical_data(
        &mut self,
        id: MachineId,
        reference: TypeReferenceHandle,
    ) -> bool {
        matches!(self.family_domain(id, reference), Ok(FamilyDomain::Data(_)))
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
    pub(crate) index_equalities: Vec<IndexEquality>,
}

/// One index equality obligation between two computed indices of the same
/// indexed family, proved via `LawProof::Integers`.
#[derive(Debug, Clone)]
pub(crate) struct IndexEquality {
    pub(crate) left: EvidenceTerm,
    pub(crate) right: EvidenceTerm,
    pub(crate) source: Option<SourceSpan>,
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
/// of the machine's own value parameters, a number, or integer arithmetic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EvidenceTerm {
    Parameter(u32),
    Literal(i128),
    Binary(IndexOp, Box<EvidenceTerm>, Box<EvidenceTerm>),
}

impl EvidenceTerm {
    pub(crate) fn is_computed(&self) -> bool {
        matches!(self, Self::Binary(..))
    }
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

/// The formals and locals of a machine that hold data with an erased field
/// of evidence, each with its type as written.
type Holders = Vec<(SymbolHandle, TypeReferenceHandle)>;

/// An index as a type reference writes it (`written_index`).
#[derive(Debug, Clone)]
enum WrittenIndex {
    Name(SymbolHandle),
    Literal(i128),
    Binary(IndexOp, Box<WrittenIndex>, Box<WrittenIndex>),
}

impl WrittenIndex {
    fn to_family_term(&self, named: &impl Fn(SymbolHandle) -> FamilyTerm) -> FamilyTerm {
        match self {
            Self::Name(symbol) => named(*symbol),
            Self::Literal(value) => FamilyTerm::Literal(*value),
            Self::Binary(op, left, right) => FamilyTerm::Binary(
                *op,
                Box::new(left.to_family_term(named)),
                Box::new(right.to_family_term(named)),
            ),
        }
    }

    fn every_name(&self, f: &mut impl FnMut(SymbolHandle) -> bool) -> bool {
        match self {
            Self::Name(symbol) => f(*symbol),
            Self::Literal(_) => true,
            Self::Binary(_, left, right) => left.every_name(f) && right.every_name(f),
        }
    }

    fn to_evidence_term<E>(
        &self,
        own: &[SymbolHandle],
        missing: impl Copy + Fn() -> E,
    ) -> Result<EvidenceTerm, E> {
        match self {
            Self::Literal(value) => Ok(EvidenceTerm::Literal(*value)),
            Self::Name(symbol) => own
                .iter()
                .position(|parameter| *parameter == *symbol)
                .map(|position| EvidenceTerm::Parameter(position as u32))
                .ok_or_else(missing),
            Self::Binary(op, left, right) => Ok(EvidenceTerm::Binary(
                *op,
                Box::new(left.to_evidence_term(own, missing)?),
                Box::new(right.to_evidence_term(own, missing)?),
            )),
        }
    }
}

/// What a call writes for one parameter of what it calls.
#[derive(Debug, Clone)]
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
                    .map_err(|reason| refuse_at(id, Unserved::Machine(reason)))?
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
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        let mut parameters = Vec::new();
        let mut plain = true;
        for parameter in typed.machine_type_parameters(machine) {
            match parameter.kind {
                TypeParameterKind::Value { type_reference } => {
                    match self.scalar_type(id, type_reference) {
                        Ok(ScalarType::Integer(integer)) => parameters.push(FamilyValue {
                            symbol: parameter.symbol,
                            name: parameter.name.as_str().to_owned(),
                            domain: FamilyDomain::Integer(integer),
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
        let mut holders: Holders = Vec::new();
        let mut constructions = Vec::new();
        let mut index_equalities = Vec::new();
        let mut found = false;
        for state in typed.machine_states(machine) {
            let formals = typed
                .state_parameters
                .span(state.parameters)
                .ok_or_else(|| unserved("machine lost its parameters"))?;
            for formal in formals.iter() {
                if self.holds_evidence(formal.type_reference) {
                    found = true;
                    self.complete_index_operators(id, machine.symbol, formal.type_reference)?;
                    holders.push((formal.symbol, formal.type_reference));
                    continue;
                }
                if !self.names_indexed_data(formal.type_reference) {
                    continue;
                }
                if formal.relevance != language_core::BindingRelevance::Erased {
                    return Err(unserved("indexed data in a formal that is not erased"));
                }
                found = true;
                self.complete_index_operators(id, machine.symbol, formal.type_reference)?;
                let applied = self.evidence_type(id, own, formal.type_reference)?;
                held.push((formal.symbol, applied));
            }
            for statement in typed.statement_table.statements(state.statement_nodes) {
                found |= self.evidence_statement(
                    id,
                    machine.symbol,
                    own,
                    statement,
                    &mut held,
                    &mut holders,
                    &mut constructions,
                    &mut index_equalities,
                )?;
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
                    scalar_type: ScalarType::Integer(
                        parameter
                            .integer()
                            .ok_or_else(|| unserved("evidence over a data index"))?,
                    ),
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
            index_equalities,
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
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        let value_type = self.value_type(id, reference)?;
        let family = self
            .family_of(value_type)
            .ok_or_else(|| unserved("evidence of data that is not indexed"))?
            .clone();
        let at = self
            .family_arguments(id, &family, &[], reference)?
            .into_iter()
            .map(|term| {
                term.try_to_evidence(&mut |leaf| match leaf {
                    FamilyTerm::Literal(value) => Ok(EvidenceTerm::Literal(*value)),
                    FamilyTerm::Name(symbol) => own
                        .iter()
                        .position(|parameter| *parameter == *symbol)
                        .map(|position| EvidenceTerm::Parameter(position as u32))
                        .ok_or_else(|| {
                            unserved(
                                "indexed data at an index that is neither a number nor one of the machine's value parameters",
                            )
                        }),
                    FamilyTerm::Index(_) | FamilyTerm::Payload(_) => Err(unserved(
                        "indexed data at one of its own indices outside its declaration",
                    )),
                    FamilyTerm::Binary(..) => unreachable!(),
                })
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
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
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

    /// Completes any builtin arithmetic operators inside `ConstExpression`
    /// index arguments of `reference` so open computed indices (`1 + Length`)
    /// do not leave unresolved `CheckedOperator` selections.
    pub(super) fn complete_index_operators(
        &mut self,
        id: MachineId,
        owner: SymbolHandle,
        reference: TypeReferenceHandle,
    ) -> Checked<()> {
        if !reference.is_valid() {
            return Ok(());
        }
        let mut operators = Vec::new();
        let mut types = vec![reference];
        while let Some(current) = types.pop() {
            if !self
                .typed
                .type_reference_table
                .contains_type_reference(current)
            {
                continue;
            }
            match self.typed.type_reference_table.type_reference(current) {
                TypeReferenceNode::Generic { arguments, .. } => {
                    types.extend_from_slice(
                        self.typed
                            .type_reference_table
                            .type_reference_handles(*arguments),
                    );
                }
                TypeReferenceNode::ConstExpression(root) => {
                    let mut expressions = vec![*root];
                    while let Some(expr) = expressions.pop() {
                        if !self.typed.expression_table.expression_is_valid(expr) {
                            continue;
                        }
                        if let ExpressionNode::Binary(binary) =
                            self.typed.expression_table.expression(expr)
                        {
                            operators.push(expr);
                            expressions.push(binary.left);
                            expressions.push(binary.right);
                        }
                    }
                }
                _ => {}
            }
        }
        if !operators.is_empty() {
            self.complete_anonymous_operators(id, owner, &operators)?;
        }
        Ok(())
    }

    /// Reads one statement of a running machine for evidence. Returns
    /// whether it held, built or passed any.
    #[allow(clippy::too_many_arguments)]
    fn evidence_statement(
        &mut self,
        id: MachineId,
        owner: SymbolHandle,
        own: &[SymbolHandle],
        statement: &StatementNode,
        held: &mut Held,
        holders: &mut Holders,
        constructions: &mut Vec<Construction>,
        index_equalities: &mut Vec<IndexEquality>,
    ) -> Checked<bool> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        if let StatementNode::LocalData(local) = statement
            && local.type_reference.is_valid()
            && self.names_indexed_data(local.type_reference)
        {
            if local.relevance != language_core::BindingRelevance::Erased {
                return Err(unserved("indexed data in a local that is not erased"));
            }
            self.complete_index_operators(id, owner, local.type_reference)?;
            let applied = self.evidence_type(id, own, local.type_reference)?;
            self.evidence_initializer(
                id,
                own,
                local.initial_value,
                &applied,
                held,
                holders,
                constructions,
                index_equalities,
            )?;
            held.push((local.symbol, applied));
            return Ok(true);
        }
        // A local that holds evidence in a field is read like any other
        // statement, its literal included, and then held.
        let holder = match statement {
            StatementNode::LocalData(local) if self.holds_evidence(local.type_reference) => {
                self.complete_index_operators(id, owner, local.type_reference)?;
                Some((local.symbol, local.type_reference))
            }
            _ => None,
        };
        // Evidence has no value: a local of another type cannot hold it.
        if let StatementNode::LocalData(local) = statement
            && local.initial_value.is_valid()
            && self
                .held_evidence(id, own, held, holders, local.initial_value)?
                .is_some()
        {
            return Err(unserved("evidence held in a local that takes none"));
        }
        // Every call the statement makes, at any depth, and every literal
        // of indexed data written anywhere but in an erased local above.
        let mut calls = Vec::new();
        let mut literals = Vec::new();
        let mut stray = false;
        match statement {
            StatementNode::Call(call) => calls.push((
                call.target_symbol,
                call.machine_arguments.to_vec(),
                typed
                    .statement_table
                    .expression_handles(call.arguments)
                    .to_vec(),
                Some(call.source_span),
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
                            Some(transition.source_span),
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
                |handle, node| match node {
                    ExpressionNode::Call(call) => calls.push((
                        call.target_symbol,
                        call.machine_arguments.to_vec(),
                        typed
                            .expression_table
                            .expression_handles(call.arguments)
                            .to_vec(),
                        Some(typed.expression_table.source_span(handle)),
                    )),
                    ExpressionNode::StructLiteral(literal) => {
                        let data = typed.unique_data_definition_by_symbol(literal.type_symbol);
                        stray |= data.is_some_and(|data| self.is_family(data));
                        if data.is_some_and(|data| {
                            !self.is_family(data) && self.evidence_fields(data).next().is_some()
                        }) {
                            literals.push(handle);
                        }
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
        let mut found = !literals.is_empty() || holder.is_some();
        for literal in literals {
            self.evidence_in_literal(id, own, literal, held, holders, index_equalities)?;
        }
        for (target, written, arguments, source) in calls {
            for argument in &written {
                self.complete_index_operators(id, owner, argument.type_reference)?;
            }
            found |= self.evidence_call(
                id,
                own,
                target,
                &written,
                &arguments,
                held,
                holders,
                source,
                index_equalities,
            )?;
        }
        holders.extend(holder);
        Ok(found)
    }

    /// What a call writes for each parameter of what it calls: for a value,
    /// a number, one of the calling machine's own value parameters, or an
    /// integer arithmetic expression built from them.
    /// `kinds` are the parameters called, each with whether it is a value.
    fn evidence_values(
        &self,
        id: MachineId,
        own: &[SymbolHandle],
        kinds: &[(SymbolHandle, bool)],
        written: &[crate::source_state::expression::StaticMachineArgument],
    ) -> Checked<Vec<Written>> {
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
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
            } else if argument.type_reference.is_valid() {
                self.written_index(id, &[], argument.type_reference)
                    .ok()
                    .and_then(|index| index.to_evidence_term(own, || ()).ok())
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
            .map(|term| {
                term.try_to_evidence(&mut |leaf| match leaf {
                    FamilyTerm::Literal(value) => Ok(EvidenceTerm::Literal(*value)),
                    FamilyTerm::Name(symbol) => {
                        let position = kinds
                            .iter()
                            .position(|(kind, _)| *kind == *symbol)
                            .ok_or(())?;
                        match written.get(position) {
                            Some(Written::Value(term)) => Ok(term.clone()),
                            _ => Err(()),
                        }
                    }
                    FamilyTerm::Index(_) | FamilyTerm::Payload(_) => Err(()),
                    FamilyTerm::Binary(..) => unreachable!(),
                })
                .ok()
            })
            .collect::<Option<_>>()?;
        Some(Applied {
            record: family.record,
            at,
        })
    }

    /// Checks that two applications of indexed data name the same family at
    /// equal indices. Plain mismatches are refused directly; mismatches
    /// involving a computed index become integer equalities for stage 06 to
    /// prove in the kernel.
    fn require_same_indices(
        id: MachineId,
        passed: &Applied,
        expected: &Applied,
        source: Option<source_text::SourceSpan>,
        mismatch: &'static str,
        index_equalities: &mut Vec<IndexEquality>,
    ) -> Checked<()> {
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        if passed.record != expected.record || passed.at.len() != expected.at.len() {
            return Err(unserved(mismatch));
        }
        for (left, right) in passed.at.iter().zip(&expected.at) {
            if left == right {
                continue;
            }
            if left.is_computed() || right.is_computed() {
                index_equalities.push(IndexEquality {
                    left: left.clone(),
                    right: right.clone(),
                    source,
                });
            } else {
                return Err(unserved(mismatch));
            }
        }
        Ok(())
    }

    /// Checks one call: each argument for a formal that takes indexed data
    /// is evidence the machine holds, at the index the formal takes.
    #[allow(clippy::too_many_arguments)]
    fn evidence_call(
        &mut self,
        id: MachineId,
        own: &[SymbolHandle],
        target: SymbolHandle,
        written: &[crate::source_state::expression::StaticMachineArgument],
        arguments: &[ExpressionHandle],
        held: &Held,
        holders: &Holders,
        source: Option<source_text::SourceSpan>,
        index_equalities: &mut Vec<IndexEquality>,
    ) -> Checked<bool> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
        // Which arguments name evidence the machine holds, in any form.
        let mut naming = Vec::with_capacity(arguments.len());
        for argument in arguments {
            naming.push(
                self.held_evidence(id, own, held, holders, *argument)?
                    .is_some(),
            );
        }
        let names_any = naming.contains(&true);
        let Some(callee) = self
            .entered_machine(target)
            .and_then(|machine| typed.machine_by_symbol(machine))
        else {
            // Not a machine: nothing here may take evidence.
            if names_any {
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
            if takes || names_any {
                return Err(unserved(
                    "call that takes evidence with another number of arguments",
                ));
            }
            return Ok(false);
        }
        if !takes {
            if names_any {
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
        for ((formal, &argument), &names) in formals.iter().zip(arguments).zip(&naming) {
            if !self.names_indexed_data(formal.type_reference) {
                if names {
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
            let passed = self
                .held_evidence(id, own, held, holders, argument)?
                .ok_or_else(|| unserved("evidence passed that is no evidence the machine holds"))?;
            Self::require_same_indices(
                id,
                &passed,
                &expected,
                source,
                "evidence passed at another index than the formal takes",
                index_equalities,
            )?;
        }
        Ok(true)
    }

    /// Checks what an erased local of indexed data is initialised with:
    /// evidence the machine already holds of the same thing, or a case
    /// literal, whose children must be evidence at the indices the case
    /// declares for them.
    #[allow(clippy::too_many_arguments)]
    fn evidence_initializer(
        &mut self,
        id: MachineId,
        own: &[SymbolHandle],
        initializer: ExpressionHandle,
        applied: &Applied,
        held: &Held,
        holders: &Holders,
        constructions: &mut Vec<Construction>,
        index_equalities: &mut Vec<IndexEquality>,
    ) -> Checked<()> {
        let typed = self.typed;
        let unserved = |what| refuse_at(id, Unserved::Machine(what));
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
        if let Some(known) = self.held_evidence(id, own, held, holders, initializer)? {
            Self::require_same_indices(
                id,
                &known,
                applied,
                source,
                "evidence held at another index than the local declares",
                index_equalities,
            )?;
            return Ok(());
        }
        let (case, fields) = match typed.expression_table.expression(initializer) {
            ExpressionNode::Name(path) => {
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
                let passed = self
                    .held_evidence(id, own, held, holders, *argument)?
                    .ok_or_else(|| {
                        unserved("evidence passed that is no evidence the machine holds")
                    })?;
                Self::require_same_indices(
                    id,
                    &passed,
                    &expected,
                    source,
                    "evidence handed on at another index than the local declares",
                    index_equalities,
                )?;
                Self::require_same_indices(
                    id,
                    &expected,
                    applied,
                    source,
                    "evidence handed on at another index than the local declares",
                    index_equalities,
                )?;
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
                .map(|term| {
                    term.try_to_evidence(&mut |leaf| match leaf {
                        FamilyTerm::Index(index) => at.get(*index as usize).cloned().ok_or(()),
                        FamilyTerm::Payload(position) => {
                            payload.get(*position as usize).cloned().ok_or(())
                        }
                        FamilyTerm::Literal(value) => Ok(EvidenceTerm::Literal(*value)),
                        FamilyTerm::Name(_) => Err(()),
                        FamilyTerm::Binary(..) => unreachable!(),
                    })
                    .ok()
                })
                .collect::<Option<_>>()
                .ok_or_else(|| {
                    unserved("child of indexed data at an index the literal does not fix")
                })?;
            let expected = Applied {
                record,
                at: expected,
            };
            let passed = self
                .held_evidence(id, own, held, holders, expression)?
                .ok_or_else(|| {
                    unserved("child of a literal that is no evidence the machine holds")
                })?;
            Self::require_same_indices(
                id,
                &passed,
                &expected,
                source,
                "child of a literal at another index than the case declares for it",
                index_equalities,
            )?;
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
