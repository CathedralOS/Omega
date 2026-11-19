//! The exact typed owner and the first accepted closure of its whole
//! compilation. Source is retained before judgement; another source cannot
//! be attached to an artifact after the route has decided it.

use crate::checking::{AcceptedClosure, CompilationRefusal, RouteVerdict};
use crate::source_state::FrozenTypedTrees;
use diagnostics::Diagnostic;
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Debug)]
pub struct AcceptedTerminalSource {
    source: Arc<FrozenTypedTrees>,
    closure: Box<AcceptedClosure>,
    correspondence: crate::lowering::SourceCorrespondence,
    requirement_reviews: Vec<crate::lowering::RequirementReview>,
    publication: terminal_codec::TerminalPublication,
    pub(crate) package_review_numeric: crate::package_review::NumericReviewInventory,
    /// What the route says about the accepted source without refusing it
    /// (`advisories`), in the order the parts were judged.
    advisories: DeferredAdvisories,
}

/// The checked module and the owned declarations, source locations and Build
/// operations needed to publish it. Dropping the source producer is safe here.
pub struct PreparedTerminalPublication {
    kind: terminal_codec::TerminalPublication,
    module: crate::checking::CheckedPublicationModule,
    selections: crate::production::source_selection_publication::PreparedSelections,
    provenance: terminal_psi::TerminalMachineProvenance,
    companions: crate::production::build_companions::BuildCompanions,
    source_records: Vec<crate::source_state::SourceRecordCorrespondence>,
}

impl PreparedTerminalPublication {
    pub fn publish(
        self,
        applications: &[crate::production::TerminalApplicationCustody<'_>],
        observer: &mut impl crate::production::PsiCompilationObserver,
    ) -> Result<crate::production::ProducedTerminalProgram, Vec<Diagnostic>> {
        let (program, evidence) =
            observer.record_result(crate::production::PsiCompilationStage::Publication, || {
                let published = match self.kind {
                    terminal_codec::TerminalPublication::Portable => {
                        self.module.publish().map(|publication| {
                            (
                                terminal_codec::TerminalProgramProduct::Portable(
                                    publication.artifact,
                                ),
                                publication.seals,
                            )
                        })
                    }
                    terminal_codec::TerminalPublication::Invocation
                    | terminal_codec::TerminalPublication::LayoutPolicyInvocation => {
                        self.module.publish_invocation().map(|publication| {
                            (
                                terminal_codec::TerminalProgramProduct::Invocation(
                                    publication.artifact,
                                ),
                                publication.seals,
                            )
                        })
                    }
                };
                published.map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "no product route yet: accepted Terminal publication failed: {error:?}"
                    ))]
                })
            })?;
        let source_selections = crate::production::TerminalSourceSelections::new(
            program.view(),
            self.selections.questions,
            self.selections.invocation_consent,
        );
        let provenance = self.provenance;
        let companions = self.companions;
        let applications = applications
            .iter()
            .map(|application| {
                application.attach(
                    program.view(),
                    program.portable(),
                    &provenance,
                    &mut crate::production::ApplicationReceiptObserver(observer),
                )
            })
            .collect();
        crate::production::build_bindings::validate_companions(
            program.view(),
            &companions.bindings,
            &companions.queries,
            &companions.requests,
            &companions.filesystem,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let build_bindings = crate::production::TerminalBuildBindings::new(
            program.view(),
            companions.bindings,
            companions.declarations,
            companions.queries,
            companions.native_formations,
        );
        let build_requests =
            crate::production::TerminalBuildRequests::new(program.view(), companions.requests);
        let build_filesystem =
            crate::production::TerminalBuildFilesystem::new(program.view(), companions.filesystem);
        Ok(crate::production::ProducedTerminalProgram {
            program,
            evidence: Some(Arc::new(evidence)),
            build_bindings,
            build_requests,
            build_filesystem,
            applications,
            provenance,
            source_records: self.source_records,
            source_selections: Some(Arc::new(source_selections)),
        })
    }
}

/// One constant the declarations of an accepted module name.
///
/// `declaration` is where the constant is declared, kept as source so a
/// frontier typed later can find the same declaration; `None` when the
/// constant has no authored source to rejoin. `value` is what the judged
/// source recorded for it, `None` when no stage had evaluated it. `name` is
/// for messages only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantRead {
    pub name: String,
    pub declaration: Option<source_admission::RetainedDeclaration>,
    pub value: Option<language_core::constant_value::EvaluatedConstant>,
}

/// One boundary requirement an accepted module declares.
///
/// `boundary` is the callable the module declares for it, `requirement` the
/// source declaration of that callable and `slot` the boundary trait that
/// owns the requirement: the two declarations a provider selection can name.
/// `name` is the authored `Trait::requirement` spelling, for messages only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredBoundaryRequirement {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub requirement: symbols::SymbolHandle,
    pub slot: symbols::SymbolHandle,
    pub name: String,
}

