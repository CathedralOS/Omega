// Canonical Omega statement rendering and source digesting for `build.omg` dependency rows.
// Caller-controlled strings remain escaped bytes inside string literals so they cannot inject syntax, lines, comments, or review prose.
// An unaliased row is one bound registration statement; an aliased row is the three-statement description journey, joined by `\n` so the inserter can indent its continuation lines.

use crate::declarations::dependencies::read::{
    DependencyPurpose, DependencySourceRequest, PackageSelection,
};
use build_declarations::{
    BUILD_TYPE_NAME, DEPENDENCY_DESCRIPTION_TYPE_NAME, DEPENDENCY_HELPER_OWNER,
    FROM_SOURCE_HELPER_NAME, WITH_ALIAS_HELPER_NAME,
};

use super::BUILDER_PARAMETER_NAME;

// `build_source` is the build file the statement joins; the registration and
// description locals take names it does not already use.
pub(crate) fn canonical_dependency_statement(
    purpose: DependencyPurpose,
    request: &DependencySourceRequest,
    build_source: &str,
) -> String {
    let operation = purpose.operation().name();
    let base = match request.explicit_alias() {
        Some(alias) => alias.as_str().to_owned(),
        None => local_base(match request {
            DependencySourceRequest::Path { location, .. } => location,
            DependencySourceRequest::Git { repository, .. } => repository,
        }),
    };
    let registration = fresh_local(build_source, &format!("{base}_registration"));
    let source = match request {
        DependencySourceRequest::Path { location, .. } => format!(
            "Source::Path {{ location: {} }}",
            source::display_literal_bytes(location.as_bytes())
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
                    source::display_literal_bytes(package.as_str().as_bytes())
                ),
            };
            format!(
                "Source::Git {{ repository: {}, revision: {}{selection} }}",
                source::display_literal_bytes(repository.as_bytes()),
                source::display_literal_bytes(revision.as_bytes())
            )
        }
    };
    let description = format!("{DEPENDENCY_HELPER_OWNER}::{FROM_SOURCE_HELPER_NAME}({source})");
    let Some(alias) = request.explicit_alias() else {
        return format!(
            "let {registration}: {REGISTRATION_TYPE_NAME} = {BUILD_TYPE_NAME}::{operation}({BUILDER_PARAMETER_NAME}, {description});"
        );
    };
    let local = fresh_local(build_source, &format!("{base}_dependency"));
    format!(
        "let mut {local}: {DEPENDENCY_DESCRIPTION_TYPE_NAME} = {description};\n\
         {DEPENDENCY_HELPER_OWNER}::{WITH_ALIAS_HELPER_NAME}(&mut {local}, {});\n\
         let {registration}: {REGISTRATION_TYPE_NAME} = {BUILD_TYPE_NAME}::{operation}({BUILDER_PARAMETER_NAME}, {local});",
        source::display_literal_bytes(alias.as_str().as_bytes())
    )
}

const REGISTRATION_TYPE_NAME: &str = "DependencyRegistration";

// A snake_case identifier from a location's last path segment, so an
// unaliased `../library/std` registration reads `std_registration`.
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
