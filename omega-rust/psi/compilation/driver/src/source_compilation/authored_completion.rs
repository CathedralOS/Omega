//! Invocation-local successful authored completions, before generated admission.
//!
//! Keys own only semantic continuation components. They never own the retention
//! handle, so cloning a continuation cannot form an Arc cycle. Independently
//! admitted Build execution may change any component and then cannot hit.

use std::sync::{Arc, Mutex};

use diagnostics::Diagnostic;

use super::{CompletedSource, SourceCompletionInput};

#[derive(Clone, Default, compiler_storage::Record)]
pub(super) struct AuthoredCompletions {
    entries: Arc<Mutex<Vec<Completion>>>,
}

#[derive(compiler_storage::Record)]
struct Completion {
    input: Arc<SourceCompletionInput>,
    source: CompletedSource,
}

impl AuthoredCompletions {
    pub(super) fn find(
        &self,
        input: &SourceCompletionInput,
    ) -> Result<Option<CompletedSource>, Vec<Diagnostic>> {
        let entries = self.entries.lock().map_err(|_| unavailable())?;
        let Some(completion) = entries
            .iter()
            .find(|entry| entry.input.same_completion_input(input))
        else {
            return Ok(None);
        };
        completion
            .source
            .accounting
            .validate_charges()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        let mut source = completion.source.clone();
        source.accounting.mark_reused();
        Ok(Some(source))
    }

    pub(super) fn retain(
        &self,
        input: Arc<SourceCompletionInput>,
        source: &CompletedSource,
    ) -> Result<(), Vec<Diagnostic>> {
        // A tolerated failed probe still has its original source outcome, but
        // the current evaluator exposes no usage for that failed attempt. Such
        // a completion stays cold rather than silently acquiring zero charge.
        if !source.accounting.has_complete_usage() {
            return Ok(());
        }
        source
            .accounting
            .validate_charges()
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        self.entries
            .lock()
            .map_err(|_| unavailable())?
            .push(Completion {
                input,
                source: source.clone(),
            });
        Ok(())
    }
}

fn unavailable() -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "authored source completion retention owner is poisoned",
    )]
}

impl AuthoredCompletions {
    pub(super) fn rejoin_current_authority(
        &self,
        authority: Option<Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>>,
    ) -> Result<Self, String> {
        let stored = self
            .entries
            .lock()
            .map_err(|_| "authored source completion retention owner is poisoned")?;
        let mut entries = Vec::new();
        for entry in stored.iter() {
            // A stored completion is optional. Changed contexts recompute it
            // from this frontier rather than returning an unrelated refusal.
            let Ok(input) = entry.input.rejoin_current_authority(authority.clone()) else {
                continue;
            };
            let mut source = entry.source.clone();
            if source.pending_pre_checks.iter_mut().any(|pending| {
                pending
                    .rejoin_source_authority(source.typing.typed(), authority.clone())
                    .is_err()
            }) {
                continue;
            }
            entries.push(Completion { input, source });
        }
        Ok(Self {
            entries: Arc::new(Mutex::new(entries)),
        })
    }
}