impl AcceptedTerminalSource {
    pub fn judge(
        source: Arc<FrozenTypedTrees>,
        compilation: crate::lowering::Compilation<'_>,
        publication: terminal_codec::TerminalPublication,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut prepared = crate::lowering::prepare_compilation(&source, compilation, publication)?;
        let verdicts = crate::checking::judge_compilation(prepared.program);
        let forms = verdicts.len();
        let accepted = accept_compilation(verdicts)?;
        if forms == 0
            || prepared.source.forms.len() != forms
            || prepared.source.forms.iter().any(Option::is_none)
        {
            return Err(vec![route_defect(
                "accepted forms lost their exact source correspondence",
            )]);
        }
        let mut requirement_reviews: Vec<_> = prepared
            .source
            .forms
            .iter_mut()
            .flatten()
            .flat_map(|form| std::mem::take(&mut form.review_requirements))
            .collect();
        requirement_reviews
            .sort_by_key(|row| (row.declaration.arena_index(), row.declaration.generation()));
        let AcceptedCompilation {
            closure,
            additional,
        } = accepted;
        Ok(Self {
            source,
            closure,
            correspondence: prepared.source,
            requirement_reviews,
            publication,
            package_review_numeric: Default::default(),
            advisories: DeferredAdvisories::new(additional),
        })
    }

