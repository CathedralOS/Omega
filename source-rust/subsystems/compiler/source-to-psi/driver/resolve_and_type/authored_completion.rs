use std::sync::{Arc, Mutex};

use diagnostics::Diagnostic;

use super::{CompletedSource, SourceCompletionInput};

#[derive(Clone, Default)]
pub(super) struct AuthoredCompletions {
    entries: Arc<Mutex<Vec<Completion>>>,
}

struct Completion {
    input: Arc<SourceCompletionInput>,
    source: CompletedSource,
}

impl AuthoredCompletions {
    /// Whether another checkpoint clone holds these completions and so could
    /// look one up later. No clone can appear once this is the only holder.
    pub(super) fn is_shared(&self) -> bool {
        Arc::strong_count(&self.entries) > 1
    }

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
