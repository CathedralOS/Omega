//! Logical hypotheses retain their complete implication and exact subjects.
//! Applying one changes proof knowledge only after its premises have been met.

use crate::calls::callable_refinement::conjunction;
use crate::substitution::clauses::{ClauseBindings, substitute_clause};
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, ContentPlaceVersion, MachineId, PlaceId, Proposition,
    PropositionContext, ScalarTerm, ScalarType, StructuralCaseId, StructuralTypeId, ValueId,
};
use std::borrow::Cow;
use terminal_psi::{
    ProofFormalSource, TerminalMachine, TerminalModule, Terminator, TheoremApplication,
    TheoremContract, TheoremParameter, TheoremSource,
};

pub mod data_subjects;

/// The declaration of each sum a contract's sum subjects name, such as
/// `module_sums` gives from a module's structural types.
pub type SumShapes<'a> =
    dyn Fn(StructuralTypeId) -> Result<proof_admission::DataSubjectShape, &'static str> + 'a;

/// The sums of a module's structural types (`data_subjects::sum_shape`).
pub fn module_sums(
    module: &TerminalModule,
) -> impl Fn(StructuralTypeId) -> Result<proof_admission::DataSubjectShape, &'static str> + '_ {
    move |sum| data_subjects::sum_shape(module, sum)
}

/// For a producer that has no sum declarations to give.
pub fn no_sums(_: StructuralTypeId) -> Result<proof_admission::DataSubjectShape, &'static str> {
    Err("logical contract's sum subject has no declaration here")
}

pub fn contract_type(
    contract: &TheoremContract,
    sums: &SumShapes<'_>,
    budget: &mut proof_admission::Budget,
) -> Result<proof_admission::BoundedImplicationType, &'static str> {
    let subjects = contract_subjects(contract)?;
    let data = contract
        .data_subjects
        .iter()
        .map(|subject| {
            Ok(proof_admission::DataSubject {
                place: subject.place,
                shape: sums(subject.structural_type)?,
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let hypotheses = hypothesis_types(contract, sums, budget)?;
    proof_admission::denote_bounded_implication_type(
        &subjects,
        &data,
        &hypotheses,
        &contract.requires,
        &conjunction(contract.ensures.clone()),
        budget,
    )
    .map_err(|_| "logical contract has no well-formed mathematical implication")
}

/// The closed implication of each of the contract's own proof parameters.
fn hypothesis_types(
    contract: &TheoremContract,
    sums: &SumShapes<'_>,
    budget: &mut proof_admission::Budget,
) -> Result<Vec<proof_admission::BoundedImplicationType>, &'static str> {
    contract
        .proof_parameters
        .iter()
        .map(|nested| contract_type(nested, sums, budget))
        .collect()
}

/// Check the stored contract's subject and clause vocabulary without proving a judgment.
/// Its own proof parameters are checked the same way, and take none of their own.
///
/// A sum subject's clauses may test its case, read a payload field of a case
/// (`StructuralRead` under `Case(c), Field(f)`), and state an Identity of two
/// whole sum subjects. Whether each names its declaration's cases, fields and
/// carriers is decided as `contract_type` denotes it.
pub fn contract_subjects(
    contract: &TheoremContract,
) -> Result<Vec<(ValueId, ScalarType)>, &'static str> {
    let mut subjects = Vec::with_capacity(contract.parameters.len());
    for (position, formal) in contract.parameters.iter().enumerate() {
        if formal.id.get() != position as u64 + 1
            || formal.qualifications != semantic_vocabulary::ScalarQualificationSetId::ZERO
        {
            return Err("logical contract has another fixed subject telescope");
        }
        if !matches!(
            formal.scalar_type,
            ScalarType::Boolean | ScalarType::IeeeFloat(_)
        ) && !matches!(formal.scalar_type, ScalarType::Integer(integer) if integer.bits().is_some())
        {
            return Err("logical contract needs a closed-width integer, float or Boolean subject");
        }
        subjects.push((formal.id, formal.scalar_type));
    }
    let mut places = Vec::with_capacity(contract.data_subjects.len());
    for (position, subject) in (0..).zip(&contract.data_subjects) {
        if subject.place.get() != u64::from(position) + 1 {
            return Err("logical contract has another sum subject telescope");
        }
        places.push((
            subject.place,
            semantic_vocabulary::StructuralPlaceKind::Parameter { position },
        ));
    }
    let is_sum = |place: PlaceId| contract.data_subjects.iter().any(|s| s.place == place);
    let context = PropositionContext::from_value_types_and_places(subjects.iter().copied(), places)
        .map_err(|_| "logical subjects do not form a proposition context")?;
    let mut pending: Vec<_> = contract.requires.iter().chain(&contract.ensures).collect();
    while let Some(clause) = pending.pop() {
        context
            .validate(clause)
            .map_err(|_| "logical clause names another subject or carrier")?;
        match clause {
            Proposition::Truth
            | Proposition::Falsehood
            | Proposition::Equal(..)
            | Proposition::LessThan(..)
            | Proposition::LessOrEqual(..)
            | Proposition::IntegerMathEqual(..)
            | Proposition::IntegerMathLessThan(..)
            | Proposition::IntegerMathLessOrEqual(..)
            | Proposition::ScalarIeeeFloatComparison { .. } => {}
            Proposition::StructuralCaseMembership { subject, .. }
                if is_sum(subject.root()) && subject.path().is_empty() => {}
            Proposition::DeclarationIdentity(identity)
                if identity.values.len() == 2
                    && identity.values.iter().all(|value| {
                        matches!(
                            (&value.carrier, &value.operand),
                            (
                                semantic_vocabulary::ClauseCarrier::Structural(_),
                                semantic_vocabulary::ClauseOperand::Input(input),
                            ) if is_sum(input.root()) && input.path().is_empty()
                        )
                    }) => {}
            Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
                pending.extend(parts)
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                pending.push(premise);
                pending.push(conclusion);
            }
            _ => return Err("logical clause has no complete fixed mathematical denotation"),
        }
    }
    for nested in &contract.proof_parameters {
        if !nested.proof_parameters.is_empty() {
            return Err("a logical contract's proof parameter takes proof parameters of its own");
        }
        contract_subjects(nested)?;
    }
    Ok(subjects)
}

