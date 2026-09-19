use super::{Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve};
use crate::lower_typed_trees;

fn generic_boundary_call_source(subject: &str) -> String {
    format!(
        "data Cell {{ value: u64; }}
         boundary trait Device<T> {{ machine consume(carrier: &mut T); }}
         data Main {{ device: Device<Cell>; cell: Cell; untouched: u64; }}
         machine Main::inspect(&mut self)
         reaches Device
         requires {subject} == 7;
         ensures {subject} == 7;
         {{ self.device.consume(&mut self.cell); }}"
    )
}

#[test]
fn generic_boundary_receiver_preserves_disjoint_caller_facts() {
    let source = generic_boundary_call_source("self.untouched");
    let syntax = parse_syntax_trees(&Lexer::new(&source).tokenize().unwrap()).unwrap();
    let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed).unwrap_or_else(|diagnostics| {
        panic!("an instantiated boundary signature must preserve disjoint facts: {diagnostics:#?}")
    });
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::inspect")
        .unwrap();
    let frame = validation::CallFrameResolver::new(&checked.typed)
        .unwrap()
        .inferred_state_write_frame(machine, &checked.typed.machine_states(machine)[0]);
    let mut paths = frame
        .into_complete_paths()
        .expect("complete boundary frame");
    paths.sort();
    assert_eq!(paths, ["self.cell", "self.device"]);
}

#[test]
fn generic_boundary_receiver_invalidates_written_caller_facts() {
    let source = generic_boundary_call_source("self.cell.value");
    let syntax = parse_syntax_trees(&Lexer::new(&source).tokenize().unwrap()).unwrap();
    let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let diagnostics = lower_typed_trees(typed)
        .expect_err("an exclusive boundary argument may overwrite the referenced field");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("cannot prove ensures")),
        "{diagnostics:#?}"
    );
}

#[test]
fn generic_boundary_owner_and_method_arguments_preserve_disjoint_facts() {
    let source = "data Cell { value: u64; }
        boundary trait Device<T> { machine consume<U>(carrier: &mut T, other: &mut U); }
        data Main { device: Device<Cell>; cell: Cell; audit: u64; untouched: u64; }
        machine Main::inspect(&mut self)
        reaches Device
        requires self.untouched == 7;
        ensures self.untouched == 7;
        { self.device.consume(&mut self.cell, &mut self.audit); }";
    let syntax = parse_syntax_trees(&Lexer::new(source).tokenize().unwrap()).unwrap();
    let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    lower_typed_trees(typed).unwrap_or_else(|diagnostics| {
        panic!("owner and method type arguments must preserve disjoint facts: {diagnostics:#?}")
    });
}

#[test]
fn generic_boundary_value_argument_does_not_invalidate_its_source() {
    let source = generic_boundary_call_source("self.untouched")
        .replace("carrier: &mut T", "carrier: &mut T, metadata: u64")
        .replace(
            "consume(&mut self.cell)",
            "consume(&mut self.cell, self.untouched)",
        );
    let syntax = parse_syntax_trees(&Lexer::new(&source).tokenize().unwrap()).unwrap();
    let resolved = resolve(ResolutionRequest::new(&syntax)).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    lower_typed_trees(typed).unwrap_or_else(|diagnostics| {
        panic!(
            "copying a scalar argument does not authorize a write to its source: {diagnostics:#?}"
        )
    });
}

/// A resolved non-boundary requirement call keeps the runtime receiver's
/// proven origin plus every exclusive argument's origin: the retained
/// `target_symbol` — or, on a nested receiver path the typer does not
/// annotate, the leaf's declared `dyn` trait — selects the requirement
/// signature, and an exclusive `self` writes that receiver place exactly.
/// No implementor storage path is invented.
#[test]
fn requirement_receiver_calls_reach_checked_trees_with_exact_frames() {
    let cases = [
        (
            "dyn_field",
            "",
            "self.handler.touch();",
            Some(vec!["self.handler"]),
            true,
        ),
        (
            "dyn_field_exclusive_argument",
            "",
            "self.handler.apply(&mut self.audit);",
            Some(vec!["self.audit", "self.handler"]),
            true,
        ),
        (
            "shared_self",
            "",
            "self.handler.peek();",
            Some(vec![]),
            true,
        ),
        (
            "dyn_parameter",
            ", s: &mut dyn Shape",
            "s.apply(&mut self.audit);",
            Some(vec!["$P0", "self.audit"]),
            true,
        ),
        (
            "nested_receiver",
            "",
            "self.group.handler.touch();",
            Some(vec!["self.group.handler"]),
            true,
        ),
        (
            "bound_result_route",
            "",
            "let r: &mut u64 = self.handler.get(); r = 1;",
            Some(vec!["self.handler"]),
            true,
        ),
        // The qualified form keeps its exact frame, but checked lowering
        // separately requires the explicit `self` argument to be writable in
        // the caller's state, which a `dyn` place is not.
        (
            "qualified_self_argument",
            "",
            "Shape::apply(&mut self.handler, &mut self.audit);",
            Some(vec!["self.audit", "self.handler"]),
            false,
        ),
        (
            "indexed_receiver",
            "",
            "self.handlers[0].touch();",
            None,
            true,
        ),
    ];
    let mut failures = Vec::new();
    for (name, parameters, body, expected, lowers) in &cases {
        let source = format!(
            r#"
            trait Shape {{
                machine touch(&mut self);
                machine apply(&mut self, value: &mut u64);
                machine peek(&self);
                machine get(&mut self) -> &mut u64;
            }}
            data Group {{ handler: dyn Shape; }}
            data Main {{ handler: dyn Shape; group: Group; handlers: [dyn Shape; 2]; audit: u64; }}
            machine Main::inspect(&mut self{parameters}) {{ {body} }}
        "#
        );
        let syntax =
            parse_syntax_trees(&Lexer::new(&source).tokenize().expect("tokenize")).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::inspect")
            .expect("caller");
        let state = &typed.machine_states(machine)[0];
        let resolver = validation::CallFrameResolver::new(&typed).expect("resolver");
        let mut actual = resolver
            .inferred_state_write_frame(machine, state)
            .into_complete_paths();
        if let Some(paths) = &mut actual {
            paths.sort();
        }
        let mut expected = expected.as_ref().map(|paths| {
            paths
                .iter()
                .map(|path| (*path).to_owned())
                .collect::<Vec<_>>()
        });
        if let Some(paths) = &mut expected {
            paths.sort();
        }
        if actual != expected {
            failures.push(format!("{name}: expected {expected:?}, actual {actual:?}"));
            continue;
        }
        if expected.is_some() && *lowers {
            lower_typed_trees(typed).unwrap_or_else(|_| panic!("{name} must lower"));
        }
    }
    assert!(failures.is_empty(), "{failures:?}");
}
