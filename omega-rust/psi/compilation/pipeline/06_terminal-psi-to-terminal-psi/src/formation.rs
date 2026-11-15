//! A condition cannot justify arithmetic needed to state that condition.
//! Form declarations before their clauses enter proof knowledge. These local
//! logical observations have no runtime representation or transferable receipt;
//! every partial computation uses the ordinary scalar goal and proof kernel.

use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, ContentPlaceVersion as Version, MachineId,
    ObligationId, PlaceId, Proposition, PropositionContext, ScalarTerm, ScalarType,
    StructuralCaseId, StructuralTypeId, ValueId,
};
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::{MachineContract, StructuralAccess, TerminalModule, ValueDeclaration};
use terminal_semantics::CanonicalScalarGoal;
use terminal_semantics::substitution::clauses::{ClauseBindings, substitute_clause};

/// An explicit formation entrance may search; replay itself must not. The
/// exclusive borrow prevents callers from editing declarations while this
/// scope is live. Replay mutates checking scratch only. Normal inference
/// uses the two private publishers that form a proposed contract before
/// changing it; no admission token is persisted.
pub struct ReplayScope<'a> {
    elaboration: &'a mut terminal_check_tables::program::Elaboration,
}

pub fn prepare_replay<'source>(
    elaboration: &'source mut terminal_check_tables::program::Elaboration,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<ReplayScope<'source>> {
    terminal_semantics::scalar_qualifications::validate(&elaboration.module).map_err(|error| {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            elaboration
                .module
                .machines
                .first()
                .map(|machine| machine.id)
                .unwrap_or_else(|| MachineId::new(1).expect("one is a machine identity")),
            Construct::Machine,
            &format!("invalid declared scalar qualifications: {error:?}"),
        )
    })?;
    // A record formal, result or field in a domain is a contract of the
    // lowered form, judged here before anything is inferred from it.
    terminal_semantics::structural_qualifications::validate(&elaboration.module).map_err(
        |error| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                error
                    .machine
                    .or_else(|| {
                        elaboration
                            .module
                            .machines
                            .first()
                            .map(|machine| machine.id)
                    })
                    .unwrap_or_else(|| MachineId::new(1).expect("one is a machine identity")),
                Construct::Machine,
                &format!(
                    "invalid declared structural qualifications: {}",
                    error.detail
                ),
            )
        },
    )?;
    module(&elaboration.module, activity)?;
    Ok(ReplayScope { elaboration })
}

/// A proof preparation borrows this exact formed module. Only the formation
/// scope can construct the view, and its mutable edit methods cannot run
/// while a view is borrowed.
#[derive(Clone, Copy)]
pub(crate) struct FormedModule<'a> {
    module: &'a TerminalModule,
}

impl<'a> FormedModule<'a> {
    pub(crate) fn source(self) -> &'a TerminalModule {
        self.module
    }
}