/// The contracts of a machine's own logical proof formals, in header order.
pub fn proof_formal_contracts(
    module: &TerminalModule,
    machine: MachineId,
) -> Result<Vec<&TheoremContract>, &'static str> {
    let owner = module
        .machine(machine)
        .ok_or("logical parameter has no owning machine")?;
    (0..owner.contract.erased_proof_formals.len())
        .map(|position| {
            let position =
                u32::try_from(position).map_err(|_| "logical parameter position overflow")?;
            Ok(&parameter(module, machine, position)?.contract)
        })
        .collect()
}

pub fn parameter(
    module: &TerminalModule,
    owner: MachineId,
    position: u32,
) -> Result<&TheoremParameter, &'static str> {
    let mut matches = module
        .proofs
        .theorem_parameters
        .iter()
        .filter(|row| row.machine == owner && row.proof_position == position);
    let parameter = matches
        .next()
        .ok_or("logical application has no declared proof parameter")?;
    if matches.next().is_some() {
        return Err("logical parameter is declared more than once");
    }
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == owner)
        .ok_or("logical parameter has no owning machine")?;
    let formal = machine
        .contract
        .erased_proof_formals
        .get(position as usize)
        .ok_or("logical parameter has no erased formal in its complete header")?;
    if !matches!(formal.source, ProofFormalSource::GenericParameter(_)) {
        return Err("logical parameter names another source coordinate kind");
    }
    Ok(parameter)
}

/// Whether every path through the body returns without running anything:
/// no block has operations, each jump goes forward from the entry, and no
/// cleanup invokes code. A jump may carry a fact-only application of a
/// proof formal. Only then do the machine's `ensures` hold wherever its
/// `requires` do; a body that runs code establishes them only once it
/// returns, and may not return at all.
fn returns_without_running(machine: &terminal_psi::TerminalMachine) -> bool {
    use terminal_psi::TerminalAffineCleanupAction as Cleanup;
    let passive = |actions: &[Cleanup]| {
        actions
            .iter()
            .all(|action| !matches!(action, Cleanup::InvokeOwned(_)))
    };
    let position = |id| machine.blocks.iter().position(|block| block.id == id);
    machine
        .blocks
        .first()
        .is_some_and(|block| block.id == machine.entry)
        && machine.blocks.iter().enumerate().all(|(at, block)| {
            block.operations.is_empty()
                && match &block.terminator {
                    Terminator::Jump {
                        target,
                        cleanup_actions,
                        ..
                    } => {
                        position(*target).is_some_and(|target| target > at)
                            && passive(cleanup_actions)
                    }
                    Terminator::ReturnUnit {
                        cleanup_actions, ..
                    } => passive(cleanup_actions),
                    _ => false,
                }
        })
}

