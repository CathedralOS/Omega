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
use source_provenance::DependencyScope;
use diagnostics::Diagnostic;
use source_admission::LoadedSource;
use source_capture::SourceFileSystem;
use source_compiler::RetainedSourceParse;
use source_compiler::{SourceInput, CompileTimings};
use source_text::SourceId;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq, Eq, record_storage::Record)]
pub(crate) struct LogicalSourceInventory {
    filesystem: SourceFileSystem,
    excluded_paths: Vec<PathBuf>,
    units: Vec<LogicalSourceUnit>,
    /// Positions in `units`, ordered by path, for admission's lookup of a
    /// retained parse. `units` keeps capture order, which selection reports.
    by_path: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, record_storage::Record)]
struct LogicalSourceUnit {
    path: PathBuf,
    module: Vec<String>,
    declares_operator_family: bool,
    /// The first name of every top-level declaration, in authored order.
    declared: Vec<String>,
    /// The unit's parse under discovery's private coordinates.
    syntax: Arc<RetainedSourceParse>,
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
        excluded_paths: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>>;
    /// Files selected by a legacy namespace import in Build. A missing
    /// declaration remains deferred until the Build prepares its dependency.
    fn select_declaring(
        &mut self,
        root: &Path,
        logical_path: &[String],
        filesystem: &SourceFileSystem,
        excluded_paths: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>>;
    fn operator_family_sources(
        &mut self,
        root: &Path,
        source: &Path,
        filesystem: &SourceFileSystem,
        excluded_paths: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>>;
}

impl LogicalSourceDiscovery for Vec<RegisteredPackageRoot> {
    fn select_owned(
        &mut self,
        root: &Path,
        logical_path: &[String],
        filesystem: &SourceFileSystem,
        excluded_paths: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let inventory = owner_inventory(self, root, filesystem, excluded_paths)?;
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
        excluded_paths: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        let inventory = owner_inventory(self, root, filesystem, excluded_paths)?;
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
    fn operator_family_sources(
        &mut self,
        root: &Path,
        source: &Path,
        filesystem: &SourceFileSystem,
        excluded_paths: Vec<PathBuf>,
    ) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
        // A Product may explicitly select one of its own Build files. It must
        // acquire the same operator homes as any other admitted Product unit.
        let selected_build = self
            .iter()
            .any(|owner| owner.root == root && owner.build_files.contains(source));
        let inventory = owner_inventory(self, root, filesystem, excluded_paths)?;
        if !selected_build && !inventory.units.iter().any(|unit| unit.path == source) {
            return Ok(Vec::new());
        }
        Ok(inventory
            .units
            .iter()
            // Closed families are declared across their owning package, even
            // when their lexical module is not imported. Acquire those units
            // through the same Product queue; this grants no namespace exposure
            // or authority and leaves exact home/duplicate checks to resolution.
            .filter(|unit| unit.declares_operator_family)
            .map(|unit| unit.path.clone())
            .collect())
    }
}

fn owner_inventory<'a>(
    owners: &'a mut Vec<RegisteredPackageRoot>,
    root: &Path,
    filesystem: &SourceFileSystem,
    mut excluded_paths: Vec<PathBuf>,
) -> Result<&'a LogicalSourceInventory, Vec<Diagnostic>> {
    let owner = RegisteredPackageRoot::for_root(owners, root);
    excluded_paths.extend(owner.build_files.iter().cloned());
    excluded_paths.sort();
    excluded_paths.dedup();
    if owner.logical_inventory.as_ref().is_none_or(|inventory| {
        inventory.filesystem != *filesystem || inventory.excluded_paths != excluded_paths
    }) {
        owner.logical_inventory = Some(LogicalSourceInventory::parse(
            root,
            filesystem,
            &excluded_paths,
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
        excluded_paths: &[PathBuf],
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
                ) && !excluded_paths
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
            .into_parsed_units(&mut CompileTimings::default())?;
        let mut result = Self {
            filesystem: filesystem.clone(),
            excluded_paths: excluded_paths.to_vec(),
            units: Vec::with_capacity(parsed.len()),
            by_path: Vec::new(),
        };
        for (path, unit) in paths.into_iter().zip(parsed) {
            let source_compiler::SourceDeclarations {
                declares_operator_family,
                declared,
            } = unit.declarations();
            let module = source_admission::directory_namespace(&path, root)
                .map_err(|message| vec![Diagnostic::error(message)])?
                .into_iter().map(str::to_owned).collect();
            result.units.push(LogicalSourceUnit {
                path,
                module,
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

    fn retained(&self, path: &Path) -> Option<&RetainedSourceParse> {
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
) -> Option<&'a RetainedSourceParse> {
    owners
        .iter()
        .filter(|owner| path.starts_with(&owner.root))
        .filter_map(|owner| owner.logical_inventory.as_ref())
        .find_map(|inventory| inventory.retained(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_capture::{CapturedSourceNode, CapturedSourceTree};
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

    fn owner_with_build_file(file: &str) -> Vec<RegisteredPackageRoot> {
        let root = capture_root();
        let mut owners = Vec::new();
        RegisteredPackageRoot::for_root(&mut owners, &root)
            .build_files
            .insert(root.join(file));
        owners
    }

    #[test]
    fn logical_membership_comes_from_directories_not_filenames() {
        let filesystem = captured(&[
            ("odd.omg", "public data Record {}"),
            ("another.omg", "public machine read() -> u8 { 7 }"),
            ("unrelated/Record.omg", "public data Hidden {}"),
            ("api/renamed.omg", "public data Export {}"),
            ("prepare.omg", "not product source syntax"),
        ]);
        let mut discovery = owner_with_build_file("prepare.omg");
        assert_eq!(
            discovery
                .select(&capture_root(), &["Record".into()], &filesystem)
                .unwrap(),
            ["another.omg", "odd.omg"].map(|path| capture_root().join(path))
        );
        assert!(
            discovery
                .operator_family_sources(
                    &capture_root(),
                    &capture_root().join("odd.omg"),
                    &filesystem,
                    vec![],
                )
                .unwrap()
                .is_empty()
        );
        assert!(
            discovery
                .select(
                    &capture_root(),
                    &["api".into(), "Export".into()],
                    &filesystem
                )
                .unwrap()
                .contains(&capture_root().join("api/renamed.omg"))
        );
    }

    // Discovery's parse of each eligible unit stays with its owner's
    // inventory, so admission can adopt it by path. Build files and paths
    // outside the inventory have none and are parsed by admission itself.
    #[test]
    fn discovery_retains_each_unit_parse_for_admission() {
        let filesystem = captured(&[
            ("odd.omg", "public data Record {}"),
            ("api/renamed.omg", "public data Export {}"),
            ("prepare.omg", "not product source syntax"),
        ]);
        let mut discovery = owner_with_build_file("prepare.omg");
        discovery
            .select(&capture_root(), &["api".into()], &filesystem)
            .unwrap();
        let retained = retained_unit(&discovery, &capture_root().join("api/renamed.omg"))
            .expect("discovery retained the unit it parsed");
        let declarations = retained.declarations();
        assert_eq!(declarations.declared, ["Export"]);
        assert!(!declarations.declares_operator_family);
        assert!(retained_unit(&discovery, &capture_root().join("odd.omg")).is_some());
        assert!(retained_unit(&discovery, &capture_root().join("prepare.omg")).is_none());
        assert!(retained_unit(&discovery, Path::new("/elsewhere/odd.omg")).is_none());
    }

    /// A Product declaration query selects the unit that declares the consumed
    /// name. An unknown selection leaves discovery empty for the query to diagnose.
    #[test]
    fn a_product_query_selects_only_the_module_that_declares_its_name() {
        let filesystem = captured(&[
            ("main.omg", "public machine entry() -> u8 { 7 }"),
            ("other.omg", "machine helper() -> u8 { 7 }"),
            (
                "build_support/support.omg",
                "public machine generated() -> u8 { 7 }",
            ),
            (
                "build_support/tables/tables.omg",
                "public data Table {}",
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
            ["build_support/support.omg", "build_support/tables/tables.omg"].map(|path| root.join(path))
        );
        assert_eq!(
            select(&["build_support", "generated"]),
            [root.join("build_support/support.omg")]
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
            ("child/child.omg", "public data Child {}"),
            ("child/deep/deep.omg", "public data Deep {}"),
        ]);
        let mut owners = Vec::<RegisteredPackageRoot>::new();
        assert_eq!(
            owners.select(&capture_root(), &[], &filesystem).unwrap(),
            ["child/child.omg", "child/deep/deep.omg", "root.omg"].map(|path| capture_root().join(path))
        );
        assert_eq!(
            owners
                .select(&capture_root(), &["child".into()], &filesystem)
                .unwrap(),
            ["child/child.omg", "child/deep/deep.omg", "root.omg"].map(|path| capture_root().join(path))
        );
    }

    #[test]
    fn product_declaration_inventory_includes_unimported_operator_homes() {
        let filesystem = captured(&[
            ("root.omg", "machine main() {}"),
            ("units/units.omg", "public domain u32::Distance;"),
            (
                "operations/operators.omg",
                "use units; public machine + Distance::add(left: u32 in Distance, right: u32 in Distance) -> u32 in Distance { left }",
            ),
            ("unrelated/ordinary.omg", "machine helper() {}"),
            (
                "child/foreign/foreign.omg",
                "public machine + Unknown::add(value: u32) {}",
            ),
            ("prepare.omg", "not Product syntax"),
        ]);
        let root = capture_root();
        let mut owners = owner_with_build_file("prepare.omg");
        for selected in ["units/units.omg", "prepare.omg"] {
            assert_eq!(
                owners
                    .operator_family_sources(
                        &root,
                        &root.join(selected),
                        &filesystem,
                        vec![root.join("child")],
                    )
                    .unwrap(),
                ["operations/operators.omg"].map(|path| root.join(path)),
                "each admitted Product unit acquires its package's operator declarations",
            );
        }
        assert_eq!(owners.len(), 1, "reuse the actual owner's cached inventory");
    }

    #[test]
    fn named_parent_namespace_can_be_introduced_only_by_a_descendant_unit() {
        let filesystem = captured(&[("outer/inner/renamed.omg", "public data Item {}")]);
        let mut owners = Vec::<RegisteredPackageRoot>::new();
        let root = &capture_root();
        assert_eq!(
            owners.select(root, &[], &filesystem).unwrap(),
            [root.join("outer/inner/renamed.omg")]
        );
        assert_eq!(
            owners.select(root, &["outer".into()], &filesystem).unwrap(),
            [root.join("outer/inner/renamed.omg")]
        );
    }

    #[test]
    fn owner_inventory_refreshes_when_capture_or_exclusions_change() {
        let first = captured(&[("one.omg", "public data One {}")]);
        let second = captured(&[
            ("hidden/one.omg", "public data One {}"),
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
            [root.join("two.omg")]
        );
        assert_eq!(
            owners
                .select_owned(root, &["Two".into()], &second, vec![root.join("child")])
                .unwrap(),
            [root.join("two.omg")]
        );
        assert_eq!(
            owners.select(root, &["Two".into()], &second).unwrap(),
            [root.join("two.omg")]
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
