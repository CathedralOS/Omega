use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) const EXPECTED_UNIQUE_ROOTED_ACTIVE_COVERAGE: usize = 795;
pub(super) const EXPECTED_UNIQUE_DIRECT_ACTIVE_COVERAGE: usize = 4;
pub(super) const EXPECTED_UNIQUE_CROSS_TARGET_COVERAGE: usize = 32;
pub(super) const EXPECTED_UNIQUE_ROOTED_TARGET_COVERAGE: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExactNativeCanaryOwner {
    pub(super) test_name: String,
    pub(super) source_path: PathBuf,
    pub(super) expected_status: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ExactTargetCanaryOwner {
    pub(super) test_name: String,
    pub(super) source_path: PathBuf,
}

#[derive(Debug)]
pub(super) struct ExactNativeCanaryCoverageIndex {
    rooted_owners: BTreeMap<String, Vec<ExactNativeCanaryOwner>>,
    direct_owners: BTreeMap<String, Vec<ExactNativeCanaryOwner>>,
    cross_target_owners: BTreeMap<(String, String), Vec<ExactTargetCanaryOwner>>,
    rooted_target_owners: BTreeMap<(String, String), Vec<ExactTargetCanaryOwner>>,
    source_file_count: usize,
    source_byte_count: usize,
    test_body_count: usize,
    qualifying_test_count: usize,
    qualifying_target_compile_count: usize,
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
            direct_owners: BTreeMap::new(),
            cross_target_owners: BTreeMap::new(),
            rooted_target_owners: BTreeMap::new(),
            source_file_count: 0,
            source_byte_count: 0,
            test_body_count: 0,
            qualifying_test_count: 0,
            qualifying_target_compile_count: 0,
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
            if let Some((kind, canary, expected_status)) = exact_native_coverage(&test.body) {
                self.qualifying_test_count += 1;
                let owners = match kind {
                    ExactNativeOwnerKind::Rooted => &mut self.rooted_owners,
                    ExactNativeOwnerKind::Direct => &mut self.direct_owners,
                };
                owners
                    .entry(canary)
                    .or_default()
                    .push(ExactNativeCanaryOwner {
                        test_name: test.name.clone(),
                        source_path: path.to_path_buf(),
                        expected_status,
                    });
            }
            for (kind, canary, target) in exact_target_coverage(&test.body) {
                self.qualifying_target_compile_count += 1;
                let owners = match kind {
                    ExactTargetOwnerKind::CrossTarget => &mut self.cross_target_owners,
                    ExactTargetOwnerKind::RootedTarget => &mut self.rooted_target_owners,
                };
                owners
                    .entry((canary, target))
                    .or_default()
                    .push(ExactTargetCanaryOwner {
                        test_name: test.name.clone(),
                        source_path: path.to_path_buf(),
                    });
            }
        }
    }

    pub(super) fn unique_rooted_owner(&self, canary: &str) -> Option<&ExactNativeCanaryOwner> {
        unique_owner(&self.rooted_owners, canary)
    }

    pub(super) fn unique_direct_owner(&self, canary: &str) -> Option<&ExactNativeCanaryOwner> {
        unique_owner(&self.direct_owners, canary)
    }

    pub(super) fn rooted_owner_count(&self, canary: &str) -> usize {
        owner_count(&self.rooted_owners, canary)
    }

    pub(super) fn direct_owner_count(&self, canary: &str) -> usize {
        owner_count(&self.direct_owners, canary)
    }

    pub(super) fn unique_cross_target_owner(
        &self,
        canary: &str,
        target: &str,
    ) -> Option<&ExactTargetCanaryOwner> {
        unique_target_owner(&self.cross_target_owners, canary, target)
    }

    pub(super) fn unique_rooted_target_owner(
        &self,
        canary: &str,
        target: &str,
    ) -> Option<&ExactTargetCanaryOwner> {
        unique_target_owner(&self.rooted_target_owners, canary, target)
    }

    pub(super) fn cross_target_owner_count(&self, canary: &str, target: &str) -> usize {
        target_owner_count(&self.cross_target_owners, canary, target)
    }

    pub(super) fn rooted_target_owner_count(&self, canary: &str, target: &str) -> usize {
        target_owner_count(&self.rooted_target_owners, canary, target)
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

    pub(super) const fn qualifying_target_compile_count(&self) -> usize {
        self.qualifying_target_compile_count
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

fn unique_target_owner<'index>(
    owners: &'index BTreeMap<(String, String), Vec<ExactTargetCanaryOwner>>,
    canary: &str,
    target: &str,
) -> Option<&'index ExactTargetCanaryOwner> {
    let owners = owners.get(&(canary.to_owned(), target.to_owned()))?;
    let [owner] = owners.as_slice() else {
        return None;
    };
    Some(owner)
}