/// One block with no operations that returns: the body does nothing and
/// terminates, whatever termination the contract publishes.
fn does_no_work(machine: &terminal_psi::TerminalMachine) -> bool {
    matches!(machine.blocks.as_slice(), [block]
        if block.id == machine.entry
            && block.operations.is_empty()
            && matches!(block.terminator, Terminator::ReturnUnit { ref cleanup_actions, .. }
                if cleanup_actions.is_empty()))
}

/// The implication a named machine states, when the machine does no work:
/// a citation of it is a fact-only invocation, which emits no call
/// (contracts.md, "Citation and induction": "erase the call if fact-only").
/// Its subjects are its scalar formals and then its erased scalar formals,
/// numbered from one, or, when it has a record formal, its formals in header
/// order with each record's scalar fields in its place (`data_subjects`).
/// Its implication is its `requires` and `ensures`.
/// The named machine is checked as any machine is, so its `ensures` hold
/// wherever its `requires` do.
pub fn named_contract(
    module: &TerminalModule,
    owner: MachineId,
    named: MachineId,
) -> Result<TheoremContract, &'static str> {
    if named == owner {
        return Err("named theorem application cites its own machine");
    }
    let machine = module
        .machine(named)
        .ok_or("named theorem application has no machine")?;
    let contract = &machine.contract;
    let refusal = if !matches!(contract.result, terminal_psi::TerminalMachineResult::Unit) {
        Some("named theorem application names a machine with a result")
    } else if !contract.structural_parameters.is_empty()
        && !contract.erased_scalar_formals.is_empty()
    {
        Some("named theorem application names a machine with a record formal and an erased formal")
    } else if !contract.erased_proof_formals.is_empty() {
        Some("named theorem application names a machine with a proof formal")
    } else if !contract.crash_routes.is_empty() {
        Some("named theorem application names a machine that may crash")
    } else if !contract.published_service_ceiling.is_empty()
        || !contract.published_synchronous_entries.is_empty()
        || contract.published_may_suspend
        || contract.published_may_block
    {
        Some("named theorem application names a machine with reach, suspension or blocking")
    } else if !contract.outcome_specific_ensures.is_empty()
        || !contract.membership_ensures.is_empty()
    {
        Some("named theorem application names a machine with a guarantee about its outcome")
    } else if !does_no_work(machine)
        && (contract.structural_parameters.is_empty() || !returns_without_running(machine))
    {
        // A record formal's root is discarded as the body returns, which
        // runs nothing.
        Some("named theorem application names a machine whose body does work")
    } else {
        None
    };
    if let Some(refusal) = refusal {
        return Err(refusal);
    }
    let ensures: Vec<_> = contract
        .ensures
        .iter()
        .map(|clause| clause.proposition.clone())
        .collect();
    let named = if contract.structural_parameters.is_empty() {
        TheoremContract {
            parameters: contract
                .parameters
                .iter()
                .chain(&contract.erased_scalar_formals)
                .cloned()
                .collect(),
            requires: contract.requires.clone(),
            data_subjects: Vec::new(),
            ensures,
            proof_parameters: Vec::new(),
        }
    } else {
        // A record formal is one subject for each of its scalar fields, as
        // in a citation (`data_subjects`).
        let subjects = data_subjects::machine_subjects(module, contract)?;
        let (parameters, requires, ensures) =
            data_subjects::flatten(&subjects, &contract.requires, &ensures)?;
        TheoremContract {
            parameters,
            requires,
            data_subjects: Vec::new(),
            ensures,
            proof_parameters: Vec::new(),
        }
    };
    contract_subjects(&named)?;
    Ok(named)
}

/// The implication an application applies: the declared proof formal's
/// contract, or the named machine's (`named_contract`).
pub fn applied<'a>(
    module: &'a TerminalModule,
    owner: MachineId,
    source: TheoremSource,
) -> Result<Cow<'a, TheoremContract>, &'static str> {
    match source {
        TheoremSource::Formal(position) => {
            Ok(Cow::Borrowed(&parameter(module, owner, position)?.contract))
        }
        TheoremSource::Named(named) => Ok(Cow::Owned(named_contract(module, owner, named)?)),
    }
}

