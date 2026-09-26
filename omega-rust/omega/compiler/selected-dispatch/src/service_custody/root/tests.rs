//! ProgramEntry service nomination reads establishment's field traversal.

use super::program_entry_service_requirements;
use crate::core_service_fixture::typed_with_core_service;
use checked_trees::CheckedTrees;

const SOURCE: &str = r#"
    pub boundary trait StorageHost {
        machine flush(value: i32);
    }
    pub boundary trait ClockHost {
        machine tick(value: i32);
    }
    pub boundary trait UnusedHost {
        machine idle(value: i32);
    }

    data Storage { host: Binding<StorageHost>; writes: i32; }
    data Wrapper { storage: Storage; }
    data Plain { count: i32; }

    data Main {
        clock: Binding<ClockHost>;
        wrapped: Wrapper;
        plain: Plain;
    }
    machine Main::main(&mut self) {}

    data Unrelated { host: Binding<UnusedHost>; }
"#;

fn checked() -> CheckedTrees {
    typed_trees_to_checked_trees::lower_typed_trees(
        typed_with_core_service("selected-dispatch/program_entry_services.omg", SOURCE),
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check program-entry service fixture")
}

fn machine_symbol(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .machines()
        .iter()
        .find(|machine| checked.symbols.display_path(machine.symbol, "::") == name)
        .unwrap_or_else(|| panic!("fixture machine `{name}`"))
        .symbol
}

fn requirement_names(checked: &CheckedTrees, machine: &str) -> Vec<String> {
    program_entry_service_requirements(checked, machine_symbol(checked, machine))
        .into_iter()
        .map(|requirement| checked.symbols.display_path(requirement, "::"))
        .collect()
}

/// A `Binding<R>` two records below the receiver is nominated beside the
/// direct one, in authored route order; a plain nested record contributes
/// nothing, and a carrier on a record the entry does not attach is ignored.
/// Establishment derives its rows from the same traversal, so a nested
/// carrier nomination missed here would reappear there as a missing Fused
/// provider.
#[test]
fn nested_record_binding_fields_are_nominated_with_direct_fields() {
    let checked = checked();
    assert_eq!(
        requirement_names(&checked, "Main::main"),
        ["ClockHost", "StorageHost"],
    );
}
