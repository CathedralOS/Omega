//! The exact typed owner and the first accepted closure of its whole
//! compilation. Source is retained before judgement; another source cannot
//! be attached to an artifact after the route has decided it.

use diagnostics::Diagnostic;
use std::sync::Arc;
use terminal_psi_to_terminal_psi::{AcceptedClosure, CompilationRefusal, RouteVerdict};
use typed_trees::FrozenTypedTrees;

#[derive(Debug)]
pub struct AcceptedTerminalSource {
    source: Arc<FrozenTypedTrees>,
    closure: Box<AcceptedClosure>,
    correspondence: typed_trees_to_terminal_psi::SourceCorrespondence,
    publication: terminal_codec::TerminalPublication,
}

/// One boundary requirement a machine of an accepted module calls.
///
/// `boundary` is the callable the module declares for it, `requirement` the
/// source declaration of that callable and `slot` the boundary trait that
/// owns the requirement: the two declarations a provider selection can name.
/// `name` is the authored `Trait::requirement` spelling, for messages only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReachedBoundaryRequirement {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub requirement: symbols::SymbolHandle,
    pub slot: symbols::SymbolHandle,
    pub name: String,
}

impl AcceptedTerminalSource {
    pub fn judge(
        source: Arc<FrozenTypedTrees>,
        compilation: typed_trees_to_terminal_psi::Compilation<'_>,
        publication: terminal_codec::TerminalPublication,
    ) -> Result<Self, Vec<Diagnostic>> {
        let prepared =
            typed_trees_to_terminal_psi::prepare_compilation(&source, compilation, publication);
        let verdicts = terminal_psi_to_terminal_psi::judge_compilation(prepared.program);
        let closure = accept_compilation(verdicts)?;
        Ok(Self {
            source,
            closure,
            correspondence: prepared.source,
            publication,
        })
    }

