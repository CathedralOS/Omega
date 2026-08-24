use omega_compiler::inspect_source_closure;
use std::path::PathBuf;

fn main() {
    let Some(arguments) = parse_arguments() else {
        eprintln!(
            "usage: omega-source-snapshot --repository-root <dir> [--target <name>] [--semantic-only] <root.omg>"
        );
        std::process::exit(2);
    };
    match inspect_source_closure(
        &arguments.repository_root,
        &arguments.root_path,
        arguments.target_name.as_deref(),
        !arguments.semantic_only,
    ) {
        Ok(snapshot) => match snapshot.to_json_pretty() {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("cannot encode source-closure snapshot: {error}");
                std::process::exit(1);
            }
        },
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                eprintln!("{diagnostic}");
            }
            std::process::exit(1);
        }
    }
}

struct Arguments {
    repository_root: PathBuf,
    root_path: PathBuf,
    target_name: Option<String>,
    semantic_only: bool,
}

fn parse_arguments() -> Option<Arguments> {
    let mut repository_root = None;
    let mut root_path = None;
    let mut target_name = None;
    let mut semantic_only = false;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--repository-root" {
            repository_root = arguments.next().map(PathBuf::from);
            repository_root.as_ref()?;
            continue;
        }
        if argument == "--target" {
            target_name = arguments
                .next()
                .and_then(|target| target.into_string().ok());
            target_name.as_ref()?;
            continue;
        }
        if argument == "--semantic-only" {
            semantic_only = true;
            continue;
        }
        if root_path.is_some() {
            return None;
        }
        root_path = Some(PathBuf::from(argument));
    }
    Some(Arguments {
        repository_root: repository_root?,
        root_path: root_path?,
        target_name,
        semantic_only,
    })
}
