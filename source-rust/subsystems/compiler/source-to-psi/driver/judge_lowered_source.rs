//! The source driver lowers the frozen program, checks that exact input, then
//! gives the publisher the resulting source-bound judgment. It keeps the
//! checker's advisories for whoever reports the compilation.
use crate::driver::prepare_typed_source;
use artifacts::compile_timings::{CompileTimings, source as stages};
use checked_program_to_psi::production;
use checked_program_to_psi::production::{AcceptedTerminalSource, TerminalSourceCompilation};
use diagnostics::{Diagnostic, DiagnosticStage};
use lowered_program_to_checked_program::checking::{
    AcceptedAdvisories, CheckedPublication, judge_publication,
};
use optimization::PsiOptimizationSelections;
use prepared_typed_trees_to_lowered_program::lowering::source_roster::{
    SourceCompilationRoster, lower_roster,
};
use prepared_typed_trees_to_lowered_program::lowering::{
    LoweredSourcePublication, PreparedTypedTrees, lower_for_publication,
};
use typed_trees_to_prepared_typed_trees as preparation;

/// Typed source with its constant inputs materialized. An authored Build settles those
/// inputs using its source consent and sponsorship. A nested call arrives with its
/// arguments already materialized by the caller.
pub(crate) struct SettledSource {
    pub(crate) program: crate::source_state::TypedTrees,
    pub(crate) values: Vec<preparation::ValueInputs>,
    pub(crate) selected_declarations: Vec<preparation::SelectedDeclaration>,
    pub(crate) accounting: crate::compile_time_invocations::SourceCompletionAccounting,
}

impl SettledSource {
    pub(crate) fn materialized_call(program: crate::source_state::TypedTrees) -> Self {
        Self {
            program,
            values: Vec::new(),
            selected_declarations: Vec::new(),
            accounting: Default::default(),
        }
    }

    pub(crate) fn freeze(
        self,
    ) -> Result<FrozenSource, prepare_typed_source::SourcePreparationFailure> {
        match preparation::prepare_typed_trees(
            self.program,
            &self.values,
            &self.selected_declarations,
        )
        .map_err(|rows| DiagnosticStage::Preparation.attribute(rows))
        {
            Ok(program) => Ok(FrozenSource {
                program,
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
        } = self.freeze()?;
        match judge_terminal_source(program, compilation, publication) {
            Ok(accepted) => Ok(JudgedSource {
                accepted,
                accounting,
            }),
            Err(diagnostics) => Err(prepare_typed_source::SourcePreparationFailure {
                diagnostics,
                accounting,
            }),
        }
    }
}

pub struct FrozenSource {
    pub program: PreparedTypedTrees,
    pub accounting: crate::compile_time_invocations::SourceCompletionAccounting,
}

pub struct JudgedSource {
    pub accepted: production::AcceptedTerminalSource,
    pub accounting: crate::compile_time_invocations::SourceCompletionAccounting,
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
    optimizations: PsiOptimizationSelections,
    timings: &mut CompileTimings,
) -> Result<
    (
        CheckedPublication<LoweredSourcePublication>,
        AcceptedAdvisories,
    ),
    Vec<Diagnostic>,
> {
    let lowered = timings
        .record_result(stages::LOWER, || match roots {
            SourceRoots::Selected(compilation, publication) => {
                lower_for_publication(source, compilation, publication)
            }
            SourceRoots::Product(roster) => lower_roster(roster, source),
        })
        .map_err(|rows| DiagnosticStage::Lowering.attribute(rows))?;
    timings
        .record_result(stages::CHECK, || judge_publication(lowered, optimizations))
        .map_err(|rows| DiagnosticStage::Checking.attribute(rows))
}

pub fn judge_terminal_source(
    source: PreparedTypedTrees,
    compilation: TerminalSourceCompilation<'_>,
    publication: terminal_codec::TerminalPublication,
) -> Result<AcceptedTerminalSource, Vec<Diagnostic>> {
    // A selected invocation is used and never reported, so its advisories are not read.
    let (judgment, _) = judge_source(
        source,
        SourceRoots::Selected(compilation, publication),
        Default::default(),
        &mut CompileTimings::default(),
    )?;
    AcceptedTerminalSource::from_judgment(judgment)
}

/// Source preparation may judge a new roster before the fixed product is published.
#[derive(Debug)]
pub struct SourcePublication {
    publication: checked_program_to_psi::SourcePublication,
    advisories: AcceptedAdvisories,
}

impl std::ops::Deref for SourcePublication {
    type Target = checked_program_to_psi::SourcePublication;
    fn deref(&self) -> &Self::Target {
        &self.publication
    }
}

impl SourcePublication {
    pub fn judge(
        source: PreparedTypedTrees,
        roster: SourceCompilationRoster,
        optimizations: PsiOptimizationSelections,
        timings: &mut CompileTimings,
    ) -> Result<Self, Vec<Diagnostic>> {
        let (judgment, advisories) =
            judge_source(source, SourceRoots::Product(roster), optimizations, timings)?;
        let publication = timings.record_result(stages::ACCEPT, || {
            checked_program_to_psi::SourcePublication::from_judgment(judgment)
        })?;
        Ok(Self {
            publication,
            advisories,
        })
    }

    /// What the checker says about the accepted source without refusing it:
    /// warnings and their notes, in the order the parts were judged. The
    /// checker computes them on the first read.
    pub fn advisories(&self) -> &[Diagnostic] {
        self.advisories.get()
    }

    /// Check the package selections against the module the source judgment owns,
    /// then return the source judgment's advisories.
    pub fn check(
        &self,
        authority: Option<&dyn declaration_scope::DeclarationSelectionAuthority>,
        timings: &mut CompileTimings,
    ) -> Result<Vec<Diagnostic>, Vec<Diagnostic>> {
        self.publication.check_selections(authority, timings)?;
        timings.record_result(stages::ADVISORIES, || Ok(self.advisories().to_vec()))
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
    let prepared = preparation::prepare_typed_trees(typed, &[], &[]).unwrap();
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
        let Err(refused) = judge(body, other, "main") else {
            panic!("false wrapper guarantee");
        };
        assert!(
            diagnostics::join_messages(&refused)
                .contains("float meaning wrapper does not establish its result identity"),
            "{refused:?}"
        );
    }
    // The same valid declaration cannot be chosen as an executable entry.
    let Err(runtime) = judge("value", "other", "observe") else {
        panic!("runtime proof result");
    };
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
    let Err(wrong) = judge_authored_ieee(&source.replace("PRECISION", "BINARY32"), "main") else {
        panic!("a binary32 rounding contract for a binary64 operation");
    };
    assert!(
        diagnostics::join_messages(&wrong)
            .contains("float meaning equality mixes permanent formats"),
        "{wrong:?}"
    );
}
