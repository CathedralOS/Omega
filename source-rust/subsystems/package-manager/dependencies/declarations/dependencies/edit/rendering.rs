// Canonical Omega statement rendering and source digesting for `build.omg` dependency rows.
// Caller-controlled strings remain escaped bytes inside string literals so they cannot inject syntax, lines, comments, or review prose.
// An unaliased row is one bound registration statement; an aliased row is the three-statement description journey, joined by `\n` so the inserter can indent its continuation lines.

use crate::declarations::dependencies::read::{DependencySourceRequest, PackageSelection};
use crate::declarations::editable_build::{
    DEPENDENCY_DESCRIPTION_TYPE_NAME, DEPENDENCY_REGISTRATION_NAME, FROM_SOURCE_HELPER_NAME,
    WITH_ALIAS_HELPER_NAME,
};
use build_declarations::BUILD_MODULE_NAME;

// `build_source` is the build file the statement joins; the registration and
// description locals take names it does not already use.
pub(crate) fn canonical_dependency_statement(
    request: &DependencySourceRequest,
    build_source: &str,
    import: &str,
    receiver: &str,
) -> String {
    let base = match request.explicit_alias() {
        Some(alias) => alias.as_str().to_owned(),
        None => local_base(match request {
            DependencySourceRequest::Bundled { name, .. } => name.as_str(),
            DependencySourceRequest::Path { location, .. } => location,
            DependencySourceRequest::Git { repository, .. } => repository,
        }),
    };
    let registration = fresh_local(build_source, &format!("{base}_registration"));
    let source = match request {
        DependencySourceRequest::Bundled { name, .. } => format!(
            "Source::Bundled {{ name: {} }}",
            source_text::display_literal_bytes(name.as_str().as_bytes())
        ),
        DependencySourceRequest::Path { location, .. } => format!(
            "Source::Path {{ location: {} }}",
            source_text::display_literal_bytes(location.as_bytes())
        ),
        DependencySourceRequest::Git {
            repository,
            revision,
            selection,
            ..
        } => {
            let selection = match selection {
                PackageSelection::Root => String::new(),
                PackageSelection::Named(package) => format!(
                    ", selection: PackageSelection::Named {{ package: {} }}",
                    source_text::display_literal_bytes(package.as_str().as_bytes())
                ),
            };
            format!(
                "Source::Git {{ repository: {}, revision: {}{selection} }}",
                source_text::display_literal_bytes(repository.as_bytes()),
                source_text::display_literal_bytes(revision.as_bytes())
            )
        }
    };
    let description = if request.explicit_alias().is_none() {
        source
    } else {
        format!("|{import}|::{FROM_SOURCE_HELPER_NAME}({source})")
    };
    let Some(alias) = request.explicit_alias() else {
        return format!(
            "let {registration}: {REGISTRATION_TYPE_NAME} = {BUILD_MODULE_NAME}::{DEPENDENCY_REGISTRATION_NAME}({receiver}, {description});"
        );
    };
    let local = fresh_local(build_source, &format!("{base}_dependency"));
    format!(
        "let mutable {local}: {DEPENDENCY_DESCRIPTION_TYPE_NAME} = {description};\n\
         |{import}|::{WITH_ALIAS_HELPER_NAME}(@mutable {local}, {});\n\
         let {registration}: {REGISTRATION_TYPE_NAME} = {BUILD_MODULE_NAME}::{DEPENDENCY_REGISTRATION_NAME}({receiver}, {local});",
        source_text::display_literal_bytes(alias.as_str().as_bytes())
    )
}

const REGISTRATION_TYPE_NAME: &str = "DependencyRegistration";

// A snake_case identifier from a location's last path segment, so an
// unaliased `../library/system` registration reads `system_registration`.
fn local_base(location: &str) -> String {
    let segment = location
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .trim_end_matches(".git");
    let mut base = String::new();
    for character in segment.chars() {
        let character = character.to_ascii_lowercase();
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            base.push(character);
        } else if !base.is_empty() && !base.ends_with('_') {
            base.push('_');
        }
    }
    let base = base.trim_end_matches('_');
    if base.starts_with(|character: char| character.is_ascii_lowercase()) {
        base.to_owned()
    } else {
        "package".to_owned()
    }
}

// The first of `name`, `name_2`, ... that no identifier in `source` spells.
fn fresh_local(source: &str, name: &str) -> String {
    let taken = |candidate: &str| {
        source
            .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
            .any(|word| word == candidate)
    };
    if !taken(name) {
        return name.to_owned();
    }
    (2u32..)
        .map(|suffix| format!("{name}_{suffix}"))
        .find(|candidate| !taken(candidate))
        .expect("an unbounded suffix sequence has an unused name")
}

pub(super) fn source_digest(source: &str) -> [u8; 32] {
    crate::digest_encoder::content_identity(source.as_bytes())
}