/// The successor cannot obtain a conclusion by arriving through another edge.
/// The ordinary edge still carries every ownership and cleanup obligation.
pub fn application<'a>(
    module: &'a TerminalModule,
    machine: &TerminalMachine,
    application: &TheoremApplication,
) -> Result<Cow<'a, TheoremContract>, &'static str> {
    if application.machine != machine.id {
        return Err("logical use belongs to another machine");
    }
    let formal = applied(module, machine.id, application.source)?;
    if application.arguments.len() != formal.parameters.len()
        || application.requirements.len() != formal.requires.len()
    {
        return Err("logical use lost a subject or premise");
    }
    if application.instantiation.len() != formal.proof_parameters.len() {
        return Err("logical use needs one proof for each of its contract's own proof parameters");
    }
    for (argument, formal) in application.arguments.iter().zip(&formal.parameters) {
        argument
            .validate_after_operands()
            .map_err(|_| "logical subject has a malformed scalar term")?;
        // A field of a record input is current contents, read where the
        // application's edge leaves; the checker names its root.
        if argument.scalar_type() != formal.scalar_type
            || !matches!(
                argument,
                ScalarTerm::Value { .. }
                    | ScalarTerm::Integer { .. }
                    | ScalarTerm::Boolean(_)
                    | ScalarTerm::IntegerField {
                        version: ContentPlaceVersion::Current,
                        ..
                    }
                    | ScalarTerm::BooleanField {
                        version: ContentPlaceVersion::Current,
                        ..
                    }
                    | ScalarTerm::IeeeFloatField {
                        version: ContentPlaceVersion::Current,
                        ..
                    }
            )
        {
            return Err("logical subject is not a fixed scalar of its declared carrier");
        }
    }
    let mut incoming = machine
        .blocks
        .iter()
        .flat_map(|block| block.terminator.successor_edges())
        .filter(|(_, target)| *target == application.successor);
    if incoming.next() != Some((application.edge, application.successor))
        || incoming.next().is_some()
        || application.successor == machine.entry
    {
        return Err("logical successor has another incoming path");
    }
    let mut source = machine.blocks.iter().filter(|block| {
        matches!(&block.terminator, Terminator::Jump {
            edge, target, arguments, erased_arguments, erased_proof_arguments, structural_arguments, ..
        } if *edge == application.edge && *target == application.successor
            && arguments.is_empty() && erased_arguments.is_empty() && erased_proof_arguments.is_empty()
            && structural_arguments.is_empty())
    });
    if source.next().is_none() || source.next().is_some() {
        return Err("logical application has no unique ordinary continuation edge");
    }
    Ok(formal)
}

struct Subjects<'a> {
    contract: &'a TheoremContract,
    actuals: &'a [ScalarTerm],
}
impl ClauseBindings for Subjects<'_> {
    type Error = &'static str;
    fn value(&mut self, id: ValueId, scalar: ScalarType) -> Result<ScalarTerm, Self::Error> {
        let position =
            usize::try_from(id.get() - 1).map_err(|_| "logical subject position overflow")?;
        let formal = self
            .contract
            .parameters
            .get(position)
            .filter(|formal| formal.id == id && formal.scalar_type == scalar)
            .ok_or("logical clause names another formal")?;
        self.actuals
            .get(position)
            .filter(|actual| actual.scalar_type() == formal.scalar_type)
            .cloned()
            .ok_or("logical subject has another carrier")
    }
    fn field(
        &mut self,
        _: ContentPlaceVersion,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: ScalarType,
    ) -> Result<ScalarTerm, Self::Error> {
        Err("logical application cannot observe live storage")
    }
    fn case(
        &mut self,
        _: PlaceId,
        _: &[CanonicalStructuralPathSegment],
        _: StructuralCaseId,
    ) -> Result<crate::substitution::clauses::CaseAnswer, Self::Error> {
        Err("logical application cannot observe a live case")
    }
}

pub fn substitute(
    contract: &TheoremContract,
    arguments: &[ScalarTerm],
    clause: &Proposition,
) -> Result<Proposition, &'static str> {
    if arguments.len() != contract.parameters.len() {
        return Err("logical substitution has another complete telescope");
    }
    substitute_clause(
        clause,
        &mut Subjects {
            contract,
            actuals: arguments,
        },
    )
}