impl ReplayScope<'_> {
    pub(crate) fn source(&self) -> &terminal_check_tables::program::Elaboration {
        self.elaboration
    }
    pub(crate) fn formed(&self) -> FormedModule<'_> {
        FormedModule {
            module: &self.elaboration.module,
        }
    }

    pub(crate) fn infer_inputs(
        &mut self,
        unreached: &mut crate::Unreached,
        inhabitation: &terminal_semantics::inhabitation::Inhabitation,
        activity: &mut crate::ProofSearchActivity,
    ) -> Checked<()> {
        crate::entry_inputs::infer(self, unreached, inhabitation, activity)
    }

    pub(crate) fn infer_results(
        &mut self,
        unreached: &mut crate::Unreached,
        inhabitation: &terminal_semantics::inhabitation::Inhabitation,
        activity: &mut crate::ProofSearchActivity,
    ) -> Checked<()> {
        crate::result_summaries::infer(self, unreached, inhabitation, activity)
    }

    pub(crate) fn require(
        &mut self,
        callee: MachineId,
        predicate: Proposition,
        sites: &crate::entry_inputs::CallSites,
        activity: &mut crate::ProofSearchActivity,
    ) -> Checked<()> {
        // This publisher forms the proposed contract before changing it.
        crate::entry_inputs::require(self.elaboration, callee, predicate, sites, activity)
    }

    pub(crate) fn summarize(
        &mut self,
        machine: MachineId,
        summary: Proposition,
        exits: &[(semantic_vocabulary::EdgeId, ValueId)],
        activity: &mut crate::ProofSearchActivity,
    ) -> Checked<()> {
        // This publisher also forms the complete replacement before use.
        crate::result_summaries::publish(self.elaboration, machine, summary, exits, activity)
    }

    pub(crate) fn finalize_callable_refinements(
        &mut self,
        activity: &mut crate::ProofSearchActivity,
    ) -> Checked<()> {
        crate::finalize_callable_refinements(&mut self.elaboration.module, activity)
    }

    // Kept private to the replay implementation. No caller can obtain a
    // mutable declaration borrow from a successfully formed scope.
    pub(super) fn checking(
        &mut self,
    ) -> (
        FormedModule<'_>,
        &semantic_vocabulary::IdTable<StructuralTypeId, Vec<Proposition>>,
        &mut terminal_check_tables::CheckTables,
        &semantic_vocabulary::IdTable<MachineId, terminal_check_tables::program::MachineSources>,
        &[MachineId],
    ) {
        (
            FormedModule {
                module: &self.elaboration.module,
            },
            &self.elaboration.domains,
            &mut self.elaboration.tables,
            &self.elaboration.sources,
            &self.elaboration.requirements,
        )
    }
}

#[derive(Clone, Copy)]
struct Root {
    place: PlaceId,
    ty: StructuralTypeId,
    borrowed: bool,
    result: bool,
    has_entry: bool,
}

#[derive(Clone, PartialEq, Eq)]
enum Observation {
    Input(ValueId),
    Field(Version, PlaceId, Vec<Segment>),
    Extent(PlaceId, Vec<Segment>),
    Computed(ScalarTerm),
}

#[derive(Clone)]
struct Value {
    observation: Observation,
    ty: ScalarType,
}

#[derive(Clone, Default)]
struct Facts {
    values: Vec<Value>,
    premises: Vec<Proposition>,
    equations: Vec<Proposition>,
}

impl crate::known::ProofValues for Facts {
    fn count(&self) -> usize {
        self.values.len()
    }
    fn identity(&self, position: usize) -> Option<ValueId> {
        self.values.get(position)?;
        ValueId::new(position as u64 + 1)
    }
    fn scalar_type(&self, value: ValueId) -> Option<ScalarType> {
        usize::try_from(value.get().checked_sub(1)?)
            .ok()
            .and_then(|index| self.values.get(index))
            .map(|value| value.ty)
    }
    fn is_storage_content(&self, value: ValueId) -> bool {
        usize::try_from(value.get() - 1)
            .ok()
            .and_then(|index| self.values.get(index))
            .is_some_and(|value| {
                matches!(
                    value.observation,
                    Observation::Field(..) | Observation::Extent(..)
                )
            })
    }
}

enum BindingError {
    Unsupported(&'static str),
    Unproved(Refusal),
}

impl From<&'static str> for BindingError {
    fn from(value: &'static str) -> Self {
        Self::Unsupported(value)
    }
}

struct Scope<'a> {
    module: &'a TerminalModule,
    diagnostic: MachineId,
    inputs: Vec<ValueDeclaration>,
    result: Option<ValueDeclaration>,
    roots: Vec<Root>,
    completion: bool,
    facts: Facts,
    activity: &'a mut crate::ProofSearchActivity,
}

