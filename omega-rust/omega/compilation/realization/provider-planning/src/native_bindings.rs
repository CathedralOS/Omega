// Native provider candidate derivation from authenticated, completed Build
// choices. Source admission supplies exact Product requirements while
// evaluation evidence retains the Terminal program and measured activation;
// neither grants execution permission until the native provider path joins the
// complete requirement schema, calling policy, target normalization, and opaque
// representation promise.

pub use package_compilation::NativeBuildEvaluation;

mod plans;
pub use plans::{NativeBuildRowOrigin, PreparedNativeBuildBindings};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBuildSupply {
    pub selection: build_time_evaluation::native_bindings::NativeBindingSelection,
    pub evaluation: std::sync::Arc<NativeBuildEvaluation>,
}
