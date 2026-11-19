//! The source driver lowers the frozen program, checks that exact input, then
//! gives the publisher the resulting source-bound judgment.
use crate::{AcceptedTerminalSource, SourceCompilationRoster, TerminalSourceCompilation};
use diagnostics::Diagnostic;
use lowered_program_to_checked_program::checking::judge_publication;
use optimization::PsiOptimizationSelections;
use std::sync::Arc;
use typed_trees::source_state::FrozenTypedTrees;
use typed_trees_to_lowered_program::lowering::LoweredSourcePublication;

pub fn judge_terminal_source(
    source: Arc<FrozenTypedTrees>,
    compilation: TerminalSourceCompilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Result<AcceptedTerminalSource, Vec<Diagnostic>> {
    let input = LoweredSourcePublication::prepare(source, compilation, publication)?;
    AcceptedTerminalSource::from_judgment(judge_publication(input)?)
}

#[cfg(any(test, feature = "test-support"))]
pub fn judge_source_roster(
    roster: SourceCompilationRoster,
    source: Arc<FrozenTypedTrees>,
) -> Result<Arc<AcceptedTerminalSource>, Vec<Diagnostic>> {
    let input = roster.lower_for_publication(source)?;
    AcceptedTerminalSource::from_judgment(judge_publication(input)?).map(Arc::new)
}

/// Source preparation may judge a new roster before the fixed product is published.
#[derive(Debug)]
pub struct SourcePublication(checked_program_to_psi::SourcePublication);

impl std::ops::Deref for SourcePublication {
    type Target = checked_program_to_psi::SourcePublication;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SourcePublication {
    pub fn new(optimizations: PsiOptimizationSelections) -> Self {
        Self(checked_program_to_psi::SourcePublication::new(
            optimizations,
        ))
    }

    pub fn judge(
        source: Arc<FrozenTypedTrees>,
        roster: SourceCompilationRoster,
        optimizations: PsiOptimizationSelections,
    ) -> Result<Self, Vec<Diagnostic>> {
        let input = roster.lower_for_publication(source)?;
        checked_program_to_psi::SourcePublication::from_judgment(
            judge_publication(input)?,
            optimizations,
        )
        .map(Self)
    }

    /// Judge an executable child's descriptors before publication, preserving
    /// the original roster and every chosen supply on the same frozen owner.
    pub fn with_declaration_metadata(
        &self,
        declarations: &[symbols::SymbolHandle],
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut roster = self.unpublished_roster()?.clone();
        for declaration in declarations {
            if !roster.declaration_metadata.contains(declaration) {
                roster.declaration_metadata.push(*declaration);
            }
        }
        Self::judge(
            Arc::clone(self.accepted()?.source()),
            roster,
            self.optimizations().clone(),
        )
    }
}