    /// Preserve every emitted site of this exact typed call and source member.
    /// The canonical caller is part of the coordinate because specializations
    /// can reuse an authored expression while emitting distinct invocations.
    pub fn source_call_sites(
        &self,
        declaration: symbols::SymbolHandle,
        expression: crate::source_state::expression::ExpressionHandle,
    ) -> impl Iterator<
        Item = (
            semantic_vocabulary::MachineId,
            semantic_vocabulary::InvocationSite,
        ),
    > + '_ {
        self.correspondence().calls.iter().filter_map(move |call| {
            (call.source == crate::source_state::SourceNode::Expression(expression)
                && call.owner == declaration
                && self.closure.elaboration().machines.get(call.source_member)
                    == Some(&declaration))
            .then_some((call.machine, call.site))
        })
    }

    pub fn emitted_source_calls(&self) -> &[crate::source_state::SourceCallCorrespondence] {
        &self.correspondence().calls
    }

    /// The advisory diagnostics of the accepted compilation: warnings and
    /// their notes, none of which is an error. They are data on the
    /// accepted judgment for whoever reports the compilation; nothing here
    /// prints them and no later stage reads them.
    pub fn advisories(&self) -> &[Diagnostic] {
        self.advisories.get(self.closure.as_ref())
    }

    /// Capture the selections and source coordinates while their typed owner is
    /// available. Publishing the resulting module no longer reads that owner.
    pub fn prepare_publication(
        &self,
        invocation_sources: Option<&dyn crate::source_inputs::InvocationSourceConsent>,
    ) -> Result<PreparedTerminalPublication, Vec<Diagnostic>> {
        let module = self.closure.prepare_publication().map_err(|error| {
            vec![Diagnostic::error(format!(
                "no product route yet: accepted Terminal publication failed: {error:?}"
            ))]
        })?;
        let elaboration = self.closure.elaboration();
        let selections = crate::production::source_selection_publication::prepare(
            self,
            module.module(),
            invocation_sources,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        let mut provenance = terminal_psi::TerminalMachineProvenance::default();
        for (machine, symbol) in elaboration.machines.iter() {
            let name = elaboration.names.get(machine).ok_or_else(|| {
                vec![Diagnostic::error(
                    "no product route yet: accepted machine lost its source name correspondence",
                )]
            })?;
            let span = self
                .source
                .symbols
                .symbol_source_span(self.source.symbols.authored_origin(*symbol));
            provenance.name(machine, name.machine.clone(), span);
        }
        // A declaration module has no entry: every machine keeps its name,
        // and there is no call chain from a root to record.
        let provenance = match self.entry() {
            Some(entry) => provenance.for_root(&elaboration.module, entry, None),
            None => provenance,
        };
        let companions = crate::production::build_companions::produce(self, module.module())
            .map_err(|diagnostic| vec![diagnostic])?;
        Ok(PreparedTerminalPublication {
            kind: self.publication,
            module,
            selections,
            provenance,
            companions,
            source_records: self.correspondence().records.clone(),
        })
    }

    pub fn produce(
        &self,
        custody: crate::production::TerminalProductionCustody<'_>,
        observer: &mut impl crate::production::PsiCompilationObserver,
    ) -> Result<crate::production::ProducedTerminalProgram, Vec<Diagnostic>> {
        self.prepare_publication(custody.invocation_sources)?
            .publish(custody.applications, observer)
    }

    /// What this accepted Build program asks of its host, read from the
    /// same rows `produce` publishes.
    pub fn build_protocol_use(
        &self,
    ) -> Result<crate::production::BuildProtocolUse, Vec<Diagnostic>> {
        let companions =
            crate::production::build_companions::produce(self, &self.closure.elaboration().module)
                .map_err(|diagnostic| vec![diagnostic])?;
        Ok(crate::production::BuildProtocolUse {
            reaches_outside: companions.unowned != 0,
            filesystem: !companions.filesystem.is_empty(),
        })
    }

    /// The constants the declarations of this accepted module name, each
    /// declaration once and in the order the declarations name them.
    ///
    /// The declarations are the ones the elaboration lowered: its machines,
    /// its boundary requirements and the records it declares. They are read
    /// in the judged source, so the answer is about this artifact and does
    /// not rest on the reader having refused constants on the way here. A
    /// Build admission settles nothing for a module that names none.
    pub fn constants_read(&self) -> Vec<ConstantRead> {
        let elaboration = self.closure.elaboration();
        let key = |symbol: symbols::SymbolHandle| (symbol.arena_index(), symbol.generation());
        let mut lowered = elaboration
            .machines
            .iter()
            .map(|(_, symbol)| *symbol)
            .chain(elaboration.boundaries.iter().map(|(_, symbol)| *symbol))
            .chain(
                self.correspondence()
                    .records
                    .iter()
                    .map(|record| record.declaration),
            )
            .collect::<Vec<_>>();
        lowered.sort_unstable_by_key(|symbol| key(*symbol));
        let constants = self
            .source
            .const_declarations()
            .iter()
            .map(|declaration| declaration.symbol)
            .collect();
        let mut uses =
            crate::source_state::references::ConstantUses::new(constants, |declaration| {
                lowered
                    .binary_search_by_key(&key(declaration), |symbol| key(*symbol))
                    .is_ok()
            });
        crate::source_state::references::visit_program(&self.source, &mut uses);
        let mut read = Vec::<symbols::SymbolHandle>::new();
        for found in uses.into_uses() {
            if !read.contains(&found.constant) {
                read.push(found.constant);
            }
        }
        read.into_iter()
            .map(|constant| ConstantRead {
                name: self.source.symbols.name(constant).to_owned(),
                declaration: crate::source_symbols::retain_declaration(
                    &self.source.symbols,
                    constant,
                )
                .ok(),
                value: self
                    .source
                    .const_declaration_by_symbol(constant)
                    .and_then(|declaration| declaration.evaluated_value.clone()),
            })
            .collect()
    }

    /// Exact lowered boundary declarations whose source owns an installation bound.
    pub fn installation_reach_requirements(&self) -> Vec<semantic_vocabulary::BoundaryMachineId> {
        self.module().installation_reach_requirements()
    }

    /// The boundary requirements this accepted module declares, in the
    /// order the module declares them.
    ///
    /// The module declares a boundary callable when one of its machines
    /// calls one, and for every requirement a chosen provider supplies. Its
    /// machines are the closure of every root of the compilation: the
    /// entries, the subject's declarations and the providers chosen for
    /// every target. So a declared requirement is not yet a demand on any
    /// one target: which of them a target must supply is decided by
    /// following its entries through the providers that target chose
    /// (`supplied_requirements`, and `target_supply`).
    /// Each row is rejoined to the source declaration the elaboration
    /// lowered it from.
    pub fn boundary_requirements_declared(
        &self,
    ) -> Result<Vec<DeclaredBoundaryRequirement>, Diagnostic> {
        let elaboration = self.closure.elaboration();
        let lost = |detail: &str| {
            Diagnostic::error(format!(
                "cannot determine executable boundary supply: {detail}"
            ))
        };
        if elaboration.module.boundary_machines.len() != elaboration.boundaries.len() {
            return Err(lost(
                "differ from the requirements the module was lowered from",
            ));
        }
        let mut declared = Vec::with_capacity(elaboration.boundaries.len());
        for (boundary, symbol) in elaboration.boundaries.iter() {
            if elaboration.module.boundary_machine(boundary).is_none() {
                return Err(lost("omit a requirement the module was lowered from"));
            }
            let declaration = elaboration
                .module
                .boundary_machine(boundary)
                .ok_or_else(|| lost("omit a retained boundary"))?;
            if terminal_semantics::closed_conformances::hosted_primitive_requirement(
                &elaboration.module,
                declaration,
            )
            .map_err(lost)?
            .is_some()
            {
                continue;
            }
            if declaration.declaration.as_ref().is_some_and(|checked| {
                checked.execution == terminal_psi::CheckedBoundaryExecution::NumericPrimitive
            }) {
                terminal_semantics::numeric_primitives::validate_boundary(
                    &elaboration.module,
                    declaration,
                )
                .map_err(|reason| {
                    lost(&format!(
                        "{reason}: {}",
                        declaration
                            .declaration
                            .as_ref()
                            .and_then(|checked| elaboration
                                .module
                                .identities
                                .text(checked.source.semantic_declaration))
                            .unwrap_or("missing declaration identity")
                    ))
                })?;
                continue;
            }
            let (owner, requirement) = self
                .source
                .trait_machine_signature(*symbol)
                .filter(|(owner, requirement)| owner.is_boundary && requirement.symbol == *symbol)
                .ok_or_else(|| lost("lost the boundary trait that owns a requirement"))?;
            declared.push(DeclaredBoundaryRequirement {
                boundary,
                requirement: *symbol,
                slot: owner.symbol,
                name: format!("{}::{}", owner.name.as_str(), requirement.name.as_str()),
            });
        }
        Ok(declared)
    }

    /// Follow the entry and its demanded scalar evaluations through actual
    /// invocations and exact selected code values. Retained headers are not roots.
    pub fn boundary_calls_reached(
        &self,
        entry: semantic_vocabulary::MachineId,
        implementations: &[crate::production::CallableBoundaryImplementation],
        target: Option<&str>,
    ) -> Result<Vec<semantic_vocabulary::BoundaryMachineId>, Diagnostic> {
        Ok(self
            .boundary_call_reach(entry, implementations, target)?
            .boundaries)
    }

    /// Preserve the actual invoked-machine summary alongside boundary demand.
    /// Both projections include the same demanded scalar evaluations and exact
    /// selected implementations; a retained body alone contributes neither.
    pub fn boundary_call_reach(
        &self,
        entry: semantic_vocabulary::MachineId,
        implementations: &[crate::production::CallableBoundaryImplementation],
        target: Option<&str>,
    ) -> Result<terminal_semantics::callable_values::ReachedBoundaryCalls, Diagnostic> {
        terminal_installation::logical_reach::boundary_call_reach(
            self.module(),
            entry,
            implementations,
            target,
        )
    }

    /// The supply a Build names as the backing of a provider readiness
    /// request: `declaration_path` is a public implementation of `package`,
    /// and the supply is that implementation for the boundary trait it
    /// implements. provider_selection.md, "Build provider readiness": the
    /// backing is an exact named implementation of the awaited requirement
    /// in the registered dependency. Nothing is judged here; a source judged
    /// with this supply in its roster holds the provider's machines
    /// (`produce_build_provider`).
    pub fn build_backing_supply(
        source: &FrozenTypedTrees,
        package: semantic_vocabulary::PackageKeyIdentity,
        declaration_path: &str,
    ) -> Result<crate::lowering::Supply, Vec<Diagnostic>> {
        let selected = build_backing(source, package, declaration_path)?;
        Ok(crate::lowering::Supply {
            slot: selected.trait_symbol,
            provider: selected.symbol,
        })
    }

    /// Publishes this accepted source as the provider `declaration_path`
    /// of `package` for a Build host to install. The source must have been
    /// judged with the backing's supply (`build_backing_supply`), so the
    /// module holds a machine for each requirement the implementation
    /// supplies and states it as a provider candidate. The identities are
    /// the ones the awaiting Build's own module states for the same
    /// declarations: the implementation and its trait by exact owner, and
    /// the trait's application.
    pub fn produce_build_provider(
        &self,
        package: semantic_vocabulary::PackageKeyIdentity,
        declaration_path: &str,
    ) -> Result<crate::production::ProducedBuildProvider, Vec<Diagnostic>> {
        let refuse = |reason: &str| vec![Diagnostic::error(reason)];
        let source = &*self.source;
        let selected = build_backing(source, package, declaration_path)?;
        let scope = crate::source_state::trait_definition::AppliedTraitScope::from_conformance(
            source, selected,
        )
        .ok_or_else(|| refuse("Build backing has no complete checked requirement application"))?;
        let sources = source_admission::source_identity::toolchain_source_identities(
            source.symbols.source_files(),
        )
        .map_err(|_| refuse("Build backing has a toolchain source with no exact identity"))?;
        let nominal = |symbol| {
            source
                .exact_owner_nominal_identity(symbol, &sources)
                .and_then(|identity| source.type_identity_text(identity))
                .filter(|text| !text.is_empty())
                .ok_or_else(|| refuse("Build provider declaration lost its exact portable owner"))
        };
        let declaration = nominal(selected.symbol)?;
        let requirement = nominal(scope.declaration.symbol)?;
        let application = scope
            .portable_application_identity(source)
            .and_then(|identity| source.type_identity_text(identity))
            .filter(|text| !text.is_empty())
            .ok_or_else(|| {
                refuse("Build backing requirement application is not closed and portable")
            })?;
        let supplied = self
            .supplied_requirements()
            .iter()
            .any(|row| row.provider == selected.symbol);
        if !supplied && !selected.rows.is_empty() {
            return Err(refuse(
                "Build backing was not judged as a supply of its dependency's source",
            ));
        }
        let code = if supplied {
            let publication = self.closure.publish().map_err(|error| {
                vec![Diagnostic::error(format!(
                    "Build provider publication: {error:?}"
                ))]
            })?;
            Some(Arc::new(publication.artifact))
        } else {
            None
        };
        Ok(crate::production::ProducedBuildProvider::new(
            code,
            declaration_path.to_owned(),
            package,
            declaration,
            requirement,
            application,
        ))
    }

    /// The machine of this module each chosen provider gives each
    /// requirement of the slot it supplies.
    pub fn supplied_requirements(&self) -> &[crate::lowering::SuppliedRequirement] {
        &self.closure.elaboration().supplied
    }

    /// The provider schema of the boundary trait `slot`: every requirement a
    /// provider of the slot must supply, with the identities a provider
    /// plan binds its rows by. provider_selection.md: "The assembled record
    /// must satisfy the slot's complete schema". It is projected from the
    /// slot's declaration in the judged source, not from the requirements
    /// this module happens to declare, so a plan missing one is refused
    /// rather than shrunk to fit. `None` when `slot` is not a boundary trait
    /// of the product.
    pub fn slot_schema(
        &self,
        slot: symbols::SymbolHandle,
    ) -> Option<Arc<declaration_contracts::service_surface::ServiceSurface>> {
        let declaration = self.source.trait_definition_by_symbol(slot)?;
        crate::source_state::service_declarations::projection::from_typed(
            crate::source_state::provider_declarations::ProviderPlanningContext::new(
                &self.source,
                declaration_contracts::source_identity::DependencyScope::Product,
            ),
            declaration,
        )
        .ok()
        .map(|projected| projected.declaration)
    }

    /// The machine of this module that elaborates the source machine
    /// `declaration`, when the module holds it.
    pub fn machine_of(
        &self,
        declaration: symbols::SymbolHandle,
    ) -> Option<semantic_vocabulary::MachineId> {
        self.closure
            .elaboration()
            .machines
            .iter()
            .find_map(|(machine, symbol)| (*symbol == declaration).then_some(machine))
    }

    /// The module the route judged and accepted, as publication emits it.
    pub fn module(&self) -> &terminal_psi::TerminalModule {
        &self.closure.elaboration().module
    }

    pub fn source(&self) -> &Arc<FrozenTypedTrees> {
        &self.source
    }

    pub fn closure(&self) -> &AcceptedClosure {
        &self.closure
    }

    /// Facts from the requirement's own accepted declaration form. Equal
    /// duplicate readings agree; an applied or ambiguous account refuses.
    pub fn requirement_review(
        &self,
        declaration: symbols::SymbolHandle,
    ) -> Result<&crate::production::RequirementReviewMetadata, String> {
        if self.source.trait_machine_signature(declaration).is_none() {
            return Err("requirement review names a foreign declaration".to_owned());
        }
        let key = (declaration.arena_index(), declaration.generation());
        let start = self.requirement_reviews.partition_point(|row| {
            (row.declaration.arena_index(), row.declaration.generation()) < key
        });
        let end = self.requirement_reviews.partition_point(|row| {
            (row.declaration.arena_index(), row.declaration.generation()) <= key
        });
        let rows = &self.requirement_reviews[start..end];
        let first = rows
            .first()
            .ok_or_else(|| "public requirement has no accepted declaration metadata".to_owned())?;
        if rows[1..].iter().any(|row| row.metadata != first.metadata) {
            return Err("public requirement has competing applied declaration metadata".to_owned());
        }
        first.metadata.as_ref().map_err(Clone::clone)
    }

    /// Source record associations for the accepted executable form.
    pub fn source_records(&self) -> &[crate::source_state::SourceRecordCorrespondence] {
        &self.correspondence().records
    }

    pub(crate) fn correspondence(&self) -> &crate::lowering::SourceFormCorrespondence {
        self.correspondence.forms[0]
            .as_ref()
            .expect("judge retained every accepted source form")
    }

    pub(crate) fn selection_forms(
        &self,
    ) -> impl ExactSizeIterator<
        Item = Option<&crate::lowering::selection_questions::RawSelectionQuestions>,
    > {
        self.correspondence.forms.iter().map(|form| {
            form.as_ref()
                .and_then(|form| form.selection_questions.as_ref())
        })
    }

    pub const fn publication(&self) -> terminal_codec::TerminalPublication {
        self.publication
    }

    pub fn entry(&self) -> Option<semantic_vocabulary::MachineId> {
        match self.closure.elaboration().module.entry {
            Some(terminal_psi::TargetMachineCallable::Machine(machine)) => Some(machine),
            _ => None,
        }
    }

    pub fn is_invocation(&self) -> bool {
        self.publication != terminal_codec::TerminalPublication::Portable
    }
}

