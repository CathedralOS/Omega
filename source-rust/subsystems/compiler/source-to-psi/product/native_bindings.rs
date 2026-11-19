//! Resolve Build's native supplies in the same source arena that will judge the Product.

use crate::SourceContinuation;
use crate::declaration_selection::CapturedNativeIntent;
use crate::product::native_binding_declarations::CapturedNativeDeclaration;
use crate::provider_choice_inputs::{NativeBindingSelection, NativeBuildSupply};
use diagnostics::Diagnostic;
use evaluation_receipts::NativeBuildEvaluation;
use std::sync::Arc;

/// One supplying dependency's source-admitted declarations and completed Build effects.
pub struct PackageNativeBindings<'a> {
    pub declarations: Vec<CapturedNativeDeclaration>,
    pub intents: &'a [CapturedNativeIntent],
    pub evaluation: Option<&'a Arc<NativeBuildEvaluation>>,
}

/// Evaluation of the local Build and source-admitted handoffs from Product dependencies.
#[derive(Clone, Default)]
pub struct NativeBindingInputs<'a> {
    pub evaluation: Option<&'a Arc<NativeBuildEvaluation>>,
    pub packages: &'a [PackageNativeBindings<'a>],
}

impl NativeBindingInputs<'_> {
    pub(crate) fn resolve(
        &self,
        source: &mut SourceContinuation,
        local: Vec<NativeBindingSelection>,
    ) -> Result<Vec<NativeBuildSupply>, Vec<Diagnostic>> {
        let mut supplies = Vec::new();
        attach(&mut supplies, local, self.evaluation)?;
        for package in self.packages {
            let selections =
                source.admit_captured_native_bindings(&package.declarations, package.intents)?;
            attach(&mut supplies, selections, package.evaluation)?;
        }
        Ok(supplies)
    }
}

fn attach(
    supplies: &mut Vec<NativeBuildSupply>,
    selections: Vec<NativeBindingSelection>,
    evaluation: Option<&Arc<NativeBuildEvaluation>>,
) -> Result<(), Vec<Diagnostic>> {
    if selections.is_empty() {
        return Ok(());
    }
    let evaluation = evaluation.ok_or_else(|| {
        vec![Diagnostic::error(
            "native Build selection lost its completed evaluation",
        )]
    })?;
    supplies.extend(selections.into_iter().map(|selection| NativeBuildSupply {
        selection,
        evaluation: Arc::clone(evaluation),
    }));
    Ok(())
}
