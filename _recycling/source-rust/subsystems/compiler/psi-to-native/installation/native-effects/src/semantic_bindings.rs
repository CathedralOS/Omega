// Former aggregate for evaluated native productions and equivalent origins.
use crate::provider_plan::EvaluatedNativeBinding;
use declaration_contracts::selection::NativeBindingProducer;

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
