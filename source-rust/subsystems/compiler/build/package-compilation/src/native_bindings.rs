// Native binding declarations, evaluated locators, and Terminal evaluation
// custody (`NativeBuildOutputs`, `NativeBindingProduction`,
// `PackageNativeBindingOrigin`) carried inside `PackageBuildHandoff`. Admitting
// native declarations into a receiving compilation uses the same immutable
// source admission as retained provider selections.

use super::PackageBuildHandoff;
use semantic_vocabulary::PackageKeyIdentity;
pub use semantic_vocabulary::source_selections::NativeBindingProducer;
use source_compiler::declaration_selection::CapturedNativeIntent;
use source_compiler::evaluation::native_bindings::CapturedNativeDeclaration;
use std::sync::Arc;

pub use effects::semantic_bindings::{NativeBindingProduction, PackageNativeBindingOrigin};

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
                || declaration.service_declaration == [0; 32]
                || declaration.requirement.kind() != symbols::SymbolKind::State
                || declaration.requirement.source().dependency_scope
                    != source_provenance::DependencyScope::Product
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
        }
        Ok(Self {
            declarations,
            intents,
            evaluation,
        })
    }
}

impl PackageBuildHandoff {
    /// Keep the complete checked declaration and evaluation for each binding
    /// this package supplied. The receiver selects the requirements it reaches.
    pub fn append_native_bindings(
        &self,
        profile: target::TargetProfile,
        bindings: &mut Vec<evaluation_receipts::NativeBindingMaterialization>,
    ) -> Result<(), String> {
        if self.target() != profile {
            return Err("installed package belongs to a different target".into());
        }
        for intent in self
            .native_intents()
            .iter()
            .filter(|intent| intent.target.canonical_name() == profile.target_name())
        {
            // The checked handoff validates every declaration index and requires
            // the completed evaluation whenever the Build produced an intent.
            let declaration = &self.native_declarations()[intent.declaration as usize];
            let evaluation = self
                .native_evaluation()
                .ok_or("installed native binding lost its completed Build evaluation")?;
            bindings.push(evaluation_receipts::NativeBindingMaterialization {
                requirement_identity: declaration.requirement_identity.clone(),
                service_declaration: declaration.service_declaration,
                target: profile.target_name().to_owned(),
                producer_identity: declaration.source.producer.callable_identity.clone(),
                supplier_package: self.package(),
                source_occurrence_digest: declaration.source.source_occurrence_digest,
                locator: intent.locator.clone(),
                evaluation: Arc::clone(evaluation),
            });
        }
        Ok(())
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
                admitted.application = declaration
                    .application
                    .as_ref()
                    .map(|application| {
                        self.admit_retained_product_application(handoff, application)
                    })
                    .transpose()?;
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
