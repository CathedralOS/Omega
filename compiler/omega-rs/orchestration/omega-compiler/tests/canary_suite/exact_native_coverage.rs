use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const EXPECTED_UNIQUE_ROOTED_ACTIVE_COVERAGE: usize = 681;
pub(super) const EXPECTED_UNIQUE_LEGACY_ACTIVE_COVERAGE: usize = 13;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExactNativeCanaryOwner {
    pub(super) test_name: String,
    pub(super) source_path: PathBuf,
    pub(super) expected_status: i32,
}

#[derive(Debug)]
pub(super) struct ExactNativeCanaryCoverageIndex {
    rooted_owners: BTreeMap<String, Vec<ExactNativeCanaryOwner>>,
    legacy_owners: BTreeMap<String, Vec<ExactNativeCanaryOwner>>,
    source_file_count: usize,
    source_byte_count: usize,
    test_body_count: usize,
    qualifying_test_count: usize,
}

impl ExactNativeCanaryCoverageIndex {
    /// Read every canary-suite source module exactly once and index only
    /// dedicated enabled native tests with an exact exit-status assertion.
    pub(super) fn discover() -> Result<Self, String> {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut source_paths = vec![manifest.join("tests/canary_suite.rs")];
        let module_directory = manifest.join("tests/canary_suite");
        let entries = fs::read_dir(&module_directory).map_err(|error| {
            format!(
                "cannot enumerate canary test modules at {}: {error}",
                module_directory.display()
            )
        })?;
        for entry in entries {
            let path = entry
                .map_err(|error| format!("cannot inspect canary test module: {error}"))?
                .path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                source_paths.push(path);
            }
        }
        source_paths.sort();

        let mut index = Self::empty();
        for path in source_paths {
            let source = fs::read_to_string(&path).map_err(|error| {
                format!("cannot read canary test module {}: {error}", path.display())
            })?;
            index.source_file_count += 1;
            index.source_byte_count += source.len();
            index.index_source(&path, &source);
        }
        Ok(index)
    }

    fn empty() -> Self {
        Self {
            rooted_owners: BTreeMap::new(),
            legacy_owners: BTreeMap::new(),
            source_file_count: 0,
            source_byte_count: 0,
            test_body_count: 0,
            qualifying_test_count: 0,
        }
    }

    fn from_sources(sources: &[(&str, &str)]) -> Self {
        let mut index = Self::empty();
        for (path, source) in sources {
            index.source_file_count += 1;
            index.source_byte_count += source.len();
            index.index_source(Path::new(path), source);
        }
        index
    }

    fn index_source(&mut self, path: &Path, source: &str) {
        let code = mask_source(source, false);
        let structure = mask_source(source, true);
        for test in enabled_test_functions(&structure, &code) {
            self.test_body_count += 1;
            let Some((kind, canary, expected_status)) = exact_native_coverage(&test.body) else {
                continue;
            };
            self.qualifying_test_count += 1;
            let owners = match kind {
                ExactNativeOwnerKind::Rooted => &mut self.rooted_owners,
                ExactNativeOwnerKind::Legacy => &mut self.legacy_owners,
            };
            owners
                .entry(canary)
                .or_default()
                .push(ExactNativeCanaryOwner {
                    test_name: test.name,
                    source_path: path.to_path_buf(),
                    expected_status,
                });
        }
    }

    pub(super) fn unique_rooted_owner(&self, canary: &str) -> Option<&ExactNativeCanaryOwner> {
        unique_owner(&self.rooted_owners, canary)
    }

    pub(super) fn unique_legacy_owner(&self, canary: &str) -> Option<&ExactNativeCanaryOwner> {
        unique_owner(&self.legacy_owners, canary)
    }

    pub(super) fn rooted_owner_count(&self, canary: &str) -> usize {
        owner_count(&self.rooted_owners, canary)
    }

    pub(super) fn legacy_owner_count(&self, canary: &str) -> usize {
        owner_count(&self.legacy_owners, canary)
    }

    pub(super) const fn source_file_count(&self) -> usize {
        self.source_file_count
    }

    pub(super) const fn source_byte_count(&self) -> usize {
        self.source_byte_count
    }

    pub(super) const fn test_body_count(&self) -> usize {
        self.test_body_count
    }

    pub(super) const fn qualifying_test_count(&self) -> usize {
        self.qualifying_test_count
    }
}

