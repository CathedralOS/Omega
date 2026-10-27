//! Invocation-owned physical answers, requested after type specialization.
use std::sync::Arc;
use typed_trees::{TypedTrees, references::DeclarationSelection};

/// Supplying geometry grants no invocation or declaration-selection authority.
pub trait BuildTimeLayoutQueries: Send + Sync {
    fn prepare(
        &self,
        program: &mut TypedTrees,
        selection: &DeclarationSelection,
    ) -> Result<(), Vec<diagnostics::Diagnostic>>;
}

/// Process-local input retained beside any judgment that consumes it. Identity
/// keeps distinct selected capsules from sharing preparation admission.
#[derive(Clone)]
pub struct PreparedLayoutQueries(Arc<dyn BuildTimeLayoutQueries>);

impl PreparedLayoutQueries {
    pub fn new(queries: impl BuildTimeLayoutQueries + 'static) -> Self {
        Self(Arc::new(queries))
    }

    pub fn prepare(
        &self,
        program: &mut TypedTrees,
        selected: &DeclarationSelection,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        let mut selection = selected.clone();
        loop {
            let mut changed = false;
            for specialization in &program.machine_specializations {
                if selection.contains(specialization.template)
                    && !selection.contains(specialization.instance)
                {
                    selection.include(specialization.instance);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        self.0.prepare(program, &selection)
    }
}

impl PartialEq for PreparedLayoutQueries {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for PreparedLayoutQueries {}
impl std::fmt::Debug for PreparedLayoutQueries {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("PreparedLayoutQueries")
            .field(&Arc::as_ptr(&self.0))
            .finish()
    }
}
