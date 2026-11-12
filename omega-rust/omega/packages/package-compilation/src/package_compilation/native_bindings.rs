// Native binding declarations, evaluated locators, and Terminal evaluation
// custody (`NativeBuildOutputs`, `NativeBindingProduction`,
// `PackageNativeBindingOrigin`) carried inside `PackageBuildHandoff`. Admitting
// native declarations into a receiving compilation uses the same immutable
// source admission as retained provider selections.

use super::PackageBuildHandoff;
use build_time_evaluation::native_bindings::CapturedNativeDeclaration;
use effects::provider_plan::EvaluatedNativeBinding;
use language_semantics::declaration_selection::CapturedNativeIntent;
pub use language_semantics::declaration_selection::NativeBindingProducer;
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
            declaration.source.producer.package != package
                || declaration.source.producer.declaration_path.is_empty()
                || declaration.source.producer.callable_identity.is_empty()
                || declaration.source.requirement_path.is_empty()
                || declaration.requirement_identity.is_empty()
                || declaration.requirement.kind() != symbols::SymbolKind::State
                || declaration.requirement.source().dependency_scope
                    != source::DependencyScope::Product
                || declaration
                    .dependency_alias
                    .as_ref()
                    .is_some_and(|alias| alias.is_empty())
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
    pub fn admit_package_native_declarations(
        &self,
        handoff: &PackageBuildHandoff,
    ) -> Result<Vec<CapturedNativeDeclaration>, Vec<diagnostics::Diagnostic>> {
        handoff
            .native_declarations()
            .iter()
            .map(|declaration| {
                let mut admitted = declaration.clone();
                admitted.requirement =
                    self.admit_retained_product_declaration(handoff, &declaration.requirement)?;
                Ok(admitted)
            })
            .collect()
    }

    pub fn product_dependency_native_handoffs(&self) -> impl Iterator<Item = &PackageBuildHandoff> {
        self.dependency_build_handoff_instances()
            .filter(|(scope, _)| *scope == build_declarations::DependencyPurpose::Product)
            .map(|(_, handoff)| handoff)
    }
}
