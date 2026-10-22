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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeIdentityId(u32);

impl TypeIdentityId {
    /// Whether this handle names a slot. The zero handle does not.
    pub fn is_valid(self) -> bool {
        self.0 != 0
    }

    fn slot(self) -> Option<usize> {
        (self.0 != 0).then(|| self.0 as usize - 1)
    }

    fn from_slot(slot: usize) -> Self {
        Self(slot as u32 + 1)
    }
}

/// The identity type stages 04, 05 and checking previously compared as
/// rendered text. It is the interned handle now: comparisons are integer
/// equality and rendering is a program query, not a carrier payload.
pub type NormalizedTypeIdentity = TypeIdentityId;

/// One program's hash-consed structural type keys. Records are append-only
/// within a revision, so a handle stays valid for as long as the table it
/// came from; the rendered text is the structural key, so the map needs no
/// revision guard of its own.
#[derive(Clone, Default)]
pub struct TypeIdentityTable {
    texts: Vec<Box<str>>,
    slots: std::collections::HashMap<Box<str>, TypeIdentityId, symbols::BuildSymbolHasher>,
}

impl TypeIdentityTable {
    /// The canonical handle for one rendered structural key, minting a slot
    /// the first time that key appears.
    pub fn intern(&mut self, text: &str) -> TypeIdentityId {
        super::intern_boxed_text(
            text,
            &mut self.texts,
            &mut self.slots,
            TypeIdentityId::from_slot,
        )
    }

    /// The retained rendering of one handle: diagnostics and artifact
    /// encoding only.
    pub fn text(&self, identity: TypeIdentityId) -> Option<&str> {
        identity
            .slot()
            .and_then(|slot| self.texts.get(slot).map(AsRef::as_ref))
    }

    /// The handle a previously interned key names: wire decoders and boundary
    /// readers rehydrate their `String` payload through this reverse join.
    pub fn by_text(&self, text: &str) -> TypeIdentityId {
        self.slots.get(text).copied().unwrap_or_default()
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum NormalizedDomainTerm {
    Arithmetic(String),
    Compiler(String),
    Declared(String),
}