fn target_owner_count(
    owners: &BTreeMap<(String, String), Vec<ExactTargetCanaryOwner>>,
    canary: &str,
    target: &str,
) -> usize {
    owners
        .get(&(canary.to_owned(), target.to_owned()))
        .map_or(0, Vec::len)
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
    Direct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExactTargetOwnerKind {
    CrossTarget,
    RootedTarget,
}

fn exact_native_coverage(body: &str) -> Option<(ExactNativeOwnerKind, String, i32)> {
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
    } else if compact.matches("compile(CanaryCompileSpec{").count() == 1
        && compact.contains("root_path:canary.join(\"main.omg\")")
        && compact.contains("target_name:None")
        && compact.contains("product:CanaryCompileProduct::NativeArtifactAndPublish")
    {
        ExactNativeOwnerKind::Direct
    } else {
        return None;
    };
    let status = if body.contains("Command::new(") && body.contains(".output()") {
        let assertion = compact.find("assert_eq!(output.status.code(),Some(")?
            + "assert_eq!(output.status.code(),Some(".len();
        let status_digits = compact[assertion..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        let status_end = assertion + status_digits.len();
        if status_digits.is_empty() || !compact[status_end..].starts_with(')') {
            return None;
        }
        status_digits.parse().ok()?
    } else {
        exact_checked_report_native_status(&compact)?
    };
    Some((kind, canary.clone(), status))
}

fn exact_checked_report_native_status(compact: &str) -> Option<i32> {
    const CALL: &str = "assert_native_exit_code(&";
    let mut cursor = 0;
    let mut expected = None;
    while let Some(relative) = compact[cursor..].find(CALL) {
        let local_start = cursor + relative + CALL.len();
        let local = compact[local_start..]
            .chars()
            .take_while(|character| character.is_ascii_alphanumeric() || *character == '_')
            .collect::<String>();
        if local.is_empty() {
            return None;
        }
        let status_start = local_start + local.len();
        let remainder = compact.get(status_start..)?.strip_prefix(',')?;
        let digits = remainder
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>();
        if digits.is_empty() || !remainder[digits.len()..].starts_with(',') {
            return None;
        }
        let status = digits.parse().ok()?;
        if expected.is_some_and(|expected| expected != status) {
            return None;
        }
        expected = Some(status);
        let ordinary = format!("let{local}=compile_rooted_canary_for_native_host(");
        let full =
            format!("let{local}=compile_rooted_canary_for_native_host_with_auxiliary_artifacts(");
        if !compact.contains(&ordinary) && !compact.contains(&full) {
            return None;
        }
        cursor = status_start + 1 + digits.len() + 1;
    }
    expected
}

fn exact_target_coverage(body: &str) -> Vec<(ExactTargetOwnerKind, String, String)> {
    let canaries = exact_pass_canary_literals(body);
    let [canary] = canaries.as_slice() else {
        return Vec::new();
    };
    let compact = body
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let mut coverage = Vec::new();

    for function in [
        "compile_canary_without_output_for_target",
        "compile",
        "compile_with_auxiliary_artifacts",
    ] {
        for call in exact_successful_calls(&compact, function) {
            let target = if function == "compile_canary_without_output_for_target" {
                let arguments = top_level_arguments(call.arguments);
                let [canary_argument, target_argument] = arguments.as_slice() else {
                    continue;
                };
                if *canary_argument != "&canary" {
                    continue;
                }
                exact_string_literal(target_argument)
            } else {
                exact_compile_options_target(call.arguments)
            };
            if let Some(target) = target {
                coverage.push((
                    ExactTargetOwnerKind::CrossTarget,
                    canary.clone(),
                    target.to_owned(),
                ));
            }
        }
    }

    for function in [
        "compile_rooted_canary_for_target",
        "compile_rooted_canary_for_target_with_auxiliary_artifacts",
    ] {
        for call in exact_successful_calls(&compact, function) {
            let arguments = top_level_arguments(call.arguments);
            let [canary_argument, _, target_argument] = arguments.as_slice() else {
                continue;
            };
            if *canary_argument != "&canary" {
                continue;
            }
            if let Some(target) = exact_string_literal(target_argument) {
                coverage.push((
                    ExactTargetOwnerKind::RootedTarget,
                    canary.clone(),
                    target.to_owned(),
                ));
            }
        }
    }
    coverage
}

struct ExactCall<'source> {
    arguments: &'source str,
}