impl Scope<'_> {
    fn observe(
        &mut self,
        observation: Observation,
        ty: ScalarType,
    ) -> Result<ScalarTerm, BindingError> {
        if let Some(index) = self
            .facts
            .values
            .iter()
            .position(|value| value.observation == observation)
        {
            if self.facts.values[index].ty != ty {
                return Err("contract observation has inconsistent scalar types".into());
            }
            return Ok(ScalarTerm::value(
                ValueId::new(index as u64 + 1).expect("nonzero"),
                ty,
            ));
        }
        let id = ValueId::new(self.facts.values.len() as u64 + 1)
            .ok_or("contract logical value identity overflow")?;
        self.facts.values.push(Value { observation, ty });
        Ok(ScalarTerm::value(id, ty))
    }

    fn prove(&mut self, goal: &Proposition) -> Result<(), BindingError> {
        let context =
            PropositionContext::from_value_types(self.facts.values.iter().enumerate().map(
                |(index, value)| (ValueId::new(index as u64 + 1).expect("nonzero"), value.ty),
            ))
            .map_err(|_| {
                BindingError::Unsupported("contract logical inputs have unsupported scalar types")
            })?;
        let probe = ObligationId::new(1).expect("nonzero");
        let certificate = crate::search_certificate(
            &context,
            &self.facts,
            self.diagnostic,
            probe,
            goal,
            &self.facts.premises,
            &self.facts.equations,
            &[],
            self.activity,
        )
        .map_err(BindingError::Unproved)?;
        crate::verify_certificate(
            &context,
            self.diagnostic,
            probe,
            goal,
            &self.facts.premises,
            &self.facts.equations,
            &certificate,
        )
        .map_err(BindingError::Unproved)?;
        Ok(())
    }

    fn root(&self, place: PlaceId) -> Result<Root, BindingError> {
        let mut roots = self.roots.iter().filter(|root| root.place == place);
        let root = *roots.next().ok_or("contract field has no declared root")?;
        if roots.next().is_some() || (root.result && !self.completion) {
            return Err("contract field has an ambiguous or unavailable root".into());
        }
        Ok(root)
    }

    fn error(&self, error: BindingError, owner: &str) -> Refusal {
        match error {
            BindingError::Unsupported(detail) => crate::refusal(
                RefusalReason::Unsupported,
                self.diagnostic,
                Construct::Machine,
                &format!("{owner}: contract term formation: {detail}"),
            ),
            BindingError::Unproved(error) => {
                let reason = match error.reason {
                    RefusalReason::Unproved(
                        terminal_check_tables::UnprovedCause::NoRuleForGoalForm,
                    ) => RefusalReason::Unsupported,
                    _ => RefusalReason::Violation,
                };
                crate::refusal(
                    reason,
                    self.diagnostic,
                    Construct::Machine,
                    &format!(
                        "{owner}: contract arithmetic is not proved total before its condition: {}",
                        error.detail
                    ),
                )
            }
        }
    }

    /// A clause joins the premises only after all its terms form. Retrying
    /// pending clauses permits independent bounds in either authored order;
    /// a mutually pending cycle supplies no facts and cannot make progress.
    fn clauses(&mut self, clauses: &[Proposition], owner: &str) -> Checked<()> {
        let mut pending = Vec::new();
        for clause in clauses {
            conjuncts(clause, &mut pending);
        }
        while !pending.is_empty() {
            let mut rest = Vec::new();
            let mut failure = None;
            let previous = pending.len();
            for clause in pending {
                let checkpoint = self.facts.clone();
                match substitute_clause(clause, self) {
                    Ok(formed) => self.facts.premises.push(formed),
                    Err(error) => {
                        self.facts = checkpoint;
                        // A total clause that this scalar binding does not
                        // serve supplies no premise here. Its ordinary
                        // declaration checks still apply. A partial occurrence
                        // must instead bind and discharge in this phase.
                        if owes([clause]) {
                            failure = Some(self.error(error, owner));
                            rest.push(clause);
                        }
                    }
                }
            }
            if rest.len() == previous {
                return Err(failure.expect("pending clause has a refusal"));
            }
            pending = rest;
        }
        Ok(())
    }
}

fn conjuncts<'a>(clause: &'a Proposition, out: &mut Vec<&'a Proposition>) {
    let mut pending = vec![clause];
    while let Some(clause) = pending.pop() {
        if let Proposition::Conjunction(parts) = clause {
            pending.extend(parts.iter().rev());
        } else {
            out.push(clause);
        }
    }
}

