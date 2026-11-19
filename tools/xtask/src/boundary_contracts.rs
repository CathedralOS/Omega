//! Keep the reviewed fields and exports of a phase contract in sync with Rust.
//! Cargo edges cannot detect a prior program hidden inside an otherwise allowed
//! data crate. This audit parses every configured source file, including cfg
//! alternatives, and rejects missing, changed or stale field/export judgments.

use quote::ToTokens;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use syn::visit::Visit;

pub const MANIFEST: &str = "tools/xtask/pipeline-contracts.json";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ContractAudit {
    version: u32,
    roots: Vec<AuditRoot>,
    items: Vec<Judgment>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AuditRoot {
    path: String,
    allowed_kinds: Vec<Kind>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Judgment {
    file: String,
    item: String,
    kind: Kind,
    signature: String,
    meaning: String,
    producer: String,
    consumers: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Primitive,
    Definition,
    CarriedRecord,
    Producer,
    Checker,
    HostOperation,
    TestSupport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ObservedItem {
    name: String,
    signature: String,
}

fn digest(tokens: impl ToTokens) -> String {
    // Token spelling removes formatting and comments, but retains attributes,
    // field types, signatures and implementation bodies that could hide work.
    let hash = Sha256::digest(tokens.into_token_stream().to_string().as_bytes());
    hash.iter().map(|byte| format!("{byte:02x}")).collect()
}

struct Inventory {
    scope: Vec<String>,
    items: Vec<ObservedItem>,
}

impl Inventory {
    fn record(&mut self, name: &str, tokens: impl ToTokens) {
        let prefix = self.scope.join("::");
        self.items.push(ObservedItem {
            name: if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}::{name}")
            },
            signature: digest(tokens),
        });
    }

    fn fields(&mut self, fields: &syn::Fields) {
        for (ordinal, field) in fields.iter().enumerate() {
            let name = field
                .ident
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| ordinal.to_string());
            self.record(&format!("field:{name}"), field);
        }
    }
}

impl<'ast> Visit<'ast> for Inventory {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        // Out-of-line modules are covered by the recursive file inventory.
        let attrs = &item.attrs;
        let visibility = &item.vis;
        let name = &item.ident;
        self.record(
            &format!("module:{name}"),
            quote::quote! { #(#attrs)* #visibility mod #name; },
        );
        self.scope.push(item.ident.to_string());
        if let Some((_, items)) = &item.content {
            for item in items {
                self.visit_item(item);
            }
        }
        self.scope.pop();
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.record(&format!("struct:{}", item.ident), item);
        self.scope.push(item.ident.to_string());
        self.fields(&item.fields);
        self.scope.pop();
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        self.record(&format!("enum:{}", item.ident), item);
        self.scope.push(item.ident.to_string());
        for variant in &item.variants {
            self.scope.push(variant.ident.to_string());
            self.fields(&variant.fields);
            self.scope.pop();
        }
        self.scope.pop();
    }