fn exact_successful_calls<'source>(
    source: &'source str,
    function: &str,
) -> Vec<ExactCall<'source>> {
    let mut calls = Vec::new();
    let needle = format!("{function}(");
    let mut cursor = 0usize;
    while let Some(offset) = source[cursor..].find(&needle) {
        let function_start = cursor + offset;
        let preceded_by_identifier = function_start
            .checked_sub(1)
            .and_then(|index| source.as_bytes().get(index))
            .is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b':'));
        let opening = function_start + function.len();
        let Some(closing) = matching_delimiter(source, opening, b'(', b')') else {
            break;
        };
        if !preceded_by_identifier && successful_result_suffix(&source[closing + 1..]) {
            calls.push(ExactCall {
                arguments: &source[opening + 1..closing],
            });
        }
        cursor = closing + 1;
    }
    calls
}

fn successful_result_suffix(suffix: &str) -> bool {
    if suffix.starts_with(".expect(") || suffix.starts_with(".unwrap(") {
        return true;
    }
    const UNWRAP_OR_ELSE: &str = ".unwrap_or_else";
    if !suffix.starts_with(UNWRAP_OR_ELSE) {
        return false;
    }
    let opening = UNWRAP_OR_ELSE.len();
    let Some(closing) = matching_delimiter(suffix, opening, b'(', b')') else {
        return false;
    };
    suffix[opening + 1..closing].contains("panic!(")
}

fn matching_delimiter(source: &str, opening: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(opening) != Some(&open) {
        return None;
    }
    let mut depth = 0usize;
    let mut index = opening;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = string_end(bytes, index)?,
            b'\'' if is_character_literal(bytes, index) => index = character_end(bytes, index)?,
            byte if byte == open => depth += 1,
            byte if byte == close => {
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

fn top_level_arguments(arguments: &str) -> Vec<&str> {
    let bytes = arguments.as_bytes();
    let mut result = Vec::new();
    let mut start = 0usize;
    let mut parens = 0usize;
    let mut braces = 0usize;
    let mut brackets = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'"' => index = string_end(bytes, index).unwrap_or(bytes.len() - 1),
            b'\'' if is_character_literal(bytes, index) => {
                index = character_end(bytes, index).unwrap_or(bytes.len() - 1);
            }
            b'(' => parens += 1,
            b')' => parens = parens.saturating_sub(1),
            b'{' => braces += 1,
            b'}' => braces = braces.saturating_sub(1),
            b'[' => brackets += 1,
            b']' => brackets = brackets.saturating_sub(1),
            b',' if parens == 0 && braces == 0 && brackets == 0 => {
                result.push(&arguments[start..index]);
                start = index + 1;
            }
            _ => {}
        }
        index += 1;
    }
    if start < arguments.len() {
        result.push(&arguments[start..]);
    }
    result
}

fn exact_compile_options_target(arguments: &str) -> Option<&str> {
    let options = arguments
        .strip_prefix("CanaryCompileSpec{")?
        .strip_suffix('}')?;
    if options.matches("root_path:").count() != 1
        || !options.contains("root_path:canary.join(\"main.omg\")")
        || options.matches("target_name:").count() != 1
        || options.matches("product:").count() != 1
        || !options.contains("product:CanaryCompileProduct::NativeArtifactAndPublish")
    {
        return None;
    }
    let target = options.split_once("target_name:Some(\"")?.1;
    let (target, suffix) = target.split_once('"')?;
    if !(suffix.starts_with(".into())") || suffix.starts_with(".to_owned())")) {
        return None;
    }
    valid_target_literal(target).then_some(target)
}

