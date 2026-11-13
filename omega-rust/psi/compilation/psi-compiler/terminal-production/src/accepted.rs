//! The exact typed owner and the first accepted closure of its whole
//! compilation. Source is retained before judgement; another source cannot
//! be attached to an artifact after the route has decided it.

use diagnostics::Diagnostic;
use lowered_discharge::{AcceptedClosure, CompilationRefusal, RouteVerdict};
use std::sync::Arc;
use typed_trees::FrozenTypedTrees;

#[derive(Debug)]
pub struct AcceptedTerminalSource {
    source: Arc<FrozenTypedTrees>,
    closure: Box<AcceptedClosure>,
    publication: terminal_codec::TerminalPublication,
}

impl AcceptedTerminalSource {
    pub fn judge(
        source: Arc<FrozenTypedTrees>,
        compilation: structure_elaboration::Compilation<'_>,
        publication: terminal_codec::TerminalPublication,
    ) -> Result<Self, Vec<Diagnostic>> {
        let verdicts = lowered_discharge::judge_compilation_for(&source, compilation, publication);
        let closure = accept_compilation(verdicts)?;
        Ok(Self {
            source,
            closure,
            publication,
        })
    }

    /// Binds source companions to this exact accepted code owner. Structural
    /// reading refuses every unserved Build selection/query/native/filesystem
    /// protocol; only genuinely emitted request rows can arrive here today.
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
        let provenance = provenance.for_root(&elaboration.module, elaboration.module.entry, None);
        let applications = custody
            .applications
            .iter()
            .map(|application| {
                application.attach(program.view(), program.portable(), &provenance, observer)
            })
            .collect();
        // These dimensions are absent because the closed structural reader
        // refuses their actual source constructs, never because of portable
        // versus invocation publication. Extend their producer before serving
        // any such construct; publication cannot discover or erase it later.
        let build_bindings = crate::TerminalBuildBindings::new(
            program.view(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let build_requests =
            crate::TerminalBuildRequests::new(program.view(), elaboration.build_requests.clone());
        let build_filesystem = crate::TerminalBuildFilesystem::new(program.view(), Vec::new());
        Ok(crate::ProducedTerminalProgram {
            program,
            evidence: Some(Arc::new(evidence)),
            build_bindings,
            build_requests,
            build_filesystem,
            applications,
            provenance,
            source_records: elaboration.source_records.clone(),
        })
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
            }) => {
                let before = diagnostics.len();
                diagnostics.extend(formations.iter().map(|entry| entry.reject_diagnostic()));
                for entry in unproved.iter().filter(|entry| !entry.cause.is_incomplete()) {
                    diagnostics.push(entry.reject_diagnostic());
                    diagnostics.extend(entry.opening_diagnostic());
                }
                diagnostics.extend(unestablished.iter().map(|entry| entry.reject_diagnostic()));
                diagnostics.extend(conflicts.iter().map(|entry| entry.reject_diagnostic()));
                if before == diagnostics.len() {
                    diagnostics.push(Diagnostic::error(
                        "no product route yet: lowered rejection names no failure",
                    ));
                }
            }
            Ok(RouteVerdict::Incomplete(unproved)) => {
                if unproved.is_empty() {
                    diagnostics.push(Diagnostic::error(
                        "no product route yet: incomplete lowered verdict names no goal",
                    ));
                }
                diagnostics.extend(unproved.iter().map(|entry| {
                    let mut diagnostic = entry.reject_diagnostic();
                    diagnostic.message = format!(
                        "no product route yet: {}; the checker cannot decide this goal form",
                        diagnostic.message
                    );
                    diagnostic
                }));
            }
            Ok(RouteVerdict::FallBack(refused)) => {
                diagnostics.push(Diagnostic::error(format!(
                    "no product route yet: {}",
                    refused.detail
                )));
            }
            Err(defect) => diagnostics.push(Diagnostic::error(format!(
                "no product route yet: lowered checker defect: {defect:?}"
            ))),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    first.ok_or_else(|| {
        vec![Diagnostic::error(
            "no product route yet: no accepted compilation root",
        )]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{IdTable, MachineId};
    use structure_elaboration::{Body, CrashCeiling, Signature, Termination};

    fn accepted() -> RouteVerdict {
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
            crashes: CrashCeiling::Authored(Vec::new()),
            termination: Termination::Silent,
        });
        lowered_discharge::judge(structure_elaboration::lower_bodies(
            MachineId::new(1).unwrap(),
            &bodies,
            &signatures,
            &IdTable::default(),
        ))
        .unwrap()
    }

    #[test]
    fn later_undecided_part_cannot_publish_the_accepted_primary_closure() {
        assert!(matches!(accepted(), RouteVerdict::Accepted(_)));
        let diagnostics = accept_compilation(vec![
            Ok(accepted()),
            Ok(RouteVerdict::Incomplete(Vec::new())),
        ])
        .unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|entry| entry.message.contains("incomplete lowered verdict"))
        );
        assert!(accept_compilation(Vec::new()).is_err());
    }
}
