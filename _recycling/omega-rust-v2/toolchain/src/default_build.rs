//! Default source Build for a complete, explicitly selected anonymous package.
//! The versioned owner record uses Build's anonymous source identity encoding. It has a
//! fixed, empty registration list only when the supplied default Build returns
//! without work. Authored entries require the package-aware Build entrance.
mod capture;

use crate::core_interface::PRODUCT_CORE;
use capture::{Node, Snapshot};
use sha2::{Digest, Sha256};
use std::{
    io,
    path::Path,
    sync::{Arc, OnceLock},
};

pub const DEFAULT_ROOT_PACKAGE: &str = "root";
const DEFAULT_ENTRY: (&str, &str) = ("defaults/build.omg", include_str!("../core/default_build.omg"));
const DEFAULT_SEMANTICS: &[u8] = b"OMEGA-LANGUAGE-DEFAULT-BUILD-SEMANTICS-V1\0";

/// Files retained before Build execution starts. Reading a later stage from
/// this capture preserves the original input even if the filesystem changes.
pub struct PackageSources {
    snapshot: Snapshot,
}

impl PackageSources {
    pub fn capture(root: &Path) -> io::Result<Self> {
        Ok(Self { snapshot: capture::local_package(root)? })
    }

    /// Some packages supply their own entry. Only an absent path selects
    /// the default. A directory or an unreadable link at build.omg is an error.
    pub fn authored_entry(&self) -> io::Result<Option<Arc<[u8]>>> {
        let entry = Path::new("build.omg");
        if self.snapshot.contains_key(entry) {
            self.source_bytes(entry).map(Some)
        } else {
            Ok(None)
        }
    }

    pub fn source_bytes(&self, logical_path: &Path) -> io::Result<Arc<[u8]>> {
        let resolved = capture::resolve(&self.snapshot, logical_path)?;
        match self.snapshot.get(&resolved) {
            Some(Node::File { bytes, .. }) => Ok(Arc::clone(bytes)),
            _ => Err(io::Error::other("expected a captured package file")),
        }
    }
}

pub struct DefaultBuild {
    sources: PackageSources,
    owner: [u8; 32],
}

impl DefaultBuild {
    pub fn capture(root: &Path) -> io::Result<Self> {
        Self::from_sources(PackageSources::capture(root)?)
    }

    /// Derive the default owner only after selecting the bundled empty entry.
    /// An authored entry can inspect its sources before any product owner exists.
    pub fn from_sources(sources: PackageSources) -> io::Result<Self> {
        if sources.authored_entry()?.is_some() {
            return Err(io::Error::other("authored build.omg requires package Build execution"));
        }
        let owner = anonymous_owner(&sources.snapshot, DEFAULT_SEMANTICS, &[default_core()?])?;
        Ok(Self { sources, owner })
    }

    pub fn package_key(&self) -> [u8; 32] {
        self.owner
    }

    pub fn source_bytes(&self, logical_path: &Path) -> io::Result<Arc<[u8]>> {
        self.sources.source_bytes(logical_path)
    }
}

fn default_core() -> io::Result<[u8; 32]> {
    static CORE: OnceLock<Result<[u8; 32], String>> = OnceLock::new();
    CORE.get_or_init(|| {
        // The trusted default entry binds only the Build activation parameter.
        // Its empty body returns unit, with zero registrations to join. Never
        // apply that interpretation to an authored or extended program.
        check_empty_default(DEFAULT_ENTRY.1)?;
        let files = PRODUCT_CORE.into_iter().chain([DEFAULT_ENTRY]);
        core_snapshot(files).map_err(|e| e.to_string())
    })
    .clone()
    .map_err(io::Error::other)
}