impl ClauseBindings for Scope<'_> {
    type Error = BindingError;
    fn requires_complete_terms(&self) -> bool {
        true
    }
    fn value(&mut self, id: ValueId, ty: ScalarType) -> Result<ScalarTerm, BindingError> {
        let mut candidates = self
            .inputs
            .iter()
            .copied()
            .chain(self.result.filter(|_| self.completion))
            .filter(|value| value.id == id);
        let value = candidates
            .next()
            .ok_or("contract value has no declared input or available result")?;
        if candidates.next().is_some() || value.scalar_type != ty {
            return Err("contract value has ambiguous or inconsistent declaration".into());
        }
        self.observe(Observation::Input(id), ty)
    }

    fn field(
        &mut self,
        version: Version,
        place: PlaceId,
        path: &[Segment],
        ty: ScalarType,
    ) -> Result<ScalarTerm, BindingError> {
        let root = self.root(place)?;
        if version == Version::Entry && !root.has_entry {
            return Err("contract result or data subject has no entry observation".into());
        }
        // A nominal view element does not prove that the element exists.
        // Fixed arrays carry their extent; open views need a separate bound.
        for (index, segment) in path.iter().enumerate() {
            if matches!(segment, Segment::FixedIndex(_)) {
                let prefix = terminal_semantics::canonical_structural_path_tip(
                    self.module.structural_types.iter(),
                    root.ty,
                    &path[..index],
                )
                .ok_or("contract index lost its exact declared collection")?;
                if !matches!(
                    prefix.shape,
                    terminal_psi::StructuralTypeShape::FixedArray { .. }
                ) {
                    return Err(
                        "contract view index needs an established extent observation".into(),
                    );
                }
            }
        }
        let found = terminal_semantics::canonical_primitive_read_type(
            self.module.structural_types.iter(),
            root.ty,
            path,
        )
        .ok_or("contract field is not an admitted scalar projection")?;
        if found != ty {
            return Err("contract field scalar type differs from its declaration".into());
        }
        let live = root.borrowed || path.contains(&Segment::Referent);
        let version = if root.has_entry && (!self.completion || !live || version == Version::Entry)
        {
            Version::Entry
        } else {
            Version::Current
        };
        self.observe(Observation::Field(version, place, path.to_vec()), ty)
    }

    fn extent(
        &mut self,
        place: PlaceId,
        path: &[Segment],
        ty: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, BindingError> {
        let root = self.root(place)?;
        let selected = terminal_semantics::canonical_structural_path_tip(
            self.module.structural_types.iter(),
            root.ty,
            path,
        )
        .ok_or("contract extent has no exact declared descriptor")?;
        if !matches!(
            selected.shape,
            terminal_psi::StructuralTypeShape::ElementView { .. }
        ) {
            return Err("contract extent is not an element view".into());
        }
        // Extents in this representation have no old-version tag. Do not
        // identify a mutable descriptor's completion extent with its input.
        if self.completion && (root.borrowed || path.contains(&Segment::Referent)) {
            return Err("completion extent needs its exact storage version".into());
        }
        self.observe(
            Observation::Extent(place, path.to_vec()),
            ScalarType::Integer(ty),
        )
    }

    fn case(
        &mut self,
        _: PlaceId,
        _: &[Segment],
        _: StructuralCaseId,
    ) -> Result<bool, BindingError> {
        Err("partial contract under a case requires its exact case observation".into())
    }

    fn computed(&mut self, term: ScalarTerm) -> Result<ScalarTerm, BindingError> {
        // The result's carrier range is unavailable while proving formation.
        // Only the already formed operands can justify the new computation.
        if let Some(goal) = CanonicalScalarGoal::for_term(&term).map_err(|_| {
            BindingError::Unsupported("contract computation has inconsistent scalar semantics")
        })? {
            let goal = goal.kernel_proposition().map_err(|_| {
                BindingError::Unsupported("contract arithmetic has no canonical scalar goal")
            })?;
            self.prove(&goal)?;
        }
        let value = self.observe(Observation::Computed(term.clone()), term.scalar_type())?;
        let equation = Proposition::Equal(value.clone(), term);
        if !self.facts.equations.contains(&equation) {
            self.facts.equations.push(equation);
        }
        Ok(value)
    }
}

