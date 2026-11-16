//! A condition cannot justify arithmetic needed to state that condition.
//! Form declarations before their clauses enter proof knowledge. These local
//! logical observations have no runtime representation or transferable receipt;
//! every partial computation uses the ordinary scalar goal and proof kernel.

use std::borrow::Cow;
use std::cell::OnceCell;
use std::collections::BTreeSet;

use semantic_vocabulary::{
    CanonicalStructuralPathSegment as Segment, ContentPlaceVersion as Version, MachineId,
    ObligationId, PlaceId, Proposition, PropositionContext, ScalarTerm, ScalarType,
    StructuralCaseId, StructuralTypeId, ValueId,
};
use terminal_check_tables::{Checked, Construct, Refusal, RefusalReason};
use terminal_psi::{
    MachineContract, StructuralAccess, StructuralTypeDeclaration, TerminalModule, ValueDeclaration,
};
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

pub fn prepare_replay(
    elaboration: &mut terminal_check_tables::program::Elaboration,
) -> Checked<ReplayScope<'_>> {
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
    module(&elaboration.module, &elaboration.predicate_sources)?;
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
    ) -> Checked<()> {
        crate::entry_inputs::infer(self, unreached, inhabitation)
    }

    pub(crate) fn infer_results(
        &mut self,
        unreached: &mut crate::Unreached,
        inhabitation: &terminal_semantics::inhabitation::Inhabitation,
    ) -> Checked<()> {
        crate::result_summaries::infer(self, unreached, inhabitation)
    }

    pub(crate) fn require(
        &mut self,
        callee: MachineId,
        predicate: Proposition,
        sites: &crate::entry_inputs::CallSites,
    ) -> Checked<()> {
        // This publisher forms the proposed contract before changing it.
        crate::entry_inputs::require(self.elaboration, callee, predicate, sites)
    }

    pub(crate) fn summarize(
        &mut self,
        machine: MachineId,
        summary: Proposition,
        exits: &[(semantic_vocabulary::EdgeId, ValueId)],
    ) -> Checked<()> {
        // This publisher also forms the complete replacement before use.
        crate::result_summaries::publish(self.elaboration, machine, summary, exits)
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
    Field(Version, usize, Vec<Segment>),
    Extent(usize, Vec<Segment>),
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
    fn scalar_type(&self, value: ValueId) -> Option<ScalarType> {
        usize::try_from(value.get().checked_sub(1)?)
            .ok()
            .and_then(|index| self.values.get(index))
            .map(|value| value.ty)
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
    subject_prefix: Vec<Segment>,
    completion: bool,
    facts: Facts,
}

impl Scope<'_> {
    fn subject_path<'path>(&self, path: &'path [Segment]) -> Cow<'path, [Segment]> {
        if self.subject_prefix.is_empty() {
            Cow::Borrowed(path)
        } else {
            Cow::Owned([self.subject_prefix.as_slice(), path].concat())
        }
    }

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

    fn prove(&self, goal: &Proposition) -> Result<(), BindingError> {
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

    fn root(&self, place: PlaceId) -> Result<(usize, Root), BindingError> {
        let mut roots = self
            .roots
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, root)| root.place == place);
        let (index, root) = roots.next().ok_or("contract field has no declared root")?;
        if roots.next().is_some() || (root.result && !self.completion) {
            return Err("contract field has an ambiguous or unavailable root".into());
        }
        Ok((index, root))
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
    fn clauses<'clause>(
        &mut self,
        clauses: impl IntoIterator<Item = &'clause Proposition>,
        owner: &str,
    ) -> Checked<()> {
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
    fn validate_float_meaning_source(&mut self, _source: &ScalarTerm) -> Result<(), Self::Error> {
        // Formation checks a symbolic declaration equation. It mints no
        // concrete projection or runtime owner. Actual call imports perform
        // source correspondence after their ordinary argument substitution.
        Ok(())
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
        let (index, root) = self.root(place)?;
        let path = self.subject_path(path);
        if version == Version::Entry && !root.has_entry {
            return Err("contract result or data subject has no entry observation".into());
        }
        // A nominal view element does not prove that the element exists.
        // Fixed arrays carry their extent; open views need a separate bound.
        for (depth, segment) in path.iter().enumerate() {
            if matches!(segment, Segment::FixedIndex(_)) {
                let prefix = terminal_semantics::canonical_structural_path_tip(
                    self.module.structural_types.iter(),
                    root.ty,
                    &path[..depth],
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
            &path,
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
        self.observe(Observation::Field(version, index, path.into_owned()), ty)
    }

    fn extent(
        &mut self,
        place: PlaceId,
        path: &[Segment],
        ty: semantic_vocabulary::IntegerType,
    ) -> Result<ScalarTerm, BindingError> {
        let (index, root) = self.root(place)?;
        let path = self.subject_path(path);
        let selected = terminal_semantics::canonical_structural_path_tip(
            self.module.structural_types.iter(),
            root.ty,
            &path,
        )
        .ok_or("contract extent has no exact declared descriptor")?;
        // A borrowed byte view is a descriptor with a count, exactly as an
        // element view is (byte_views.md, "Collection view establishment"):
        // `out.len` of a `&mut [u8]` formal is that count.
        if !matches!(
            selected.shape,
            terminal_psi::StructuralTypeShape::ElementView { .. }
                | terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                )
        ) {
            return Err("contract extent is not a borrowed view".into());
        }
        // Extents in this representation have no old-version tag. Do not
        // identify a mutable descriptor's completion extent with its input.
        if self.completion && (root.borrowed || path.contains(&Segment::Referent)) {
            return Err("completion extent needs its exact storage version".into());
        }
        self.observe(
            Observation::Extent(index, path.into_owned()),
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
        subject_prefix: Vec::new(),
        completion: false,
        facts: Facts::default(),
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
    predicate_sources: &[terminal_check_tables::program::PredicateSource],
) -> Checked<()> {
    let diagnostic = module
        .machines
        .first()
        .map(|machine| machine.id)
        .unwrap_or_else(|| MachineId::new(1).expect("nonzero"));
    if module
        .scalar_predicate_definitions
        .windows(2)
        .any(|pair| pair[0].declaration >= pair[1].declaration)
    {
        return Err(crate::refusal(
            RefusalReason::ForeignIdentity,
            diagnostic,
            Construct::Machine,
            "mathematical predicate declarations are duplicated or unordered",
        ));
    }
    for definition in &module.scalar_predicate_definitions {
        let source = predicate_sources
            .binary_search_by_key(&definition.declaration, |source| source.declaration)
            .ok()
            .map(|position| &predicate_sources[position]);
        let diagnostic = source.map_or(diagnostic, |source| source.machine);
        let invalid = |detail: &str| {
            crate::refusal(
                RefusalReason::ForeignIdentity,
                diagnostic,
                Construct::Machine,
                detail,
            )
        };
        if module
            .identities
            .text(definition.declaration)
            .is_none_or(str::is_empty)
        {
            return Err(invalid(
                "mathematical predicate has no exact declaration identity",
            ));
        }
        if definition
            .parameters
            .iter()
            .any(|parameter| !parameter.qualifications.is_empty())
        {
            return Err(invalid(
                "qualified mathematical predicate input is not supported",
            ));
        }
        PropositionContext::from_value_types(
            definition
                .parameters
                .iter()
                .map(|parameter| (parameter.id, parameter.scalar_type)),
        )
        .and_then(|context| context.validate(&definition.body))
        .map_err(|error| invalid(&format!("mathematical predicate formal scope: {error:?}")))?;
        let mut scope = Scope {
            module,
            diagnostic,
            inputs: definition.parameters.clone(),
            result: None,
            roots: Vec::new(),
            subject_prefix: Vec::new(),
            completion: false,
            facts: Facts::default(),
        };
        // The body defines a proposition; it supplies no premise, including
        // when a conjunct states a bound another conjunct would need.
        substitute_clause(&definition.body, &mut scope).map_err(|error| {
            let owner = source.map_or_else(
                || "mathematical definition".to_owned(),
                |source| format!("named predicate `{}`", source.name),
            );
            scope
                .error(error, &owner)
                .with_source_span(source.and_then(|source| source.source_span))
        })?;
    }
    for machine in &module.machines {
        contract(module, machine.id, &machine.contract)?;
    }
    for declared in &module.declaration_contracts {
        contract(module, diagnostic, declared)?;
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
            subject_prefix: Vec::new(),
            completion: false,
            facts: Facts::default(),
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
    data_domains(module, diagnostic)?;
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
                subject_prefix: Vec::new(),
                completion: false,
                facts: Facts::default(),
            };
            scope.clauses(
                std::slice::from_ref(&predicate.proposition),
                "scalar domain",
            )?;
        }
    }
    Ok(())
}

struct DataSubject<'a> {
    carrier: &'a StructuralTypeDeclaration,
    subject: PlaceId,
    prefix: Vec<Segment>,
    clauses: &'a [Proposition],
}

/// Read the storage coordinates already served by scalar clause binding.
fn clause_storage(term: &ScalarTerm) -> Option<(Version, PlaceId, &[Segment])> {
    match term {
        ScalarTerm::IntegerField {
            version,
            root,
            path,
            ..
        }
        | ScalarTerm::BooleanField {
            version,
            root,
            path,
        } => Some((*version, *root, path)),
        ScalarTerm::ViewExtent { root, path, .. } => Some((Version::Current, *root, path)),
        _ => None,
    }
}

impl DataSubject<'_> {
    fn observed_inline_paths(&self) -> Vec<Vec<Segment>> {
        let mut paths = Vec::new();
        for clause in self
            .clauses
            .iter()
            .filter(|clause| self.prefix.is_empty() || self.inline_clause(clause))
        {
            clause.visit_scalar_terms(|term| {
                let Some((Version::Current, root, path)) = clause_storage(term) else {
                    return;
                };
                let owners = if matches!(term, ScalarTerm::ViewExtent { .. }) {
                    path
                } else {
                    path.split_last().map_or(&[][..], |(_, owners)| owners)
                };
                if root == self.subject && !owners.is_empty() {
                    paths.push(owners.to_vec());
                }
            });
        }
        paths.sort_unstable();
        paths.dedup();
        paths
    }

    fn inline_clause(&self, clause: &Proposition) -> bool {
        let mut inline = true;
        clause.visit_scalar_terms(|term| {
            if let Some((version, place, path)) = clause_storage(term) {
                // A nested carrier does not establish the contents of
                // its borrowed backing or any case-local storage here.
                inline &= place == self.subject
                    && version == Version::Current
                    && !path
                        .iter()
                        .any(|step| matches!(step, Segment::Referent | Segment::Case(_)))
                    && !matches!(term, ScalarTerm::ViewExtent { .. });
            }
        });
        inline
    }
}

