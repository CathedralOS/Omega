//! The normalized identity carriers: type identities, domain expressions,
//! result dispatch sets and domain terms.
//!
//! A named callable's identity is not among them: it is interned once per
//! program revision in `callable_identity.rs` and named by a handle, so no
//! carrier holds its path and parameter signature as rendered text.

/// Handle into the program-owned [`TypeIdentityTable`]. Zero is the absence
/// state; no consumer wraps this handle in an `Option`. Equality and hashing
/// are handle comparisons inside one program's table — two tables each mint
/// their own handle space, so joins across programs rehydrate through text.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
)]
pub struct TypeIdentityId(u32);

impl TypeIdentityId {
    /// Whether this handle names a slot. The zero handle does not.
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }

    /// Coordinate for invocation-owned scratch indexed by this program's
    /// append-only identity table. It carries no identity across programs.
    pub fn storage_index(self) -> Option<usize> {
        self.slot()
    }

    fn slot(self) -> Option<usize> {
        (self.0 != 0).then(|| self.0 as usize - 1)
    }

    fn from_slot(slot: usize) -> Self {
        Self(slot as u32 + 1)
    }
}

/// The identity type Psi 05, 06 and checking previously compared as
/// rendered text. It is the interned handle now: comparisons are integer
/// equality and rendering is a program query, not a carrier payload.
pub type NormalizedTypeIdentity = TypeIdentityId;

/// One program's hash-consed structural type keys. Records are append-only
/// within a revision, so a handle stays valid for as long as the table it
/// came from; the rendered text is the structural key, so the table needs no
/// revision guard of its own.
///
/// Parallel checking workers intern through `&self`. Finding an existing key
/// takes no lock, and only a key minted for the first time waits on other
/// mints; see `arena::OwnedTextInterner`. Handles are numbered in first-intern
/// order and compare only within this table.
#[derive(Clone, Default, PartialEq, Eq, Debug, compiler_storage::Record)]
pub struct TypeIdentityTable {
    texts: arena::OwnedTextInterner,
}

impl TypeIdentityTable {
    /// The canonical handle for one rendered structural key, minting a slot
    /// the first time that key appears.
    pub fn intern(&self, text: &str) -> TypeIdentityId {
        TypeIdentityId::from_slot(self.texts.intern(text))
    }

    /// The retained rendering of one handle: diagnostics and artifact
    /// encoding only.
    pub fn text(&self, identity: TypeIdentityId) -> Option<&str> {
        self.texts.text(identity.slot()?)
    }

    /// The handle a previously interned key names: wire decoders and boundary
    /// readers rehydrate their `String` payload through this reverse join.
    pub fn by_text(&self, text: &str) -> TypeIdentityId {
        self.texts
            .slot(text)
            .map(TypeIdentityId::from_slot)
            .unwrap_or_default()
    }
}

/// The canonical set of domains that may select a named-machine or
/// requirement overload from its expected result type.
///
/// This set expands transparent domain aliases and omits predicate-only
/// refinements. Arithmetic policies, domains with a semantic role or
/// establishment route, and explicit empty tags remain dispatch-bearing. Terms
/// are sorted and deduplicated so authored conjunction order cannot affect
/// overload identity.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, compiler_storage::Record)]
pub struct NormalizedResultDispatchSet {
    pub(crate) terms: Vec<NormalizedDomainTerm>,
}

impl NormalizedResultDispatchSet {
    pub fn terms(&self) -> &[NormalizedDomainTerm] {
        &self.terms
    }

    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn identity(&self) -> String {
        self.terms
            .iter()
            .map(|term| match term {
                NormalizedDomainTerm::Arithmetic(name) => format!("arithmetic:{name}"),
                NormalizedDomainTerm::Compiler(identity) => format!("compiler:{identity}"),
                NormalizedDomainTerm::Declared(name) => format!("declared:{name}"),
            })
            .collect::<Vec<_>>()
            .join("&")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
pub enum NormalizedDomainTerm {
    Arithmetic(String),
    Compiler(String),
    Declared(String),
}