fn unique_owner<'index>(
    owners: &'index BTreeMap<String, Vec<ExactNativeCanaryOwner>>,
    canary: &str,
) -> Option<&'index ExactNativeCanaryOwner> {
    let owners = owners.get(canary)?;
    let [owner] = owners.as_slice() else {
        return None;
    };
    Some(owner)
}

fn owner_count(owners: &BTreeMap<String, Vec<ExactNativeCanaryOwner>>, canary: &str) -> usize {
    owners.get(canary).map_or(0, Vec::len)
}

struct TestFunction {
    name: String,
    body: String,
}

fn enabled_test_functions(structure: &str, code: &str) -> Vec<TestFunction> {
    let mut tests = Vec::new();
    let mut line_start = 0usize;
    while line_start < structure.len() {
        let line_end = structure[line_start..]
            .find('\n')
            .map_or(structure.len(), |offset| line_start + offset + 1);
        if structure[line_start..line_end].trim() == "#[test]"
            && !preceding_attributes_disable(structure, line_start)
            && let Some(test) = test_function_after(structure, code, line_end)
        {
            tests.push(test);
        }
        line_start = line_end;
    }
    tests
}

fn preceding_attributes_disable(structure: &str, test_start: usize) -> bool {
    for line in structure[..test_start].lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !line.starts_with("#[") {
            break;
        }
        if line.starts_with("#[ignore") || line.starts_with("#[cfg") {
            return true;
        }
    }
    false
}

fn test_function_after(structure: &str, code: &str, attribute_end: usize) -> Option<TestFunction> {
    let mut cursor = attribute_end;
    let mut disabled = false;
    let function_start = loop {
        if cursor >= structure.len() {
            return None;
        }
        let line_end = structure[cursor..]
            .find('\n')
            .map_or(structure.len(), |offset| cursor + offset + 1);
        let line = structure[cursor..line_end].trim();
        if line == "#[test]" {
            return None;
        }
        disabled |= line.starts_with("#[ignore") || line.starts_with("#[cfg");
        if line.starts_with("fn ") {
            break cursor + structure[cursor..line_end].find("fn ")?;
        }
        cursor = line_end;
    };
    if disabled {
        return None;
    }

    let name_start = function_start + "fn ".len();
    let name_end = structure[name_start..].find('(')? + name_start;
    let name = structure[name_start..name_end].trim();
    if name.is_empty()
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return None;
    }
    let body_start = structure[name_end..].find('{')? + name_end;
    let body_end = matching_brace(structure, body_start)?;
    Some(TestFunction {
        name: name.to_owned(),
        body: code[body_start..=body_end].to_owned(),
    })
}

fn matching_brace(source: &str, opening: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = opening;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = string_end(bytes, index)?,
            b'\'' if is_character_literal(bytes, index) => index = character_end(bytes, index)?,
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn string_end(bytes: &[u8], opening: usize) -> Option<usize> {
    let mut index = opening + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            b'"' => return Some(index),
            _ => index += 1,
        }
    }
    None
}

fn is_character_literal(bytes: &[u8], opening: usize) -> bool {
    let Some(next) = bytes.get(opening + 1) else {
        return false;
    };
    *next == b'\\' || bytes.get(opening + 2) == Some(&b'\'')
}