/// The advisories of every accepted part: the entry closure and each
/// generic check form. One authored machine may be judged in more than one
/// part, and its advisory is the same in each, so an advisory already listed
/// is not listed again.
fn advisories<'part>(parts: impl Iterator<Item = &'part AcceptedClosure>) -> Vec<Diagnostic> {
    let mut advisories: Vec<Diagnostic> = Vec::new();
    for accepted in parts {
        // A warning and the note after it are one advisory.
        let part = accepted.advisories();
        let mut rest = &part[..];
        while !rest.is_empty() {
            let length = 1 + rest[1..]
                .iter()
                .take_while(|diagnostic| {
                    diagnostic.severity == diagnostics::DiagnosticSeverity::Note
                })
                .count();
            let (advisory, after) = rest.split_at(length);
            if !advisories.windows(length).any(|listed| listed == advisory) {
                advisories.extend_from_slice(advisory);
            }
            rest = after;
        }
    }
    advisories
}

/// All semantic verdicts accepted. Additional parts supply only their
/// diagnostics after the source correspondence has been checked.
#[derive(Debug)]
struct AcceptedCompilation {
    closure: Box<AcceptedClosure>,
    additional: Vec<Box<AcceptedClosure>>,
}

/// Reporters request the same warnings by default. Transient source jobs use
/// the accepted code without reporting it, so they must not start advisory
/// proof searches just to discard the diagnostics. Keep every accepted part
/// until the first read, then release the additional forms after aggregation.
struct DeferredAdvisories {
    additional: Mutex<Vec<Box<AcceptedClosure>>>,
    cached: OnceLock<Vec<Diagnostic>>,
}

