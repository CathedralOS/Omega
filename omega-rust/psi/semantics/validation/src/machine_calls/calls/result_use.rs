use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::statement::TableCall;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// FROZEN DECISION 9 -- STRICT RESULT USE: a statement-position call whose callee
/// returns a non-unit value must not silently drop that value. Intentional
/// discards are spelled `_ = call();` (which sets `discards_result`). "Non-unit"
/// means the resolved callee declares a return type (`-> T`) that is not `()`.
///
/// PROOF-MACHINE callees are exempt (owner, 2026-07-12): a bare statement
/// call to a proof machine is a CITATION (ch10 "Citing Proofs") -- the
/// lemma is invoked for its ensures and erases at codegen, so there is no
/// runtime result to drop. The exemption is a property of the callee's
/// (computed) classification, visible at its declaration -- never of the
/// call site's context.
pub(super) fn validate_result_use(
    program: &TypedTrees,
    call: &TableCall,
    target_name: &str,
    return_type: TypeReferenceHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if call.discards_result || !return_type.is_valid() {
        return;
    }

    if matches!(
        program.type_reference_table.type_reference(return_type),
        TypeReferenceNode::Unit
    ) {
        return;
    }

    if call.receiver.is_empty() {
        let classification = typed_trees::proof_only::classify(program);
        let is_citation = program
            .machines()
            .iter()
            .find(|candidate| {
                candidate.attached_data.is_none() && candidate.name.as_str() == call.target.as_str()
            })
            .is_some_and(|callee| classification.is_proof_machine(program, callee));
        if is_citation {
            return;
        }
    }

    diagnostics.push(Diagnostic::error(format!(
        "call to `{target_name}` discards its non-unit `{}` result; consume the value or discard it explicitly with `_ = {target_name}(...);`",
        program.display_type_reference_with_constraints(return_type)
    )));
}
