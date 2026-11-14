//! Prepare source for the lowered-form checker without producing an old judgment.
//!
//! Frozen typed source is an input to structural lowering, never an acceptance
//! certificate. Only the lowered-form checker can publish an executable product.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    ExecutedProductSelection, ProductDeclarationRequest,
};
use std::sync::Arc;
use typed_trees::opaque_representations::OpaqueRepresentationSelection;

mod constant_entries {
    pub(super) fn retain_evaluation_entries(
        program: &mut typed_trees::TypedTrees,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let mut entries = Vec::new();
        for declaration in program.const_declarations() {
            if declaration.evaluated_value.is_some() || !declaration.evaluation_entry.is_valid() {
                continue;
            }
            if !program
                .tables
                .machines
                .is_valid(declaration.evaluation_entry)
            {
                return Err(vec![diagnostics::Diagnostic::error(
                    "retained constant lost its complete evaluation entry",
                )]);
            }
            let entry = program.tables.machines.get(declaration.evaluation_entry);
            if program.machine_by_symbol(entry.symbol).is_none() {
                entries.push(entry.clone());
            }
        }
        for entry in entries {
            program.push_machine(entry);
        }
        Ok(())
    }
}
#[derive(Default)]
pub struct SourceAdmission<'request> {
    pub external_roots: &'request [symbols::SymbolHandle],
    pub completion: typed_trees::boundary_completions::SourceCompletionAdmission<'request>,
    pub representations: ProductRepresentationInput<'request>,
    pub layout_scopes: &'request [build_time_evaluation::FixedLayoutTargetScope],
    pub layout_selection_authority:
        Option<Arc<dyn build_time_evaluation::BuildTimeSelectionAuthority>>,
    pub asm_authority: Option<language_core::inline_assembly::AsmAuthorityAdmission>,
    pub wire_demands: &'request [language_semantics::wire::WireCompatibilityDemand],
    pub declarations: Option<
        &'request dyn semantic_vocabulary::compiler_evaluation::DeclarationSelectionAuthority,
    >,
}

#[derive(Default)]
pub struct ProductRepresentationInput<'request> {
    pub activation: Option<symbols::SymbolHandle>,
    pub declared: &'request [Arc<ProductDeclarationRequest>],
    pub executed: &'request [ExecutedProductSelection],
    pub retained: &'request [OpaqueRepresentationSelection],
}

impl ProductRepresentationInput<'_> {
    /// Whether this Build asked for an opaque representation: it authored or
    /// executed a representation selection, or an earlier admission retained
    /// one. A Build activation that made other requests asks for none.
    fn requested(&self) -> bool {
        build_time_evaluation::build_requests_any(
            self.declared,
            self.executed,
            &[language_semantics::declaration_selection::BuildOperation::RepresentationSelection],
        ) || !self.retained.is_empty()
    }

    /// The planner that admitted an opaque representation was removed with
    /// the old checker, and nothing on the lowered-form route admits one
    /// yet. A program that asks for none owes that admission nothing. One
    /// that asks, whether by an authored request it never ran, an executed
    /// one, or a retained application, is refused: the request may not be
    /// dropped and presented to the route as an empty choice.
    pub(crate) fn require_none_requested(&self) -> Result<(), Vec<Diagnostic>> {
        if self.requested() {
            return Err(vec![Diagnostic::error(
                "no product route yet: opaque representation admission",
            )]);
        }
        Ok(())
    }
}

/// Accepts the pre-Psi04 typed frontier. Preparing an already specialized
/// frontier again is not part of this boundary.
pub struct SourcePreparationRequest<'request> {
    pub program: typed_trees::TypedTrees,
    pub admission: SourceAdmission<'request>,
    /// The exact entries, subjects, bindings and supplies later judged.
    pub compilation: crate::TerminalSourceCompilation<'request>,
    pub accounting: build_time_evaluation::SourceCompletionAccounting,
}

pub struct FrozenSource {
    pub program: Arc<typed_trees::FrozenTypedTrees>,
    pub accounting: build_time_evaluation::SourceCompletionAccounting,
}