/// Discover only inline values observed by a clause. Their own requirements
/// are established at those projections, and may themselves observe another
/// nested value. Form deeper declarations first, before any enclosing clause
/// can contribute assumptions.
fn add_projected_defaults<'types>(
    scope: &mut Scope<'types>,
    ordered: &OnceCell<Vec<&'types StructuralTypeDeclaration>>,
    mut pending: Vec<DataSubject<'types>>,
    default_subject: PlaceId,
) -> Checked<()> {
    let types = &scope.module.structural_types;
    let mut visited = BTreeSet::new();
    let mut defaults = Vec::new();
    while let Some(subject) = pending.pop() {
        // A finite inline value has no type repeated on one storage path.
        if subject.prefix.len() >= types.len() {
            return Err(scope.error(
                "contract projection has a cyclic inline carrier".into(),
                "data domain",
            ));
        }
        for path in subject.observed_inline_paths() {
            let declarations = ordered.get_or_init(|| {
                let mut rows = types.iter().collect::<Vec<_>>();
                rows.sort_unstable_by_key(|row| row.id);
                rows
            });
            let lookup = |id| {
                let start = declarations.partition_point(|row| row.id < id);
                let row = *declarations.get(start)?;
                (row.id == id && declarations.get(start + 1).is_none_or(|next| next.id != id))
                    .then_some(row)
            };
            let selected = terminal_semantics::visit_canonical_inline_carriers(
                declarations.iter().copied(),
                lookup,
                subject.carrier,
                &path,
                |depth, carrier| {
                    if carrier.requires.is_empty() {
                        return;
                    }
                    let prefix = [subject.prefix.as_slice(), &path[..depth]].concat();
                    if visited.insert(prefix.clone()) {
                        pending.push(DataSubject {
                            carrier,
                            subject: default_subject,
                            prefix,
                            clauses: &carrier.requires,
                        });
                    }
                },
            );
            if selected.is_none() {
                return Err(scope.error(
                    "contract projection has no exact inline carrier".into(),
                    "data domain",
                ));
            }
        }
        if !subject.prefix.is_empty() {
            defaults.push(subject);
        }
    }
    defaults.sort_unstable_by(|left, right| {
        right
            .prefix
            .len()
            .cmp(&left.prefix.len())
            .then_with(|| left.prefix.cmp(&right.prefix))
    });
    for mut default in defaults {
        scope.subject_prefix = std::mem::take(&mut default.prefix);
        let clauses = default
            .clauses
            .iter()
            .filter(|clause| default.inline_clause(clause));
        scope.clauses(clauses, "data domain")?;
    }
    scope.subject_prefix.clear();
    Ok(())
}

