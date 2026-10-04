use super::sourced_symbol_table;
use crate::{SymbolKind, SymbolNameRef, SymbolTableBuilder};
use source::SourceOrigin;

#[test]
fn generated_extension_retains_exact_rows_and_transitive_origins() {
    let (base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
    let mut candidate = base.clone();
    assert!(candidate.is_generated_extension_of(&base));
    let instance = candidate.insert_generated_root_from(authored[0], SymbolKind::Data, "Instance");
    let fields = candidate.insert_generated_children(instance, [(SymbolKind::Field, "value")]);
    let field = SymbolTableBuilder::child_handles(fields).next().unwrap();
    candidate.insert_generated_root_from(field, SymbolKind::Data, "Nested");
    assert!(candidate.is_generated_extension_of(&base));
    let retained = candidate.clone();
    candidate.insert_generated_root_from(instance, SymbolKind::Data, "Later");
    assert!(candidate.is_generated_extension_of(&retained));
    assert!(candidate.is_generated_extension_of(&base));
}

#[test]
fn generated_extension_rejects_changed_retained_spelling() {
    let (base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
    let mut candidate = base.clone();
    candidate.rename(authored[0], "substituted");
    assert!(!candidate.is_generated_extension_of(&base));
}

#[test]
fn generated_extension_rejects_appended_authored_children() {
    let (base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
    let mut candidate = base.clone();
    candidate.append_children(
        authored[0],
        [(SymbolKind::Local, SymbolNameRef::Borrowed("new_local"))],
    );
    assert!(!candidate.is_generated_extension_of(&base));
}

#[test]
fn generated_extension_rejects_changed_existing_child_range() {
    let (mut base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
    let instance = base.insert_generated_root_from(authored[0], SymbolKind::Data, "Instance");
    let mut candidate = base.clone();
    candidate.insert_generated_children(instance, [(SymbolKind::Field, "late")]);
    assert!(!candidate.is_generated_extension_of(&base));
}

#[test]
fn generated_extension_rejects_unknown_generated_declaration() {
    let (base, authored) = sourced_symbol_table([(SourceOrigin::User, None)]);
    let mut candidate = base.clone();
    candidate.insert_generated_root_from(authored[0], SymbolKind::Unknown, "unknown");
    assert!(!candidate.is_generated_extension_of(&base));
}

#[test]
fn generated_extension_rejects_rebound_source_vocabulary() {
    let base = super::main_entry_symbols();
    let candidate = base
        .clone()
        .begin_extension(
            None,
            vec![crate::SourceScopedTopLevelBinding::new(
                source::SourceId(0),
                source::SourceId(1),
                "main",
            )],
        )
        .finish();
    assert!(!candidate.is_generated_extension_of(&base));
}
