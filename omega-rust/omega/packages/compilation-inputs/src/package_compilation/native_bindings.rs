//! Completed Build intent under the existing package source/output custody.
//! Product requirements and native ABI plans are resolved once, in the final
//! checked consumer. This output grants no foreign execution authority.
use super::{PackageCompilationInputError, PackageCompilationInputs, PackageGeneratedSourceBundle};
use effects::provider_plan::EvaluatedNativeBinding;
pub use language_semantics::declaration_selection::NativeBindingProducer;
use language_semantics::declaration_selection::{CapturedNativeDeclaration, CapturedNativeIntent};
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageNativeBindingBundle {
    contents: Arc<Contents>,
}
#[derive(Debug, PartialEq, Eq)]
struct Contents {
    source: PackageGeneratedSourceBundle,
    declarations: Vec<CapturedNativeDeclaration>,
    intents: Vec<CapturedNativeIntent>,
    evaluation: Option<Arc<super::NativeBuildEvaluation>>,
}
impl PackageNativeBindingBundle {
    #[doc(hidden)]
    pub fn from_checked(
        source: PackageGeneratedSourceBundle,
        declarations: Vec<CapturedNativeDeclaration>,
        intents: Vec<CapturedNativeIntent>,
        evaluation: Option<Arc<super::NativeBuildEvaluation>>,
    ) -> Result<Self, String> {
        if declarations.iter().any(|decl| {
            decl.producer.package != source.package()
                || decl.producer.declaration_path.is_empty()
                || decl.producer.callable_identity.is_empty()
                || decl.requirement_path.is_empty()
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
        Ok(Self {
            contents: Arc::new(Contents {
                source,
                declarations,
                intents,
                evaluation,
            }),
        })
    }
    pub fn source(&self) -> &PackageGeneratedSourceBundle {
        &self.contents.source
    }
    pub fn declarations(&self) -> &[CapturedNativeDeclaration] {
        &self.contents.declarations
    }
    pub fn intents(&self) -> &[CapturedNativeIntent] {
        &self.contents.intents
    }
    pub fn evaluation(&self) -> Option<&Arc<super::NativeBuildEvaluation>> {
        self.contents.evaluation.as_ref()
    }
}

impl PackageCompilationInputs {
    /// Join every completed dependency output, including empty native rosters.
    /// Equality with the already-validated source bundle prevents a native
    /// choice from another source, role or activation profile being substituted.
    pub fn with_complete_dependency_native_bindings(
        mut self,
        bundles: Vec<PackageNativeBindingBundle>,
    ) -> Result<Self, Vec<PackageCompilationInputError>> {
        let mut joined = std::collections::BTreeMap::new();
        let mut errors = Vec::new();
        for bundle in bundles {
            let source = bundle.source();
            let key = (source.package(), source.purpose());
            if self.target.dependency_generated_sources.get(&key) != Some(source) {
                errors.push(PackageCompilationInputError::NativeBindingBundleMismatch {
                    package: key.0,
                    reason: "native output has no equal fresh source/output custody".into(),
                });
                continue;
            }
            if joined.insert(key, bundle).is_some() {
                errors.push(PackageCompilationInputError::NativeBindingBundleMismatch {
                    package: key.0,
                    reason: "native output repeats one checked package purpose".into(),
                });
            }
        }
        for key in self.target.dependency_generated_sources.keys() {
            if !joined.contains_key(key) {
                errors.push(PackageCompilationInputError::NativeBindingBundleMismatch {
                    package: key.0,
                    reason: "native output roster omitted a completed dependency".into(),
                });
            }
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        self.target.dependency_native_bindings = joined;
        Ok(self)
    }
    /// Relative import scope remains separate from the producing activation's
    /// purpose. A Build-role dependency must never grant Product native supply.
    pub fn product_dependency_native_bindings(
        &self,
    ) -> impl Iterator<Item = &PackageNativeBindingBundle> {
        self.dependency_generated_source_instances()
            .filter(|(scope, _)| *scope == build_declarations::DependencyPurpose::Product)
            .filter_map(|(_, source)| {
                self.target
                    .dependency_native_bindings
                    .get(&(source.package(), source.purpose()))
            })
    }
    pub fn dependency_native_binding_bundles(
        &self,
    ) -> impl Iterator<Item = &PackageNativeBindingBundle> {
        self.target.dependency_native_bindings.values()
    }
}
