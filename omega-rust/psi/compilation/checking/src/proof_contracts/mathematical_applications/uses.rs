//! Actual prepared reads scoped to one original source contract judgment.
//!
//! The borrowed input preserves its original typed program, signature, captures
//! and invocation memos. Recording happens at the existing prepared application
//! read, without a source walk or mathematical replay. Only the original source
//! acceptance owner may publish the resulting roster; consulted subattempts are
//! reads of that operation, not individual kernel premises or complete closure.

use arena::Handle;
use checked_trees::CheckedMathematicalApplication;
use diagnostics::Diagnostic;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub(super) struct MathematicalApplicationReads {
    applications: Mutex<Vec<Handle<CheckedMathematicalApplication>>>,
}

impl MathematicalApplicationReads {
    pub(super) fn record(&self, application: Handle<CheckedMathematicalApplication>) {
        // A poisoned observation scope remains poisoned. Its acceptance owner
        // rejects publication through finish rather than presenting no reads.
        if let Ok(mut applications) = self.applications.lock()
            && !applications.contains(&application)
        {
            applications.push(application);
        }
    }

    fn finish(self) -> Result<Vec<Handle<CheckedMathematicalApplication>>, Diagnostic> {
        self.applications.into_inner().map_err(|_| {
            Diagnostic::error(
                "internal: original source entailment lost its mathematical application reads",
            )
        })
    }
}

/// One original judging operation borrows the existing checking input. The
/// read roster is construction scratch consumed only after its acceptance gate.
#[derive(Debug)]
pub(crate) struct SourceContractJudgment<'program> {
    original: &'program dyn crate::CheckingInput,
    reads: MathematicalApplicationReads,
}

impl<'program> SourceContractJudgment<'program> {
    pub(crate) fn new(original: &'program dyn crate::CheckingInput) -> Self {
        Self {
            original,
            reads: MathematicalApplicationReads::default(),
        }
    }

    pub(crate) fn finish(self) -> Result<Vec<Handle<CheckedMathematicalApplication>>, Diagnostic> {
        self.reads.finish()
    }
}

impl crate::CheckingInput for SourceContractJudgment<'_> {
    fn typed_program(&self) -> &typed_trees::TypedTrees {
        self.original.typed_program()
    }

    fn authored_declaration_selections(
        &self,
    ) -> &language_semantics::declaration_selection::AuthoredDeclarationSelections {
        self.original.authored_declaration_selections()
    }

    fn checking_memos(&self) -> Option<&crate::CheckingMemos> {
        self.original.checking_memos()
    }

    fn prepared_effect_plans(&self) -> Option<&crate::PreparedEffectPlans> {
        self.original.prepared_effect_plans()
    }

    fn prepared_mathematics(&self) -> Option<super::PreparedMathematicalView<'_>> {
        self.original
            .prepared_mathematics()
            .map(|view| view.observing(&self.reads))
    }
}
