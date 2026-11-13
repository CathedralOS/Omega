//! A domain cast, `value as =Keep -Remove +Add`, states what happens to every
//! domain its operand's type carries (`wiki/spec/language/domains.md`,
//! "Domain-only casts"). Typing reads the entries of one cast together: it
//! selects the domain each one names, checks that they account for the
//! operand's declared type, and leaves every entry with its result type.
//!
//! One program pins the accounted forms from source, and five twins each
//! break one rule and are rejected with a diagnostic that names the domain.

use typed_trees::deferred_refusals::DeferredRequest;
use typed_trees::expression::ExpressionNode;

// Compilation shares process-wide counters; run one at a time.
static COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Psi 00 through 04 on one fixture, without its Build.
fn typed_frontier(name: &str) -> Result<typed_trees::FrozenTypedTrees, Vec<String>> {
    let _compilation = COMPILATIONS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let main = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../tests/fixtures")
        .join(name)
        .join("main.omg");
    compiler::CompilationSession::default()
        .typed_frontier_without_build(
            compiler::CheckedCompileRequest::new(&main, Some("linux_x86_64")),
            None,
        )
        .map(|frontier| frontier.typed)
        .map_err(|failure| {
            failure
                .into_diagnostics()
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect()
        })
}

#[test]
fn each_entry_of_an_accounted_cast_carries_its_result_type() {
    let typed = &typed_frontier("pass/domains/cast_accounting")
        .unwrap_or_else(|diagnostics| panic!("Psi 00 through 04 must complete: {diagnostics:?}"));
    // Every accounted entry, as `entry => the type after it`. A domain the
    // operand's type wrote keeps that spelling; one the cast supplies is
    // spelled by its declaration.
    let mut entries = Vec::new();
    for (_, expression) in typed.expression_table.expression_entries() {
        let ExpressionNode::Cast(cast) = expression else {
            continue;
        };
        if cast.form.entry().is_none() || !cast.result_type.is_valid() {
            continue;
        }
        entries.push(format!(
            "{} => {}",
            typed
                .expression_table
                .name_path_string(cast.target_label)
                .trim(),
            typed.display_type_reference_with_constraints(cast.result_type),
        ));
    }
    for expected in [
        // `distance as -Km -Positive`, then `(plain - 1) as +Km`.
        "-Km => i32[in Positive]",
        "-Positive => i32",
        "+Km => i32[in i32::Km]",
        // `both as =Km -Positive`: a kept domain changes nothing.
        "=Km => i32[in Km, in Positive]",
        "-Positive => i32[in Km]",
        // `5 as +Km +Positive`.
        "+Positive => i32[in i32::Km, in i32::Positive]",
        // A type spells a child without its parent; removing the child
        // spells the parent the cast kept.
        "=Positive => i32[in Positive::Small]",
        "-Positive::Small => i32[in i32::Positive]",
        // `near as -Km as i64 +Far`: the addition lands on the new carrier.
        "+Far => i64[in i64::Far]",
    ] {
        assert!(
            entries.iter().any(|entry| entry == expected),
            "missing `{expected}` in {entries:#?}"
        );
    }

    // An added domain keeps its declaration on the node, for the reader that
    // judges whether this machine may issue it.
    let added = typed
        .expression_table
        .expression_entries()
        .filter_map(|(_, expression)| match expression {
            ExpressionNode::Cast(cast) if cast.semantic_domain_symbol.is_valid() => {
                Some(typed.symbols.name(cast.semantic_domain_symbol).to_owned())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(added.iter().any(|name| name.ends_with("Km")), "{added:?}");
    assert!(added.iter().any(|name| name.ends_with("Far")), "{added:?}");

    // The cast of a call result is recorded on the machine that holds it
    // and on no other declaration.
    let machine = |name: &str| {
        typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap_or_else(|| panic!("machine `{name}` is declared"))
            .symbol
    };
    assert_eq!(
        typed
            .deferred_refusal(machine("deferred"))
            .map(|refusal| refusal.request),
        Some(DeferredRequest::DomainCastOperandType)
    );
    for accounted in ["previous", "keeps", "literal", "ancestors", "widened"] {
        assert!(
            typed.deferred_refusal(machine(accounted)).is_none(),
            "`{accounted}` is accounted"
        );
    }
}

fn rejected_with(name: &str, expected: &str) {
    let Err(diagnostics) = typed_frontier(name) else {
        panic!("`{name}` breaks the accounting and must be rejected");
    };
    assert!(
        diagnostics.iter().any(|message| message.contains(expected)),
        "{diagnostics:#?}"
    );
}

#[test]
fn an_attached_domain_the_cast_does_not_name_is_rejected() {
    rejected_with(
        "fail/domains/cast_unaccounted_domain",
        "domain cast leaves attached domain `Positive` unaccounted for",
    );
}

#[test]
fn adding_a_domain_the_operand_already_carries_is_rejected() {
    rejected_with(
        "fail/domains/cast_adds_carried_domain",
        "cast entry `+Km` adds domain `Km`, but the operand's type `i32[in Km, in Positive]` already carries `Km`",
    );
}

#[test]
fn a_child_kept_without_its_parent_is_rejected() {
    rejected_with(
        "fail/domains/cast_orphans_child",
        "domain cast would leave `Positive::Small` without its parent `Positive`",
    );
}

#[test]
fn removing_a_domain_the_operand_lacks_is_rejected() {
    rejected_with(
        "fail/domains/cast_removes_absent_domain",
        "cast entry `-Km` names domain `Km`, but the operand's type `i32` does not carry `Km`",
    );
}

#[test]
fn a_carrier_cast_of_a_qualified_operand_is_rejected() {
    rejected_with(
        "fail/domains/cast_carrier_of_qualified",
        "carrier cast `as i64` converts a value whose type `i32[in Km]` carries domain `Km`",
    );
}