fn character_end(bytes: &[u8], opening: usize) -> Option<usize> {
    let mut index = opening + 1;
    if bytes.get(index) == Some(&b'\\') {
        index += 2;
    } else {
        index += 1;
    }
    (bytes.get(index) == Some(&b'\'')).then_some(index)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactNativeOwnerKind {
    Rooted,
    Legacy,
}

fn exact_native_coverage(body: &str) -> Option<(ExactNativeOwnerKind, String, i32)> {
    if !body.contains("Command::new(") || !body.contains(".output()") {
        return None;
    }
    let canaries = exact_pass_canary_literals(body);
    let [canary] = canaries.as_slice() else {
        return None;
    };
    let compact = body
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let kind = if body.contains("compile_rooted_canary_for_native_host") {
        ExactNativeOwnerKind::Rooted
    } else if compact.matches("compile(CompileOptions{").count() == 1
        && compact.contains("root_path:canary.join(\"main.omg\")")
        && compact.contains("target_name:None")
        && compact.contains("write_output:true")
    {
        ExactNativeOwnerKind::Legacy
    } else {
        return None;
    };
    let assertion = compact.find("assert_eq!(output.status.code(),Some(")?
        + "assert_eq!(output.status.code(),Some(".len();
    let status_digits = compact[assertion..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if status_digits.is_empty() {
        return None;
    }
    let status_end = assertion + status_digits.len();
    if !compact[status_end..].starts_with(')') {
        return None;
    }
    Some((kind, canary.clone(), status_digits.parse().ok()?))
}

fn exact_pass_canary_literals(body: &str) -> Vec<String> {
    const PREFIX: &str = "pass_canary(\"";
    let mut canaries = Vec::new();
    let mut remainder = body;
    while let Some(start) = remainder.find(PREFIX) {
        remainder = &remainder[start + PREFIX.len()..];
        let Some(end) = remainder.find("\")") else {
            break;
        };
        let canary = &remainder[..end];
        if !canary.is_empty()
            && canary
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'_' | b'-'))
        {
            canaries.push(canary.to_owned());
        }
        remainder = &remainder[end + 2..];
    }
    canaries
}

fn mask_source(source: &str, mask_literals: bool) -> String {
    let bytes = source.as_bytes();
    let mut masked = bytes.to_vec();
    let mut index = 0usize;
    let mut block_depth = 0usize;
    while index < bytes.len() {
        if block_depth > 0 {
            if bytes[index..].starts_with(b"/*") {
                masked[index..index + 2].fill(b' ');
                block_depth += 1;
                index += 2;
            } else if bytes[index..].starts_with(b"*/") {
                masked[index..index + 2].fill(b' ');
                block_depth -= 1;
                index += 2;
            } else {
                if bytes[index] != b'\n' {
                    masked[index] = b' ';
                }
                index += 1;
            }
            continue;
        }
        if bytes[index..].starts_with(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                masked[index] = b' ';
                index += 1;
            }
        } else if bytes[index..].starts_with(b"/*") {
            masked[index..index + 2].fill(b' ');
            block_depth = 1;
            index += 2;
        } else if let Some(end) = raw_string_end(bytes, index) {
            if mask_literals {
                masked[index..=end].fill(b' ');
            }
            index = end + 1;
        } else if bytes[index] == b'"' {
            let end = string_end(bytes, index).unwrap_or(bytes.len() - 1);
            if mask_literals {
                masked[index..=end].fill(b' ');
            }
            index = end + 1;
        } else if bytes[index] == b'\'' && is_character_literal(bytes, index) {
            let end = character_end(bytes, index).unwrap_or(bytes.len() - 1);
            if mask_literals {
                masked[index..=end].fill(b' ');
            }
            index = end + 1;
        } else {
            index += 1;
        }
    }
    String::from_utf8(masked).expect("masking comments preserves UTF-8 source bytes")
}

fn raw_string_end(bytes: &[u8], start: usize) -> Option<usize> {
    let raw = if bytes.get(start) == Some(&b'r') {
        start
    } else if bytes.get(start) == Some(&b'b') && bytes.get(start + 1) == Some(&b'r') {
        start + 1
    } else {
        return None;
    };
    let mut opening_quote = raw + 1;
    while bytes.get(opening_quote) == Some(&b'#') {
        opening_quote += 1;
    }
    if bytes.get(opening_quote) != Some(&b'"') {
        return None;
    }
    let hashes = opening_quote - raw - 1;
    let mut cursor = opening_quote + 1;
    while cursor < bytes.len() {
        if bytes[cursor] == b'"'
            && bytes.get(cursor + 1..cursor + 1 + hashes) == Some(&bytes[raw + 1..opening_quote])
        {
            return Some(cursor + hashes);
        }
        cursor += 1;
    }
    None
}