    fn visit_item_union(&mut self, item: &'ast syn::ItemUnion) {
        self.record(&format!("union:{}", item.ident), item);
        self.scope.push(item.ident.to_string());
        self.fields(&syn::Fields::Named(item.fields.clone()));
        self.scope.pop();
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        // Private aliases can hide the same earlier form as public ones.
        self.record(&format!("alias:{}", item.ident), item);
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.record(&format!("function:{}", item.sig.ident), item);
        // Local types and aliases must not bypass the type inventory.
        self.scope.push(item.sig.ident.to_string());
        syn::visit::visit_block(self, &item.block);
        self.scope.pop();
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        let owner = item.self_ty.to_token_stream().to_string();
        let implemented = item
            .trait_
            .as_ref()
            .map(|(path, _)| path.to_token_stream().to_string());
        self.scope.push(match implemented {
            Some(trait_) => format!("impl:{trait_} for {owner}"),
            None => format!("impl:{owner}"),
        });
        self.record("implementation", item);
        for child in &item.items {
            match child {
                syn::ImplItem::Fn(method) => {
                    self.record(&format!("method:{}", method.sig.ident), method);
                    self.scope.push(method.sig.ident.to_string());
                    syn::visit::visit_block(self, &method.block);
                    self.scope.pop();
                }
                syn::ImplItem::Type(alias) => self.record(&format!("alias:{}", alias.ident), alias),
                syn::ImplItem::Macro(mac) => self.record("macro", mac),
                _ => {}
            }
        }
        self.scope.pop();
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        self.record(&format!("trait:{}", item.ident), item);
        self.scope.push(item.ident.to_string());
        for child in &item.items {
            match child {
                syn::TraitItem::Fn(method) => {
                    self.record(&format!("method:{}", method.sig.ident), method);
                    if let Some(body) = &method.default {
                        self.scope.push(method.sig.ident.to_string());
                        syn::visit::visit_block(self, body);
                        self.scope.pop();
                    }
                }
                syn::TraitItem::Type(alias) => {
                    self.record(&format!("alias:{}", alias.ident), alias)
                }
                syn::TraitItem::Const(value) => {
                    self.record(&format!("const:{}", value.ident), value)
                }
                syn::TraitItem::Macro(mac) => self.record("macro", mac),
                _ => {}
            }
        }
        self.scope.pop();
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        // Pin imports too: retargeting an alias must invalidate its judgment.
        self.record(&format!("use:{}", item.tree.to_token_stream()), item);
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        let name = item
            .ident
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| item.mac.path.to_token_stream().to_string());
        self.record(&format!("macro:{name}"), item);
    }

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        self.record(&format!("const:{}", item.ident), item);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.record(&format!("static:{}", item.ident), item);
    }
}

fn inspect(source: &str) -> Result<Vec<ObservedItem>, String> {
    let file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let mut inventory = Inventory {
        scope: Vec::new(),
        items: Vec::new(),
    };
    inventory.visit_file(&file);
    // cfg alternatives and repeated macro invocations remain separate rows.
    let mut counts = BTreeMap::<String, usize>::new();
    for item in &mut inventory.items {
        let occurrence = counts.entry(item.name.clone()).or_default();
        *occurrence += 1;
        item.name.push_str(&format!("#{occurrence}"));
    }
    Ok(inventory.items)
}

fn source_files(root: &Path, paths: &[String]) -> Result<Vec<PathBuf>, String> {
    fn collect(
        root: &Path,
        path: &Path,
        files: &mut BTreeSet<PathBuf>,
        directories: &mut BTreeSet<PathBuf>,
    ) -> Result<(), String> {
        let canonical = path
            .canonicalize()
            .map_err(|error| format!("{}: {error}", path.display()))?;
        if !canonical.starts_with(root) {
            return Err(format!(
                "contract root escapes repository: {}",
                path.display()
            ));
        }
        if canonical.is_dir() {
            if !directories.insert(canonical.clone()) {
                return Ok(());
            }
            for entry in std::fs::read_dir(&canonical).map_err(|error| error.to_string())? {
                collect(
                    root,
                    &entry.map_err(|error| error.to_string())?.path(),
                    files,
                    directories,
                )?;
            }
        } else if canonical
            .extension()
            .is_some_and(|extension| extension == "rs")
        {
            files.insert(canonical);
        }
        Ok(())
    }
    let canonical = root.canonicalize().map_err(|error| error.to_string())?;
    let mut files = BTreeSet::new();
    let mut directories = BTreeSet::new();
    for path in paths {
        collect(
            &canonical,
            &canonical.join(path),
            &mut files,
            &mut directories,
        )?;
    }
    Ok(files.into_iter().collect())
}

