/*
    One symbol: the durable handle and span, with the name, path and kind
    metadata that lookup and display carry around it.
*/

pub(crate) mod kind {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
    pub enum SymbolKind {
        #[default]
        Unknown,
        Root,
        Module,
        BuiltinType,
        BuiltinFunction,
        Data,
        Domain,
        Field,
        Variant,
        Machine,
        Operator,
        /* A package-private ranking declaration owning its measured parameter. */
        Measure,
        Proposition,
        /*
            A top-level mathematical `let`/`boundary let` declaration: a proof-side
            term definition or named assumption with a dependent result type
            (PROOF-CONTRACT-MIGRATION). Deliberately distinct from `Proposition`,
            which is the retired dedicated-formula surface this kind replaces.
        */
        MathematicalDefinition,
        State,
        Trait,
        Conformance,
        /*
            A compile-time value declaration retained only as authored-selection
            provenance after its value has been substituted.
        */
        Const,
        /*
            A proof-static machine-telescope binder whose concrete argument is one
            exact package-scoped conformance.
        */
        ConformanceParameter,
        WireSchema,
        Parameter,
        TypeParameter,
        /*
            A compile-time machine-symbol parameter. Unlike a type parameter this
            symbol is callable inside its generic machine through its authored
            signature contract.
        */
        MachineParameter,
        /* A structural runtime callable contract with its own formal namespace. */
        CallableContract,
        /*
            A generic proof-formula family. It is applicable only in proof-fact
            position and owns the value-parameter symbols of its authored
            proposition signature.
        */
        PropositionParameter,
        /*
            An opaque proof-static machine identity used as a proposition-family
            index. It is deliberately not callable and owns no signature children.
        */
        PropositionMachineParameter,
        Local,
        HostCapability,
        Object,
        Function,
        Section,
        Import,
        /*
            A transparent source type declaration. Its selected header and
            telescope survive normalization; it never owns runtime data storage.
        */
        TypeAlias,
    }
}
pub(crate) mod name {
    use std::borrow::Cow;
    use std::sync::Arc;

    use source_text::SourceSpan;

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct SymbolName {
        storage: SymbolNameStorage,
    }

    #[derive(Clone, Default, record_storage::Record)]
    enum SymbolNameStorage {
        #[default]
        Missing,
        Source(SourceSpan),
        SourceSlice {
            source: Arc<str>,
            source_span: SourceSpan,
        },
        OwnedSource {
            value: Arc<str>,
            source_span: SourceSpan,
        },
        Static(Cow<'static, str>),
        Owned(Arc<str>),
    }

    impl SymbolNameStorage {
        fn retained_spelling(&self) -> Option<(&str, SourceSpan)> {
            match self {
                Self::SourceSlice {
                    source,
                    source_span,
                } => Some((
                    &source[source_span.span.start..source_span.span.end],
                    *source_span,
                )),
                Self::OwnedSource { value, source_span } => Some((value, *source_span)),
                _ => None,
            }
        }
    }

    impl PartialEq for SymbolNameStorage {
        fn eq(&self, other: &Self) -> bool {
            if let (Some(left), Some(right)) = (self.retained_spelling(), other.retained_spelling())
            {
                return left == right;
            }
            match (self, other) {
                (Self::Missing, Self::Missing) => true,
                (Self::Source(left), Self::Source(right)) => left == right,
                (Self::Static(left), Self::Static(right)) => left == right,
                (Self::Owned(left), Self::Owned(right)) => left == right,
                _ => false,
            }
        }
    }

    impl Eq for SymbolNameStorage {}

