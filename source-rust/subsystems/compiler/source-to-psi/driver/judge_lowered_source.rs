//! The source driver lowers the frozen program, checks that exact input, then
//! gives the publisher the resulting source-bound judgment.
use crate::driver::prepare_typed_source;
use checked_program_to_psi::production;
use checked_program_to_psi::production::{AcceptedTerminalSource, TerminalSourceCompilation};
use diagnostics::{Diagnostic, DiagnosticStage};
use lowered_program_to_checked_program::checking::judge_publication;
use optimization::PsiOptimizationSelections;
use prepared_typed_trees_to_lowered_program::lowering::source_roster::SourceCompilationRoster;
use prepared_typed_trees_to_lowered_program::lowering::{
    LoweredSourcePublication, PreparedTypedTrees,
};
use typed_trees::source_state::FrozenTypedTrees;
use typed_trees_to_prepared_typed_trees as preparation;

/// Typed source with its constant inputs materialized. An authored Build settles those
/// inputs using its source consent and sponsorship. A nested call arrives with its
/// arguments already materialized by the caller.
pub(crate) struct SettledSource<'request> {
    pub(crate) program: crate::source_state::TypedTrees,
    pub(crate) wire_demands: &'request [crate::WireCompatibilityDemand],
    pub(crate) demanded_constants: Vec<symbols::SymbolHandle>,
    pub(crate) accounting: crate::compile_time_invocations::SourceCompletionAccounting,
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

    pub(crate) fn freeze(
        self,
    ) -> Result<FrozenSource, prepare_typed_source::SourcePreparationFailure> {
        let result = (|| {
            let program = preparation::prepare_typed_trees(self.program, self.wire_demands)
                .map_err(|rows| DiagnosticStage::Preparation.attribute(rows))?;
            let constants = constant_reads(&program, self.demanded_constants)
                .map_err(|rows| DiagnosticStage::Preparation.attribute(rows))?;
            Ok((program, constants))
        })();
        match result {
            Ok((program, constants)) => Ok(FrozenSource {
                program,
                constants,
                accounting: self.accounting,
            }),
            Err(diagnostics) => Err(prepare_typed_source::SourcePreparationFailure {
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
    ) -> Result<JudgedSource, prepare_typed_source::SourcePreparationFailure> {
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
            Err(diagnostics) => Err(prepare_typed_source::SourcePreparationFailure {
                diagnostics,
                accounting,
            }),
        }
    }
}

fn constant_reads(
    program: &FrozenTypedTrees,
    demanded_constants: Vec<symbols::SymbolHandle>,
) -> Result<Vec<production::ConstantRead>, Vec<Diagnostic>> {
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
            Ok(production::ConstantRead {
                name: program.symbols.name(symbol).to_owned(),
                declaration: crate::source_symbols::retain_declaration(&program.symbols, symbol)
                    .ok(),
                value: declaration.evaluated_value.clone(),
            })
        })
        .collect()
}

pub struct FrozenSource {
    pub program: PreparedTypedTrees,
    pub accounting: crate::compile_time_invocations::SourceCompletionAccounting,
    /// Actual demand before named uses became literals. This is result
    /// correspondence data, not declaration-selection or execution authority.
    pub constants: Vec<production::ConstantRead>,
}

pub struct JudgedSource {
    pub accepted: production::AcceptedTerminalSource,
    pub accounting: crate::compile_time_invocations::SourceCompletionAccounting,
    pub constants: Vec<production::ConstantRead>,
}

pub fn judge_terminal_source(
    source: PreparedTypedTrees,
    compilation: TerminalSourceCompilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Result<AcceptedTerminalSource, Vec<Diagnostic>> {
    let input = LoweredSourcePublication::prepare(source, compilation, publication)
        .map_err(|rows| DiagnosticStage::Lowering.attribute(rows))?;
    let judgment =
        judge_publication(input).map_err(|rows| DiagnosticStage::Checking.attribute(rows))?;
    AcceptedTerminalSource::from_judgment(judgment)
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
    pub fn judge(
        source: PreparedTypedTrees,
        roster: SourceCompilationRoster,
        optimizations: PsiOptimizationSelections,
    ) -> Result<Self, Vec<Diagnostic>> {
        let input = roster
            .lower_for_publication(source)
            .map_err(|rows| DiagnosticStage::Lowering.attribute(rows))?;
        checked_program_to_psi::SourcePublication::from_judgment(
            judge_publication(input).map_err(|rows| DiagnosticStage::Checking.attribute(rows))?,
            optimizations,
        )
        .map(Self)
    }
}