    /// Binds source companions to this exact accepted code owner. The Build
    /// companions are read from the accepted elaboration (`build_companions`).
    pub fn produce(
        &self,
        custody: crate::TerminalProductionCustody<'_>,
        observer: &mut impl crate::PsiCompilationObserver,
    ) -> Result<crate::ProducedTerminalProgram, Vec<Diagnostic>> {
        let (program, evidence) =
            observer.record_result(crate::PsiCompilationStage::Publication, || {
                let published = match self.publication {
                    terminal_codec::TerminalPublication::Portable => {
                        self.closure.publish().map(|publication| {
                            (
                                terminal_codec::TerminalProgramProduct::Portable(
                                    publication.artifact,
                                ),
                                publication.seals,
                            )
                        })
                    }
                    terminal_codec::TerminalPublication::Invocation => {
                        self.closure.publish_invocation().map(|publication| {
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
        let elaboration = &self.closure.elaboration;
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
        let entry = self.entry().ok_or_else(|| {
            vec![Diagnostic::error(
                "no product route yet: accepted source module has no machine entry",
            )]
        })?;
        let provenance = provenance.for_root(&elaboration.module, entry, None);
        let applications = custody
            .applications
            .iter()
            .map(|application| {
                application.attach(program.view(), program.portable(), &provenance, observer)
            })
            .collect();
        // The published module is the elaboration's module, so the rows
        // name the artifact's own operations and original source formations.
        let companions =
            crate::build_companions::produce(&self.source, elaboration, &self.correspondence)
                .map_err(|diagnostic| vec![diagnostic])?;
        crate::build_bindings::validate_companions(
            program.view(),
            &companions.bindings,
            &companions.queries,
            &companions.requests,
            &companions.filesystem,
        )
        .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let build_bindings = crate::TerminalBuildBindings::new(
            program.view(),
            companions.bindings,
            companions.declarations,
            companions.queries,
            companions.native_formations,
        );
        let build_requests = crate::TerminalBuildRequests::new(program.view(), companions.requests);
        let build_filesystem =
            crate::TerminalBuildFilesystem::new(program.view(), companions.filesystem);
        Ok(crate::ProducedTerminalProgram {
            program,
            evidence: Some(Arc::new(evidence)),
            build_bindings,
            build_requests,
            build_filesystem,
            applications,
            provenance,
            source_records: self.correspondence.records.clone(),
        })
    }

    /// What this accepted Build program asks of its host, read from the
    /// same rows `produce` publishes.
    pub fn build_protocol_use(&self) -> Result<crate::BuildProtocolUse, Vec<Diagnostic>> {
        let companions = crate::build_companions::produce(
            &self.source,
            &self.closure.elaboration,
            &self.correspondence,
        )
        .map_err(|diagnostic| vec![diagnostic])?;
        Ok(crate::BuildProtocolUse {
            reaches_outside: companions.unowned != 0,
            filesystem: !companions.filesystem.is_empty(),
        })
    }

    /// The constants the declarations of this accepted module name, by
    /// name, each once and in the order the declarations name them.
    ///
    /// The declarations are the ones the elaboration lowered: its machines,
    /// its boundary requirements and the records it declares. They are read
    /// in the judged source, so the answer is about this artifact and does
    /// not rest on the reader having refused constants on the way here. A
    /// Build admission settles nothing for a module that names none.
    pub fn constants_read(&self) -> Vec<String> {
        let elaboration = &self.closure.elaboration;
        let key = |symbol: symbols::SymbolHandle| (symbol.arena_index(), symbol.generation());
        let mut lowered = elaboration
            .machines
            .iter()
            .map(|(_, symbol)| *symbol)
            .chain(elaboration.boundaries.iter().map(|(_, symbol)| *symbol))
            .chain(
                self.correspondence
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
        let mut uses = typed_trees::references::ConstantUses::new(constants, |declaration| {
            lowered
                .binary_search_by_key(&key(declaration), |symbol| key(*symbol))
                .is_ok()
        });
        typed_trees::references::visit_program(&self.source, &mut uses);
        let mut names = Vec::<String>::new();
        for found in uses.into_uses() {
            let name = self.source.symbols.name(found.constant);
            if !names.iter().any(|known| known == name) {
                names.push(name.to_owned());
            }
        }
        names
    }

    /// The boundary requirements this accepted module's machines call, in
    /// the order the module declares them.
    ///
    /// The module holds the closure of its entry and nothing else, and it
    /// declares a boundary callable exactly when a machine of that closure
    /// calls one, so these are the requirements a product of this module
    /// needs a provider for. An empty answer is a statement about the
    /// artifact: its entry reaches no boundary requirement. Each row is
    /// rejoined to the source declaration the elaboration lowered it from.
    pub fn boundary_requirements_reached(
        &self,
    ) -> Result<Vec<ReachedBoundaryRequirement>, Diagnostic> {
        let elaboration = &self.closure.elaboration;
        let lost = |detail: &str| {
            Diagnostic::error(format!(
                "no product route yet: accepted boundary declarations {detail}"
            ))
        };
        if elaboration.module.boundary_machines.len() != elaboration.boundaries.len() {
            return Err(lost(
                "differ from the requirements the module was lowered from",
            ));
        }
        let mut reached = Vec::with_capacity(elaboration.boundaries.len());
        for (boundary, symbol) in elaboration.boundaries.iter() {
            if elaboration.module.boundary_machine(boundary).is_none() {
                return Err(lost("omit a requirement the module was lowered from"));
            }
            let (owner, requirement) = self
                .source
                .trait_machine_signature(*symbol)
                .filter(|(owner, requirement)| owner.is_boundary && requirement.symbol == *symbol)
                .ok_or_else(|| lost("lost the boundary trait that owns a requirement"))?;
            reached.push(ReachedBoundaryRequirement {
                boundary,
                requirement: *symbol,
                slot: owner.symbol,
                name: format!("{}::{}", owner.name.as_str(), requirement.name.as_str()),
            });
        }
        Ok(reached)
    }

    /// The module the route judged and accepted, as publication emits it.
    pub fn module(&self) -> &terminal_psi::TerminalModule {
        &self.closure.elaboration.module
    }

    pub fn source(&self) -> &Arc<FrozenTypedTrees> {
        &self.source
    }

    pub fn closure(&self) -> &AcceptedClosure {
        &self.closure
    }

    pub const fn publication(&self) -> terminal_codec::TerminalPublication {
        self.publication
    }

    pub fn entry(&self) -> Option<semantic_vocabulary::MachineId> {
        match self.closure.elaboration.module.entry {
            Some(terminal_psi::TargetMachineCallable::Machine(machine)) => Some(machine),
            _ => None,
        }
    }

    pub fn is_invocation(&self) -> bool {
        self.publication == terminal_codec::TerminalPublication::Invocation
    }
}

fn accept_compilation(
    verdicts: Vec<Result<RouteVerdict, CompilationRefusal>>,
) -> Result<Box<AcceptedClosure>, Vec<Diagnostic>> {
    let mut first = None;
    let mut diagnostics = Vec::new();
    for (position, verdict) in verdicts.into_iter().enumerate() {
        match verdict {
            Ok(RouteVerdict::Accepted(accepted)) => {
                if position == 0 {
                    first = Some(accepted);
                }
            }
            Ok(RouteVerdict::Rejected {
                formations,
                unproved,
                unestablished,
                conflicts,
                exceeded,
            }) => {
                let before = diagnostics.len();
                diagnostics.extend(formations.iter().map(|entry| entry.reject_diagnostic()));
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
                diagnostics.push(Diagnostic::error(format!(
                    "not supported yet: {}; the checker does not support this construct yet, so the program is refused, with no claim that it is wrong",
                    refused.detail
                )));
            }
            Err(defect) => diagnostics.push(route_defect(&format!("{defect:?}"))),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    first.ok_or_else(|| vec![route_defect("no accepted compilation root")])
}

fn route_defect(detail: &str) -> Diagnostic {
    Diagnostic::error(format!(
        "not supported yet: the checker could not judge this program ({detail}); this is a defect of the checker, and the program is refused rather than accepted without a verdict"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IdTable, MachineId};
    use typed_trees_to_terminal_psi::{Body, CrashCeiling, Signature, Termination};

    fn elaboration() -> typed_trees_to_terminal_psi::Elaboration {
        let mut bodies = IdTable::default();
        bodies.push(Body {
            slots: Vec::new(),
            statements: Vec::new(),
        });
        let mut signatures = IdTable::default();
        signatures.push(Signature {
            parameters: Vec::new(),
            erased: Vec::new(),
            borrowed: Vec::new(),
            result: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            reaches: typed_trees_to_terminal_psi::Ceiling::Inferred,
            invokes: typed_trees_to_terminal_psi::Ceiling::Inferred,
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
        });
        typed_trees_to_terminal_psi::lower_bodies(
            MachineId::new(1).unwrap(),
            &bodies,
            &signatures,
            &IdTable::default(),
        )
        .unwrap()
    }

    fn accepted() -> RouteVerdict {
        terminal_psi_to_terminal_psi::judge(Ok(elaboration())).unwrap()
    }

    #[test]
    fn invocation_publication_keeps_the_judged_module_and_scope() {
        let mut stated = elaboration();
        // Raw test bodies declare no identities. Supply the real scope before
        // judgment; production receives it from structural lowering itself.
        stated.module.identities =
            terminal_psi::TerminalIdentityTableBuilder::for_invocation().build();
        let closure = accept_compilation(vec![
            terminal_psi_to_terminal_psi::judge(Ok(stated))
                .map_err(CompilationRefusal::Elaboration),
        ])
        .unwrap();
        let published = closure.publish_invocation().unwrap();
        assert_eq!(
            published.artifact.view().module(),
            &closure.elaboration.module
        );
        assert_eq!(published.seals.module, published.artifact.view().identity());
        assert_eq!(published.seals.seals, closure.seals);
        assert!(closure.publish().is_err());
        let portable = accept_compilation(vec![Ok(accepted())]).unwrap();
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
