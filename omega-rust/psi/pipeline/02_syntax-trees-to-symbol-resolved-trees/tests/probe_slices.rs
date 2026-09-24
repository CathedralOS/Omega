//! The build-time-evaluation seam on real source custody: generic data
//! normalization runs before symbol resolution with the loader's `SourceMap`,
//! so package visibility and module-local precedence are live. The unit tests
//! beside `module_normalization` pin the same selection law on source-free
//! forests; these probes pin the custody-bearing path.
use source::SourceMap;
use source_files_to_tokens::Lexer;
use std::{path::PathBuf, sync::Arc};
use symbol_resolved_trees::SymbolResolvedTrees;
use syntax_trees::SyntaxTrees;
use syntax_trees_to_symbol_resolved_trees::pre_resolution::{
    GenericDataRequest, normalize_generic_data,
};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees_into_with_id;

fn lower_multi(sources: &[(&str, &str)]) -> Result<SymbolResolvedTrees, String> {
    let mut map = SourceMap::default();
    let mut syntax = SyntaxTrees::default();
    for (name, text) in sources {
        let id = map.add(PathBuf::from(name), (*text).to_owned()).source_id;
        let tokens = Lexer::new(text).tokenize().expect("tokenize");
        parse_syntax_trees_into_with_id(&mut syntax, id, &tokens).expect("parse");
    }
    let syntax = normalize_generic_data(GenericDataRequest {
        syntax,
        sources: Some(Arc::new(map.clone())),
        top_level_bindings: Vec::new(),
        retained_base: None,
    })
    .map_err(|errors| {
        errors
            .iter()
            .map(|error| error.message.clone())
            .collect::<Vec<_>>()
            .join("\n")
    })?;
    resolve(ResolutionRequest {
        syntax: &syntax,
        sources: Some(Arc::new(map)),
        top_level_bindings: Vec::new(),
    })
    .map_err(|errors| {
        errors
            .iter()
            .map(|error| error.message.clone())
            .collect::<Vec<_>>()
            .join("\n")
    })
}

fn instance_path(program: &SymbolResolvedTrees) -> String {
    let instance = program
        .data_definitions
        .iter()
        .find(|definition| definition.generic_instance.is_some())
        .expect("one closed instance");
    program.symbols.display_path(instance.symbol, "::")
}

/// `use units::Box;` carries the leaf application `Box<u64>` to the exact
/// module template: one closed instance synthesizes under the declaring
/// module's logical path and resolves there.
#[test]
fn module_generic_data_template_import_synthesizes_closed_instance() {
    let program = lower_multi(&[
        ("units.omg", "module units; pub data Box<T> { value: T; }"),
        (
            "main.omg",
            "use units::Box; data Holder { field: Box<u64>; }",
        ),
    ])
    .expect("the imported module template normalizes and resolves");
    assert_eq!(instance_path(&program), "units::Box<u64>");
}

/// The compiler-owned proof-algebra leaf inside a module is ordinary user
/// data: both the qualified `units::IntervalSet<u64>` spelling and the
/// imported leaf select the module template and materialize its instance.
#[test]
fn module_algebra_leaf_normalizes_through_qualified_and_imported_spellings() {
    for use_decl in ["", "use units::IntervalSet;"] {
        let program = lower_multi(&[
            (
                "units.omg",
                "module units; pub data IntervalSet<T> { value: T; }",
            ),
            (
                "main.omg",
                &format!("{use_decl} data Holder {{ field: units::IntervalSet<u64>; }}"),
            ),
        ])
        .expect("the module algebra leaf normalizes");
        assert_eq!(instance_path(&program), "units::IntervalSet<u64>");
    }
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub data IntervalSet<T> { value: T; }",
        ),
        (
            "main.omg",
            "use units::IntervalSet; data Holder { field: IntervalSet<u64>; }",
        ),
    ])
    .expect("the imported algebra leaf normalizes");
    assert_eq!(instance_path(&program), "units::IntervalSet<u64>");
}

/// An unmoduled `IntervalSet` declared beside a same-leaf module template
/// keeps the compiler-algebra exemption: the leaf application selects the
/// same-source declaration and retains its structural generic argument.
#[test]
fn unmoduled_algebra_carrier_keeps_its_generic_spelling_on_real_sources() {
    let program = lower_multi(&[
        (
            "units.omg",
            "module units; pub data IntervalSet<T> { value: T; }",
        ),
        (
            "main.omg",
            "data IntervalSet<Space> { start: u64; end: u64; } data Holder { field: IntervalSet<u64>; }",
        ),
    ])
    .expect("the exempt carrier resolves");
    assert!(
        program
            .data_definitions
            .iter()
            .all(|definition| definition.generic_instance.is_none()),
        "no closed instance may stand in for the unmoduled algebra"
    );
    let holder = program
        .data_definitions
        .iter()
        .find(|definition| definition.name.as_str() == "Holder")
        .expect("Holder");
    let [symbol_resolved_trees::data::DataMember::Field(field)] =
        program.data_members(holder.members)
    else {
        panic!("one field")
    };
    let symbol_resolved_trees::types::TypeReference::Generic(application) = &field.type_reference
    else {
        panic!("the exempt application keeps its authored generic spelling")
    };
    let owner = program
        .data_definitions
        .iter()
        .find(|definition| definition.symbol == application.base_symbol)
        .expect("the selected base declaration");
    assert_eq!(
        program.symbols.display_path(owner.symbol, "::"),
        "IntervalSet",
        "the leaf selects the same-source unmoduled carrier"
    );
}
