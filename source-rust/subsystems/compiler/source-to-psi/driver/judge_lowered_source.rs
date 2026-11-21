//! The source driver lowers the frozen program, checks that exact input, then
//! gives the publisher the resulting source-bound judgment.
use crate::driver::prepare_typed_source;
use checked_program_to_psi::production;
use checked_program_to_psi::production::{AcceptedTerminalSource, TerminalSourceCompilation};
use diagnostics::{Diagnostic, DiagnosticStage};
use lowered_program_to_checked_program::checking::{CheckedPublication, judge_publication};
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
    pub(crate) selected_providers: Vec<symbols::SymbolHandle>,
    pub(crate) accounting: crate::compile_time_invocations::SourceCompletionAccounting,
}

impl SettledSource<'_> {
    pub(crate) fn materialized_call(program: crate::source_state::TypedTrees) -> Self {
        Self {
            program,
            wire_demands: &[],
            demanded_constants: Vec::new(),
            selected_providers: Vec::new(),
            accounting: Default::default(),
        }
    }

    pub(crate) fn freeze(
        self,
    ) -> Result<FrozenSource, prepare_typed_source::SourcePreparationFailure> {
        let result = (|| {
            let program = preparation::prepare_with_selected_providers(
                self.program,
                self.wire_demands,
                &self.selected_providers,
            )
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

enum SourceRoots<'a> {
    Selected(
        TerminalSourceCompilation<'a>,
        terminal_codec::TerminalPublication,
    ),
    Product(SourceCompilationRoster),
}

fn judge_source(
    source: PreparedTypedTrees,
    roots: SourceRoots<'_>,
) -> Result<CheckedPublication<LoweredSourcePublication>, Vec<Diagnostic>> {
    let lowered = match roots {
        SourceRoots::Selected(compilation, publication) => {
            LoweredSourcePublication::prepare(source, compilation, publication)
        }
        SourceRoots::Product(roster) => roster.lower_for_publication(source),
    }
    .map_err(|rows| DiagnosticStage::Lowering.attribute(rows))?;
    judge_publication(lowered).map_err(|rows| DiagnosticStage::Checking.attribute(rows))
}

pub fn judge_terminal_source(
    source: PreparedTypedTrees,
    compilation: TerminalSourceCompilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Result<AcceptedTerminalSource, Vec<Diagnostic>> {
    let judgment = judge_source(source, SourceRoots::Selected(compilation, publication))?;
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
        let judgment = judge_source(source, SourceRoots::Product(roster))?;
        checked_program_to_psi::SourcePublication::from_judgment(judgment, optimizations).map(Self)
    }
}

#[cfg(test)]
fn judge_authored_ieee(
    source: &str,
    entry: &str,
) -> Result<AcceptedTerminalSource, Vec<Diagnostic>> {
    let package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x46; 32]).unwrap();
    let library = crate::test_support::fixture_repository::root().join("source/library");
    let typed = crate::test_support::schema_customer_source::source_with_core(
        &library,
        source,
        package,
        &[],
    )
    .unwrap();
    let entry = typed
        .machines()
        .iter()
        .find(|machine| {
            machine.name.as_str() == entry
                && typed.symbols.symbol_package_identity(machine.symbol) == Some(package)
        })
        .expect("the authored package entry")
        .symbol;
    let prepared = preparation::prepare_typed_trees(typed, &[]).unwrap();
    judge_terminal_source(
        prepared,
        TerminalSourceCompilation {
            entries: &[entry],
            subjects: &[Some(package)],
            purpose: source_provenance::DependencyScope::Product,
            declaration_metadata: &[],
            selected: &[],
            bindings: &[],
            calling: None,
            providers: None,
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
}

#[test]
fn a_float_meaning_wrapper_must_hold_before_call_arguments_are_supplied() {
    let judge = |body: &str, other: &str, entry: &str| {
        // Both implementations compute the same result at the sole call.
        // Only the first establishes observe's guarantee for every input.
        let source = r"
            use omega\language\core\FloatMeaning;
            use omega\language\core\F32::meaning;
            machine observe(value: f32, other: f32) -> FloatMeaning
            ensures core\Identity::<FloatMeaning>(result, F32::meaning(value))
            ensures core\Identity::<FloatMeaning>(F32::meaning(other), F32::meaning(OTHER_VALUE))
            { F32::meaning(WRAPPER_BODY) }
            machine main(value: f32) -> f32
            ensures core\Identity::<FloatMeaning>(observe(2.0f32, 2.0f32), F32::meaning(2.0f32))
            { value }
        "
        .replace("WRAPPER_BODY", body)
        .replace("OTHER_VALUE", other);
        judge_authored_ieee(&source, entry)
    };
    judge("value", "other", "main")
        .unwrap_or_else(|rows| panic!("{}", diagnostics::join_messages(&rows)));
    for (body, other) in [("2.0f32", "other"), ("value", "value")] {
        let refused = judge(body, other, "main")
            .err()
            .expect("false wrapper guarantee");
        assert!(
            diagnostics::join_messages(&refused)
                .contains("float meaning wrapper does not establish its result identity"),
            "{refused:?}"
        );
    }
    // The same valid declaration cannot be chosen as an executable entry.
    let runtime = judge("value", "other", "observe")
        .err()
        .expect("runtime proof result");
    assert!(
        diagnostics::join_messages(&runtime).contains("proof-only FloatMeaning has no runtime ABI"),
        "{runtime:?}"
    );
}

#[test]
fn a_float_meaning_equation_keeps_the_precision_of_its_materialized_format() {
    let source = r"
        use omega\language\core\FloatMeaning;
        use omega\language\core\FloatFormat::BINARY32;
        use omega\language\core\FloatFormat::BINARY64;
        use omega\language\core\F64::meaning;
        use omega\language\core\F64::square_root;
        use omega\language\core\FloatSemantics::square_root;
        machine main(value: f64) -> f64
        ensures core\Identity::<FloatMeaning>(
            F64::meaning(result),
            FloatSemantics::square_root(FloatFormat::PRECISION, F64::meaning(value)))
        { F64::square_root(value) }
    ";
    judge_authored_ieee(&source.replace("PRECISION", "BINARY64"), "main")
        .unwrap_or_else(|rows| panic!("{}", diagnostics::join_messages(&rows)));
    let wrong = judge_authored_ieee(&source.replace("PRECISION", "BINARY32"), "main")
        .err()
        .expect("a binary32 rounding contract for a binary64 operation");
    assert!(
        diagnostics::join_messages(&wrong)
            .contains("float meaning equality mixes permanent formats"),
        "{wrong:?}"
    );
}
