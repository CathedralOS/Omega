//! Native choices are a facet of the one complete dependency Build handoff.
//! Formation, evaluated locators and the original Terminal execution travel
//! together; this retained output supplies no foreign execution authority.

use super::PackageBuildHandoff;
use effects::provider_plan::EvaluatedNativeBinding;
pub use language_semantics::declaration_selection::NativeBindingProducer;
use language_semantics::declaration_selection::{CapturedNativeDeclaration, CapturedNativeIntent};
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBindingProduction {
    pub producer: NativeBindingProducer,
    pub evaluated: EvaluatedNativeBinding,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageNativeBindingOrigin {
    pub primary: NativeBindingProduction,
    pub equivalents: Vec<NativeBindingProduction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct NativeBuildOutputs {
    pub declarations: Vec<CapturedNativeDeclaration>,
    pub intents: Vec<CapturedNativeIntent>,
    pub evaluation: Option<Arc<super::NativeBuildEvaluation>>,
}

impl NativeBuildOutputs {
    pub fn checked(
        package: PackageKeyIdentity,
        declarations: Vec<CapturedNativeDeclaration>,
        intents: Vec<CapturedNativeIntent>,
        evaluation: Option<Arc<super::NativeBuildEvaluation>>,
    ) -> Result<Self, String> {
        if declarations.iter().any(|declaration| {
            declaration.producer.package != package
                || declaration.producer.declaration_path.is_empty()
                || declaration.producer.callable_identity.is_empty()
                || declaration.requirement_path.is_empty()
        }) {
            return Err(
                "native output changes its exact checked package or source declaration".into(),
            );
        }
        if intents
            .iter()
            .any(|intent| intent.declaration as usize >= declarations.len())
            || !intents.is_empty() && evaluation.is_none()
        {
            return Err("native output lost its source formation or completed evaluation".into());
        }
        if let Some(evaluation) = &evaluation {
            evaluation
                .usage
                .validate()
                .map_err(|reason| format!("native Build evaluation usage: {reason:?}"))?;
            if !matches!(
                evaluation.usage.measurement,
                evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi { .. }
            ) {
                return Err(
                    "native Build bindings require their actual Terminal evaluation".into(),
                );
            }
        }
        Ok(Self {
            declarations,
            intents,
            evaluation,
        })
    }
}

impl super::PackageCompilationInputs {
    /// A dependency's Build may name requirements outside the module imported
    /// by its consumer. Reconstruct the supplier's ordinary Product catalogue
    /// before Psi admits every captured formation, including unused bindings.
    /// Loading these roots grants neither private nameability to the consumer
    /// nor Build visibility; exact declaration admission remains independent.
    pub(crate) fn native_binding_source_seeds(
        &self,
    ) -> Result<Vec<std::path::PathBuf>, Vec<diagnostics::Diagnostic>> {
        let mut sources = Vec::new();
        for handoff in self.product_dependency_native_handoffs() {
            if handoff.native_declarations().is_empty() {
                continue;
            }
            let root = self.package_root(handoff.package()).ok_or_else(|| {
                vec![diagnostics::Diagnostic::error(
                    "captured native declarations have no admitted Product source supplier",
                )]
            })?;
            let project = crate::source_imports::project_roots(root, self.source_filesystem());
            // Module-only packages need not declare a Product entry root.
            // Their imported modules still owe every captured declaration to
            // the same admission query; absence here skips no binding check.
            sources.extend(project.sources.into_iter().filter(|source| {
                Some(source) != project.build.as_ref() && self.source_filesystem().is_file(source)
            }));
        }
        Ok(sources)
    }

    /// Relative import purpose, rather than the producer's own purpose, decides
    /// whether these outputs may supply the receiving Product.
    pub fn product_dependency_native_handoffs(&self) -> impl Iterator<Item = &PackageBuildHandoff> {
        self.dependency_build_handoff_instances()
            .filter(|(scope, _)| *scope == build_declarations::DependencyPurpose::Product)
            .map(|(_, handoff)| handoff)
    }
}
