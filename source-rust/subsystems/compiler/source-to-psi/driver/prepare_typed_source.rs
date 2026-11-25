//! Materialize the constant, representation and layout inputs demanded by a
//! source invocation. Stage 04 can then issue the typed snapshot consumed by
//! lowering. The verdict comes from checking that particular lowering.

use crate::compile_product::declaration_lookup::collect_opaque_representations;
use crate::declaration_selection::ExecutedProductSelection;
use crate::declaration_selection::ProductDeclarationRequest;
use crate::driver::judge_lowered_source::{FrozenSource, JudgedSource, SettledSource};
use crate::source_state::opaque_representations::OpaqueRepresentationSelection;
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees_to_prepared_typed_trees::{EntryApplication, SelectedDeclaration, SelectedProvider};

#[derive(Default)]
pub struct SourceAdmission<'request> {
    pub invocation_sources:
        Option<Arc<dyn crate::compile_time_invocations::InvocationSourceConsent>>,
    pub entry_applications: &'request [EntryApplication],
    pub external_roots: &'request [symbols::SymbolHandle],
    pub representations: ProductRepresentationInput<'request>,
    pub layout_scopes: &'request [crate::compile_time_invocations::FixedLayoutTargetScope],
    pub layout_selection_authority: Option<Arc<declaration_scope::PackageDeclarationScope>>,
    pub wire_demands: &'request [crate::WireCompatibilityDemand],
    pub declarations: Option<&'request dyn declaration_scope::DeclarationSelectionAuthority>,
}

#[derive(Default)]
pub struct ProductRepresentationInput<'request> {
    pub activation: Option<symbols::SymbolHandle>,
    pub declared: &'request [Arc<ProductDeclarationRequest>],
    pub executed: &'request [ExecutedProductSelection],
    pub retained: &'request [OpaqueRepresentationSelection],
}

