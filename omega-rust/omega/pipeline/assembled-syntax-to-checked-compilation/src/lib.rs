#![forbid(unsafe_code)]

//! Assembled syntax to one sealed checked compilation.
//!
//! Start at `checking.rs`. This stage takes the assembled forest, evaluates
//! its build machine, resolves and types the source that build generated
//! against the retained base, settles providers, dispatch, entry bindings and
//! task activations, admits package declarations, checks, and seals the
//! result with its source custody. The `checking` folder holds the build
//! continuation, the phase transitions into the Psi stages, execution
//! settlement, and the sealed carrier; `admission` settles the checked
//! program's trust obligations against the owner's admissions and writes the
//! optional observations; `package` owns declaration admission;
//! `optimization` owns the checked optimization handoff and the release
//! rollback request every product settles against.

mod admission;
mod checking;
mod optimization;
mod package;

pub use admission::{CheckedAdmission, admit_checked_compilation};
pub use checking::compile_thread::run_on_compile_thread;
pub use checking::{
    CheckedCompilation, CheckedCompileRequest, IndependentComponentDiscovery,
    IndependentComponentSelection, PreparedCheckedSource, compile_to_checked,
};
pub use optimization::rollback::{
    OptimizationRollback, OptimizationRollbackInputError, OptimizationRollbackSettlement,
};
