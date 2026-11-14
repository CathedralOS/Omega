//! Keep accepted source selections attached to the code they describe.
//! Decoding a program never recreates this source-issued companion.

use semantic_vocabulary::source_selections::SelectionQuestions;
use std::sync::Arc;
use terminal_codec::{TerminalExecutionIdentity, TerminalExecutionView};
use terminal_installation::selection_questions::{SelectionAuthorities, admit_selection_questions};

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalSourceSelections {
    identity: TerminalExecutionIdentity,
    questions: Arc<SelectionQuestions>,
}

impl TerminalSourceSelections {
    pub fn declaration_custody(
        &self,
        artifact: TerminalExecutionView<'_>,
        declaration: &str,
        purpose: semantic_vocabulary::source_selections::SelectionPurpose,
    ) -> Result<semantic_vocabulary::compiler_evaluation::DeclarationCustody, String> {
        let questions = self
            .questions_for(artifact)
            .ok_or("source declaration custody belongs to another program or invocation")?;
        let mut custody = None;
        for (_, source) in questions
            .declarations
            .iter()
            .filter(|(_, source)| source.identity == declaration && source.purpose == purpose)
        {
            if custody
                .replace(source.custody)
                .is_some_and(|previous| previous != source.custody)
            {
                return Err("selected declaration has conflicting source package custody".into());
            }
        }
        custody.ok_or_else(|| "selected declaration has no authenticated source custody".into())
    }

    pub(crate) fn new(artifact: TerminalExecutionView<'_>, questions: SelectionQuestions) -> Self {
        Self {
            identity: artifact.custody_identity(),
            questions: Arc::new(questions),
        }
    }

    pub fn questions_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&SelectionQuestions> {
        (self.identity == artifact.custody_identity()).then_some(self.questions.as_ref())
    }

    pub fn admit(
        &self,
        artifact: TerminalExecutionView<'_>,
        authorities: &SelectionAuthorities<'_>,
    ) -> Result<(), String> {
        let questions = self
            .questions_for(artifact)
            .ok_or("source selections belong to a different program or invocation")?;
        admit_selection_questions(artifact.module(), questions, authorities)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invocation_code() -> terminal_codec::InvocationTerminalCode {
        let mut module = terminal_psi::TerminalModule::declarations();
        module.identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation().build();
        terminal_codec::InvocationTerminalCode::new(module).unwrap()
    }

    #[test]
    fn equal_code_in_another_invocation_cannot_reuse_source_selections() {
        let first = invocation_code();
        let second = invocation_code();
        assert_eq!(first.view().identity(), second.view().identity());
        let selections = TerminalSourceSelections::new(first.view(), SelectionQuestions::default());
        assert!(
            selections
                .questions_for(first.retain_code().view())
                .is_some()
        );
        assert!(selections.questions_for(second.view()).is_none());
        assert!(
            selections
                .admit(
                    second.view(),
                    &SelectionAuthorities {
                        build: None,
                        product: None
                    }
                )
                .is_err()
        );
    }
}