fn check_empty_default(text: &str) -> Result<(), String> {
    let tokens = omega_lexer::lex(text.as_bytes()).map_err(|e| format!("bundled default Build: {e:?}"))?;
    let tree = omega_parser::parse_file(text.as_bytes(), &tokens).map_err(|e| format!("bundled default Build: {e:?}"))?;
    use omega_parser::NodeKind::*;
    let syntax = tree
        .index(text, &tokens)
        .ok_or("invalid bundled default Build syntax tree")?;
    let nodes = &syntax.nodes;
    let [entry] = nodes[0].children.as_slice() else {
        return Err("default Build must supply exactly one free entry".into());
    };
    let machine = &nodes[*entry];
    let [name, params, body] = machine.children.as_slice() else {
        return Err("unsupported default Build signature".into());
    };
    if machine.kind != Machine
        || !machine.leaves.iter().map(|leaf| leaf.text).eq(["machine"])
        || nodes[*name].kind != Path
        || nodes[*name].path() != "build"
        || nodes[*params].kind != Params
    {
        return Err("default Build must supply the free build entry".into());
    }
    let [param] = nodes[*params].children.as_slice() else {
        return Err("default Build requires one activation parameter".into());
    };
    let [reference] = nodes[*param].children.as_slice() else {
        return Err("default Build requires a mutable Build parameter".into());
    };
    let reference = &nodes[*reference];
    let [activation] = reference.children.as_slice() else {
        return Err("default Build requires a mutable Build parameter".into());
    };
    if nodes[*param].kind != Param
        || reference.kind != ReferenceType
        || !reference.leaves.iter().map(|leaf| leaf.text).eq(["@", "mutable"])
        || nodes[*activation].kind != PathType
        || !nodes[*activation].children.is_empty()
        || !nodes[*activation]
            .leaves
            .iter()
            .map(|leaf| leaf.text)
            .eq(["Build"])
    {
        return Err("default Build requires a mutable Build parameter".into());
    }
    if nodes[*body].kind != Body || !nodes[*body].children.is_empty() {
        return Err("bundled default Build requires unsupported activation work".into());
    }
    Ok(())
}

fn field(
    digest: &mut Sha256,
    bytes: &[u8],
) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

fn core_snapshot<'a>(files: impl IntoIterator<Item = (&'a str, &'a str)>) -> io::Result<[u8; 32]> {
    let mut snapshot = Snapshot::new();
    for (path, text) in files {
        let path = Path::new(path);
        for directory in path
            .ancestors()
            .skip(1)
            .take_while(|p| !p.as_os_str().is_empty())
        {
            snapshot.insert(directory.to_owned(), Node::Directory);
        }
        snapshot.insert(
            path.to_owned(),
            Node::File {
                bytes: Arc::from(text.as_bytes()),
                executable: false,
            },
        );
    }
    let mut digest = Sha256::new();
    field(&mut digest, b"OMEGA-ANONYMOUS-CORE-SNAPSHOT-V1\0");
    hash_snapshot(&mut digest, &snapshot)?;
    Ok(digest.finalize().into())
}

fn anonymous_owner(
    snapshot: &Snapshot,
    semantics: &[u8],
    semantic_dependencies: &[[u8; 32]],
) -> io::Result<[u8; 32]> {
    let mut digest = Sha256::new();
    field(&mut digest, b"OMEGA-ANONYMOUS-CONTENT-OWNER-V1\0");
    field(&mut digest, semantics);
    hash_snapshot(&mut digest, snapshot)?;
    digest.update(0u64.to_le_bytes()); // The default Build registered no dependency requests.
    let mut dependencies = semantic_dependencies.to_vec();
    dependencies.sort_unstable();
    dependencies.dedup();
    digest.update((dependencies.len() as u64).to_le_bytes());
    for dependency in dependencies {
        field(&mut digest, &dependency);
    }
    let key = digest.finalize().into();
    if key == [0; 32] {
        return Err(io::Error::other("anonymous owner has the reserved zero digest"));
    }
    Ok(key)
}

fn os_bytes(value: &std::ffi::OsStr) -> io::Result<&[u8]> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        Ok(value.as_bytes())
    }
    #[cfg(not(unix))]
    value
        .to_str()
        .map(str::as_bytes)
        .ok_or_else(|| io::Error::other("package input path is not portable UTF-8"))
}

fn hash_snapshot(
    digest: &mut Sha256,
    snapshot: &Snapshot,
) -> io::Result<()> {
    let mut rows = Vec::with_capacity(snapshot.len());
    for (path, node) in snapshot {
        let mut bytes = Vec::new();
        for part in path.components() {
            if !bytes.is_empty() {
                bytes.push(b'/');
            }
            bytes.extend_from_slice(os_bytes(part.as_os_str())?);
        }
        rows.push((bytes, node));
    }
    rows.sort_unstable_by(|left, right| left.0.cmp(&right.0));
    digest.update((rows.len() as u64).to_le_bytes());
    for (path, node) in rows {
        field(digest, &path);
        match node {
            Node::Directory => digest.update([0]),
            Node::File { bytes, executable } => {
                digest.update([1, u8::from(*executable)]);
                field(digest, bytes);
            }
            Node::Link { target } => {
                digest.update([2]);
                field(digest, os_bytes(target.as_os_str())?);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