/// A machine may satisfy its runtime guarantee by never returning. A logical
/// citation must prove the implication over its admitted inputs, then check
/// that its premises and guarantee satisfy the alias's complete contract.
///
/// The cited machine's own proof formals, `formals`, are each given a proof
/// in one of two ways. A citation may instantiate them, as `lifted::<inner>`
/// does: `instantiated` counts those supplies, which are checked separately
/// against the machine's aliases. Or, when the expected contract declares
/// proof parameters of its own, the machine's formals are exactly those, in
/// order, and receive whatever proofs the contract's user later gives it.
/// The three goals here use neither: the cited implication is proved from
/// its contract alone.
pub fn citation_goals(
    module: &TerminalModule,
    expected: &TheoremContract,
    supplied: &terminal_psi::TerminalMachine,
    formals: &[&TheoremContract],
    instantiated: usize,
) -> Result<[Proposition; 3], &'static str> {
    citation_goals_with(
        module,
        expected,
        supplied,
        formals,
        instantiated,
        |contract| {
            contract_type(
                contract,
                &module_sums(module),
                &mut proof_admission::Budget::default(),
            )
            .map(drop)
        },
    )
}

/// Recover the root judgments and their subjects from the recorded declarations.
/// Canonical inspection checks their coordinates without rechecking their proofs.
pub fn recorded_citation_goals(
    module: &TerminalModule,
    expected: &TheoremContract,
    supplied: &terminal_psi::TerminalMachine,
    formals: &[&TheoremContract],
    instantiated: usize,
) -> Result<[Proposition; 3], &'static str> {
    citation_goals_with(
        module,
        expected,
        supplied,
        formals,
        instantiated,
        |contract| contract_subjects(contract).map(drop),
    )
}

fn citation_goals_with(
    module: &TerminalModule,
    expected: &TheoremContract,
    supplied: &terminal_psi::TerminalMachine,
    formals: &[&TheoremContract],
    instantiated: usize,
    check: impl Fn(&TheoremContract) -> Result<(), &'static str>,
) -> Result<[Proposition; 3], &'static str> {
    // A citation runs nothing, so it may use the cited guarantee only where
    // that guarantee holds without the body returning
    // (OWNER_QUESTIONS.md#bundle-evidence-field-type).
    if !returns_without_running(supplied) {
        return Err("cited theorem's body does work, so its guarantee holds only after it returns");
    }
    let supplied = &supplied.contract;
    if supplied.erased_proof_formals.len() != formals.len()
        || supplied
            .erased_proof_formals
            .iter()
            .any(|formal| !matches!(formal.source, ProofFormalSource::GenericParameter(_)))
    {
        return Err("cited theorem has another roster of logical proof formals");
    }
    if expected.proof_parameters.is_empty() {
        if formals.len() != instantiated {
            return Err("cited theorem needs one logical supply for each of its own proof formals");
        }
    } else if instantiated != 0 || !formals.iter().copied().eq(expected.proof_parameters.iter()) {
        return Err(
            "cited theorem's own proof formals are not exactly the proof parameters of the aliased contract",
        );
    }
    if !matches!(supplied.result, terminal_psi::TerminalMachineResult::Unit)
        || !supplied.erased_scalar_formals.is_empty()
        || !supplied.entry_claims.is_empty()
        || !supplied.content_entry_claims.is_empty()
        || !supplied.membership_ensures.is_empty()
        || !supplied.outcome_specific_ensures.is_empty()
        || !supplied.published_service_ceiling.is_empty()
        || !supplied.published_synchronous_entries.is_empty()
        || supplied.published_may_suspend
        || supplied.published_may_block
        || !supplied.crash_routes.is_empty()
        || supplied
            .result_lifetimes
            .as_ref()
            .is_some_and(|rows| !rows.is_empty())
    {
        return Err("cited theorem needs its complete fixed mathematical contract");
    }
    // A record input is one subject for each of its scalar fields
    // (`data_subjects`), so its clauses are restated over those subjects.
    let subjects = data_subjects::machine_subjects(module, supplied)?;
    let ensures: Vec<_> = supplied
        .ensures
        .iter()
        .map(|clause| clause.proposition.clone())
        .collect();
    let (parameters, requires, ensures) =
        data_subjects::flatten(&subjects, &supplied.requires, &ensures)?;
    if parameters != expected.parameters {
        return Err(
            "cited theorem's subjects are not the aliased contract's subjects and carriers",
        );
    }
    let actual = TheoremContract {
        parameters,
        data_subjects: Vec::new(),
        requires,
        ensures,
        proof_parameters: Vec::new(),
    };
    check(expected)?;
    check(&actual)?;
    let aliases = conjunction(expected.requires.clone());
    let requires = conjunction(actual.requires);
    let ensures = conjunction(actual.ensures);
    let expected = conjunction(expected.ensures.clone());
    let implication = |premise, conclusion| Proposition::Implication {
        premise: Box::new(premise),
        conclusion: Box::new(conclusion),
    };
    Ok([
        implication(requires.clone(), ensures.clone()),
        implication(aliases.clone(), requires),
        implication(aliases, implication(ensures, expected)),
    ])
}