impl std::fmt::Debug for DeferredAdvisories {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeferredAdvisories")
            .field("materialized", &self.cached.get().is_some())
            .finish_non_exhaustive()
    }
}

impl DeferredAdvisories {
    fn new(additional: Vec<Box<AcceptedClosure>>) -> Self {
        Self {
            additional: Mutex::new(additional),
            cached: OnceLock::new(),
        }
    }

    fn get(&self, primary: &AcceptedClosure) -> &[Diagnostic] {
        self.cached.get_or_init(|| {
            let mut additional = self
                .additional
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Retain the parts if a diagnostic search panics, just as each
            // accepted closure retains its own deferred warning inputs.
            let result = advisories(
                std::iter::once(primary).chain(additional.iter().map(|part| part.as_ref())),
            );
            additional.clear();
            result
        })
    }
}

fn accept_compilation(
    verdicts: Vec<Result<RouteVerdict, CompilationRefusal>>,
) -> Result<AcceptedCompilation, Vec<Diagnostic>> {
    let mut first = None;
    let mut additional = Vec::new();
    let mut diagnostics = Vec::new();
    for (position, verdict) in verdicts.into_iter().enumerate() {
        match verdict {
            Ok(RouteVerdict::Accepted(accepted)) => {
                if position == 0 {
                    first = Some(accepted);
                } else {
                    additional.push(accepted);
                }
            }
            Ok(RouteVerdict::Rejected {
                formations,
                violations,
                unproved,
                unestablished,
                conflicts,
                exceeded,
            }) => {
                let before = diagnostics.len();
                diagnostics.extend(formations.iter().map(|entry| entry.reject_diagnostic()));
                diagnostics.extend(violations.iter().map(|entry| entry.reject_diagnostic()));
                for entry in unproved.iter().filter(|entry| !entry.cause.is_incomplete()) {
                    diagnostics.push(entry.reject_diagnostic());
                    diagnostics.extend(entry.opening_diagnostic());
                }
                diagnostics.extend(unestablished.iter().map(|entry| entry.reject_diagnostic()));
                diagnostics.extend(conflicts.iter().map(|entry| entry.reject_diagnostic()));
                diagnostics.extend(exceeded.iter().map(|entry| entry.reject_diagnostic()));
                if before == diagnostics.len() {
                    diagnostics.push(route_defect("a rejection names no failure"));
                }
            }
            Ok(RouteVerdict::Incomplete(unproved)) => {
                if unproved.is_empty() {
                    diagnostics.push(route_defect("an incomplete verdict names no goal"));
                }
                diagnostics.extend(unproved.iter().map(|entry| {
                    let mut diagnostic = entry.reject_diagnostic();
                    diagnostic.message = format!(
                        "not supported yet: {}; the checker cannot prove a goal of this form yet, so the program is refused, with no claim that it is wrong",
                        diagnostic.message
                    );
                    diagnostic
                }));
            }
            Ok(RouteVerdict::FallBack(refused)) => {
                let mut diagnostic = Diagnostic::error(format!(
                    "not supported yet: {}; the checker does not support this construct yet, so the program is refused, with no claim that it is wrong",
                    refused.detail
                ));
                diagnostic.source_span = refused.source_span;
                diagnostics.push(diagnostic);
            }
            Err(defect) => diagnostics.push(route_defect(&format!("{defect:?}"))),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    let closure = first.ok_or_else(|| vec![route_defect("no accepted compilation root")])?;
    // Classification requests no warnings. The source owner finishes its
    // correspondence checks before consuming these transient parts.
    Ok(AcceptedCompilation {
        closure,
        additional,
    })
}

fn route_defect(detail: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "not supported yet: the checker could not judge this program ({detail}); this is a defect of the checker, and the program is refused rather than accepted without a verdict"
    ))
}