fn owes(clauses: impl IntoIterator<Item = impl std::borrow::Borrow<Proposition>>) -> bool {
    let mut owes = false;
    for clause in clauses {
        clause.borrow().visit_scalar_terms(|term| {
            owes |= !matches!(CanonicalScalarGoal::for_term(term), Ok(None));
        });
    }
    owes
}

fn roots(parameters: &[terminal_psi::StructuralParameterDeclaration]) -> Vec<Root> {
    parameters
        .iter()
        .map(|formal| Root {
            place: formal.place,
            ty: formal.structural_type,
            borrowed: formal.access != StructuralAccess::Owned,
            result: false,
            has_entry: true,
        })
        .collect()
}

fn crash_clauses(routes: &[terminal_psi::CrashRouteBucket]) -> impl Iterator<Item = &Proposition> {
    routes
        .iter()
        .flat_map(|route| &route.alternatives)
        .filter_map(|guard| match guard {
            terminal_psi::CrashRouteGuard::Truth => None,
            terminal_psi::CrashRouteGuard::Predicate(predicate) => Some(predicate.proposition()),
        })
}

pub(crate) fn contract(
    module: &TerminalModule,
    diagnostic: MachineId,
    contract: &MachineContract,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    let guards: Vec<_> = crash_clauses(&contract.crash_routes)
        .chain(
            contract
                .declared_crash_routes
                .iter()
                .flat_map(|routes| crash_clauses(routes)),
        )
        .collect();
    if !owes(
        contract
            .requires
            .iter()
            .chain(&contract.source_checked_requires)
            .chain(contract.ensures.iter().map(|clause| &clause.proposition))
            .chain(
                contract
                    .outcome_specific_ensures
                    .iter()
                    .map(|clause| &clause.proposition),
            )
            .chain(guards.iter().copied()),
    ) {
        return Ok(());
    }
    let mut scope = Scope {
        module,
        diagnostic,
        inputs: contract
            .parameters
            .iter()
            .chain(&contract.erased_scalar_formals)
            .copied()
            .collect(),
        result: contract.result.scalar(),
        roots: roots(&contract.structural_parameters),
        completion: false,
        facts: Facts::default(),
        activity: &mut *activity,
    };
    if let Some(result) = contract.result.structural() {
        scope.roots.push(Root {
            place: result.place,
            ty: result.structural_type,
            borrowed: false,
            result: true,
            has_entry: false,
        });
    }
    scope.clauses(&contract.requires, "callable requires")?;
    // Source-checked contracts are independently retained interfaces, not an
    // additional premise that can establish the executable input contract.
    let entry = scope.facts.clone();
    scope.clauses(&contract.source_checked_requires, "source requires")?;
    scope.facts = entry.clone();
    for guard in guards {
        scope.clauses(std::slice::from_ref(guard), "crash guard")?;
        scope.facts = entry.clone();
    }
    scope.completion = true;
    scope.clauses(
        &contract
            .ensures
            .iter()
            .map(|clause| clause.proposition.clone())
            .collect::<Vec<_>>(),
        "normal ensures",
    )?;
    let normal = scope.facts.clone();
    for clause in &contract.outcome_specific_ensures {
        scope.clauses(std::slice::from_ref(&clause.proposition), "outcome ensures")?;
        scope.facts = normal.clone();
    }
    Ok(())
}

