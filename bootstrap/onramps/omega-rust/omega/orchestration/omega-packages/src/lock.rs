use crate::manifest::{AliasName, PackageCapabilityManifest, PackageName};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const PACKAGE_LOCK_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedDependency {
    pub alias: AliasName,
    pub package: PackageName,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LockedPackage {
    pub package: PackageName,
    pub source_kind: String,
    pub source_locator: String,
    pub source_identity: String,
    pub manifest_fingerprint: String,
    pub build_observation: String,
    pub dependencies: Vec<LockedDependency>,
    pub trust_receipts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLockValidationError {
    MissingRootPackage {
        package: String,
    },
    DuplicatePackage {
        package: String,
    },
    DuplicateDependencyAlias {
        package: String,
        alias: String,
    },
    MissingDependencyPackage {
        package: String,
        alias: String,
        dependency: String,
    },
    MissingSourceIdentity {
        package: String,
    },
    MissingManifestFingerprint {
        package: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLockParseError {
    InvalidJson { message: String },
    MissingField { field: String },
    UnexpectedField { field: String },
    InvalidField { field: String, message: String },
    UnsupportedSchemaVersion { found: u32, supported: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLockPersistenceError {
    Io { path: PathBuf, message: String },
    Parse(PackageLockParseError),
    InvalidClosure(Vec<PackageLockValidationError>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageLockAssemblyError {
    DuplicateManifest { package: String },
    InvalidClosure(Vec<PackageLockValidationError>),
}

impl LockedPackage {
    pub fn normalized(mut self) -> Self {
        self.dependencies.sort();
        self.trust_receipts = sorted_unique(self.trust_receipts);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLock {
    pub schema_version: u32,
    pub root_package: PackageName,
    pub packages: Vec<LockedPackage>,
}

impl PackageLock {
    pub fn new(root_package: PackageName) -> Self {
        Self {
            schema_version: PACKAGE_LOCK_SCHEMA_VERSION,
            root_package,
            packages: Vec::new(),
        }
    }

    pub fn normalized(mut self) -> Self {
        self.packages = self
            .packages
            .into_iter()
            .map(LockedPackage::normalized)
            .collect();
        self.packages.sort();
        self
    }

    pub fn fingerprint(&self) -> String {
        format_sha256(&Sha256::digest(
            self.normalized_clone().to_json().as_bytes(),
        ))
    }

    pub fn normalized_clone(&self) -> Self {
        self.clone().normalized()
    }

    pub fn package(&self, package: &PackageName) -> Option<&LockedPackage> {
        self.packages.iter().find(|entry| &entry.package == package)
    }

    pub fn from_manifests(
        root_package: PackageName,
        manifests: &[PackageCapabilityManifest],
    ) -> Result<Self, PackageLockAssemblyError> {
        let mut seen = BTreeSet::new();
        let mut lock = Self::new(root_package);
        for manifest in manifests {
            let manifest = manifest.normalized_clone();
            if !seen.insert(manifest.package.as_str().to_owned()) {
                return Err(PackageLockAssemblyError::DuplicateManifest {
                    package: manifest.package.as_str().to_owned(),
                });
            }
            let manifest_fingerprint = manifest.fingerprint();
            lock.packages.push(LockedPackage {
                package: manifest.package,
                source_kind: manifest.source.kind,
                source_locator: manifest.source.locator,
                source_identity: manifest.source.resolved,
                manifest_fingerprint,
                build_observation: manifest.build_machine.observation_class,
                dependencies: manifest
                    .dependency_aliases
                    .into_iter()
                    .map(|dependency| LockedDependency {
                        alias: dependency.alias,
                        package: dependency.package,
                    })
                    .collect(),
                trust_receipts: manifest
                    .trust_receipts
                    .into_iter()
                    .map(|receipt| receipt.identity)
                    .collect(),
            });
        }
        lock.validate_closure()
            .map_err(PackageLockAssemblyError::InvalidClosure)?;
        Ok(lock.normalized())
    }

    pub fn validate_closure(&self) -> Result<(), Vec<PackageLockValidationError>> {
        let mut errors = Vec::new();
        let mut package_names = BTreeSet::new();
        for package in &self.packages {
            if !package_names.insert(package.package.as_str().to_owned()) {
                errors.push(PackageLockValidationError::DuplicatePackage {
                    package: package.package.as_str().to_owned(),
                });
            }
            if package.source_identity.is_empty() {
                errors.push(PackageLockValidationError::MissingSourceIdentity {
                    package: package.package.as_str().to_owned(),
                });
            }
            if package.manifest_fingerprint.is_empty() {
                errors.push(PackageLockValidationError::MissingManifestFingerprint {
                    package: package.package.as_str().to_owned(),
                });
            }

            let mut aliases = BTreeSet::new();
            for dependency in &package.dependencies {
                if !aliases.insert(dependency.alias.as_str().to_owned()) {
                    errors.push(PackageLockValidationError::DuplicateDependencyAlias {
                        package: package.package.as_str().to_owned(),
                        alias: dependency.alias.as_str().to_owned(),
                    });
                }
            }
        }

        if !package_names.contains(self.root_package.as_str()) {
            errors.push(PackageLockValidationError::MissingRootPackage {
                package: self.root_package.as_str().to_owned(),
            });
        }
        for package in &self.packages {
            for dependency in &package.dependencies {
                if !package_names.contains(dependency.package.as_str()) {
                    errors.push(PackageLockValidationError::MissingDependencyPackage {
                        package: package.package.as_str().to_owned(),
                        alias: dependency.alias.as_str().to_owned(),
                        dependency: dependency.package.as_str().to_owned(),
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn from_json(json: &str) -> Result<Self, PackageLockParseError> {
        let value = JsonParser::new(json).parse()?;
        let root = value.as_object("$")?;
        ensure_fields(root, "$", &["schema_version", "root_package", "packages"])?;

        let schema_version = value_u32(field(root, "schema_version", "$")?, "$.schema_version")?;
        if schema_version != PACKAGE_LOCK_SCHEMA_VERSION {
            return Err(PackageLockParseError::UnsupportedSchemaVersion {
                found: schema_version,
                supported: PACKAGE_LOCK_SCHEMA_VERSION,
            });
        }

        let root_package = parse_package_name(
            value_string(field(root, "root_package", "$")?, "$.root_package")?,
            "$.root_package",
        )?;
        let packages = parse_locked_packages(field(root, "packages", "$")?, "$.packages")?;
        Ok(Self {
            schema_version,
            root_package,
            packages,
        })
    }

    pub fn read_from_path(path: impl AsRef<Path>) -> Result<Self, PackageLockPersistenceError> {
        let path = path.as_ref();
        let contents =
            fs::read_to_string(path).map_err(|error| PackageLockPersistenceError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
        let lock = Self::from_json(&contents).map_err(PackageLockPersistenceError::Parse)?;
        lock.validate_closure()
            .map_err(PackageLockPersistenceError::InvalidClosure)?;
        Ok(lock.normalized())
    }

    pub fn write_to_path(&self, path: impl AsRef<Path>) -> Result<(), PackageLockPersistenceError> {
        self.validate_closure()
            .map_err(PackageLockPersistenceError::InvalidClosure)?;
        let path = path.as_ref();
        let temp_path = temporary_lock_path(path, self);
        fs::write(&temp_path, self.to_json()).map_err(|error| PackageLockPersistenceError::Io {
            path: temp_path.clone(),
            message: error.to_string(),
        })?;
        if let Err(error) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(PackageLockPersistenceError::Io {
                path: path.to_path_buf(),
                message: error.to_string(),
            });
        }
        Ok(())
    }

    pub fn to_json(&self) -> String {
        let normalized = self.normalized_clone();
        let mut json = String::new();
        json.push_str("{\n");
        push_number_field(
            &mut json,
            1,
            "schema_version",
            normalized.schema_version,
            true,
        );
        push_string_field(
            &mut json,
            1,
            "root_package",
            normalized.root_package.as_str(),
            true,
        );
        push_packages(&mut json, &normalized.packages);
        json.push_str("\n}\n");
        json
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum JsonValue {
    Object(Vec<(String, JsonValue)>),
    Array(Vec<JsonValue>),
    String(String),
    Number(u64),
    Bool,
    Null,
}

impl JsonValue {
    fn as_object(&self, path: &str) -> Result<&[(String, JsonValue)], PackageLockParseError> {
        match self {
            JsonValue::Object(fields) => Ok(fields),
            _ => Err(PackageLockParseError::InvalidField {
                field: path.to_owned(),
                message: "expected JSON object".to_owned(),
            }),
        }
    }

    fn as_array(&self, path: &str) -> Result<&[JsonValue], PackageLockParseError> {
        match self {
            JsonValue::Array(values) => Ok(values),
            _ => Err(PackageLockParseError::InvalidField {
                field: path.to_owned(),
                message: "expected JSON array".to_owned(),
            }),
        }
    }
}

struct JsonParser<'a> {
    input: &'a str,
    index: usize,
}

impl<'a> JsonParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, index: 0 }
    }

    fn parse(mut self) -> Result<JsonValue, PackageLockParseError> {
        self.skip_whitespace();
        let value = self.parse_value()?;
        self.skip_whitespace();
        if self.index != self.input.len() {
            return self.invalid_json("trailing characters after JSON value");
        }
        Ok(value)
    }

    fn parse_value(&mut self) -> Result<JsonValue, PackageLockParseError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => self.parse_string().map(JsonValue::String),
            Some(b'0'..=b'9') => self.parse_number().map(JsonValue::Number),
            Some(b't') => {
                self.consume_literal("true")?;
                Ok(JsonValue::Bool)
            }
            Some(b'f') => {
                self.consume_literal("false")?;
                Ok(JsonValue::Bool)
            }
            Some(b'n') => {
                self.consume_literal("null")?;
                Ok(JsonValue::Null)
            }
            Some(_) => self.invalid_json("unexpected character while parsing JSON value"),
            None => self.invalid_json("unexpected end of JSON input"),
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, PackageLockParseError> {
        self.expect_byte(b'{')?;
        self.skip_whitespace();
        let mut fields = Vec::new();
        let mut names = BTreeSet::new();
        if self.consume_byte(b'}') {
            return Ok(JsonValue::Object(fields));
        }
        loop {
            self.skip_whitespace();
            let name = self.parse_string()?;
            if !names.insert(name.clone()) {
                return Err(PackageLockParseError::InvalidJson {
                    message: format!("duplicate JSON object field `{name}`"),
                });
            }
            self.skip_whitespace();
            self.expect_byte(b':')?;
            let value = self.parse_value()?;
            fields.push((name, value));
            self.skip_whitespace();
            if self.consume_byte(b'}') {
                break;
            }
            self.expect_byte(b',')?;
        }
        Ok(JsonValue::Object(fields))
    }

    fn parse_array(&mut self) -> Result<JsonValue, PackageLockParseError> {
        self.expect_byte(b'[')?;
        self.skip_whitespace();
        let mut values = Vec::new();
        if self.consume_byte(b']') {
            return Ok(JsonValue::Array(values));
        }
        loop {
            values.push(self.parse_value()?);
            self.skip_whitespace();
            if self.consume_byte(b']') {
                break;
            }
            self.expect_byte(b',')?;
        }
        Ok(JsonValue::Array(values))
    }

    fn parse_string(&mut self) -> Result<String, PackageLockParseError> {
        self.expect_byte(b'"')?;
        let mut value = String::new();
        while let Some(byte) = self.next_byte() {
            match byte {
                b'"' => return Ok(value),
                b'\\' => value.push(self.parse_escape()?),
                0x00..=0x1f => {
                    return self.invalid_json("unescaped control character in JSON string");
                }
                _ => {
                    let ch = self.input[self.index - 1..]
                        .chars()
                        .next()
                        .expect("byte index points at a character");
                    value.push(ch);
                    self.index = self.index - 1 + ch.len_utf8();
                }
            }
        }
        self.invalid_json("unterminated JSON string")
    }

    fn parse_escape(&mut self) -> Result<char, PackageLockParseError> {
        let Some(byte) = self.next_byte() else {
            return self.invalid_json("unterminated JSON escape");
        };
        match byte {
            b'"' => Ok('"'),
            b'\\' => Ok('\\'),
            b'/' => Ok('/'),
            b'b' => Ok('\u{0008}'),
            b'f' => Ok('\u{000c}'),
            b'n' => Ok('\n'),
            b'r' => Ok('\r'),
            b't' => Ok('\t'),
            b'u' => self.parse_unicode_escape(),
            _ => self.invalid_json("invalid JSON escape"),
        }
    }

    fn parse_unicode_escape(&mut self) -> Result<char, PackageLockParseError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let Some(byte) = self.next_byte() else {
                return self.invalid_json("unterminated unicode escape");
            };
            let Some(digit) = (byte as char).to_digit(16) else {
                return self.invalid_json("invalid unicode escape digit");
            };
            value = (value << 4) | digit;
        }
        char::from_u32(value).ok_or_else(|| PackageLockParseError::InvalidJson {
            message: "invalid unicode scalar value".to_owned(),
        })
    }

    fn parse_number(&mut self) -> Result<u64, PackageLockParseError> {
        let start = self.index;
        if self.consume_byte(b'0') {
            if matches!(self.peek(), Some(b'0'..=b'9')) {
                return self.invalid_json("JSON numbers cannot have leading zeroes");
            }
        } else {
            self.expect_digit()?;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.index += 1;
            }
        }
        if matches!(self.peek(), Some(b'.' | b'e' | b'E')) {
            return self.invalid_json("lock schema accepts integer JSON numbers only");
        }
        self.input[start..self.index]
            .parse::<u64>()
            .map_err(|error| PackageLockParseError::InvalidJson {
                message: format!("invalid JSON number: {error}"),
            })
    }

    fn consume_literal(&mut self, literal: &str) -> Result<(), PackageLockParseError> {
        if self.input[self.index..].starts_with(literal) {
            self.index += literal.len();
            Ok(())
        } else {
            self.invalid_json("invalid JSON literal")
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.index += 1;
        }
    }

    fn expect_byte(&mut self, expected: u8) -> Result<(), PackageLockParseError> {
        if self.consume_byte(expected) {
            Ok(())
        } else {
            self.invalid_json(&format!("expected `{}`", expected as char))
        }
    }

    fn expect_digit(&mut self) -> Result<(), PackageLockParseError> {
        match self.peek() {
            Some(b'1'..=b'9') => {
                self.index += 1;
                Ok(())
            }
            _ => self.invalid_json("expected digit"),
        }
    }

    fn consume_byte(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn next_byte(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.index += 1;
        Some(byte)
    }

    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.index).copied()
    }

    fn invalid_json<T>(&self, message: &str) -> Result<T, PackageLockParseError> {
        Err(PackageLockParseError::InvalidJson {
            message: format!("{message} at byte {}", self.index),
        })
    }
}

fn parse_locked_packages(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<LockedPackage>, PackageLockParseError> {
    value
        .as_array(path)?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_locked_package(value, &format!("{path}[{index}]")))
        .collect()
}

fn parse_locked_package(
    value: &JsonValue,
    path: &str,
) -> Result<LockedPackage, PackageLockParseError> {
    let fields = value.as_object(path)?;
    ensure_fields(
        fields,
        path,
        &[
            "package",
            "source_kind",
            "source_locator",
            "source_identity",
            "manifest_fingerprint",
            "build_observation",
            "dependencies",
            "trust_receipts",
        ],
    )?;
    Ok(LockedPackage {
        package: parse_package_name(
            value_string(field(fields, "package", path)?, &format!("{path}.package"))?,
            &format!("{path}.package"),
        )?,
        source_kind: value_string(
            field(fields, "source_kind", path)?,
            &format!("{path}.source_kind"),
        )?
        .to_owned(),
        source_locator: value_string(
            field(fields, "source_locator", path)?,
            &format!("{path}.source_locator"),
        )?
        .to_owned(),
        source_identity: value_string(
            field(fields, "source_identity", path)?,
            &format!("{path}.source_identity"),
        )?
        .to_owned(),
        manifest_fingerprint: value_string(
            field(fields, "manifest_fingerprint", path)?,
            &format!("{path}.manifest_fingerprint"),
        )?
        .to_owned(),
        build_observation: value_string(
            field(fields, "build_observation", path)?,
            &format!("{path}.build_observation"),
        )?
        .to_owned(),
        dependencies: parse_locked_dependencies(
            field(fields, "dependencies", path)?,
            &format!("{path}.dependencies"),
        )?,
        trust_receipts: parse_string_array(
            field(fields, "trust_receipts", path)?,
            &format!("{path}.trust_receipts"),
        )?,
    })
}

fn parse_locked_dependencies(
    value: &JsonValue,
    path: &str,
) -> Result<Vec<LockedDependency>, PackageLockParseError> {
    value
        .as_array(path)?
        .iter()
        .enumerate()
        .map(|(index, value)| parse_locked_dependency(value, &format!("{path}[{index}]")))
        .collect()
}

fn parse_locked_dependency(
    value: &JsonValue,
    path: &str,
) -> Result<LockedDependency, PackageLockParseError> {
    let fields = value.as_object(path)?;
    ensure_fields(fields, path, &["alias", "package"])?;
    Ok(LockedDependency {
        alias: parse_alias_name(
            value_string(field(fields, "alias", path)?, &format!("{path}.alias"))?,
            &format!("{path}.alias"),
        )?,
        package: parse_package_name(
            value_string(field(fields, "package", path)?, &format!("{path}.package"))?,
            &format!("{path}.package"),
        )?,
    })
}

fn parse_string_array(value: &JsonValue, path: &str) -> Result<Vec<String>, PackageLockParseError> {
    value
        .as_array(path)?
        .iter()
        .enumerate()
        .map(|(index, value)| value_string(value, &format!("{path}[{index}]")).map(str::to_owned))
        .collect()
}

fn ensure_fields(
    fields: &[(String, JsonValue)],
    path: &str,
    allowed: &[&str],
) -> Result<(), PackageLockParseError> {
    let actual = fields
        .iter()
        .map(|(name, _)| name.as_str())
        .collect::<BTreeSet<_>>();
    let expected = allowed.iter().copied().collect::<BTreeSet<_>>();
    for name in actual.difference(&expected) {
        return Err(PackageLockParseError::UnexpectedField {
            field: format!("{path}.{name}"),
        });
    }
    for name in expected.difference(&actual) {
        return Err(PackageLockParseError::MissingField {
            field: format!("{path}.{name}"),
        });
    }
    Ok(())
}

fn field<'a>(
    fields: &'a [(String, JsonValue)],
    name: &str,
    path: &str,
) -> Result<&'a JsonValue, PackageLockParseError> {
    fields
        .iter()
        .find(|(field_name, _)| field_name == name)
        .map(|(_, value)| value)
        .ok_or_else(|| PackageLockParseError::MissingField {
            field: format!("{path}.{name}"),
        })
}

fn value_string<'a>(value: &'a JsonValue, path: &str) -> Result<&'a str, PackageLockParseError> {
    match value {
        JsonValue::String(value) => Ok(value),
        _ => Err(PackageLockParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON string".to_owned(),
        }),
    }
}

fn value_u32(value: &JsonValue, path: &str) -> Result<u32, PackageLockParseError> {
    match value {
        JsonValue::Number(value) => {
            u32::try_from(*value).map_err(|error| PackageLockParseError::InvalidField {
                field: path.to_owned(),
                message: error.to_string(),
            })
        }
        _ => Err(PackageLockParseError::InvalidField {
            field: path.to_owned(),
            message: "expected JSON integer".to_owned(),
        }),
    }
}

fn parse_package_name(value: &str, path: &str) -> Result<PackageName, PackageLockParseError> {
    PackageName::parse(value).map_err(|message| PackageLockParseError::InvalidField {
        field: path.to_owned(),
        message,
    })
}

fn parse_alias_name(value: &str, path: &str) -> Result<AliasName, PackageLockParseError> {
    AliasName::parse(value).map_err(|message| PackageLockParseError::InvalidField {
        field: path.to_owned(),
        message,
    })
}

fn temporary_lock_path(path: &Path, lock: &PackageLock) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("omega.lock");
    let temp_name = format!(
        ".{file_name}.tmp-{}-{}",
        std::process::id(),
        lock.fingerprint()
    );
    path.with_file_name(temp_name)
}

fn push_packages(json: &mut String, packages: &[LockedPackage]) {
    push_indent(json, 1);
    push_json_string(json, "packages");
    json.push_str(": [");
    for (index, package) in packages.iter().enumerate() {
        if index == 0 {
            json.push('\n');
        } else {
            json.push_str(",\n");
        }
        push_indent(json, 2);
        json.push_str("{\n");
        push_string_field(json, 3, "package", package.package.as_str(), true);
        push_string_field(json, 3, "source_kind", &package.source_kind, true);
        push_string_field(json, 3, "source_locator", &package.source_locator, true);
        push_string_field(json, 3, "source_identity", &package.source_identity, true);
        push_string_field(
            json,
            3,
            "manifest_fingerprint",
            &package.manifest_fingerprint,
            true,
        );
        push_string_field(
            json,
            3,
            "build_observation",
            &package.build_observation,
            true,
        );
        push_dependencies(json, &package.dependencies);
        push_string_array_field(json, 3, "trust_receipts", &package.trust_receipts, false);
        push_indent(json, 2);
        json.push('}');
    }
    if !packages.is_empty() {
        json.push('\n');
        push_indent(json, 1);
    }
    json.push(']');
}

fn push_dependencies(json: &mut String, dependencies: &[LockedDependency]) {
    push_indent(json, 3);
    push_json_string(json, "dependencies");
    json.push_str(": [");
    for (index, dependency) in dependencies.iter().enumerate() {
        if index == 0 {
            json.push('\n');
        } else {
            json.push_str(",\n");
        }
        push_indent(json, 4);
        json.push('{');
        push_inline_string_field(json, "alias", dependency.alias.as_str(), true);
        push_inline_string_field(json, "package", dependency.package.as_str(), false);
        json.push('}');
    }
    if !dependencies.is_empty() {
        json.push('\n');
        push_indent(json, 3);
    }
    json.push_str("],\n");
}

fn push_number_field(json: &mut String, indent: usize, name: &str, value: u32, comma: bool) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    json.push_str(&value.to_string());
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_string_field(json: &mut String, indent: usize, name: &str, value: &str, comma: bool) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": ");
    push_json_string(json, value);
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_string_array_field(
    json: &mut String,
    indent: usize,
    name: &str,
    values: &[String],
    comma: bool,
) {
    push_indent(json, indent);
    push_json_string(json, name);
    json.push_str(": [");
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(json, value);
    }
    json.push(']');
    if comma {
        json.push(',');
    }
    json.push('\n');
}

fn push_inline_string_field(json: &mut String, name: &str, value: &str, comma: bool) {
    push_json_string(json, name);
    json.push_str(": ");
    push_json_string(json, value);
    if comma {
        json.push_str(", ");
    }
}

fn push_json_string(json: &mut String, value: &str) {
    json.push('"');
    for ch in value.chars() {
        match ch {
            '"' => json.push_str("\\\""),
            '\\' => json.push_str("\\\\"),
            '\n' => json.push_str("\\n"),
            '\r' => json.push_str("\\r"),
            '\t' => json.push_str("\\t"),
            ch if ch.is_control() => json.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => json.push(ch),
        }
    }
    json.push('"');
}

fn push_indent(json: &mut String, level: usize) {
    for _ in 0..level {
        json.push_str("  ");
    }
}

fn sorted_unique(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn format_sha256(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{DependencyAlias, SourceIdentity, TrustReceipt};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn package(name: &str) -> PackageName {
        PackageName::parse(name).unwrap()
    }

    fn alias(name: &str) -> AliasName {
        AliasName::parse(name).unwrap()
    }

    fn locked_package(name: &str) -> LockedPackage {
        LockedPackage {
            package: package(name),
            source_kind: "git".to_owned(),
            source_locator: format!("https://github.com/CathedralOS/{name}"),
            source_identity: "tree:abc".to_owned(),
            manifest_fingerprint: "manifest:def".to_owned(),
            build_observation: "pure".to_owned(),
            dependencies: Vec::new(),
            trust_receipts: Vec::new(),
        }
    }

    fn manifest(name: &str) -> PackageCapabilityManifest {
        PackageCapabilityManifest::new(
            package(name),
            SourceIdentity {
                kind: "git".to_owned(),
                locator: format!("https://github.com/CathedralOS/{name}"),
                resolved: format!("commit:{name}"),
            },
        )
    }

    fn test_lock() -> PackageLock {
        let mut root = locked_package("graph-workbench");
        root.dependencies = vec![
            LockedDependency {
                alias: alias("file_journal"),
                package: package("file-journal"),
            },
            LockedDependency {
                alias: alias("arithmetic_kernels"),
                package: package("arithmetic-kernels"),
            },
        ];
        root.trust_receipts = vec!["receipt:b".to_owned(), "receipt:a".to_owned()];

        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![
            root,
            locked_package("file-journal"),
            locked_package("arithmetic-kernels"),
        ];
        lock
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "omega-packages-lock-test-{}-{name}-{nanos}",
            std::process::id()
        ));
        fs::create_dir(&dir).expect("temp test directory should be creatable");
        dir
    }

    #[test]
    fn lock_json_and_fingerprint_are_normalized() {
        let left = test_lock();

        let mut right = PackageLock::new(package("graph-workbench"));
        let mut graph = locked_package("graph-workbench");
        graph.dependencies = vec![
            LockedDependency {
                alias: alias("file_journal"),
                package: package("file-journal"),
            },
            LockedDependency {
                alias: alias("arithmetic_kernels"),
                package: package("arithmetic-kernels"),
            },
        ];
        graph.trust_receipts = vec!["receipt:a".to_owned(), "receipt:b".to_owned()];
        graph.dependencies.reverse();
        right.packages = vec![
            locked_package("file-journal"),
            locked_package("arithmetic-kernels"),
            graph,
        ];

        assert_eq!(left.to_json(), right.to_json());
        assert_eq!(left.fingerprint(), right.fingerprint());
        assert!(
            left.to_json()
                .contains("\"package\": \"arithmetic-kernels\"")
        );
    }

    #[test]
    fn lock_json_round_trip_is_normalized() {
        let lock = test_lock();
        let parsed = PackageLock::from_json(&lock.to_json()).expect("lock JSON should parse");

        assert_eq!(parsed.to_json(), lock.to_json());
        assert_eq!(parsed.fingerprint(), lock.fingerprint());
        assert_eq!(parsed.validate_closure(), Ok(()));
    }

    #[test]
    fn lock_json_parser_rejects_unknown_or_invalid_schema_fields() {
        let extra_field = "{\n  \"schema_version\": 1,\n  \"root_package\": \"graph-workbench\",\n  \"unexpected\": \"value\",\n  \"packages\": []\n}\n";
        assert_eq!(
            PackageLock::from_json(extra_field),
            Err(PackageLockParseError::UnexpectedField {
                field: "$.unexpected".to_owned()
            })
        );

        let bad_name = test_lock()
            .to_json()
            .replace("\"graph-workbench\"", "\"graph_workbench\"");
        assert!(matches!(
            PackageLock::from_json(&bad_name),
            Err(PackageLockParseError::InvalidField { field, .. }) if field == "$.root_package"
        ));

        let bad_schema = test_lock()
            .to_json()
            .replace("\"schema_version\": 1", "\"schema_version\": 2");
        assert_eq!(
            PackageLock::from_json(&bad_schema),
            Err(PackageLockParseError::UnsupportedSchemaVersion {
                found: 2,
                supported: PACKAGE_LOCK_SCHEMA_VERSION,
            })
        );
    }

    #[test]
    fn lock_read_write_round_trip_validates_and_normalizes() {
        let dir = temp_dir("round-trip");
        let path = dir.join("omega.lock");
        let lock = test_lock();

        lock.write_to_path(&path)
            .expect("valid lock should be written");
        let written = fs::read_to_string(&path).expect("lock file should exist");
        let read = PackageLock::read_from_path(&path).expect("valid lock should read");

        assert_eq!(written, lock.to_json());
        assert_eq!(read.to_json(), lock.to_json());

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    #[test]
    fn lock_write_rejects_invalid_closure_before_touching_file() {
        let dir = temp_dir("invalid-write");
        let path = dir.join("omega.lock");
        fs::write(&path, "previous").expect("seed file should be writable");

        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![locked_package("file-journal")];

        assert!(matches!(
            lock.write_to_path(&path),
            Err(PackageLockPersistenceError::InvalidClosure(_))
        ));
        assert_eq!(
            fs::read_to_string(&path).expect("seed file should still exist"),
            "previous"
        );

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    #[test]
    fn lock_read_rejects_invalid_closure() {
        let dir = temp_dir("invalid-read");
        let path = dir.join("omega.lock");
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![locked_package("file-journal")];
        fs::write(&path, lock.to_json()).expect("invalid lock fixture should be writable");

        assert!(matches!(
            PackageLock::read_from_path(&path),
            Err(PackageLockPersistenceError::InvalidClosure(_))
        ));

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    #[test]
    fn lock_lookup_uses_canonical_package_name() {
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages.push(locked_package("file-journal"));

        assert!(lock.package(&package("file-journal")).is_some());
        assert!(PackageName::parse("file_journal").is_err());
    }

    #[test]
    fn lock_assembly_from_manifests_records_edges_and_receipts() {
        let mut root = manifest("graph-workbench");
        root.dependency_aliases.push(DependencyAlias {
            alias: alias("file_journal"),
            package: package("file-journal"),
            source_fingerprint: "source:file-journal".to_owned(),
        });
        let mut child = manifest("file-journal");
        child.trust_receipts.push(TrustReceipt {
            kind: "review".to_owned(),
            subject: "filesystem reach".to_owned(),
            identity: "receipt:filesystem".to_owned(),
        });

        let lock =
            PackageLock::from_manifests(package("graph-workbench"), &[child.clone(), root.clone()])
                .expect("closed manifest set should assemble a lock");

        assert_eq!(lock.validate_closure(), Ok(()));
        let root_entry = lock
            .package(&package("graph-workbench"))
            .expect("root entry should exist");
        assert_eq!(
            root_entry.dependencies,
            vec![LockedDependency {
                alias: alias("file_journal"),
                package: package("file-journal"),
            }]
        );
        let child_entry = lock
            .package(&package("file-journal"))
            .expect("child entry should exist");
        assert_eq!(
            child_entry.trust_receipts,
            vec!["receipt:filesystem".to_owned()]
        );
        assert_eq!(child_entry.manifest_fingerprint, child.fingerprint());
    }

    #[test]
    fn lock_assembly_rejects_duplicate_manifest_package() {
        let root = manifest("graph-workbench");

        assert_eq!(
            PackageLock::from_manifests(package("graph-workbench"), &[root.clone(), root]),
            Err(PackageLockAssemblyError::DuplicateManifest {
                package: "graph-workbench".to_owned()
            })
        );
    }

    #[test]
    fn lock_assembly_rejects_open_dependency_edge() {
        let mut root = manifest("graph-workbench");
        root.dependency_aliases.push(DependencyAlias {
            alias: alias("file_journal"),
            package: package("file-journal"),
            source_fingerprint: "source:file-journal".to_owned(),
        });

        let error = PackageLock::from_manifests(package("graph-workbench"), &[root])
            .expect_err("missing dependency manifest must reject");

        assert!(matches!(
            error,
            PackageLockAssemblyError::InvalidClosure(errors)
                if errors.contains(&PackageLockValidationError::MissingDependencyPackage {
                    package: "graph-workbench".to_owned(),
                    alias: "file_journal".to_owned(),
                    dependency: "file-journal".to_owned(),
                })
        ));
    }

    #[test]
    fn lock_validation_accepts_closed_package_graph() {
        let mut root = locked_package("graph-workbench");
        root.dependencies.push(LockedDependency {
            alias: alias("file_journal"),
            package: package("file-journal"),
        });
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![root, locked_package("file-journal")];

        assert_eq!(lock.validate_closure(), Ok(()));
    }

    #[test]
    fn lock_validation_rejects_missing_root_package() {
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![locked_package("file-journal")];

        let errors = lock
            .validate_closure()
            .expect_err("missing root must reject");

        assert!(
            errors.contains(&PackageLockValidationError::MissingRootPackage {
                package: "graph-workbench".to_owned()
            })
        );
    }

    #[test]
    fn lock_validation_rejects_duplicate_packages() {
        let mut lock = PackageLock::new(package("file-journal"));
        lock.packages = vec![
            locked_package("file-journal"),
            locked_package("file-journal"),
        ];

        let errors = lock
            .validate_closure()
            .expect_err("duplicate package must reject");

        assert!(
            errors.contains(&PackageLockValidationError::DuplicatePackage {
                package: "file-journal".to_owned()
            })
        );
    }

    #[test]
    fn lock_validation_rejects_duplicate_alias_and_missing_dependency_target() {
        let mut root = locked_package("graph-workbench");
        root.dependencies = vec![
            LockedDependency {
                alias: alias("file_journal"),
                package: package("file-journal"),
            },
            LockedDependency {
                alias: alias("file_journal"),
                package: package("network-overreach"),
            },
        ];
        let mut lock = PackageLock::new(package("graph-workbench"));
        lock.packages = vec![root];

        let errors = lock
            .validate_closure()
            .expect_err("bad dependency closure must reject");

        assert!(
            errors.contains(&PackageLockValidationError::DuplicateDependencyAlias {
                package: "graph-workbench".to_owned(),
                alias: "file_journal".to_owned(),
            })
        );
        assert!(
            errors.contains(&PackageLockValidationError::MissingDependencyPackage {
                package: "graph-workbench".to_owned(),
                alias: "file_journal".to_owned(),
                dependency: "file-journal".to_owned(),
            })
        );
        assert!(
            errors.contains(&PackageLockValidationError::MissingDependencyPackage {
                package: "graph-workbench".to_owned(),
                alias: "file_journal".to_owned(),
                dependency: "network-overreach".to_owned(),
            })
        );
    }

    #[test]
    fn lock_validation_rejects_missing_source_and_manifest_identity() {
        let mut locked = locked_package("file-journal");
        locked.source_identity.clear();
        locked.manifest_fingerprint.clear();
        let mut lock = PackageLock::new(package("file-journal"));
        lock.packages = vec![locked];

        let errors = lock
            .validate_closure()
            .expect_err("missing identities must reject");

        assert!(
            errors.contains(&PackageLockValidationError::MissingSourceIdentity {
                package: "file-journal".to_owned(),
            })
        );
        assert!(
            errors.contains(&PackageLockValidationError::MissingManifestFingerprint {
                package: "file-journal".to_owned(),
            })
        );
    }
}