/// The one public implementation of `package` that a Build backing path
/// names: a closed implementation of a boundary trait.
fn build_backing<'source>(
    source: &'source FrozenTypedTrees,
    package: semantic_vocabulary::PackageKeyIdentity,
    declaration_path: &str,
) -> Result<&'source crate::source_state::trait_definition::Conformance, Vec<Diagnostic>> {
    let refuse = |reason: &str| vec![Diagnostic::error(reason)];
    let mut candidates = source.conformances().iter().filter(|conformance| {
        conformance.is_public
            && source
                .symbols
                .symbol_product_package_identity(conformance.symbol)
                == Some(package)
            && source.symbols.display_path(conformance.symbol, "::") == declaration_path
    });
    let selected = candidates.next().ok_or_else(|| {
        refuse("Build backing names no public implementation in its exact dependency package")
    })?;
    if candidates.next().is_some() {
        return Err(refuse(
            "Build backing declaration is ambiguous in its dependency package",
        ));
    }
    let boundary = source
        .trait_definition_by_symbol(selected.trait_symbol)
        .is_some_and(|declaration| declaration.is_boundary);
    if !boundary || !selected.type_parameters.is_empty() || !selected.lifetime_parameters.is_empty()
    {
        return Err(refuse(
            "Build backing must name a closed implementation of a boundary requirement",
        ));
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lowering::Compilation;

    #[test]
    fn publication_survives_releasing_the_typed_source_and_checker() {
        let accepted = contract_fixture(
            "specialized_branch_selection_quiet_exit",
            terminal_codec::TerminalPublication::Portable,
        );
        let expected = terminal_codec::terminal_psi_identity(accepted.module()).unwrap();
        let owner = Arc::downgrade(accepted.source());
        let prepared = accepted.prepare_publication(None).unwrap();
        drop(accepted);
        assert!(owner.upgrade().is_none());
        let published = prepared
            .publish(&[], &mut crate::production::UnmeasuredProduction)
            .unwrap()
            .into_shared_portable()
            .unwrap();
        assert_eq!(published.artifact().view().identity(), expected);
    }

    fn contract_fixture(
        name: &str,
        publication: terminal_codec::TerminalPublication,
    ) -> AcceptedTerminalSource {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../tests/fixtures/pass/contracts")
            .join(name)
            .join("main.omg");
        let text = std::fs::read_to_string(path).unwrap();
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0xa7; 32]).unwrap();
        let mut sources = crate::source_inputs::SourceMap::default();
        let id = sources
            .add_checked_instance(
                format!("accepted/{name}/main.omg").into(),
                text.as_str(),
                "accepted".into(),
                Some(package),
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                declaration_contracts::source_identity::DependencyScope::Product,
            )
            .source_id;
        let tokens = crate::production::frontend_fixture::Lexer::new(&text)
            .tokenize()
            .unwrap();
        let syntax =
            crate::production::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
        let resolved = crate::resolution::resolve(crate::resolution::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        let entry = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "main")
            .unwrap()
            .symbol;
        let (frozen, _) = crate::preparation::prepare_typed_trees(
            typed,
            &crate::preparation::PreparationRequest::settled(),
        )
        .unwrap();
        AcceptedTerminalSource::judge(
            Arc::new(frozen),
            Compilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[Some(package)],
                purpose: declaration_contracts::source_identity::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                calling: None,
                providers: None,
                supplies: &[],
            },
            publication,
        )
        .unwrap()
    }

    #[test]
    fn authored_advisories_wait_for_read_and_release_additional_forms_once() {
        // This kept CLI fixture has one authored contradiction and two real
        // specializations. Its extra check forms must not multiply warnings.
        let accepted = contract_fixture(
            "generic_branch_contract_contradiction_exit",
            terminal_codec::TerminalPublication::Invocation,
        );
        assert!(accepted.advisories.cached.get().is_none());
        assert!(!accepted.advisories.additional.lock().unwrap().is_empty());
        let diagnostics = accepted.advisories();
        let warnings: Vec<_> = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == diagnostics::DiagnosticSeverity::Warning
                    && diagnostic.message.starts_with("unreachable branch:")
            })
            .collect();
        assert_eq!(warnings.len(), 1, "{diagnostics:?}");
        assert!(warnings[0].message.contains("machine `pick`"));
        assert!(warnings[0].source_span.is_some());
        assert!(accepted.advisories.additional.lock().unwrap().is_empty());
        assert!(std::ptr::eq(
            diagnostics.as_ptr(),
            accepted.advisories().as_ptr()
        ));
        assert_eq!(accepted.advisories(), diagnostics);
    }

    #[test]
    fn specialization_only_advisories_remain_quiet_after_a_lazy_read() {
        let accepted = contract_fixture(
            "specialized_branch_selection_quiet_exit",
            terminal_codec::TerminalPublication::Invocation,
        );
        assert!(accepted.advisories.cached.get().is_none());
        assert!(accepted.advisories().is_empty());
        assert!(accepted.advisories.cached.get().is_some());
        assert!(accepted.advisories.additional.lock().unwrap().is_empty());
        assert!(accepted.advisories().is_empty());
    }

    fn elaboration(
        publication: terminal_codec::TerminalPublication,
    ) -> crate::lowering::Elaboration {
        const TEXT: &str = "machine entry() crashes none {}";
        let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0xa7; 32]).unwrap();
        let mut sources = crate::source_inputs::SourceMap::default();
        let source = sources
            .add_checked_instance(
                "accepted/source.omg".into(),
                TEXT,
                "accepted".into(),
                Some(package),
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                declaration_contracts::source_identity::DependencyScope::Product,
            )
            .source_id;
        let tokens = crate::production::frontend_fixture::Lexer::new(TEXT)
            .tokenize()
            .unwrap();
        let syntax =
            crate::production::frontend_fixture::parse_syntax_trees_with_id(source, &tokens)
                .unwrap();
        let resolved = crate::resolution::resolve(crate::resolution::ResolutionRequest {
            syntax,
            sources: Some(Arc::new(sources)),
            top_level_bindings: Vec::new(),
            retained_base: None,
        })
        .unwrap();
        let typed = crate::typing::lower_symbol_resolved_trees(&resolved).unwrap();
        let machines = typed.machines();
        assert_eq!(
            machines.len(),
            1,
            "publication fixture must retain its one actual source machine"
        );
        let entry = machines.at(0);
        crate::lowering::elaborate_for(
            &typed,
            Compilation {
                declaration_metadata: &[],
                entries: &[entry.symbol],
                subjects: &[],
                purpose: declaration_contracts::source_identity::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                calling: None,
                providers: None,
                supplies: &[],
            },
            publication,
        )
        .unwrap()
    }

    fn accepted() -> RouteVerdict {
        crate::checking::judge(Ok(elaboration(
            terminal_codec::TerminalPublication::Portable,
        )))
        .unwrap()
    }

    #[test]
    fn invocation_publication_keeps_the_judged_module_and_scope() {
        let stated = elaboration(terminal_codec::TerminalPublication::Invocation);
        let closure = accept_compilation(vec![
            crate::checking::judge(Ok(stated)).map_err(CompilationRefusal::Elaboration),
        ])
        .unwrap()
        .closure;
        let published = closure.publish_invocation().unwrap();
        assert_eq!(
            published.artifact.view().module(),
            &closure.elaboration().module
        );
        assert_eq!(published.seals.module, published.artifact.view().identity());
        assert_eq!(published.seals.seals, closure.seals());
        assert!(closure.publish().is_err());
        let portable = accept_compilation(vec![Ok(accepted())]).unwrap().closure;
        assert!(portable.publish_invocation().is_err());
    }

    #[test]
    fn later_undecided_part_cannot_publish_the_accepted_primary_closure() {
        assert!(matches!(accepted(), RouteVerdict::Accepted(_)));
        let diagnostics = accept_compilation(vec![
            Ok(accepted()),
            Ok(RouteVerdict::Incomplete(Vec::new())),
        ])
        .unwrap_err();
        assert!(diagnostics.iter().any(|entry| {
            entry
                .message
                .contains("an incomplete verdict names no goal")
        }));
        assert!(accept_compilation(Vec::new()).is_err());
    }
}
