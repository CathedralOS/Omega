//! Warnings about a compilation the checker accepted, computed on first read.
use super::AcceptedClosure;
use crate::Discharge;
use crate::verdict::AcceptedProgram;
use checked_program::CheckedProgram;
use diagnostics::Diagnostic;
use std::sync::{Arc, Mutex, OnceLock};
use terminal_check_tables::program::Elaboration;

/// The advisories of an accepted compilation: warnings and their notes, none
/// of which is an error. They are data for whoever reports the compilation;
/// no verdict and no later stage reads them.
///
/// Reporters request the same warnings by default. Transient source jobs use
/// the accepted code without reporting it, so they must not start advisory
/// proof searches just to discard the diagnostics. Keep every accepted part
/// until the first read, then release the parts after aggregation.
pub struct AcceptedAdvisories {
    pending: Mutex<AdvisoryParts>,
    cached: OnceLock<Vec<Diagnostic>>,
}

#[derive(Default)]
struct AdvisoryParts {
    /// The root closure, whose program publication also holds, with the
    /// discharge that accepted it. Absent when `original` reports instead.
    root: Option<(Arc<CheckedProgram>, Discharge)>,
    // A transformation may expose new constants. Source warnings must still
    // describe the program as written, from its first judgment.
    original: Option<Box<AcceptedClosure>>,
    additional: Vec<(Arc<Elaboration>, Discharge)>,
}

impl std::fmt::Debug for AcceptedAdvisories {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AcceptedAdvisories")
            .field("materialized", &self.cached.get().is_some())
            .finish_non_exhaustive()
    }
}

impl AcceptedAdvisories {
    pub(super) fn new(
        root: Arc<CheckedProgram>,
        discharge: Discharge,
        original: Option<Box<AcceptedClosure>>,
        additional: Vec<(Arc<Elaboration>, Discharge)>,
    ) -> Self {
        Self {
            pending: Mutex::new(AdvisoryParts {
                root: original.is_none().then_some((root, discharge)),
                original,
                additional,
            }),
            cached: OnceLock::new(),
        }
    }

    /// The advisories of every accepted part: the entry closure and each
    /// generic check form, in the order the parts were judged.
    pub fn get(&self) -> &[Diagnostic] {
        self.cached.get_or_init(|| {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Retain the parts if a diagnostic search panics, just as each
            // accepted closure retains its own deferred warning inputs.
            let first = match (&pending.original, &pending.root) {
                (Some(original), _) => Some(AcceptedProgram {
                    elaboration: &original.elaboration,
                    discharge: &original.discharge,
                }),
                (None, Some((program, discharge))) => Some(AcceptedProgram {
                    elaboration: program.elaboration(),
                    discharge,
                }),
                (None, None) => None,
            };
            let parts: Vec<_> = first
                .into_iter()
                .chain(pending.additional.iter().map(|(elaboration, discharge)| {
                    AcceptedProgram {
                        elaboration,
                        discharge,
                    }
                }))
                .collect();
            let result = aggregate(part_advisories(&parts).into_iter());
            *pending = AdvisoryParts::default();
            result
        })
    }
}

/// The advisories of each part, in the order the parts were judged. A large
/// program has about a hundred generic check forms, each with its own
/// branch searches, so the parts run on the invocation's workers. Outside
/// a budget they stay on this thread: Rayon's global pool has small stacks.
fn part_advisories(parts: &[AcceptedProgram<'_>]) -> Vec<Vec<Diagnostic>> {
    if worker_budget::current_workers() == 1 || parts.len() < 2 {
        return parts.iter().map(AcceptedProgram::advisories).collect();
    }
    use rayon::prelude::*;
    // Indexed collection keeps part order, which `aggregate` relies on.
    parts
        .par_iter()
        .with_max_len(1)
        .map(AcceptedProgram::advisories)
        .collect()
}

/// One authored machine may be judged in more than one part, and its advisory
/// is the same in each, so an advisory already listed is not listed again.
fn aggregate(parts: impl Iterator<Item = Vec<Diagnostic>>) -> Vec<Diagnostic> {
    let mut advisories: Vec<Diagnostic> = Vec::new();
    for part in parts {
        // A warning and the note after it are one advisory.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn judged(name: &str) -> AcceptedAdvisories {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../../../../tests/fixtures/contracts/pass")
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
                source_provenance::DependencyScope::Product,
            )
            .source_id;
        let tokens = crate::frontend_fixture::Lexer::new(&text)
            .tokenize()
            .unwrap();
        let syntax = crate::frontend_fixture::parse_syntax_trees_with_id(id, &tokens).unwrap();
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
        let lowered = crate::lowering::lower_for_publication(
            crate::prepare_fixture(typed),
            crate::lowering::Compilation {
                declaration_metadata: &[],
                entries: &[entry],
                subjects: &[Some(package)],
                purpose: source_provenance::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                calling: None,
                providers: None,
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .unwrap();
        let (_, advisories) = crate::judge_publication(lowered, Default::default()).unwrap();
        advisories
    }

    #[test]
    fn authored_advisories_wait_for_read_and_release_additional_forms_once() {
        // This kept CLI fixture has one authored contradiction and two real
        // specializations. Its extra check forms must not multiply warnings.
        let advisories = judged("generic_branch_contract_contradiction_exit");
        assert!(advisories.cached.get().is_none());
        assert!(!advisories.pending.lock().unwrap().additional.is_empty());
        let diagnostics = advisories.get();
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
        assert!(advisories.pending.lock().unwrap().additional.is_empty());
        assert!(advisories.pending.lock().unwrap().root.is_none());
        assert!(std::ptr::eq(
            diagnostics.as_ptr(),
            advisories.get().as_ptr()
        ));
        assert_eq!(advisories.get(), diagnostics);
    }

    #[test]
    fn parallel_reads_list_the_same_advisories_in_the_same_order() {
        // Parts and machines search on the budget's workers; the warnings
        // and their order must match a read on one thread.
        let name = "generic_branch_contract_contradiction_exit";
        let serial = judged(name).get().to_vec();
        assert!(!serial.is_empty());
        let budget = worker_budget::ExecutionBudget::with_workers(4).unwrap();
        let parallel = budget.run(|| {
            assert_eq!(worker_budget::current_workers(), 4);
            judged(name).get().to_vec()
        });
        assert_eq!(parallel, serial);
    }

    #[test]
    fn specialization_only_advisories_remain_quiet_after_a_lazy_read() {
        let advisories = judged("specialized_branch_selection_quiet_exit");
        assert!(advisories.cached.get().is_none());
        assert!(advisories.get().is_empty());
        assert!(advisories.cached.get().is_some());
        assert!(advisories.pending.lock().unwrap().additional.is_empty());
        assert!(advisories.get().is_empty());
    }
}
