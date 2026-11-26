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

/// A file under the installed Build library root. Only an importing file receives its alias.
#[derive(Clone, Copy)]
pub struct BuildLibraryFile {
    pub namespace: &'static str,
    pub source: &'static str,
}

pub fn build_file(root: Option<&str>, path: &str) -> Option<BuildLibraryFile> {
    if root != Some("omega") {
        return None;
    }
    let (namespace, source) = match path {
        "build/dependency.omg" => ("dependency", DEPENDENCY),
        "build/dependencies/providers/git.omg" => (
            "git",
            include_str!("../../../../../../source/library/build/dependencies/providers/git.omg"),
        ),
        _ => return None,
    };
    Some(BuildLibraryFile { namespace, source })
}

const DEPENDENCY: &str = include_str!("prelude/modules/dependency.omg");

const MODULES: &[(&str, &str)] = &[
    ("dependency", DEPENDENCY),
    ("Build", include_str!("prelude/modules/build_operations.omg")),
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
        "BuildProduct",
        include_str!("prelude/modules/build_product.omg"),
    ),
];
