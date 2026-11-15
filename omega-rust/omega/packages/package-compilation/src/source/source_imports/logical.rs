//! Discover authored logical modules from immutable captured source units.
//!
//! Discovery uses the normal parser, one standalone page per unit. Its
//! coordinates and declarations never join a checked source cohort: the
//! selected paths go through the ordinary scope, owner and stratum admission
//! afterwards. This cache records module membership, operator declaration
//! units and the names each unit declares at its top level, not visibility or
//! declaration resolution.
//!
//! Each unit's parse is retained beside that record. When admission later
//! selects the same bytes, it adopts the retained page under the admitted
//! source identity (`retained_unit`) instead of lexing and parsing the file a
//! second time. The page carries syntax only: owner, scope, stratum and
//! bindings are still decided by admission.

use super::frontier::RegisteredPackageRoot;
use diagnostics::Diagnostic;
use source::{DependencyScope, LoadedSource, SourceFileSystem, SourceId};
use source_compiler::{SourceInput, UnmeasuredProduction};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use syntax_trees::{ParsedUnit, SyntaxTrees, item::Item};

#[derive(Clone, Debug, PartialEq, Eq, compiler_storage::Record)]
pub(crate) struct LogicalSourceInventory {
    filesystem: SourceFileSystem,
    excluded_roots: Vec<PathBuf>,
    units: Vec<LogicalSourceUnit>,
    /// Positions in `units`, ordered by path, for admission's lookup of a
    /// retained parse. `units` keeps capture order, which selection reports.
    by_path: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, compiler_storage::Record)]
struct LogicalSourceUnit {
    path: PathBuf,
    module: Vec<String>,
    declares_operator_family: bool,
    /// The first name of every top-level declaration, in authored order.
    declared: Vec<String>,
    /// The unit's parse under discovery's private coordinates.
    syntax: Arc<ParsedUnit>,
}

