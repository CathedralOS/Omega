/*
    A builtin type name such as `u32`, `bool` or `u7` exists only in the type
    role, and no declaration may introduce a competing type or namespace name
    with its spelling (`wiki/spec/language/modules.md#builtin-type-names`).
    `data u7 { ... }` would otherwise make every later `u7` mean one of two
    types, so it is refused where it is written, before any later pass expands
    an alias or reads a declaration header.

    The resolver checks names introduced by data declarations, trait schemas,
    type aliases and generic type binders. The parser checks a file import's
    explicit alias or implicit filename stem when it constructs the import.
    Locals, parameters, fields, constants, value binders and machines may use
    a builtin spelling because they introduce names in the value role.
*/

use crate::source_symbols::is_builtin_type_spelling;
use diagnostics::Diagnostic;
use syntax_trees::SyntaxTrees;
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{Item, TypeParameterKind};

pub(crate) fn validate(syntax: &SyntaxTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut refuse = |name: &Identifier, role: &str| {
        let spelling = name.as_str(syntax);
        if is_builtin_type_spelling(spelling) {
            diagnostics.push(
                Diagnostic::error(format!(
                    "{role} `{spelling}` is spelled like the builtin type `{spelling}`: a \
                     declaration that introduces a type or namespace name may not redefine a \
                     builtin type name; choose another name"
                ))
                .with_source_span(name.source_span()),
            );
        }
    };
    let mut pending = syntax.root_items().collect::<Vec<_>>();
    while let Some(item) = pending.pop() {
        match item {
            Item::Data(definition) => refuse(&definition.name, "data declaration"),
            Item::TypeAlias(definition) => refuse(&definition.name, "type alias"),
            Item::Trait(definition) => refuse(&definition.name, "trait"),
            Item::Machine(machine) => pending.extend(
                machine
                    .nested_machines
                    .handles()
                    .map(|handle| syntax.items.item(handle)),
            ),
            _ => {}
        }
    }
    /*
        Every generic binder of every declaration lives in one table, so a type
        binder is checked once wherever it is written.
    */
    for parameter in syntax.items.type_parameter_entries() {
        if matches!(parameter.kind, TypeParameterKind::Type) {
            refuse(&parameter.name, "generic type binder");
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}