    impl std::fmt::Debug for SymbolNameStorage {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Missing => formatter.write_str("Missing"),
                Self::Source(span) => formatter.debug_tuple("Source").field(span).finish(),
                Self::Static(value) => formatter.debug_tuple("Static").field(value).finish(),
                Self::Owned(value) => formatter.debug_tuple("Owned").field(value).finish(),
                Self::SourceSlice {
                    source,
                    source_span,
                } => formatter
                    .debug_struct("OwnedSource")
                    .field(
                        "value",
                        &&source[source_span.span.start..source_span.span.end],
                    )
                    .field("source_span", source_span)
                    .finish(),
                Self::OwnedSource { value, source_span } => formatter
                    .debug_struct("OwnedSource")
                    .field("value", value)
                    .field("source_span", source_span)
                    .finish(),
            }
        }
    }

    impl SymbolName {
        pub fn from_ref(name: SymbolNameRef<'_>) -> Self {
            Self {
                storage: match name {
                    SymbolNameRef::Borrowed(value) => SymbolNameStorage::Owned(Arc::from(value)),
                    SymbolNameRef::Source(source_span) => SymbolNameStorage::Source(source_span),
                    SymbolNameRef::OwnedSource { value, source_span } => {
                        SymbolNameStorage::OwnedSource {
                            value: Arc::from(value),
                            source_span,
                        }
                    }
                    SymbolNameRef::Static(value) => SymbolNameStorage::Static(Cow::Borrowed(value)),
                },
            }
        }

        /// Share the exact authored byte slice when it matches this spelling.
        pub fn from_ref_with_text(name: SymbolNameRef<'_>, source: Option<&Arc<str>>) -> Self {
            if let SymbolNameRef::OwnedSource { value, source_span } = name
                && let Some(source) = source
                && source.get(source_span.span.start..source_span.span.end) == Some(value)
            {
                return Self {
                    storage: SymbolNameStorage::SourceSlice {
                        source: source.clone(),
                        source_span,
                    },
                };
            }
            Self::from_ref(name)
        }

        /// An unresolved source coordinate has no spelling in this record.
        pub fn spelling(&self) -> Option<&str> {
            Some(match &self.storage {
                SymbolNameStorage::Missing => "",
                SymbolNameStorage::Source(_) => return None,
                SymbolNameStorage::SourceSlice {
                    source,
                    source_span,
                } => &source[source_span.span.start..source_span.span.end],
                SymbolNameStorage::OwnedSource { value, .. } => value.as_ref(),
                SymbolNameStorage::Static(value) => value,
                SymbolNameStorage::Owned(value) => value.as_ref(),
            })
        }

        pub fn source_span(&self) -> Option<SourceSpan> {
            match self.storage {
                SymbolNameStorage::Source(source_span)
                | SymbolNameStorage::SourceSlice { source_span, .. } => Some(source_span),
                SymbolNameStorage::OwnedSource { source_span, .. } => Some(source_span),
                SymbolNameStorage::Missing
                | SymbolNameStorage::Static(_)
                | SymbolNameStorage::Owned(_) => None,
            }
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum SymbolNameRef<'name> {
        Borrowed(&'name str),
        Source(SourceSpan),
        /* Owned semantic spelling attached to one exact authored declaration span. */
        OwnedSource {
            value: &'name str,
            source_span: SourceSpan,
        },
        Static(&'static str),
    }

    impl<'name> SymbolNameRef<'name> {
        pub fn as_str(self) -> &'name str {
            match self {
                Self::Borrowed(value) => value,
                Self::Source(_) => "",
                Self::OwnedSource { value, .. } => value,
                Self::Static(value) => value,
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::{SourceSpan, SymbolName, SymbolNameRef};
        use source_text::{SourceId, Span};
        use std::sync::Arc;

        #[test]
        fn retained_names_compare_and_format_only_the_selected_spelling() {
            let source_span = SourceSpan::new(SourceId(0), Span::new(0, 5));
            let seed = SymbolNameRef::OwnedSource {
                value: "Ready",
                source_span,
            };
            let owned = SymbolName::from_ref(seed);
            for text in ["Ready first", "Ready other"] {
                let text: Arc<str> = Arc::from(text);
                let borrowed = SymbolName::from_ref_with_text(seed, Some(&text));
                assert_eq!(borrowed, owned);
                assert_eq!(format!("{borrowed:?}"), format!("{owned:?}"));
            }
        }
    }
}
pub(crate) mod path {
    use crate::{SymbolHandle, SymbolSpan};

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub struct SymbolPath {
        pub root: SymbolHandle,
        pub members: SymbolSpan,
    }
}

use arena::{Handle, HandleSpan, HierarchyNode};

use super::{SymbolKind, SymbolName};

pub type SymbolHandle = Handle<Symbol>;
pub type SymbolSpan = HandleSpan<SymbolHandle>;
pub type SymbolNameHandle = Handle<SymbolName>;

#[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
pub struct Symbol {
    pub parent: SymbolHandle,
    /*
        Authored declaration scope when physical allocation uses another parent.
        Inline implementation members retain their map here without changing
        namespace paths, runtime arguments or source-selection authority.
    */
    pub declaration_owner: SymbolHandle,
    pub children: HandleSpan<Symbol>,
    pub kind: SymbolKind,
    pub name: SymbolNameHandle,
    /*
        Exact earlier symbol whose declaration provenance this compiler-created
        symbol derives from. Authored symbols use the invalid handle.
    */
    pub generated_from: SymbolHandle,
}

impl HierarchyNode for Symbol {
    fn parent(&self) -> Handle<Self> {
        self.parent
    }

    fn set_parent(&mut self, parent: Handle<Self>) {
        self.parent = parent;
    }

    fn children(&self) -> HandleSpan<Self> {
        self.children
    }

    fn set_children(&mut self, children: HandleSpan<Self>) {
        self.children = children;
    }
}