pub fn violations(root: &Path, manifest: &Path) -> Result<Vec<String>, String> {
    let bytes =
        std::fs::read(manifest).map_err(|error| format!("{}: {error}", manifest.display()))?;
    let audit: ContractAudit = serde_json::from_slice(&bytes)
        .map_err(|error| format!("{}: {error}", manifest.display()))?;
    if audit.version != 1
        || audit.roots.is_empty()
        || audit.roots.iter().any(|root| root.allowed_kinds.is_empty())
    {
        return Err("unsupported or empty pipeline contract audit".into());
    }
    let canonical = root.canonicalize().map_err(|error| error.to_string())?;
    let mut expected = BTreeMap::new();
    let mut violations = Vec::new();
    for judgment in &audit.items {
        let owner = audit
            .roots
            .iter()
            .filter(|owner| {
                judgment.file == owner.path
                    || judgment
                        .file
                        .strip_prefix(&owner.path)
                        .is_some_and(|suffix| suffix.starts_with('/'))
            })
            .max_by_key(|owner| owner.path.len());
        if owner.is_none_or(|owner| !owner.allowed_kinds.contains(&judgment.kind)) {
            violations.push(format!(
                "{} {}: classification is not permitted by this owner",
                judgment.file, judgment.item
            ));
        }
        if judgment.meaning.trim().is_empty()
            || judgment.producer.trim().is_empty()
            || judgment.consumers.is_empty()
            || judgment
                .consumers
                .iter()
                .any(|value| value.trim().is_empty())
        {
            violations.push(format!(
                "{} {}: contract judgment requires meaning, producer and consumers",
                judgment.file, judgment.item
            ));
        }
        if expected
            .insert((judgment.file.clone(), judgment.item.clone()), judgment)
            .is_some()
        {
            violations.push(format!(
                "{} {}: repeated contract judgment",
                judgment.file, judgment.item
            ));
        }
    }
    let roots = audit
        .roots
        .iter()
        .map(|root| root.path.clone())
        .collect::<Vec<_>>();
    for path in source_files(root, &roots)? {
        let relative = path
            .strip_prefix(&canonical)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let source =
            std::fs::read_to_string(&path).map_err(|error| format!("{relative}: {error}"))?;
        for item in inspect(&source).map_err(|error| format!("{relative}: {error}"))? {
            match expected.remove(&(relative.clone(), item.name.clone())) {
                None => violations.push(format!("{relative} {}: missing field/export contract judgment", item.name)),
                Some(judgment) if judgment.signature != item.signature => violations.push(format!("{relative} {}: Rust contract changed; review its meaning, producer and consumers", item.name)),
                Some(_) => {}
            }
        }
    }
    for ((file, item), _) in expected {
        violations.push(format!(
            "{file} {item}: stale field/export contract judgment"
        ));
    }
    Ok(violations)
}

