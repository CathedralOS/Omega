//! Restricted source lookup for the fixture leaf explicitly declared by an owner.
//! This does not interpret execution tables or import another owner's inventory.

use super::{mask_source, matching_delimiter, string_end, top_level_arguments};
use std::collections::BTreeMap;

pub(super) fn load(
    source: &str,
    mut read: impl FnMut(&str) -> Result<String, String>,
) -> Result<BTreeMap<String, String>, String> {
    let code = mask_source(source, false);
    let structure = mask_source(source, true);
    let mut leaf = None;
    for (start, end) in top_level_statements(&structure) {
        let statement = compact(&code[start..end]);
        let Some(declaration) = statement.strip_prefix("#[path=\"") else {
            continue;
        };
        let Some((path, suffix)) = declaration.split_once("\"]") else {
            continue;
        };
        if !matches!(
            suffix,
            "pub(super)modfixture_roster;" | "modfixture_roster;" | "pubmodfixture_roster;"
        ) {
            continue;
        }
        let Some(name) = path
            .strip_prefix("../fixture_rosters/")
            .or_else(|| path.strip_prefix("fixture_rosters/"))
            .and_then(|name| name.strip_suffix(".rs"))
        else {
            return Err("fixture roster must name one adjacent fixture_rosters leaf".into());
        };
        if !identifier(name) || leaf.replace(path.to_owned()).is_some() {
            return Err("fixture roster declaration is invalid or ambiguous".into());
        }
    }
    let Some(leaf) = leaf else {
        return Ok(BTreeMap::new());
    };
    declarations(&read(&leaf)?)
}

fn declarations(source: &str) -> Result<BTreeMap<String, String>, String> {
    let code = mask_source(source, false);
    let structure = mask_source(source, true);
    let statements = top_level_statements(&structure)
        .into_iter()
        .map(|(start, end)| compact(&code[start..end]))
        .collect::<Vec<_>>();
    let repository_macro = exact_repository_macro(&statements);
    let mut repository_macro_is_in_scope = false;
    let mut constants = BTreeMap::new();
    for statement in statements {
        if repository_macro && statement.starts_with("macro_rules!repository_fixture") {
            repository_macro_is_in_scope = true;
            continue;
        }
        if repository_macro_is_in_scope
            && let Some(arguments) = statement
                .strip_prefix("repository_fixture!(")
                .and_then(|value| value.strip_suffix(");"))
        {
            if arguments.ends_with(',') {
                return Err("repository fixture matcher does not accept a trailing comma".into());
            }
            let arguments = top_level_arguments(arguments);
            let [short, relative, literal] = arguments.as_slice() else {
                return Err("repository fixture invocation must have three exact arguments".into());
            };
            let Some(path) = path_literal(literal) else {
                return Err("repository fixture invocation must have a literal path".into());
            };
            if !identifier(short) || !identifier(relative) {
                return Err("repository fixture invocation must name exact constants".into());
            }
            insert_constant(&mut constants, short, path.to_owned())?;
            insert_constant(&mut constants, relative, format!("tests/omega/pass/{path}"))?;
            continue;
        }
        let Some(declaration) = statement
            .strip_prefix("pubconst")
            .or_else(|| statement.strip_prefix("pub(crate)const"))
            .or_else(|| statement.strip_prefix("pub(super)const"))
        else {
            continue;
        };
        let Some((name, value)) = declaration.split_once(":&str=") else {
            continue;
        };
        let Some(value) = value.strip_suffix(';').and_then(path_literal) else {
            continue;
        };
        if identifier(name) {
            insert_constant(&mut constants, name, value.to_owned())?;
        }
    }
    Ok(constants)
}

fn insert_constant(
    constants: &mut BTreeMap<String, String>,
    name: &str,
    value: String,
) -> Result<(), String> {
    if constants.insert(name.to_owned(), value).is_some() {
        return Err("fixture roster repeats a named path constant".into());
    }
    Ok(())
}

/// This is one fixed local declaration, not a macro evaluator. Its complete
/// matcher, visibility, two expansions, and repository prefix must agree.
fn exact_repository_macro(statements: &[String]) -> bool {
    const DEFINITION: &str = r#"macro_rules! repository_fixture {
        ($short:ident, $relative:ident, $path:literal) => {
            pub(crate) const $short: &str = $path;
            pub(crate) const $relative: &str = concat!("tests/omega/pass/", $path);
        };
    }"#;
    let mut definitions = statements
        .iter()
        .filter(|statement| statement.contains("macro_rules!repository_fixture"));
    definitions
        .next()
        .is_some_and(|definition| *definition == compact(DEFINITION))
        && definitions.next().is_none()
}

/// Only an exact call argument can acquire a fixture identity. Any unresolved
/// call invalidates the whole body's identity, including mixed literal/name cases.
pub(super) fn pass_canaries(body: &str, constants: &BTreeMap<String, String>) -> Vec<String> {
    let structure = mask_source(body, true);
    let bytes = structure.as_bytes();
    let mut cursor = 0;
    let mut canaries = Vec::new();
    while let Some(relative) = structure[cursor..].find("pass_canary") {
        let start = cursor + relative;
        cursor = start + "pass_canary".len();
        if start
            .checked_sub(1)
            .is_some_and(|index| identifier_byte(bytes[index]) || bytes[index] == b':')
            || bytes.get(cursor).is_some_and(|byte| identifier_byte(*byte))
        {
            continue;
        }
        while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'(') {
            continue;
        }
        let Some(end) = matching_delimiter(&structure, cursor, b'(', b')') else {
            return Vec::new();
        };
        let arguments = top_level_arguments(body[cursor + 1..end].trim());
        let [argument] = arguments.as_slice() else {
            return Vec::new();
        };
        let argument = compact(argument);
        let value = path_literal(&argument).map(str::to_owned).or_else(|| {
            let name = argument.strip_prefix("fixture_roster::")?;
            identifier(name)
                .then(|| constants.get(name).cloned())
                .flatten()
        });
        let Some(value) = value else {
            return Vec::new();
        };
        canaries.push(value);
        cursor = end + 1;
    }
    canaries
}

fn path_literal(value: &str) -> Option<&str> {
    let path = value.strip_prefix('"')?.strip_suffix('"')?;
    (!path.is_empty()
        && path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-')))
    .then_some(path)
}

fn identifier(name: &str) -> bool {
    name.bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && name.bytes().all(identifier_byte)
}

fn identifier_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn compact(source: &str) -> String {
    let mut output = String::new();
    let mut cursor = 0;
    while cursor < source.len() {
        if source.as_bytes()[cursor] == b'"' {
            let end = string_end(source.as_bytes(), cursor).unwrap_or(source.len() - 1);
            output.push_str(&source[cursor..=end]);
            cursor = end + 1;
        } else {
            let character = source[cursor..]
                .chars()
                .next()
                .expect("cursor is in source");
            if !character.is_whitespace() {
                output.push(character);
            }
            cursor += character.len_utf8();
        }
    }
    output
}

/// Statement ranges use the literal/comment-masked source, so braces and
/// semicolons in generated programs never become Rust declarations.
fn top_level_statements(structure: &str) -> Vec<(usize, usize)> {
    let mut statements = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (index, byte) in structure.bytes().enumerate() {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    statements.push((start, index + 1));
                    start = index + 1;
                }
            }
            b';' if depth == 0 => {
                statements.push((start, index + 1));
                start = index + 1;
            }
            _ => {}
        }
    }
    statements
}

#[cfg(test)]
#[path = "fixture_constants/tests.rs"]
mod tests;