#[test]
fn exact_native_source_index_is_strict_and_ambiguity_fails_closed() {
    let rooted = format!(
        "#[test]\nfn rooted() {{ let canary = pass_{}(\"demo/rooted\"); \
         compile_rooted_canary_for_native_host(&canary, build).unwrap(); \
         let output = Command::new(path).output().unwrap(); \
         assert_eq!(output.status.code(), Some(70)); }}",
        "canary"
    );
    let legacy = format!(
        "#[test]\nfn legacy() {{ let canary = pass_{}(\"demo/legacy\"); \
         compile(CompileOptions {{ root_path: canary.join(\"main.omg\"), \
         build_dir: Some(build), target_name: None, write_output: true }}).unwrap(); \
         let output = Command::new(path).output().unwrap(); \
         assert_eq!(output.status.code(), Some(71)); }}",
        "canary"
    );
    let ignored = format!("#[test]\n#[ignore]\n{}", &rooted[8..]);
    let configured = format!("#[test]\n#[cfg(windows)]\n{}", &rooted[8..]);
    let configured_before = format!("#[cfg(windows)]\n{rooted}");
    let configured_before_blank = format!("#[cfg(windows)]\n\n{rooted}");
    let weak_status = rooted.replace(
        "assert_eq!(output.status.code(), Some(70));",
        "assert!(output.status.code().is_some());",
    );
    let wrong_target = legacy.replace("target_name: None", "target_name: Some(target)");
    let no_output = legacy.replace("write_output: true", "write_output: false");
    let wrong_root = legacy.replace(
        "root_path: canary.join(\"main.omg\")",
        "root_path: other.join(\"main.omg\")",
    );
    let auxiliary = legacy.replace(
        "compile(CompileOptions",
        "compile_with_auxiliary_artifacts(CompileOptions",
    );
    let multiple_canaries = legacy.replace(
        "let canary =",
        "let other = pass_canary(\"demo/other\"); let canary =",
    );
    let no_execution = legacy.replace(".output()", ".status()");
    let ambiguous =
        legacy.replace("fn legacy", "fn first") + "\n" + &legacy.replace("fn legacy", "fn second");
    let index = ExactNativeCanaryCoverageIndex::from_sources(&[
        ("rooted.rs", &rooted),
        ("legacy.rs", &legacy),
        ("ignored.rs", &ignored),
        ("configured.rs", &configured),
        ("configured_before.rs", &configured_before),
        ("configured_before_blank.rs", &configured_before_blank),
        ("weak.rs", &weak_status),
        ("wrong_target.rs", &wrong_target),
        ("no_output.rs", &no_output),
        ("wrong_root.rs", &wrong_root),
        ("auxiliary.rs", &auxiliary),
        ("multiple_canaries.rs", &multiple_canaries),
        ("no_execution.rs", &no_execution),
    ]);
    let owner = index
        .unique_rooted_owner("demo/rooted")
        .expect("one strict enabled rooted exact-native owner should qualify");
    assert_eq!(
        (owner.test_name.as_str(), owner.expected_status),
        ("rooted", 70)
    );
    let owner = index
        .unique_legacy_owner("demo/legacy")
        .expect("one strict enabled legacy exact-native owner should qualify");
    assert_eq!(
        (owner.test_name.as_str(), owner.expected_status),
        ("legacy", 71)
    );
    assert_eq!(index.rooted_owner_count("demo/legacy"), 0);
    assert_eq!(index.legacy_owner_count("demo/rooted"), 0);

    let ambiguous = ExactNativeCanaryCoverageIndex::from_sources(&[("ambiguous.rs", &ambiguous)]);
    assert_eq!(ambiguous.legacy_owner_count("demo/legacy"), 2);
    assert!(ambiguous.unique_legacy_owner("demo/legacy").is_none());
}
