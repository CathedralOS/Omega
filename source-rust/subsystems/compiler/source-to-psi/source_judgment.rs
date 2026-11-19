//! The source driver lowers the frozen program, checks that exact input, then
//! gives the publisher the resulting source-bound judgment.
use crate::{AcceptedTerminalSource, SourceCompilationRoster, TerminalSourceCompilation};
use diagnostics::Diagnostic;
use lowered_program_to_checked_program::checking::judge_publication;
use optimization::PsiOptimizationSelections;
use std::sync::Arc;
use typed_trees::source_state::FrozenTypedTrees;
use prepared_typed_trees_to_lowered_program::lowering::LoweredSourcePublication;

/// Typed source with its constant inputs materialized. An authored Build settles those
/// inputs using its source consent and sponsorship. A nested call arrives with its
/// arguments already materialized by the caller.
pub(crate) struct SettledSource<'request> {
    pub(crate) program: crate::source_state::TypedTrees,
    pub(crate) wire_demands: &'request [crate::WireCompatibilityDemand],
    pub(crate) demanded_constants: Vec<symbols::SymbolHandle>,
    pub(crate) accounting: crate::compile_time::SourceCompletionAccounting,
}

impl SettledSource<'_> {
    pub(crate) fn materialized_call(program: crate::source_state::TypedTrees) -> Self {
        Self {
            program,
            wire_demands: &[],
            demanded_constants: Vec::new(),
            accounting: Default::default(),
        }
    }

    pub(crate) fn freeze(self) -> Result<FrozenSource, crate::SourcePreparationFailure> {
        let result = (|| {
            let preparation = crate::preparation::PreparationRequest::settled()
                .with_wire_compatibility_demands(self.wire_demands);
            let (program, _) = crate::preparation::prepare_typed_trees(self.program, &preparation)?;
            let constants = constant_reads(&program, self.demanded_constants)?;
            Ok((Arc::new(program), constants))
        })();
        match result {
            Ok((program, constants)) => Ok(FrozenSource {
                program,
                constants,
                accounting: self.accounting,
            }),
            Err(diagnostics) => Err(crate::SourcePreparationFailure {
                diagnostics,
                accounting: self.accounting,
            }),
        }
    }

    /// Freeze and judge one exact invocation, retaining the costs of any failed judgment.
    pub(crate) fn judge(
        self,
        compilation: TerminalSourceCompilation<'_>,
        publication: terminal_codec::TerminalPublication,
    ) -> Result<JudgedSource, crate::SourcePreparationFailure> {
        let FrozenSource {
            program,
            accounting,
            constants,
        } = self.freeze()?;
        match judge_terminal_source(program, compilation, publication) {
            Ok(accepted) => Ok(JudgedSource {
                accepted,
                accounting,
                constants,
            }),
            Err(diagnostics) => Err(crate::SourcePreparationFailure {
                diagnostics,
                accounting,
            }),
        }
    }
}

fn constant_reads(
    program: &FrozenTypedTrees,
    demanded_constants: Vec<symbols::SymbolHandle>,
) -> Result<Vec<crate::ConstantRead>, Vec<Diagnostic>> {
    if demanded_constants.is_empty() {
        return Ok(Vec::new());
    }
    let key = |symbol: symbols::SymbolHandle| (symbol.arena_index(), symbol.generation());
    let mut declarations = program.const_declarations().iter().collect::<Vec<_>>();
    declarations.sort_unstable_by_key(|declaration| key(declaration.symbol));
    demanded_constants
        .into_iter()
        .map(|symbol| {
            let index = declarations
                .binary_search_by_key(&key(symbol), |declaration| key(declaration.symbol))
                .map_err(|_| {
                    vec![Diagnostic::error(
                        "source preparation lost a demanded constant declaration",
                    )]
                })?;
            let declaration = declarations[index];
            Ok(crate::ConstantRead {
                name: program.symbols.name(symbol).to_owned(),
                declaration: crate::source_symbols::retain_declaration(&program.symbols, symbol)
                    .ok(),
                value: declaration.evaluated_value.clone(),
            })
        })
        .collect()
}

pub struct FrozenSource {
    pub program: Arc<crate::source_state::FrozenTypedTrees>,
    pub accounting: crate::compile_time::SourceCompletionAccounting,
    /// Actual demand before named uses became literals. This is result
    /// correspondence data, not declaration-selection or execution authority.
    pub constants: Vec<crate::ConstantRead>,
}

pub struct JudgedSource {
    pub accepted: crate::AcceptedTerminalSource,
    pub accounting: crate::compile_time::SourceCompletionAccounting,
    pub constants: Vec<crate::ConstantRead>,
}

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
