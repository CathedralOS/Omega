//! Prepare source for the lowered-form checker without producing an old judgment.
//!
//! Frozen typed source is an input to structural lowering, never an acceptance
//! certificate. Only the lowered-form checker can publish an executable product.

use crate::declaration_selection::ExecutedProductSelection;
use crate::declaration_selection::ProductDeclarationRequest;
use crate::source_state::opaque_representations::OpaqueRepresentationSelection;
use diagnostics::Diagnostic;
use std::sync::Arc;

mod constant_entries {
    pub(super) fn retain_evaluation_entries(
        program: &mut crate::source_state::TypedTrees,
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
    pub invocation_sources: Option<Arc<dyn crate::evaluation::InvocationSourceConsent>>,
    pub external_roots: &'request [symbols::SymbolHandle],
    pub representations: ProductRepresentationInput<'request>,
    pub layout_scopes: &'request [crate::evaluation::FixedLayoutTargetScope],
    pub layout_selection_authority: Option<Arc<dyn crate::evaluation::BuildTimeSelectionAuthority>>,
    pub wire_demands: &'request [declaration_contracts::wire::WireCompatibilityDemand],
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
    fn rederive(
        &self,
        typed: &crate::source_state::TypedTrees,
    ) -> Result<Vec<OpaqueRepresentationSelection>, Vec<Diagnostic>> {
        let selected = crate::evaluation::opaque_representations::collect(
            typed,
            self.activation,
            self.declared,
            self.executed,
        )?;
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
    pub compilation: crate::TerminalSourceCompilation<'request>,
    pub accounting: crate::evaluation::SourceCompletionAccounting,
}

pub struct FrozenSource {
    pub program: Arc<crate::source_state::FrozenTypedTrees>,
    pub accounting: crate::evaluation::SourceCompletionAccounting,
    /// Actual demand before named uses became literals. This is result
    /// correspondence data, not declaration-selection or execution authority.
    pub constants: Vec<crate::ConstantRead>,
}

pub struct JudgedSource {
    pub accepted: crate::AcceptedTerminalSource,
    pub accounting: crate::evaluation::SourceCompletionAccounting,
    pub constants: Vec<crate::ConstantRead>,
}

/// A failed preparation keeps actual invocation usage, including successful
/// earlier jobs and a metered failed run. No frozen product accompanies it.
#[derive(Debug)]
pub struct SourcePreparationFailure {
    pub diagnostics: Vec<Diagnostic>,
    pub accounting: crate::evaluation::SourceCompletionAccounting,
}

impl SourcePreparationRequest<'_> {
    /// Complete preparation and let the lowered-form checker judge this exact
    /// request. Build execution receives the accepted program and actual costs.
    pub fn judge(
        self,
        publication: terminal_codec::TerminalPublication,
    ) -> Result<JudgedSource, SourcePreparationFailure> {
        let compilation = self.compilation;
        let FrozenSource {
            program,
            accounting,
            constants,
        } = self.prepare()?;
        match crate::AcceptedTerminalSource::judge(program, compilation, publication) {
            Ok(accepted) => Ok(JudgedSource {
                accepted,
                accounting,
                constants,
            }),
            Err(diagnostics) => Err(SourcePreparationFailure {
                diagnostics,
                accounting,
            }),
        }
    }

    pub(crate) fn prepare(self) -> Result<FrozenSource, SourcePreparationFailure> {
        let mut accounting = self.accounting;
        let result = (|| -> Result<_, Vec<Diagnostic>> {
            // Opaque property receipts came from the deleted checker. Supplying one
            // cannot stand in for evidence from the new route.
            if !self.admission.external_roots.is_empty() || self.admission.declarations.is_some() {
                return Err(vec![Diagnostic::error(
                    "no product route yet: source selection authority admission",
                )]);
            }
            let mut program = self.program;
            program.opaque_representation_selections =
                self.admission.representations.rederive(&program)?;
            crate::evaluation::evaluate_fixed_layout_applications(
                &mut program,
                self.admission.layout_scopes,
                self.admission.layout_selection_authority.clone(),
                self.admission.invocation_sources.clone(),
                self.compilation.purpose,
                &mut accounting,
            )?;
            // `domain D access Policy;` leaves who may issue `as +D` to the
            // policy's plan. Each demanded domain's plan runs once here, so
            // Psi 05 reads its issuers from the checked result.
            crate::evaluation::evaluate_domain_access_plans(
                &mut program,
                self.compilation,
                self.admission.layout_selection_authority.clone(),
                self.admission.invocation_sources.clone(),
                &mut accounting,
            )?;
            // `Buffer<capacity()>`: a const argument written as a call gets
            // its value first, because a constant below may be computed
            // through a type that is applied at one.
            crate::evaluation::evaluate_const_arguments(
                &mut program,
                self.compilation,
                self.admission.layout_selection_authority.clone(),
                self.admission.invocation_sources.clone(),
                &mut accounting,
            )?;
            constant_entries::retain_evaluation_entries(&mut program)?;
            let constants = crate::evaluation::evaluate_constant_initializers_with_source_consent(
                &mut program,
                self.compilation,
                self.admission.layout_scopes,
                self.admission.layout_selection_authority,
                self.admission.invocation_sources,
                &mut accounting,
            )?;
            crate::evaluation::validate_constant_evaluations(&program, &accounting).map_err(
                |reason| {
                    vec![Diagnostic::error(format!(
                        "constant materialization receipt join: {reason}"
                    ))]
                },
            )?;
            let preparation = crate::preparation::PreparationRequest::settled()
                .with_wire_compatibility_demands(self.admission.wire_demands);
            let (program, _) = crate::preparation::prepare_typed_trees(program, &preparation)?;
            let key = |symbol: symbols::SymbolHandle| (symbol.arena_index(), symbol.generation());
            let mut declarations = program.const_declarations().iter().collect::<Vec<_>>();
            declarations.sort_unstable_by_key(|declaration| key(declaration.symbol));
            let constants = constants
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
                        declaration: crate::source_symbols::retain_declaration(
                            &program.symbols,
                            symbol,
                        )
                        .ok(),
                        value: declaration.evaluated_value.clone(),
                    })
                })
                .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
            Ok((Arc::new(program), constants))
        })();
        match result {
            Ok((program, constants)) => Ok(FrozenSource {
                program,
                accounting,
                constants,
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
            compilation: crate::TerminalSourceCompilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[],
                subjects: &[],
                purpose: declaration_contracts::source_identity::DependencyScope::Product,
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
            program: crate::source_state::TypedTrees::default(),
            accounting: Default::default(),
            compilation: crate::TerminalSourceCompilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[],
                subjects: &[],
                purpose: declaration_contracts::source_identity::DependencyScope::Product,
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
                compilation: crate::TerminalSourceCompilation {
                    calling: None,
                    providers: None,
                    declaration_metadata: &[],
                    entries: &[],
                    subjects: &[],
                    purpose: declaration_contracts::source_identity::DependencyScope::Product,
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
            static_environment: None,
            target: None,
            request,
            selecting_machine: None,
            composition: None,
        }];
        SourcePreparationRequest {
            program: crate::source_state::TypedTrees::default(),
            accounting: Default::default(),
            compilation: crate::TerminalSourceCompilation {
                calling: None,
                providers: None,
                declaration_metadata: &[],
                entries: &[],
                subjects: &[],
                purpose: declaration_contracts::source_identity::DependencyScope::Product,
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