fn unreviewed_inventory(root: &Path, roots: &[String]) -> Result<serde_json::Value, String> {
    let canonical = root.canonicalize().map_err(|error| error.to_string())?;
    let mut items = Vec::new();
    for path in source_files(root, roots)? {
        let relative = path
            .strip_prefix(&canonical)
            .map_err(|error| error.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let source = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
        for item in inspect(&source).map_err(|error| format!("{relative}: {error}"))? {
            items.push(serde_json::json!({
                "file": relative, "item": item.name, "signature": item.signature,
                "kind": null, "meaning": "", "producer": "", "consumers": []
            }));
        }
    }
    Ok(serde_json::json!({"version": 1,
        "roots": roots.iter().map(|path| serde_json::json!({"path": path, "allowed_kinds": []})).collect::<Vec<_>>(),
        "items": items
    }))
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let root = crate::repo_root()?;
    let result = match args {
        [command, paths @ ..] if command == "inventory" && !paths.is_empty() => {
            unreviewed_inventory(&root, paths).map(|inventory| {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&inventory).expect("JSON values serialize")
                );
                Vec::new()
            })
        }
        [command] if command == "check" => violations(&root, &root.join(MANIFEST)),
        [command, path] if command == "check" => violations(&root, &root.join(path)),
        _ => {
            eprintln!(
                "usage: cargo xtask pipeline-contracts inventory <source-root>... | check [manifest]"
            );
            return Err(2);
        }
    };
    match result {
        Ok(violations) if violations.is_empty() => Ok(()),
        Ok(violations) => {
            for violation in violations {
                eprintln!("{violation}");
            }
            Err(1)
        }
        Err(reason) => {
            eprintln!("pipeline contract audit: {reason}");
            Err(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let nonce = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "omega-contract-audit-{}-{nonce}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            std::fs::create_dir(path.join("src")).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn audit_refuses_changes_missing_rows_stale_rows_and_wrong_owners() {
        let fixture = Fixture::new();
        let file = fixture.0.join("src/form.rs");
        let manifest = fixture.0.join("audit.json");
        let source = "struct Form { current: u64 }";
        std::fs::write(&file, source).unwrap();
        let mut inventory = unreviewed_inventory(&fixture.0, &["src".into()]).unwrap();
        assert!(
            serde_json::from_value::<ContractAudit>(inventory.clone()).is_err(),
            "generated observations are not approved judgments"
        );
        inventory["roots"][0]["allowed_kinds"] = serde_json::json!(["definition"]);
        for row in inventory["items"].as_array_mut().unwrap() {
            row["kind"] = "definition".into();
            row["meaning"] = "numeric current-form field".into();
            row["producer"] = "fixture input".into();
            row["consumers"] = serde_json::json!(["fixture receiver"]);
        }
        let write = |value: &serde_json::Value| {
            std::fs::write(&manifest, serde_json::to_vec(value).unwrap()).unwrap()
        };
        write(&inventory);
        assert!(violations(&fixture.0, &manifest).unwrap().is_empty());
        std::fs::write(
            &file,
            "struct Form { current: std::sync::Arc<prior::Program> }",
        )
        .unwrap();
        assert!(
            violations(&fixture.0, &manifest)
                .unwrap()
                .iter()
                .any(|reason| reason.contains("Rust contract changed"))
        );
        std::fs::write(&file, source).unwrap();
        let added = fixture.0.join("src/hidden.rs");
        std::fs::write(&added, "type Hidden = prior::Program;").unwrap();
        assert!(
            violations(&fixture.0, &manifest)
                .unwrap()
                .iter()
                .any(|reason| reason.contains("missing field/export"))
        );
        std::fs::remove_file(added).unwrap();
        let mut changed = inventory.clone();
        changed["items"][0]["kind"] = "producer".into();
        write(&changed);
        assert!(
            violations(&fixture.0, &manifest)
                .unwrap()
                .iter()
                .any(|reason| reason.contains("classification is not permitted"))
        );
        let mut changed = inventory;
        changed["items"][0]["item"] = "removed-item".into();
        write(&changed);
        assert!(
            violations(&fixture.0, &manifest)
                .unwrap()
                .iter()
                .any(|reason| reason.contains("stale field/export"))
        );
    }

    #[test]
    fn private_fields_aliases_and_inactive_cfg_are_audited() {
        let items = inspect("type Hidden = old::Program; #[cfg(feature=\"later\")] struct Carrier { prior: std::sync::Arc<Hidden> } pub use old::produce as inspect;").unwrap();
        assert!(items.iter().any(|item| item.name == "alias:Hidden#1"));
        assert!(
            items
                .iter()
                .any(|item| item.name == "Carrier::field:prior#1")
        );
        assert!(
            items
                .iter()
                .any(|item| item.name.starts_with("use:old :: produce as inspect"))
        );
        let changed = inspect("type Hidden = current::Facts;").unwrap();
        assert_ne!(items[0].signature, changed[0].signature);
    }

    #[test]
    fn formatting_is_irrelevant_but_hidden_work_changes_the_judgment() {
        let first = inspect("impl Data { pub fn read(&self) -> u8 { 1 } }").unwrap();
        let formatted =
            inspect("// documentation\nimpl Data {\n pub fn read( &self )->u8{1}\n}").unwrap();
        assert_eq!(first, formatted);
        let producer =
            inspect("impl Data { pub fn read(&self) -> u8 { prior::compile() } }").unwrap();
        assert_ne!(first[0].signature, producer[0].signature);
    }

    #[test]
    fn cfg_alternatives_cannot_silently_replace_each_other() {
        let items =
            inspect("#[cfg(test)] struct Data { n: u8 } #[cfg(not(test))] struct Data { n: u64 }")
                .unwrap();
        assert_eq!(
            items
                .iter()
                .filter(|item| item.name.starts_with("struct:Data#"))
                .count(),
            2
        );
        assert_ne!(items[0].signature, items[2].signature);
    }
}