// A premise can itself be a conjunction. Prefer its exact assumption before
// grouping separate header clauses; the full input context stays unchanged.
fn joined_premises_proof(
    premises: &[Proposition],
) -> Result<terminal_psi::ProofNode, &'static str> {
    use terminal_psi::{PrimitiveJudgment, ProofNode, ProofRule};
    let premise_proof = |premise: &Proposition| {
        premises
            .iter()
            .position(|original| original == premise)
            .map(|index| ProofNode {
                conclusion: premise.clone(),
                rule: ProofRule::Assumption { index },
            })
            .ok_or("logical conjunction lost its exact premise")
    };
    let conclusion = conjunction(premises.iter().cloned());
    if premises.is_empty() {
        Ok(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        })
    } else if let Ok(proof) = premise_proof(&conclusion) {
        Ok(proof)
    } else if let Proposition::Conjunction(parts) = conclusion {
        Ok(ProofNode {
            rule: ProofRule::ConjunctionIntroduction(
                parts
                    .iter()
                    .map(premise_proof)
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            conclusion: Proposition::Conjunction(parts),
        })
    } else {
        Err("logical conjunction lost its complete premise roster")
    }
}

// A closed derivation's local assumptions follow the new ambient hypotheses.
// Every resulting composition is checked again; this operation grants no fact.
fn weaken_closed_proof(
    mut proof: terminal_psi::ProofNode,
    ambient: usize,
) -> Result<terminal_psi::ProofNode, &'static str> {
    use terminal_psi::ProofRule as R;
    let mut pending = vec![&mut proof];
    while let Some(node) = pending.pop() {
        match &mut node.rule {
            R::Assumption { index } => {
                *index = index
                    .checked_add(ambient)
                    .ok_or("logical proof context overflow")?
            }
            R::ConjunctionIntroduction(parts)
            | R::IeeeFloatOrderChain(parts)
            | R::FloatMeaningNormalization(parts) => pending.extend(parts),
            R::ConjunctionElimination { conjunction, .. } => {
                pending.push(std::sync::Arc::make_mut(conjunction))
            }
            R::DisjunctionElimination {
                disjunction,
                branches,
            } => {
                pending.push(disjunction);
                pending.extend(branches);
            }
            R::ImplicationElimination {
                implication,
                premise,
            } => {
                pending.push(implication);
                pending.push(premise);
            }
            R::DisjunctionIntroduction {
                disjunct: premise, ..
            }
            | R::ImplicationIntroduction { body: premise }
            | R::PredicateDenotation { premise }
            | R::EqualitySymmetry { equality: premise }
            | R::IntegerOrderWeakening { relation: premise }
            | R::IntegerOrderDiscreteness { relation: premise }
            | R::IntegerMathOrderTranslation {
                relation: premise, ..
            }
            | R::IntegerAffineBound {
                root_bound: premise,
                ..
            }
            | R::IntegerCastBound {
                root_bound: premise,
                ..
            }
            | R::IntegerPowerOfTwoOrder { order: premise } => pending.push(premise),
            R::FloatIntegerConversion { premise, computed } => {
                pending.push(premise);
                pending.extend(computed.iter_mut().map(|proof| proof.as_mut()));
            }
            R::ValueEqualityTransport {
                premise,
                equalities,
            } => {
                pending.push(premise);
                pending.extend(equalities);
            }
            R::EqualityTransitivity {
                left_equals_middle: first,
                middle_equals_right: second,
            }
            | R::IntegerOrderAntisymmetry {
                forward: first,
                backward: second,
            }
            | R::IntegerSubtractOrder {
                difference: first,
                positive: second,
            }
            | R::IntegerAddOrder {
                sum: first,
                positive: second,
            }
            | R::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: first,
                middle_less_or_equal_right: second,
            }
            | R::IntegerStrictOrderTransitivity {
                left_to_middle: first,
                middle_to_right: second,
            }
            | R::IntegerOrderSubstitution {
                relation: first,
                equality: second,
                ..
            }
            | R::IntegerExactAddDefinitionBound {
                left_bound: first,
                right_bound: second,
                ..
            }
            | R::IntegerExactSubtractDefinitionBound {
                left_bound: first,
                right_bound: second,
                ..
            } => {
                pending.push(first);
                pending.push(second);
            }
            R::IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => {
                pending.push(smaller);
                pending.push(larger);
                pending.push(order);
            }
            R::IntegerFloatExact
            | R::Primitive(_)
            | R::SemanticAxiom { .. }
            | R::IntegerCorrelatedForbiddenRoots { .. } => {}
        }
    }
    Ok(proof)
}

