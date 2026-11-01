//! Private copies born at a checked transition target's entry. The same exact
//! producer roster serves scalar returns, composed returns and successor edges.
//! Selection supplies no storage or permission: ordinary emission must produce
//! these checked values before any target operand and settle their normal death.
use crate::checked_facts::{flow_facts, value_facts};
use crate::lowering_error::{LoweringError, unsupported};

pub(crate) fn roots(
    checked: &checked_trees::CheckedTrees,
    owner: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
) -> Result<Vec<&checked_trees::CheckedStructuralValueRoot>, LoweringError> {
    let Some(transition) = checked
        .facts
        .executable_bindings
        .transition(state, statement as usize)
    else {
        return Ok(Vec::new());
    };
    let Some(flow_state) = flow_facts(checked).control.unique_state(owner, state) else {
        return Ok(Vec::new());
    };
    let mut outgoing = flow_facts(checked)
        .control
        .outgoing
        .span_or_empty(flow_state.outgoing)
        .iter()
        .filter(|row| {
            row.statement_index == statement as usize && row.transition_target == transition.primary
        });
    let Some(row) = outgoing.next() else {
        return Ok(Vec::new());
    };
    if outgoing.next().is_some() {
        return unsupported("selected payload entry has ambiguous completed flow");
    }
    let mut roots = Vec::new();
    for copy in flow_facts(checked)
        .control
        .operands
        .span_or_empty(row.copied_payloads)
    {
        if checked
            .primitive_type_reference(copy.type_reference)
            .is_some()
            && validation::reference_shell(&checked.typed, copy.type_reference).is_none()
        {
            continue;
        }
        let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } = copy.completion
        else {
            return unsupported("selected payload lost its completed copy identity");
        };
        let root = value_facts(checked)
            .structural_values
            .root_for_captured_copy(state, statement, identity)
            .ok_or(LoweringError::Unsupported(
                "selected structural payload has no checked copied-value root",
            ))?;
        if !validation::type_satisfies_declared_property(
            &checked.typed,
            &[],
            root.type_reference,
            validation::DeclaredPropertyRequirement::Copy,
        ) || !validation::has_contents(
            &checked.typed,
            root.type_reference,
            &[],
            validation::ContentsRequirement::DescriptorCleanup,
        ) {
            return unsupported(
                "selected copied value lost its independent copy/discard capability",
            );
        }
        roots.push(root);
    }
    Ok(roots)
}