impl SourcePreparationRequest<'_> {
    pub fn prepare(self) -> Result<FrozenSource, Vec<Diagnostic>> {
        // Opaque property receipts came from the deleted checker. Supplying one
        // cannot stand in for evidence from the new route.
        if !self.admission.external_roots.is_empty() || self.admission.declarations.is_some() {
            return Err(vec![Diagnostic::error(
                "no product route yet: source selection authority admission",
            )]);
        }
        self.admission.representations.require_none_requested()?;
        let mut program = self.program;
        typed_trees::boundary_completions::prepare_source_completion(
            &mut program,
            self.admission.completion,
        )?;
        build_time_evaluation::evaluate_fixed_layout_applications(
            &mut program,
            self.admission.layout_scopes,
            self.admission.layout_selection_authority.clone(),
        )?;
        constant_entries::retain_evaluation_entries(&mut program)?;
        let mut accounting = self.accounting;
        accounting.append(build_time_evaluation::evaluate_constant_initializers(
            &mut program,
            self.compilation,
            self.admission.layout_selection_authority,
        )?);
        build_time_evaluation::validate_constant_evaluations(&program, &accounting).map_err(
            |reason| {
                vec![Diagnostic::error(format!(
                    "constant materialization receipt join: {reason}"
                ))]
            },
        )?;
        let mut preparation = typed_trees_to_typed_trees::PreparationRequest::settled()
            .with_wire_compatibility_demands(self.admission.wire_demands);
        if let Some(authority) = self.admission.asm_authority {
            preparation = preparation.with_asm_authority(authority);
        }
        let (program, _) = typed_trees_to_typed_trees::prepare_typed_trees(program, &preparation)?;
        Ok(FrozenSource {
            program: Arc::new(program),
            accounting,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{SourceAdmission, SourcePreparationRequest};

    #[test]
    fn preparation_returns_source_without_a_checked_product() {
        let prepared = SourcePreparationRequest {
            program: typed_trees::TypedTrees::default(),
            accounting: Default::default(),
            compilation: crate::TerminalSourceCompilation {
                entries: &[],
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            admission: SourceAdmission::default(),
        }
        .prepare()
        .expect("empty typed source requires no legacy evidence");
        assert!(prepared.program.machines().is_empty());
    }

    #[test]
    fn an_old_external_root_request_is_not_silently_discarded() {
        let roots = [symbols::SymbolHandle::invalid()];
        let result = SourcePreparationRequest {
            program: typed_trees::TypedTrees::default(),
            accounting: Default::default(),
            compilation: crate::TerminalSourceCompilation {
                entries: &[],
                subjects: &[],
                purpose: source::DependencyScope::Product,
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
        let Err(diagnostics) = result else {
            panic!("legacy authority must not become a prepared product");
        };
        assert!(diagnostics[0].message.contains("no product route yet"));
    }

    #[test]
    fn unserved_representation_inputs_are_never_silently_discarded() {
        use super::ProductRepresentationInput;
        use language_semantics::declaration_selection::{
            BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
        };
        use typed_trees::opaque_representations::{
            OpaqueRepresentationApplicationOrigin, OpaqueRepresentationCopyDisposition,
            OpaqueRepresentationLifecycleDisposition, OpaqueRepresentationSelection,
        };

        // These records deliberately carry no accepted semantic evidence. Their
        // presence alone must refuse before preparation can erase their request.
        let request = std::sync::Arc::new(ProductDeclarationRequest {
            source_span: source::SourceSpan::default(),
            operation: BuildOperation::RepresentationSelection,
            operands: Box::new([]),
        });
        let declared = [std::sync::Arc::clone(&request)];
        let executed = [ExecutedProductSelection {
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
            typed_trees::typed_trees::ClosedConformanceApplication::default(),
            OpaqueRepresentationLifecycleDisposition::Inert,
            OpaqueRepresentationCopyDisposition::PlacementOnly,
            OpaqueRepresentationApplicationOrigin::NamedConformance,
            symbols::SymbolHandle::invalid(),
            source::SourceSpan::default(),
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
                program: typed_trees::TypedTrees::default(),
                accounting: Default::default(),
                compilation: crate::TerminalSourceCompilation {
                    entries: &[],
                    subjects: &[],
                    purpose: source::DependencyScope::Product,
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
            let Err(diagnostics) = result else {
                panic!("{dimension} input was silently discarded");
            };
            assert_eq!(
                diagnostics[0].message, "no product route yet: opaque representation admission",
                "{dimension} must reach the missing admission obligation",
            );
        }
    }

    /// The twin: a Build that ran and made requests, none of them a
    /// representation selection, asks nothing of the missing admission.
    #[test]
    fn a_build_that_selects_no_representation_owes_no_representation_admission() {
        use super::ProductRepresentationInput;
        use language_semantics::declaration_selection::{
            BuildOperation, ExecutedProductSelection, ProductDeclarationRequest,
        };

        let request = std::sync::Arc::new(ProductDeclarationRequest {
            source_span: source::SourceSpan::default(),
            operation: BuildOperation::ServiceAudit,
            operands: Box::new([]),
        });
        let declared = [std::sync::Arc::clone(&request)];
        let executed = [ExecutedProductSelection {
            static_environment: None,
            target: None,
            request,
            selecting_machine: None,
            composition: None,
        }];
        SourcePreparationRequest {
            program: typed_trees::TypedTrees::default(),
            accounting: Default::default(),
            compilation: crate::TerminalSourceCompilation {
                entries: &[],
                subjects: &[],
                purpose: source::DependencyScope::Product,
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
