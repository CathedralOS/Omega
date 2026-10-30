//! Derive native candidates from authenticated, completed Build choices.
//!
//! Source admission supplies exact Product requirements. Evaluation evidence
//! retains the actual Terminal program and measured activation; neither is an
//! executable permission. The native provider path still joins the complete
//! requirement schema, calling policy, target normalization and opaque promise.

mod evaluation;
pub use evaluation::{NativeBuildEvaluation, NativeBuildSupply};

mod plans;
pub use plans::NativeBuildRowOrigin;
pub(crate) use plans::derive;

use checked_trees::typed_trees::TypedTrees;
use symbols::SymbolHandle;

pub(crate) fn table_parameter_type(
    typed: &TypedTrees,
    requirement: SymbolHandle,
    table: SymbolHandle,
) -> Result<checked_trees::typed_trees::types::TypeReferenceHandle, String> {
    use checked_trees::typed_trees::types::TypeReferenceNode;
    let parameter = if let Some((_, signature)) = typed.trait_machine_signature(requirement) {
        typed
            .state_signature_parameters(signature)
            .first()
            .map(|parameter| parameter.type_reference)
    } else {
        typed
            .machine(requirement)
            .and_then(|machine| typed.machine_states(machine).first())
            .and_then(|state| typed.state_parameters(state).first())
            .map(|parameter| parameter.type_reference)
    }
    .ok_or("table requirement must have an explicit first table borrow")?;
    let strip = |mut reference| {
        while let TypeReferenceNode::Constrained { base_type, .. } =
            typed.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        reference
    };
    let TypeReferenceNode::Reference {
        referee, access, ..
    } = typed.type_reference_table.type_reference(strip(parameter))
    else {
        return Err("table requirement first argument must be a table borrow".into());
    };
    if *access == language_semantics::ReferenceAccess::WriteOnly {
        return Err("table dispatch requires a readable table borrow".into());
    }
    let table_type = strip(*referee);
    let definition = typed
        .closed_data_definition(table_type)
        .ok_or("table requirement has no exact closed nominal table application")?;
    if definition.symbol != table {
        return Err("table requirement does not borrow the exact selected data declaration".into());
    }
    Ok(table_type)
}