/// Replay all three supplied derivations before forming the complete Strict
/// implication. Named selection identity remains derivation provenance only.
pub fn denote_citation(
    module: &TerminalModule,
    expected: &TheoremContract,
    supplied: &terminal_psi::TerminalMachine,
    formals: &[&TheoremContract],
    instantiated: usize,
    derivations: &[terminal_psi::CertificateEnvelope],
    budget: &mut proof_admission::Budget,
) -> Result<proof_admission::BoundedTheorem, &'static str> {
    use terminal_psi::{ProofNode, ProofRule};
    if !expected.data_subjects.is_empty() {
        return Err("theorem citation over a sum subject has no case analysis yet");
    }
    let goals = citation_goals(module, expected, supplied, formals, instantiated)?;
    let subjects: Vec<_> = expected
        .parameters
        .iter()
        .map(|formal| (formal.id, formal.scalar_type))
        .collect();
    let context = PropositionContext::from_value_types(subjects.iter().copied())
        .map_err(|_| "logical subjects are invalid")?;
    if derivations.len() != goals.len() {
        return Err("theorem citation lost a complete derivation");
    }
    for (position, (derivation, goal)) in derivations.iter().zip(&goals).enumerate() {
        if derivation.identity.get() != position as u64 + 1
            || derivation.proof_system_marker != terminal_psi::ProofSystemMarker::CURRENT
        {
            return Err("theorem derivation has another role or proof system");
        }
        proof_admission::accept_certificate(&context, goal, &[], &[], &derivation.proof)
            .map_err(|_| "cited theorem implication was refused by the proof kernel")?;
    }
    let [actual, entry, refinement]: [ProofNode; 3] = derivations
        .iter()
        .map(|derivation| weaken_closed_proof(derivation.proof.clone(), expected.requires.len()))
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "theorem citation has another derivation roster")?;
    let aliases = joined_premises_proof(&expected.requires)?;
    let apply = |function: ProofNode, argument: ProofNode| -> Result<ProofNode, &'static str> {
        let Proposition::Implication {
            premise,
            conclusion,
        } = &function.conclusion
        else {
            return Err("cited theorem proof has no implication type");
        };
        if **premise != argument.conclusion {
            return Err("cited theorem proof has another supplied premise");
        }
        Ok(ProofNode {
            conclusion: (**conclusion).clone(),
            rule: ProofRule::ImplicationElimination {
                implication: Box::new(function),
                premise: Box::new(argument),
            },
        })
    };
    let requires = apply(entry, aliases.clone())?;
    let ensures = apply(actual, requires)?;
    let refinement = apply(refinement, aliases)?;
    let proof = apply(refinement, ensures)?;
    let hypotheses = hypothesis_types(expected, &module_sums(module), budget)?;
    proof_admission::verify_bounded_theorem(
        &subjects,
        &hypotheses,
        &conjunction(expected.ensures.clone()),
        &expected.requires,
        &[],
        &proof,
        budget,
    )
    .map_err(|_| "complete theorem implication was refused by the mathematical kernel")
}

#[cfg(test)]
mod premise_tests {
    use super::*;

