//! Source units installed by the toolchain for an admitted Build. The host
//! authenticates these declarations and the activation that issued its values.
mod optimizations;
mod targets;

pub struct BuildPrelude {
    pub declarations: String,
    pub modules: Vec<(&'static str, String)>,
}

pub fn build_prelude() -> BuildPrelude {
    // Keep the virtual source units and their byte coordinates stable.
    let declarations = [
        "\n",
        include_str!("prelude/declarations/deployment.omg"),
        &targets::declarations(),
        "\n",
        &optimizations::declarations(),
        include_str!("prelude/declarations/file_handles.omg"),
        include_str!("prelude/declarations/product_selection.omg"),
        include_str!("prelude/declarations/outputs.omg"),
        include_str!("prelude/declarations/builder.omg"),
        include_str!("prelude/declarations/dependency_requests.omg"),
        include_str!("prelude/declarations/filesystem.omg"),
        "\n",
    ]
    .concat();
    let mut modules: Vec<_> = MODULES
        .iter()
        .map(|&(namespace, source)| (namespace, format!("\n{source}\n")))
        .collect();
    modules.push(("Optimizations", optimizations::module_source()));
    BuildPrelude {
        declarations,
        modules,
    }
}

const MODULES: &[(&str, &str)] = &[
    (crate::BUILD_MODULE_NAME, include_str!("prelude/modules/build_operations.omg")),
    (
        "BuildSource",
        include_str!("prelude/modules/build_source.omg"),
    ),
    (
        "BuildOutput",
        include_str!("prelude/modules/build_output.omg"),
    ),
    (
        "BuildInputs",
        include_str!("prelude/modules/build_inputs.omg"),
    ),
    (
        "RequiredOutput",
        include_str!("prelude/modules/required_output.omg"),
    ),
    (
        "ProductTypeSchema",
        include_str!("prelude/modules/product_type_schema.omg"),
    ),
    (
        "ProductProviderRef",
        include_str!("prelude/modules/product_provider_ref.omg"),
    ),
    ("TestGroup", include_str!("prelude/modules/test_group.omg")),
    ("BuildLog", include_str!("prelude/modules/build_log.omg")),
    (
        "BuildDeclarations",
        include_str!("prelude/modules/build_declarations.omg"),
    ),
];
