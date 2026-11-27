//! Materialize the constant, representation and layout inputs demanded by a
//! source invocation. Stage 04 can then issue the typed snapshot consumed by
//! lowering. The verdict comes from checking that particular lowering.

use crate::driver::judge_lowered_source::{FrozenSource, JudgedSource, SettledSource};
use crate::source_state::opaque_representations::OpaqueRepresentationSelection;
use checked_program_to_psi::provider_choice_inputs::ProviderSelection;
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees_to_prepared_typed_trees::{
    EntryApplication, SelectedDeclaration, SelectedProvider,
};

/// Authored overrides and package defaults for a single interpreted invocation.
#[derive(Clone, Copy, Default)]
pub(super) struct InvocationProviders<'a> {
    pub(super) overrides: &'a [ProviderSelection],
    pub(super) defaults: &'a [ProviderSelection],
}

#[derive(Default)]
pub struct SourceAdmission<'request> {
    pub invocation_sources:
        Option<Arc<dyn crate::compile_time_invocations::InvocationSourceConsent>>,
    pub entry_applications: &'request [EntryApplication],
    /// The representation choices Product policy admission collected from
    /// this Build's executed requests (`collect_opaque_representations`).
    pub representations: &'request [OpaqueRepresentationSelection],
    pub layout_scopes: &'request [crate::compile_time_invocations::FixedLayoutTargetScope],
    pub layout_selection_authority: Option<Arc<declaration_scope::PackageDeclarationScope>>,
}

/// Accepts the pre-Psi04 typed frontier. Preparing an already specialized
/// frontier again is not part of this boundary.
pub struct SourcePreparationRequest<'request> {
    pub program: crate::source_state::TypedTrees,
    pub admission: SourceAdmission<'request>,
    /// The exact entries, subjects, bindings and supplies later judged.
    pub compilation: checked_program_to_psi::TerminalSourceCompilation<'request>,
    pub accounting: crate::compile_time_invocations::SourceCompletionAccounting,
}

pub use source_completion_accounting::SourcePreparationFailure;

impl<'request> SourcePreparationRequest<'request> {
    /// Complete preparation and let the lowered-form checker judge this exact
    /// request. Build execution receives the accepted program and actual costs.
    pub(super) fn judge(
        self,
        publication: terminal_codec::TerminalPublication,
        providers: InvocationProviders<'_>,
    ) -> Result<JudgedSource, SourcePreparationFailure> {
        let compilation = self.compilation;
        self.settle(providers)?
            .judge(compilation, publication, providers)
    }

    pub(crate) fn prepare(self) -> Result<FrozenSource, SourcePreparationFailure> {
        self.settle(Default::default())?.freeze()
    }

    fn settle(
        self,
        providers: InvocationProviders<'_>,
    ) -> Result<SettledSource, SourcePreparationFailure> {
        let mut accounting = self.accounting;
        let result = (|| -> Result<_, Vec<Diagnostic>> {
            let mut program = self.program;
            program.opaque_representation_selections = self.admission.representations.to_vec();
            // Layouts, access plans, const arguments and constants can each
            // need another, so they are evaluated in the order their reads
            // require rather than kind by kind.
            let evaluated = crate::compile_time_invocations::evaluate_demanded_values(
                &mut program,
                &crate::compile_time_invocations::ValueDemand {
                    compilation: self.compilation,
                    layout_scopes: self.admission.layout_scopes,
                    authority: self.admission.layout_selection_authority,
                    source_consent: self.admission.invocation_sources,
                },
                &mut accounting,
            )?;
            crate::compile_time_invocations::validate_constant_evaluations(
                &program,
                &accounting,
                &evaluated.constant_reaches,
            )
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "constant materialization receipt join: {reason}"
                ))]
            })?;
            Ok((program, evaluated.inputs))
        })();
        accounting.release_constant_inputs();
        match result {
            Ok((program, values)) => Ok(SettledSource {
                program,
                accounting,
                selected_declarations: self
                    .admission
                    .layout_scopes
                    .iter()
                    .flat_map(|scope| scope.provider_declarations.iter())
                    .chain(providers.overrides)
                    .chain(providers.defaults)
                    .map(|selection| {
                        let provider = &selection.provider_type;
                        SelectedDeclaration::Provider(SelectedProvider {
                            declaration: provider.symbol,
                            application: provider.applied.then(|| {
                                checked_program_to_psi::provider_choice_inputs::source_application(
                                    provider.application,
                                )
                            }),
                        })
                    })
                    .chain(
                        self.admission
                            .entry_applications
                            .iter()
                            .copied()
                            .map(SelectedDeclaration::MachineApplication),
                    )
                    .collect(),
                values,
            }),
            Err(diagnostics) => Err(SourcePreparationFailure {
                diagnostics,
                accounting,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceAdmission, SourcePreparationRequest};

    #[test]
    fn preparation_returns_source_without_a_checked_product() {
        let prepared = SourcePreparationRequest {
            program: crate::source_state::TypedTrees::default(),
            accounting: Default::default(),
            compilation: checked_program_to_psi::TerminalSourceCompilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[],
                subjects: &[],
                purpose: source_provenance::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            admission: SourceAdmission::default(),
        }
        .prepare()
        .expect("empty typed source requires no legacy evidence");
        assert!(prepared.program.trees().machines().is_empty());
    }

    /// Product policy admission already collected these choices from the
    /// executed Build; preparation installs exactly those choices.
    #[test]
    fn preparation_installs_the_admitted_representation_choices() {
        use crate::source_state::opaque_representations::{
            OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
            OpaqueRepresentationLifecycleDisposition, OpaqueRepresentationSelection,
        };
        let admitted = [OpaqueRepresentationSelection::from_validated_application(
            symbols::SymbolHandle::invalid(),
            symbols::SymbolHandle::invalid(),
            Default::default(),
            crate::source_state::ClosedConformanceApplication::default(),
            OpaqueRepresentationLifecycleDisposition::Inert,
            OpaqueRepresentationCopyDisposition::PlacementOnly,
            OpaqueRepresentationApplicationOrigin::NamedConformance,
            symbols::SymbolHandle::invalid(),
            source_text::SourceSpan::default(),
        )];
        let prepared = SourcePreparationRequest {
            program: crate::source_state::TypedTrees::default(),
            accounting: Default::default(),
            compilation: checked_program_to_psi::TerminalSourceCompilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[],
                subjects: &[],
                purpose: source_provenance::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            admission: SourceAdmission {
                representations: &admitted,
                ..SourceAdmission::default()
            },
        }
        .prepare()
        .expect("admitted representation choices prepare");
        assert_eq!(
            prepared.program.trees().opaque_representation_selections,
            admitted
        );
    }
}
