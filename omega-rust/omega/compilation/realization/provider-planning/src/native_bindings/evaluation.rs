pub use package_compilation::NativeBuildEvaluation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBuildSupply {
    pub selection: build_time_evaluation::native_bindings::NativeBindingSelection,
    pub evaluation: std::sync::Arc<NativeBuildEvaluation>,
}