pub(crate) trait LogicalSourceDiscovery {
    fn select(
        &mut self,
        root: &Path,
        logical_path: &[String],
        filesystem: &SourceFileSystem,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        self.select_owned(root, logical_path, filesystem, Vec::new())
    }
    fn select_owned(
        &mut self,
        root: &Path,
        logical_path: &[String],
        filesystem: &SourceFileSystem,
        excluded_roots: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>>;
    /// The units a Build source's import of `logical_path` selects in the
    /// package at `root`: the modules the path names or lies under, and the
    /// module that declares the path's next name. Nothing else is acquired,
    /// so an import that names no declaration of this package selects no
    /// unit and stays deferred (`wiki/spec/language/modules.md`, "Names and
    /// imports": "A logical `use dependency;` may remain deferred").
    ///
    /// `select_owned` instead acquires every enclosing namespace. That is
    /// what a Product source needs, whose module siblings are loaded anyway.
    /// A Build source has no such siblings: acquiring the package's module-less
    /// Product units for a dependency name would check them in the Build
    /// context, where their Product imports mean nothing.
    fn select_declaring(
        &mut self,
        root: &Path,
        logical_path: &[String],
        filesystem: &SourceFileSystem,
        excluded_roots: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>>;
    fn siblings(
        &mut self,
        root: &Path,
        source: &Path,
        filesystem: &SourceFileSystem,
        excluded_roots: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>>;
}

impl LogicalSourceDiscovery for Vec<RegisteredPackageRoot> {
    fn select_owned(
        &mut self,
        root: &Path,
        logical_path: &[String],
        filesystem: &SourceFileSystem,
        excluded_roots: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let inventory = owner_inventory(self, root, filesystem, excluded_roots)?;
        // A namespace import also permits qualified descendant references.
        // Ordinary acquisition observes use declarations, not expression paths;
        // acquire that namespace here and leave exposure to semantic selection.
        Ok(inventory
            .units
            .iter()
            .filter(|unit| {
                logical_path.starts_with(&unit.module) || unit.module.starts_with(logical_path)
            })
            .map(|unit| unit.path.clone())
            .collect())
    }
    fn select_declaring(
        &mut self,
        root: &Path,
        logical_path: &[String],
        filesystem: &SourceFileSystem,
        excluded_roots: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let inventory = owner_inventory(self, root, filesystem, excluded_roots)?;
        let declares = |unit: &LogicalSourceUnit| {
            logical_path.starts_with(&unit.module)
                && logical_path
                    .get(unit.module.len())
                    .is_some_and(|name| unit.declared.contains(name))
        };
        // Every unit of a declaring module is selected: its declarations may
        // name one another across files.
        let declaring = inventory
            .units
            .iter()
            .filter(|unit| declares(unit))
            .map(|unit| unit.module.as_slice())
            .collect::<Vec<_>>();
        Ok(inventory
            .units
            .iter()
            .filter(|unit| {
                unit.module.starts_with(logical_path) || declaring.contains(&unit.module.as_slice())
            })
            .map(|unit| unit.path.clone())
            .collect())
    }
    fn siblings(
        &mut self,
        root: &Path,
        source: &Path,
        filesystem: &SourceFileSystem,
        excluded_roots: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let inventory = owner_inventory(self, root, filesystem, excluded_roots)?;
        let Some(selected) = inventory.units.iter().find(|unit| unit.path == source) else {
            return Ok(Vec::new());
        };
        Ok(inventory
            .units
            .iter()
            // Closed families are declared across their owning package, even
            // when their lexical module is not imported. Acquire those units
            // through the same Product queue; this grants no namespace exposure
            // or authority and leaves exact home/duplicate checks to resolution.
            .filter(|unit| unit.module == selected.module || unit.declares_operator_family)
            .map(|unit| unit.path.clone())
            .collect())
    }
}

fn owner_inventory<'a>(
    owners: &'a mut Vec<RegisteredPackageRoot>,
    root: &Path,
    filesystem: &SourceFileSystem,
    mut excluded_roots: Vec<PathBuf>,
) -> Result<&'a LogicalSourceInventory, Vec<Diagnostic>> {
    excluded_roots.sort();
    excluded_roots.dedup();
    let position = match owners.iter().position(|owner| owner.root == root) {
        Some(position) => position,
        None => {
            // Anonymous and compiler-owned captures acquire their row here;
            // reconciled packages have already registered this exact root.
            owners.push(RegisteredPackageRoot {
                root: root.to_path_buf(),
                identity: None,
                logical_inventory: None,
            });
            owners.len() - 1
        }
    };
    let owner = &mut owners[position];
    if owner.logical_inventory.as_ref().is_none_or(|inventory| {
        inventory.filesystem != *filesystem || inventory.excluded_roots != excluded_roots
    }) {
        owner.logical_inventory = Some(LogicalSourceInventory::parse(
            root,
            filesystem,
            &excluded_roots,
        )?);
    }
    Ok(owner
        .logical_inventory
        .as_ref()
        .expect("owner inventory was retained"))
}

impl LogicalSourceInventory {
    fn parse(
        root: &Path,
        filesystem: &SourceFileSystem,
        excluded_roots: &[PathBuf],
    ) -> Result<Self, Vec<Diagnostic>> {
        let inventory = filesystem.captured_files(root).map_err(|error| {
            vec![Diagnostic::error(format!(
                "cannot discover logical modules in {}: {error}",
                root.display()
            ))]
        })?;
        // Nested package owners come from the admitted graph, never from a
        // filename convention or a second package-discovery authority.
        let paths = inventory
            .iter()
            .filter(|path| {
                matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("omg")
                ) && !path.file_name().is_some_and(|name| name == "build.omg")
                    && !excluded_roots
                        .iter()
                        .any(|package| path.starts_with(package))
            })
            .cloned()
            .collect::<Vec<_>>();
        let loaded = paths
            .iter()
            .enumerate()
            .map(|(index, path)| {
                filesystem
                    .read_to_string(path)
                    .map(|source| LoadedSource {
                        // Private IDs count down from the top so they never
                        // equal the default ID a parser leaves on spans that
                        // name no source; adoption moves only this unit's ID.
                        source_id: SourceId(usize::MAX - index),
                        path: path.clone(),
                        source,
                        origin: None,
                        scope: DependencyScope::Product,
                    })
                    .map_err(|error| {
                        Diagnostic::error(format!(
                            "cannot read captured logical source {}: {error}",
                            path.display()
                        ))
                    })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| vec![error])?;
        // SourceInput renders lexical/parser locations against these private
        // loaded units before returning diagnostics. A private source ID or
        // page generation never reaches the checked-source cohort: adoption
        // moves the retained page to admitted coordinates first.
        let parsed = SourceInput(loaded.into_iter().collect())
            .into_parsed_units(&mut UnmeasuredProduction)?;
        let mut result = Self {
            filesystem: filesystem.clone(),
            excluded_roots: excluded_roots.to_vec(),
            units: Vec::with_capacity(parsed.len()),
            by_path: Vec::new(),
        };
        for (path, unit) in paths.into_iter().zip(parsed) {
            let syntax = &unit.syntax;
            let mut module = None;
            let mut declares_operator_family = false;
            let mut declared = Vec::new();
            for item in unit.root_items() {
                declared.extend(declared_names(syntax, syntax.root_item(*item)));
                if matches!(syntax.root_item(*item), Item::Machine(machine) if machine.spelling.is_some())
                {
                    declares_operator_family = true;
                }
                if let Item::Module(declaration) = syntax.root_item(*item) {
                    if module.is_some() {
                        return Err(vec![Diagnostic::error(format!(
                            "source {} declares more than one logical module",
                            path.display()
                        ))]);
                    }
                    module = Some(
                        syntax
                            .items
                            .identifier_path_members(declaration.path)
                            .iter()
                            .map(|name| name.as_str(syntax).to_owned())
                            .collect::<Vec<_>>(),
                    );
                }
            }
            result.units.push(LogicalSourceUnit {
                path,
                module: module.unwrap_or_default(),
                declares_operator_family,
                declared,
                syntax: Arc::new(unit),
            });
        }
        let count = u32::try_from(result.units.len()).map_err(|_| {
            vec![Diagnostic::error(format!(
                "{} holds more logical sources than discovery can index",
                root.display()
            ))]
        })?;
        result.by_path = (0..count).collect();
        result.by_path.sort_by(|left, right| {
            result.units[*left as usize]
                .path
                .cmp(&result.units[*right as usize].path)
        });
        Ok(result)
    }