fn exact_string_literal(argument: &str) -> Option<&str> {
    let target = argument.strip_prefix('"')?.strip_suffix('"')?;
    valid_target_literal(target).then_some(target)
}

fn valid_target_literal(target: &str) -> bool {
    !target.is_empty()
        && target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
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
    let direct = format!(
        "#[test]\nfn direct() {{ let canary = pass_{}(\"demo/direct\"); \
         compile(CanaryCompileSpec {{ root_path: canary.join(\"main.omg\"), \
         build_dir: Some(build), target_name: None, \
         product: CanaryCompileProduct::NativeArtifactAndPublish }}).unwrap(); \
         let output = Command::new(path).output().unwrap(); \
         assert_eq!(output.status.code(), Some(71)); }}",
        "canary"
    );
    let checked_report = format!(
        "#[test]\nfn checked_report() {{ let canary = pass_{}(\"demo/checked-report\"); \
         let compilation = compile_rooted_canary_for_native_host(&canary, build).unwrap(); \
         assert_native_exit_code(&compilation, 72, \"checked report\"); }}",
        "canary"
    );
    let wrong_checked_report = checked_report.replace("&compilation, 72", "&other, 72");
    let ignored = format!("#[test]\n#[ignore]\n{}", &rooted[8..]);
    let configured = format!("#[test]\n#[cfg(windows)]\n{}", &rooted[8..]);
    let configured_before = format!("#[cfg(windows)]\n{rooted}");
    let configured_before_blank = format!("#[cfg(windows)]\n\n{rooted}");
    let weak_status = rooted.replace(
        "assert_eq!(output.status.code(), Some(70));",
        "assert!(output.status.code().is_some());",
    );
    let wrong_target = direct.replace("target_name: None", "target_name: Some(target)");
    let no_native_product = direct.replace(
        "CanaryCompileProduct::NativeArtifactAndPublish",
        "CanaryCompileProduct::Check",
    );
    let wrong_root = direct.replace(
        "root_path: canary.join(\"main.omg\")",
        "root_path: other.join(\"main.omg\")",
    );
    let auxiliary = direct.replace(
        "compile(CanaryCompileSpec",
        "compile_with_auxiliary_artifacts(CanaryCompileSpec",
    );
    let multiple_canaries = direct.replace(
        "let canary =",
        "let other = pass_canary(\"demo/other\"); let canary =",
    );
    let no_execution = direct.replace(".output()", ".status()");
    let ambiguous =
        direct.replace("fn direct", "fn first") + "\n" + &direct.replace("fn direct", "fn second");
    let index = ExactNativeCanaryCoverageIndex::from_sources(&[
        ("rooted.rs", &rooted),
        ("direct.rs", &direct),
        ("checked_report.rs", &checked_report),
        ("wrong_checked_report.rs", &wrong_checked_report),
        ("ignored.rs", &ignored),
        ("configured.rs", &configured),
        ("configured_before.rs", &configured_before),
        ("configured_before_blank.rs", &configured_before_blank),
        ("weak.rs", &weak_status),
        ("wrong_target.rs", &wrong_target),
        ("no_native_product.rs", &no_native_product),
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
        .unique_direct_owner("demo/direct")
        .expect("one strict enabled direct exact-native owner should qualify");
    assert_eq!(
        (owner.test_name.as_str(), owner.expected_status),
        ("direct", 71)
    );
    assert_eq!(index.rooted_owner_count("demo/direct"), 0);
    let owner = index
        .unique_rooted_owner("demo/checked-report")
        .expect("one checked-report rooted exact-native owner should qualify");
    assert_eq!(
        (owner.test_name.as_str(), owner.expected_status),
        ("checked_report", 72)
    );
    assert_eq!(index.direct_owner_count("demo/rooted"), 0);

    let ambiguous = ExactNativeCanaryCoverageIndex::from_sources(&[("ambiguous.rs", &ambiguous)]);
    assert_eq!(ambiguous.direct_owner_count("demo/direct"), 2);
    assert!(ambiguous.unique_direct_owner("demo/direct").is_none());
}

#[test]
fn exact_target_source_index_preserves_entry_semantics_and_fails_closed() {
    let cross = r#"
        #[test]
        fn cross() {
            let canary = pass_canary("demo/cross");
            compile(CanaryCompileSpec {
                root_path: canary.join("main.omg"),
                build_dir: Some(build),
                target_name: Some("linux_x64".into()),
                product: CanaryCompileProduct::NativeArtifactAndPublish,
            }).expect("cross target should compile");
        }
    "#;
    let cross_helper = r#"
        #[test]
        fn cross_helper() {
            let canary = pass_canary("demo/cross-helper");
            compile_canary_without_output_for_target(&canary, "uefi_x64").unwrap();
        }
    "#;
    let rooted = r#"
        #[test]
        fn rooted() {
            let canary = pass_canary("demo/rooted-target");
            compile_rooted_canary_for_target(&canary, x64_build, "linux_x64").unwrap();
            compile_rooted_canary_for_target_with_auxiliary_artifacts(
                &canary,
                arm_build,
                "linux_arm64",
            ).expect("rooted arm target should compile");
        }
    "#;
    let ignored = cross.replace("#[test]", "#[test]\n#[ignore]");
    let configured = cross.replace("#[test]", "#[cfg(windows)]\n\n#[test]");
    let dynamic_canary = cross.replace("pass_canary(\"demo/cross\")", "pass_canary(canary_name)");
    let synthesized = cross.replace(
        "root_path: canary.join(\"main.omg\")",
        "root_path: source.join(\"main.omg\")",
    );
    let dynamic_target = cross.replace(
        "target_name: Some(\"linux_x64\".into())",
        "target_name: Some(target.into())",
    );
    let no_success = cross.replace(").expect(\"cross target should compile\");", ");");
    let recovered_error = cross.replace(
        ").expect(\"cross target should compile\");",
        ").unwrap_or_else(|_| fallback_report);",
    );
    let production_rooted = cross.replace(
        "compile(CanaryCompileSpec",
        "production_compile(CanaryCompileSpec",
    );
    let multiple_canaries = cross.replace(
        "let canary =",
        "let other = pass_canary(\"demo/other\"); let canary =",
    );
    let ambiguous = cross.replace("fn cross", "fn first") + &cross.replace("fn cross", "fn second");
    let index = ExactNativeCanaryCoverageIndex::from_sources(&[
        ("cross.rs", cross),
        ("cross_helper.rs", cross_helper),
        ("rooted.rs", rooted),
        ("ignored.rs", &ignored),
        ("configured.rs", &configured),
        ("dynamic_canary.rs", &dynamic_canary),
        ("synthesized.rs", &synthesized),
        ("dynamic_target.rs", &dynamic_target),
        ("no_success.rs", &no_success),
        ("recovered_error.rs", &recovered_error),
        ("production_rooted.rs", &production_rooted),
        ("multiple_canaries.rs", &multiple_canaries),
    ]);

    let owner = index
        .unique_cross_target_owner("demo/cross", "linux_x64")
        .expect("one enabled exact direct-entry target owner should qualify");
    assert_eq!(owner.test_name, "cross");
    assert!(owner.source_path.ends_with("cross.rs"));
    assert!(
        index
            .unique_cross_target_owner("demo/cross-helper", "uefi_x64")
            .is_some()
    );
    assert!(
        index
            .unique_rooted_target_owner("demo/rooted-target", "linux_x64")
            .is_some()
    );
    assert!(
        index
            .unique_rooted_target_owner("demo/rooted-target", "linux_arm64")
            .is_some()
    );
    assert_eq!(
        index.rooted_target_owner_count("demo/cross", "linux_x64"),
        0
    );
    assert_eq!(
        index.cross_target_owner_count("demo/rooted-target", "linux_x64"),
        0
    );

    let ambiguous = ExactNativeCanaryCoverageIndex::from_sources(&[("ambiguous.rs", &ambiguous)]);
    assert_eq!(
        ambiguous.cross_target_owner_count("demo/cross", "linux_x64"),
        2
    );
    assert!(
        ambiguous
            .unique_cross_target_owner("demo/cross", "linux_x64")
            .is_none()
    );
}