impl ProductRepresentationInput<'_> {
    fn rederive(
        &self,
        typed: &crate::source_state::TypedTrees,
    ) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
        let selected =
            collect_opaque_representations(typed, self.activation, self.declared, self.executed)?;
        if selected != self.retained {
            return Err(vec![Diagnostic::error(
                "retained opaque representation choices disagree with the authoritative Build execution",
            )]);
        }
        Ok(selected)
    }
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
    pub fn judge(
        self,
        publication: terminal_codec::TerminalPublication,
    ) -> Result<JudgedSource, SourcePreparationFailure> {
        let compilation = self.compilation;
        self.settle()?.judge(compilation, publication)
    }

    pub(crate) fn prepare(self) -> Result<FrozenSource, SourcePreparationFailure> {
        self.settle()?.freeze()
    }

    fn settle(self) -> Result<SettledSource, SourcePreparationFailure> {
        let mut accounting = self.accounting;
        let result = (|| -> Result<_, Vec<Diagnostic>> {
            // Opaque property receipts came from the deleted checker. Supplying one
            // cannot stand in for evidence from the new route.
            if !self.admission.external_roots.is_empty() || self.admission.declarations.is_some() {
                return Err(vec![Diagnostic::error(
                    "no product route yet: source selection authority admission",
                )]);
            }
            if !self.admission.wire_demands.is_empty() {
                return Err(diagnostics::DiagnosticStage::Preparation.attribute(vec![Diagnostic::error("no product route yet: typed preparation: requested work: wire compatibility demands")]));
            }
            let mut program = self.program;
            program.opaque_representation_selections =
                self.admission.representations.rederive(&program)?;
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

    #[test]
    fn an_unserved_wire_schema_edge_is_refused_before_freezing() {
        let demand = crate::WireCompatibilityDemand {
            edge: "request".into(),
            lineage: "protocol".into(),
            local_schema: "v2".into(),
            peer_schema: "v1".into(),
            require_readable: true,
            require_writable: false,
            require_unknown_preservation: false,
            require_canonical: false,
            require_complete_migration: false,
        };
        let result = SourcePreparationRequest {
            admission: SourceAdmission {
                wire_demands: &[demand],
                ..Default::default()
            },
            program: Default::default(),
            accounting: Default::default(),
            compilation: checked_program_to_psi::TerminalSourceCompilation {
                entries: &[],
                subjects: &[],
                purpose: source_provenance::DependencyScope::Product,
                declaration_metadata: &[],
                selected: &[],
                bindings: &[],
                supplies: &[],
                calling: None,
                providers: None,
            },
        }
        .prepare();
        let failure = result.err().expect("an unsupported wire edge must refuse");
        assert!(
            failure.diagnostics[0]
                .message
                .contains("wire compatibility demands")
        );
    }

    #[test]
    fn an_old_external_root_request_is_not_silently_discarded() {
        let roots = [symbols::SymbolHandle::invalid()];
        let result = SourcePreparationRequest {
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
                external_roots: &roots,
                ..SourceAdmission::default()
            },
        }
        .prepare();
        let Err(failure) = result else {
            panic!("legacy authority must not become a prepared product");
        };
        assert!(
            failure.diagnostics[0]
                .message
                .contains("no product route yet")
        );
    }

    #[test]
    fn unauthored_representation_inputs_are_refused() {
        use super::ProductRepresentationInput;
        use crate::declaration_selection::BuildOperation;
        use crate::declaration_selection::ExecutedProductSelection;
        use crate::declaration_selection::ProductDeclarationRequest;
        use crate::source_state::opaque_representations::{
            OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
            OpaqueRepresentationLifecycleDisposition, OpaqueRepresentationSelection,
        };

        // These records deliberately carry no accepted semantic evidence. Their
        // presence alone must refuse before preparation can erase their request.
        let request = std::sync::Arc::new(ProductDeclarationRequest {
            source_span: source_text::SourceSpan::default(),
            operation: BuildOperation::RepresentationSelection,
            operands: Box::new([]),
        });
        let declared = [std::sync::Arc::clone(&request)];
        let executed = [ExecutedProductSelection {
            test_group: None,
            static_environment: None,
            target: None,
            request,
            selecting_machine: None,
            composition: None,
        }];
        let retained = [OpaqueRepresentationSelection::from_validated_application(
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
        for (dimension, representations) in [
            (
                "declared",
                ProductRepresentationInput {
                    declared: &declared,
                    ..ProductRepresentationInput::default()
                },
            ),
            (
                "executed",
                ProductRepresentationInput {
                    executed: &executed,
                    ..ProductRepresentationInput::default()
                },
            ),
            (
                "retained",
                ProductRepresentationInput {
                    retained: &retained,
                    ..ProductRepresentationInput::default()
                },
            ),
        ] {
            let result = SourcePreparationRequest {
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
                    representations,
                    ..SourceAdmission::default()
                },
            }
            .prepare();
            let Err(failure) = result else {
                panic!("{dimension} input was silently discarded");
            };
            assert_eq!(
                failure.diagnostics[0].message,
                match dimension {
                    "declared" => "Build request has no original authored occurrence",
                    "executed" => "executed Build request has no original Build activation",
                    _ =>
                        "retained opaque representation choices disagree with the authoritative Build execution",
                },
                "{dimension} must retain its original authority",
            );
        }
    }

    /// An unrelated request creates no representation choice.
    #[test]
    fn a_build_that_selects_no_representation_owes_no_representation_admission() {
        use super::ProductRepresentationInput;
        use crate::declaration_selection::BuildOperation;
        use crate::declaration_selection::ExecutedProductSelection;
        use crate::declaration_selection::ProductDeclarationRequest;

        let request = std::sync::Arc::new(ProductDeclarationRequest {
            source_span: source_text::SourceSpan::default(),
            operation: BuildOperation::ServiceAudit,
            operands: Box::new([]),
        });
        let declared = [std::sync::Arc::clone(&request)];
        let executed = [ExecutedProductSelection {
            test_group: None,
            static_environment: None,
            target: None,
            request,
            selecting_machine: None,
            composition: None,
        }];
        SourcePreparationRequest {
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
                representations: ProductRepresentationInput {
                    activation: Some(symbols::SymbolHandle::invalid()),
                    declared: &declared,
                    executed: &executed,
                    retained: &[],
                },
                ..SourceAdmission::default()
            },
        }
        .prepare()
        .expect("a Build without a representation selection prepares");
    }
}
