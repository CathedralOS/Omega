//! The first L2 operation binds direct calls to ordinary machines in one admitted package.
//! The resolver reads the parser's tree and uses lexer spans for each occurrence.

#![forbid(unsafe_code)]

mod directory_lookup;

use omega_lexer::{SourceSpan, Token};
use omega_parser::{NodeKind, SyntaxTree};

pub use directory_lookup::resolve_machine_calls;

/// A whole-file parse and its original UTF-8 source and tokens. The package loader
/// supplies a canonical, slash-separated logical path relative to the package root.
pub struct ParsedSource<'a> {
    pub logical_path: &'a str,
    pub text: &'a str,
    pub tokens: &'a [Token],
    pub tree: &'a SyntaxTree,
}

/// Identity for an unapplied machine with a unique name in its package directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MachineIdentity<'a> {
    pub package: &'a str,
    pub directory: &'a str,
    pub name: &'a str,
}

impl MachineIdentity<'_> {
    /// The package-local Omega spelling, including each directory component.
    pub fn qualified_name(&self) -> String {
        if self.directory.is_empty() {
            self.name.to_owned()
        } else {
            format!("{}::{}", self.directory.replace('/', "::"), self.name)
        }
    }
}

/// A binding at the caller's half-open byte span.
#[derive(Debug, PartialEq, Eq)]
pub struct ResolvedCall<'a> {
    pub logical_path: &'a str,
    pub span: SourceSpan,
    pub target: MachineIdentity<'a>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResolutionError<'a> {
    /// The caller has no eligible machine in its directory.
    Unresolved { logical_path: &'a str, span: SourceSpan, name: &'a str },
    /// Several declarations share this name. Selecting an overload needs a later contract.
    CompetingMachines { logical_path: &'a str, span: SourceSpan, name: &'a str },
    /// The source needs a resolution rule this operation does not implement yet.
    Unsupported {
        logical_path: &'a str,
        span: SourceSpan,
        syntax: NodeKind,
    },
    /// The supplied tree, token indices or spans cannot be read against this source.
    InconsistentParse { logical_path: &'a str },
    /// Two inputs claim the same logical file in one package.
    DuplicateFile { logical_path: &'a str },
}