fn data_domains(module: &TerminalModule, diagnostic: MachineId) -> Checked<()> {
    let mut predicates = module
        .structural_domains
        .iter()
        .filter_map(|domain| {
            let predicate = domain.predicate.as_ref()?;
            owes([&predicate.proposition]).then_some((domain.carrier, domain.id, Some(predicate)))
        })
        .collect::<Vec<_>>();
    predicates.sort_unstable_by_key(|&(carrier, domain, _)| (carrier, domain));
    let carrier_error = |detail| {
        crate::refusal(
            RefusalReason::ForeignIdentity,
            diagnostic,
            Construct::Machine,
            detail,
        )
    };
    let mut remaining = predicates.len();
    let ordered = OnceCell::new();
    let default_subject = PlaceId::new(1).expect("one is a place");
    for declared in &module.structural_types {
        let first = predicates.partition_point(|row| row.0 < declared.id);
        let end = predicates.partition_point(|row| row.0 <= declared.id);
        let domains = &mut predicates[first..end];
        if domains.is_empty() && !owes(&declared.requires) {
            continue;
        }
        let mut scope = Scope {
            module,
            diagnostic,
            inputs: Vec::new(),
            result: None,
            roots: vec![Root {
                place: default_subject,
                ty: declared.id,
                borrowed: false,
                result: false,
                has_entry: false,
            }],
            subject_prefix: Vec::new(),
            completion: false,
            facts: Facts::default(),
        };
        let subjects = std::iter::once((default_subject, declared.requires.as_slice()))
            .chain(domains.iter().filter_map(|(_, _, predicate)| {
                let predicate = predicate.as_ref()?;
                Some((
                    predicate.subject,
                    std::slice::from_ref(&predicate.proposition),
                ))
            }))
            .map(|(subject, clauses)| DataSubject {
                carrier: declared,
                subject,
                prefix: Vec::new(),
                clauses,
            })
            .collect();
        add_projected_defaults(&mut scope, &ordered, subjects, default_subject)?;
        scope.clauses(&declared.requires, "data domain")?;
        let carrier_facts = std::mem::take(&mut scope.facts);
        for (_, _, predicate) in domains {
            let predicate = predicate
                .take()
                .ok_or_else(|| carrier_error("record domain has an ambiguous carrier"))?;
            // Both clauses constrain the same occupied carrier. Rebind its
            // source subject while observations keep the same scope slot.
            scope.roots[0].place = predicate.subject;
            scope.facts = carrier_facts.clone();
            scope.clauses(std::slice::from_ref(&predicate.proposition), "data domain")?;
            remaining -= 1;
        }
    }
    if remaining != 0 {
        return Err(carrier_error("record domain has no declared carrier"));
    }
    Ok(())
}
