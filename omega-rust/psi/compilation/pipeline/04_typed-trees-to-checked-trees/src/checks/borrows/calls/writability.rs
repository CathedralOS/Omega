use checked_trees::expression::ExpressionNode;
use checked_trees::{BorrowAccessKind, BorrowCallFact, CheckFacts, FlowStateFact};
use diagnostics::Diagnostic;

use crate::checks::borrows::details::binding_reference_access;
use crate::checks::borrows::resources::invalid_reborrow_attenuation_diagnostic;
use crate::semantic::calls::{call_site_argument_expressions, find_call_site};

pub(super) fn check_mutable_argument_writability(
    program: &typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    borrow_call: &BorrowCallFact,
    entry_constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    target_name: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(call_site) = find_call_site(
        program,
        state_flow.machine_symbol,
        state_flow.state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) else {
        return;
    };

    for argument in call_site_argument_expressions(program, &call_site) {
        let ExpressionNode::Borrow(inner_expression) =
            program.expression_table.expression(*argument)
        else {
            continue;
        };
        if !inner_expression.access.is_exclusive() {
            continue;
        }

        // Recast operands lend their validated backing place. Resolve the same
        // place as borrow admission, rather than recognizing a second set of
        // source spellings or granting writability to the viewed type.
        let Some(place) = crate::borrow::accesses::borrow_access_place(
            program,
            state_flow.state_symbol,
            borrow_call.statement_index,
            *argument,
            state_flow.machine_symbol,
        ) else {
            diagnostics.push(Diagnostic::error(format!(
                "mutable argument for state `{target_name}` must be a named place"
            )));
            continue;
        };

        let root_symbol =
            if place.root_symbol == state_flow.machine_symbol && place.segments.is_empty() {
                crate::semantic::calls::find_state(program, state_flow.state_symbol)
                    .and_then(|state| {
                        program
                            .state_parameters(state)
                            .iter()
                            .find(|parameter| parameter.is_self)
                    })
                    .map_or(place.root_symbol, |parameter| parameter.symbol)
            } else {
                place.root_symbol
            };
        let root_name = program.symbols.name(root_symbol);
        let root_is_writable = facts
            .flow
            .borrow_writable_root_constraints(entry_constraints)
            .any(|root| {
                let root = facts.borrow.writable_roots.get(root);
                root.symbol == root_symbol
            });
        if !root_is_writable {
            diagnostics.push(Diagnostic::error(format!(
                "mutable argument `{root_name}` for state `{target_name}` is not writable in this state"
            )));
            continue;
        }

        // A direct `&mut`/`&write` argument formed on a reference-typed
        // binding is a transient referent reborrow: the binding's own storage
        // is a writable local root, so the writable-root test above cannot
        // see the referent's authority. The binding's declared access is the
        // parent authority and the lattice's access-pair rule decides the
        // formation — the same edge a `let`-bound reborrow of this binding
        // faces. `&write r` on `r: &u8` can never derive write authority from
        // a shared referent, no matter how briefly the borrow lives.
        let child_access = match inner_expression.access {
            language_semantics::ReferenceAccess::Shared => BorrowAccessKind::Read,
            language_semantics::ReferenceAccess::Mutable => BorrowAccessKind::Mutable,
            language_semantics::ReferenceAccess::WriteOnly => BorrowAccessKind::WriteOnly,
        };
        if let Some(parent_access) = binding_reference_access(
            program,
            state_flow.machine_symbol,
            state_flow.state_symbol,
            borrow_call.statement_index,
            root_symbol,
        ) && parent_access
            .direct_reborrow_effect(&child_access)
            .is_none()
        {
            diagnostics.push(invalid_reborrow_attenuation_diagnostic(
                &parent_access,
                &child_access,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::tests::front_end::checked_program_result;

    const INLINE_SUFFIX: &str = r#"
        data Buffer { bytes: [u8; 8]; }
        machine enter(root: &mut Buffer, index: u32 [0..=4]) {
            transition true {
                true -> consume(&mut root.bytes[index] as &mut [u8])
            }
            state consume(window: &mut [u8]) {}
        }
    "#;

    #[test]
    fn inline_recast_writability_uses_the_validated_backing_root() {
        for source in [
            INLINE_SUFFIX.to_owned(),
            INLINE_SUFFIX.replace("bytes[index]", "bytes[1]"),
            INLINE_SUFFIX.replace("machine enter(", "machine Buffer::enter("),
            INLINE_SUFFIX
                .replace("machine enter(", "machine Buffer::enter(")
                .replace(
                    "&mut root.bytes[index] as &mut [u8]",
                    "&mut root.bytes[index]",
                )
                .replace("window: &mut [u8]", "window: &mut u8"),
        ] {
            checked_program_result(&source)
                .unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}\n{source}"));
        }
    }

    #[test]
    fn inline_recast_writability_cannot_escalate_a_shared_backing_root() {
        let shared = INLINE_SUFFIX.replace("root: &mut Buffer", "root: &Buffer");
        for source in [
            shared.clone(),
            shared.replace("machine enter(", "machine Buffer::enter("),
            shared
                .replace("machine enter(", "machine Buffer::enter(")
                .replace(
                    "&mut root.bytes[index] as &mut [u8]",
                    "&mut root.bytes[index]",
                )
                .replace("window: &mut [u8]", "window: &mut u8"),
        ] {
            let diagnostics = checked_program_result(&source)
                .expect_err("a mutable result view cannot authorize a shared source");
            assert!(
                diagnostics.iter().any(|diagnostic| {
                    diagnostic.message.contains("not writable")
                        || diagnostic.message.contains("mutable")
                        || diagnostic.message.contains("exclusive")
                }),
                "{diagnostics:#?}\n{source}"
            );
        }
    }
}