/// Reconstruct from each real declaration namespace, including interfaces
/// without bodies. No source witness, cached row or executable arrival stands
/// in for a declaration's complete term inventory.
pub(crate) fn module(
    module: &TerminalModule,
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    let diagnostic = module
        .machines
        .first()
        .map(|machine| machine.id)
        .unwrap_or_else(|| MachineId::new(1).expect("nonzero"));
    for machine in &module.machines {
        contract(module, machine.id, &machine.contract, activity)?;
    }
    for declared in &module.declaration_contracts {
        contract(module, diagnostic, declared, activity)?;
    }
    for boundary in &module.boundary_machines {
        let guards: Vec<_> = crash_clauses(&boundary.crash_routes).collect();
        if !owes(
            boundary
                .scalar_requires
                .iter()
                .chain(&boundary.scalar_ensures)
                .chain(guards.iter().copied()),
        ) {
            continue;
        }
        let mut scope = Scope {
            module,
            diagnostic,
            inputs: boundary.scalar_contract_parameters().ok_or_else(|| {
                crate::refusal(
                    RefusalReason::Unsupported,
                    diagnostic,
                    Construct::Machine,
                    "boundary contract formal identities overflow",
                )
            })?,
            result: boundary.scalar_contract_result(),
            roots: roots(&boundary.structural_parameters),
            completion: false,
            facts: Facts::default(),
            activity: &mut *activity,
        };
        if let terminal_psi::BoundaryMachineResult::Structural(result) = &boundary.result {
            // Boundary clause coordinates follow the same dense input roster
            // as machine contracts; the result is the next structural place.
            let place =
                PlaceId::new(boundary.structural_parameters.len() as u64 + 1).ok_or_else(|| {
                    crate::refusal(
                        RefusalReason::Unsupported,
                        diagnostic,
                        Construct::Machine,
                        "boundary result identity overflow",
                    )
                })?;
            scope.roots.push(Root {
                place,
                ty: result.structural_type,
                borrowed: false,
                result: true,
                has_entry: false,
            });
        }
        scope.clauses(&boundary.scalar_requires, "boundary requires")?;
        let entry = scope.facts.clone();
        for guard in guards {
            scope.clauses(std::slice::from_ref(guard), "boundary crash guard")?;
            scope.facts = entry.clone();
        }
        scope.completion = true;
        scope.clauses(&boundary.scalar_ensures, "boundary ensures")?;
    }
    for declared in &module.structural_types {
        data(
            module,
            diagnostic,
            declared.id,
            PlaceId::new(1).expect("nonzero"),
            &declared.requires,
            activity,
        )?;
    }
    for domain in &module.structural_domains {
        if let Some(predicate) = &domain.predicate {
            data(
                module,
                diagnostic,
                domain.carrier,
                predicate.subject,
                std::slice::from_ref(&predicate.proposition),
                activity,
            )?;
        }
    }
    for domain in &module.scalar_qualifications.domains {
        if let Some(predicate) = &domain.predicate
            && owes([&predicate.proposition])
        {
            let mut scope = Scope {
                module,
                diagnostic,
                inputs: vec![ValueDeclaration {
                    id: predicate.subject,
                    scalar_type: domain.carrier,
                    qualifications: semantic_vocabulary::ScalarQualificationSetId::ZERO,
                }],
                result: None,
                roots: Vec::new(),
                completion: false,
                facts: Facts::default(),
                activity: &mut *activity,
            };
            scope.clauses(
                std::slice::from_ref(&predicate.proposition),
                "scalar domain",
            )?;
        }
    }
    Ok(())
}

fn data(
    module: &TerminalModule,
    diagnostic: MachineId,
    ty: StructuralTypeId,
    place: PlaceId,
    clauses: &[Proposition],
    activity: &mut crate::ProofSearchActivity,
) -> Checked<()> {
    if !owes(clauses) {
        return Ok(());
    }
    let mut scope = Scope {
        module,
        diagnostic,
        inputs: Vec::new(),
        result: None,
        roots: vec![Root {
            place,
            ty,
            borrowed: false,
            result: false,
            has_entry: false,
        }],
        completion: false,
        facts: Facts::default(),
        activity: &mut *activity,
    };
    scope.clauses(clauses, "data domain")
}