    fn contract(data_subjects: &[u64]) -> TheoremContract {
        let n = ValueId::new(1).unwrap();
        let u8_type = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
        );
        TheoremContract {
            parameters: vec![terminal_psi::ValueDeclaration {
                id: n,
                scalar_type: u8_type,
                qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
            }],
            data_subjects: data_subjects
                .iter()
                .map(|&place| terminal_psi::TheoremDataSubject {
                    place: PlaceId::new(place).unwrap(),
                    structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
                })
                .collect(),
            requires: vec![Proposition::Truth],
            ensures: vec![Proposition::Equal(
                ScalarTerm::Value {
                    id: n,
                    scalar_type: u8_type,
                },
                ScalarTerm::Value {
                    id: n,
                    scalar_type: u8_type,
                },
            )],
            proof_parameters: Vec::new(),
        }
    }

    #[test]
    fn a_contract_without_sum_subjects_keeps_its_scalar_subjects() {
        assert_eq!(contract_subjects(&contract(&[])).unwrap().len(), 1);
    }

    #[test]
    fn a_sum_subject_is_a_subject_whose_type_needs_its_declaration() {
        let with = contract(&[1]);
        assert_eq!(contract_subjects(&with).unwrap().len(), 1);
        assert_eq!(
            contract_type(&with, &no_sums, &mut proof_admission::Budget::default()).err(),
            Some("logical contract's sum subject has no declaration here")
        );
    }

    fn choice(sum: StructuralTypeId) -> Result<proof_admission::DataSubjectShape, &'static str> {
        let u8_type = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
                .unwrap(),
        );
        Ok(proof_admission::DataSubjectShape {
            structural_type: sum,
            cases: [1, 2]
                .into_iter()
                .map(|raw| {
                    (
                        StructuralCaseId::new(raw).unwrap(),
                        vec![(
                            semantic_vocabulary::StructuralFieldId::new(raw).unwrap(),
                            proof_admission::DataFieldShape::Scalar(u8_type),
                        )],
                    )
                })
                .collect(),
        })
    }

    fn case_test(place: u64) -> Proposition {
        Proposition::StructuralCaseMembership {
            subject: semantic_vocabulary::StructuralCaseSubject::new(
                PlaceId::new(place).unwrap(),
                Vec::new(),
            ),
            case: StructuralCaseId::new(1).unwrap(),
        }
    }

    #[test]
    fn a_case_test_of_a_sum_subject_has_a_type_over_its_declaration() {
        let mut tested = contract(&[1]);
        tested.requires = vec![case_test(1)];
        contract_type(&tested, &choice, &mut proof_admission::Budget::default()).unwrap();
    }

    #[test]
    fn a_case_test_names_a_sum_subject() {
        let mut tested = contract(&[1]);
        tested.requires = vec![case_test(2)];
        assert_eq!(
            contract_subjects(&tested).err(),
            Some("logical clause names another subject or carrier")
        );
    }

    #[test]
    fn sum_subjects_number_their_places_from_one() {
        for places in [&[2][..], &[1, 3], &[1, 1]] {
            assert_eq!(
                contract_subjects(&contract(places)),
                Err("logical contract has another sum subject telescope"),
                "{places:?}"
            );
        }
    }

    #[test]
    fn header_premises_keep_their_exact_assumption_indices() {
        let nested = conjunction([Proposition::Truth, Proposition::Falsehood]);
        for premises in [
            Vec::new(),
            vec![Proposition::Truth],
            vec![nested],
            vec![Proposition::Falsehood, Proposition::Truth],
        ] {
            let proof = joined_premises_proof(&premises).unwrap();
            let context = PropositionContext::from_value_types([]).unwrap();
            proof_admission::accept_certificate(
                &context,
                &proof.conclusion,
                &premises,
                &[],
                &proof,
            )
            .unwrap_or_else(|error| panic!("{premises:?}: {error:?}"));
        }
    }

    #[test]
    fn grouping_premises_cannot_invent_an_assumption() {
        let premises = [Proposition::Falsehood, Proposition::Truth];
        let mut proof = joined_premises_proof(&premises).unwrap();
        let terminal_psi::ProofRule::ConjunctionIntroduction(children) = &mut proof.rule else {
            panic!("the two header clauses form a conjunction");
        };
        children[0].rule = terminal_psi::ProofRule::Assumption {
            index: premises.len(),
        };
        let context = PropositionContext::from_value_types([]).unwrap();
        assert!(
            proof_admission::accept_certificate(
                &context,
                &proof.conclusion,
                &premises,
                &[],
                &proof,
            )
            .is_err()
        );
    }
}