    fn retained(&self, path: &Path) -> Option<&ParsedUnit> {
        let position = self
            .by_path
            .binary_search_by(|unit| self.units[*unit as usize].path.as_path().cmp(path))
            .ok()?;
        Some(&self.units[self.by_path[position] as usize].syntax)
    }
}

/// The parse discovery already made of the captured file at `path`, if an
/// owner's current inventory holds it. The caller adopts it only for the same
/// bytes (`SourceInput::append_reusing`), so a changed capture parses afresh.
pub(crate) fn retained_unit<'a>(
    owners: &'a [RegisteredPackageRoot],
    path: &Path,
) -> Option<&'a ParsedUnit> {
    owners
        .iter()
        .filter(|owner| path.starts_with(&owner.root))
        .filter_map(|owner| owner.logical_inventory.as_ref())
        .find_map(|inventory| inventory.retained(path))
}

/// The names an import path can start with to select `item`: its own name,
/// and for a machine attached to a data type, that type.
fn declared_names(syntax: &SyntaxTrees, item: &Item) -> Vec<String> {
    let first = |name: &syntax_trees::identifier::Identifier| {
        let text = name.as_str(syntax);
        text.split("::").next().unwrap_or(text).to_owned()
    };
    let path = |members| {
        syntax
            .items
            .identifier_path_members(members)
            .first()
            .map(first)
    };
    match item {
        Item::Capability(definition) => vec![first(&definition.name)],
        Item::Conformance(conformance) => conformance.alias.iter().map(first).collect(),
        Item::Const(definition) => vec![first(&definition.name)],
        Item::Data(definition) => vec![first(&definition.name)],
        Item::Domain(definition) => vec![first(&definition.name)],
        Item::Measure(definition) => path(definition.name).into_iter().collect(),
        Item::Operator(definition) => path(definition.name).into_iter().collect(),
        Item::Proposition(definition) => vec![first(&definition.name)],
        Item::Machine(machine) => match &machine.attached_data {
            Some(owner) => vec![first(owner)],
            None => vec![first(&machine.name)],
        },
        Item::Trait(definition) => vec![first(&definition.name)],
        Item::TypeAlias(definition) => vec![first(&definition.name)],
        Item::Module(_) | Item::Package(_) | Item::Use(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source::{CapturedSourceNode, CapturedSourceTree};
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn capture_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("logical-test-capture")
    }

    fn captured(rows: &[(&str, &str)]) -> SourceFileSystem {
        let mut entries = BTreeMap::new();
        for (name, body) in rows {
            let path = PathBuf::from(name);
            let mut parent = path.parent();
            while let Some(directory) = parent.filter(|path| !path.as_os_str().is_empty()) {
                entries.insert(directory.to_path_buf(), CapturedSourceNode::Directory);
                parent = directory.parent();
            }
            entries.insert(
                path,
                CapturedSourceNode::File {
                    bytes: Arc::from(body.as_bytes()),
                    executable: false,
                },
            );
        }
        let mut filesystem = SourceFileSystem::default();
        filesystem
            .add_capture(
                capture_root(),
                Arc::new(CapturedSourceTree::new(entries).unwrap()),
            )
            .unwrap();
        filesystem
    }

    #[test]
    fn logical_membership_comes_from_headers_not_filenames() {
        let filesystem = captured(&[
            ("odd.omg", "public data Record {}"),
            ("another.omg", "public machine read() -> u8 { 7 }"),
            ("Record.omg", "module unrelated; public data Hidden {}"),
            ("renamed.omg", "module api; public data Export {}"),
            ("build.omg", "not product source syntax"),
        ]);
        let mut discovery = Vec::<RegisteredPackageRoot>::new();
        assert_eq!(
            discovery
                .select(&capture_root(), &["Record".into()], &filesystem)
                .unwrap(),
            ["another.omg", "odd.omg"].map(|path| capture_root().join(path))
        );
        assert_eq!(
            discovery
                .siblings(
                    &capture_root(),
                    &capture_root().join("odd.omg"),
                    &filesystem,
                    Vec::new()
                )
                .unwrap(),
            ["another.omg", "odd.omg"].map(|path| capture_root().join(path))
        );
        assert!(
            discovery
                .select(
                    &capture_root(),
                    &["api".into(), "Export".into()],
                    &filesystem
                )
                .unwrap()
                .contains(&capture_root().join("renamed.omg"))
        );
    }

    // Discovery's parse of each eligible unit stays with its owner's
    // inventory, so admission can adopt it by path. Build files and paths
    // outside the inventory have none and are parsed by admission itself.
    #[test]
    fn discovery_retains_each_unit_parse_for_admission() {
        let filesystem = captured(&[
            ("odd.omg", "public data Record {}"),
            ("renamed.omg", "module api; public data Export {}"),
            ("build.omg", "not product source syntax"),
        ]);
        let mut discovery = Vec::<RegisteredPackageRoot>::new();
        discovery
            .select(&capture_root(), &["api".into()], &filesystem)
            .unwrap();
        let retained = retained_unit(&discovery, &capture_root().join("renamed.omg"))
            .expect("discovery retained the unit it parsed");
        assert_eq!(&*retained.source, "module api; public data Export {}");
        assert_eq!(retained.root_items().len(), 2);
        assert!(retained_unit(&discovery, &capture_root().join("odd.omg")).is_some());
        assert!(retained_unit(&discovery, &capture_root().join("build.omg")).is_none());
        assert!(retained_unit(&discovery, Path::new("/elsewhere/odd.omg")).is_none());
    }

    /// What a Build source's import acquires in its own package. The first
    /// two paths name a dependency, which this package declares nothing of:
    /// they select no unit, so the import can wait for that dependency. The
    /// rest name a declaration or a module of the package and select the
    /// module that holds it, never the unrelated module-less units.
    #[test]
    fn a_build_import_selects_only_the_module_that_declares_its_name() {
        let filesystem = captured(&[
            ("main.omg", "public machine entry() -> u8 { 7 }"),
            ("other.omg", "machine helper() -> u8 { 7 }"),
            (
                "support.omg",
                "module build_support; public machine generated() -> u8 { 7 }",
            ),
            (
                "tables.omg",
                "module build_support::tables; public data Table {}",
            ),
        ]);
        let root = &capture_root();
        let mut owners = Vec::<RegisteredPackageRoot>::new();
        let mut select = |path: &[&str]| {
            let path = path
                .iter()
                .map(|name| (*name).to_owned())
                .collect::<Vec<_>>();
            owners
                .select_declaring(root, &path, &filesystem, Vec::new())
                .unwrap()
        };
        assert!(select(&["kit"]).is_empty());
        assert!(select(&["kit", "v"]).is_empty());
        assert_eq!(
            select(&["helper"]),
            ["main.omg", "other.omg"].map(|path| root.join(path)),
            "one module-less unit declares the name, so its whole module is selected"
        );
        assert_eq!(
            select(&["build_support"]),
            ["support.omg", "tables.omg"].map(|path| root.join(path))
        );
        assert_eq!(
            select(&["build_support", "generated"]),
            [root.join("support.omg")]
        );
        // The twin: the acquisition a Product source gets takes every
        // enclosing namespace, the module-less units included.
        assert_eq!(
            owners.select(root, &["kit".into()], &filesystem).unwrap(),
            ["main.omg", "other.omg"].map(|path| root.join(path))
        );
    }

    #[test]
    fn root_import_acquires_descendants_for_qualified_namespace_lookup() {
        let filesystem = captured(&[
            ("root.omg", "public data Root {}"),
            ("child.omg", "module child; public data Child {}"),
            ("deep.omg", "module child::deep; public data Deep {}"),
        ]);
        let mut owners = Vec::<RegisteredPackageRoot>::new();
        assert_eq!(
            owners.select(&capture_root(), &[], &filesystem).unwrap(),
            ["child.omg", "deep.omg", "root.omg"].map(|path| capture_root().join(path))
        );
        assert_eq!(
            owners
                .select(&capture_root(), &["child".into()], &filesystem)
                .unwrap(),
            ["child.omg", "deep.omg", "root.omg"].map(|path| capture_root().join(path))
        );
    }

    #[test]
    fn product_declaration_inventory_includes_unimported_operator_homes() {
        let filesystem = captured(&[
            ("root.omg", "machine main() {}"),
            ("units.omg", "module units; public domain u32::Distance;"),
            (
                "operators.omg",
                "module operations; use units; public machine + Distance::add(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { left }",
            ),
            ("ordinary.omg", "module unrelated; machine helper() {}"),
            (
                "child/foreign.omg",
                "module foreign; public machine + Unknown::add(value: u32) {}",
            ),
            ("build.omg", "not Product syntax"),
        ]);
        let root = capture_root();
        let mut owners = Vec::<RegisteredPackageRoot>::new();
        assert_eq!(
            owners
                .siblings(
                    &root,
                    &root.join("units.omg"),
                    &filesystem,
                    vec![root.join("child")]
                )
                .unwrap(),
            ["operators.omg", "units.omg"].map(|path| root.join(path)),
            "family declarations must enter normal Product admission even without a namespace import",
        );
        assert_eq!(owners.len(), 1, "reuse the actual owner's cached inventory");
    }

    #[test]
    fn named_parent_namespace_can_be_introduced_only_by_a_descendant_unit() {
        let filesystem = captured(&[("renamed.omg", "module outer::inner; public data Item {}")]);
        let mut owners = Vec::<RegisteredPackageRoot>::new();
        let root = &capture_root();
        assert_eq!(
            owners.select(root, &[], &filesystem).unwrap(),
            [root.join("renamed.omg")]
        );
        assert_eq!(
            owners.select(root, &["outer".into()], &filesystem).unwrap(),
            [root.join("renamed.omg")]
        );
    }

    #[test]
    fn owner_inventory_refreshes_when_capture_or_exclusions_change() {
        let first = captured(&[("one.omg", "public data One {}")]);
        let second = captured(&[
            ("one.omg", "module hidden; public data One {}"),
            ("two.omg", "public data Two {}"),
            ("child/three.omg", "public data Three {}"),
        ]);
        let root = &capture_root();
        let mut owners = Vec::<RegisteredPackageRoot>::new();
        assert_eq!(
            owners.select(root, &[], &first).unwrap(),
            [root.join("one.omg")]
        );
        assert_eq!(
            owners.select(root, &["Two".into()], &second).unwrap(),
            [root.join("child/three.omg"), root.join("two.omg")]
        );
        assert_eq!(
            owners
                .select_owned(root, &["Two".into()], &second, vec![root.join("child")])
                .unwrap(),
            [root.join("two.omg")]
        );
        assert_eq!(
            owners.select(root, &["Two".into()], &second).unwrap(),
            [root.join("child/three.omg"), root.join("two.omg")]
        );
        assert_eq!(
            owners.len(),
            1,
            "capture refresh retains its existing owner row"
        );
    }

    #[test]
    fn owned_inventory_does_not_parse_another_packages_source() {
        let filesystem = captured(&[
            ("unit.omg", "public data Record {}"),
            ("child/not-ours.omg", "deliberately invalid foreign source"),
        ]);
        let paths = Vec::<RegisteredPackageRoot>::new()
            .select_owned(
                &capture_root(),
                &["Record".into()],
                &filesystem,
                vec![capture_root().join("child")],
            )
            .unwrap();
        assert_eq!(paths, [capture_root().join("unit.omg")]);
    }

    #[test]
    fn discovery_coordinates_do_not_escape_into_checked_source_ids() {
        let filesystem = captured(&[("bad.omg", "machine")]);
        let errors = Vec::<RegisteredPackageRoot>::new()
            .select(&capture_root(), &[], &filesystem)
            .unwrap_err();
        assert!(errors.iter().all(|error| error.source_span.is_none()));
        assert!(errors.iter().any(|error| {
            error
                .message
                .starts_with(&format!("{}:1:", capture_root().join("bad.omg").display()))
        }));
    }

    #[test]
    fn missing_capture_cannot_fall_back_to_live_directory_membership() {
        let error = SourceFileSystem::default()
            .captured_files(&capture_root())
            .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }
}
