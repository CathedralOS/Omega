//! The closed vocabulary of canonical type identity text.
//!
//! `typed-trees` writes identities and `grammar`/`index` read them while package
//! owners are visited. The two halves used to agree only through string
//! literals, so a tag one side added or reshaped surfaced at the other only when
//! a package lock was built from a full compile. Every tag now has one variant
//! here. The renderer writes a tag only through these types, and each reading
//! position matches its enum exhaustively: a new variant does not compile until
//! the grammar reads it or refuses it with an [`Unadmitted`] reason.
//!
//! A position that dispatches on the next tag owns an enum ([`TypeForm`],
//! [`IndexForm`], ...). [`Label`] names the fixed children a production carries
//! at one place; the reader expects them instead of dispatching, so a new label
//! is closed by its spelling and its position by the renderer round-trip test in
//! `typed-trees`.
//!
//! Spellings are persisted in package policy identity. Do not respell a variant.

/// A fixed spelling in identity text.
pub trait Spelled: Copy {
    fn spelling(self) -> &'static str;
}

/// The closed set of spellings read at one position.
pub trait Vocabulary: Spelled + 'static {
    const ALL: &'static [Self];

    fn parse(spelling: &str) -> Option<Self>;
}

/// Why the renderer can write a production that no retained package policy
/// contains. The grammar refuses it with this reason instead of a malformed
/// identity, so a deliberate refusal never reads as drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unadmitted {
    /// Written beside the failure flag that stops an exact-owner request from
    /// producing an identity at all.
    InvalidIdentity,
    /// Written only while resolving regions under application scopes, which an
    /// exact-owner identity never has.
    ApplicationScope,
}

macro_rules! vocabulary {
    ($($(#[$enum_meta:meta])* $name:ident { $($(#[$variant_meta:meta])* $variant:ident = $spelling:literal,)+ })+) => {
        $(
            $(#[$enum_meta])*
            #[derive(Debug, Clone, Copy, PartialEq, Eq)]
            pub enum $name {
                $($(#[$variant_meta])* $variant,)+
            }

            impl Spelled for $name {
                fn spelling(self) -> &'static str {
                    match self {
                        $(Self::$variant => $spelling,)+
                    }
                }
            }

            impl Vocabulary for $name {
                const ALL: &'static [Self] = &[$(Self::$variant,)+];

                fn parse(spelling: &str) -> Option<Self> {
                    match spelling {
                        $($spelling => Some(Self::$variant),)+
                        _ => None,
                    }
                }
            }
        )+

        /// Whether any enum of the vocabulary spells `spelling`.
        pub fn is_vocabulary_spelling(spelling: &str) -> bool {
            false $(|| $name::parse(spelling).is_some())+
        }

        /// Check every enum of the vocabulary.
        #[cfg(test)]
        pub(crate) fn audit() {
            $(audit_enum::<$name>(stringify!($name));)+
        }
    };
}

/// A spelling is unique within its enum, parses back to its variant, and lexes
/// whole, so the reader can never meet a tag it cannot read.
#[cfg(test)]
fn audit_enum<F: Vocabulary + PartialEq + std::fmt::Debug>(name: &str) {
    let mut seen = Vec::new();
    for form in F::ALL {
        let spelling = form.spelling();
        assert!(!seen.contains(&spelling), "{name} repeats `{spelling}`");
        seen.push(spelling);
        assert_eq!(
            F::parse(spelling).as_ref(),
            Some(form),
            "{name} `{spelling}`"
        );
        let mut reader = super::framing::Reader::new(spelling);
        assert!(
            reader.production::<F>().as_ref() == Ok(form) && reader.finish().is_ok(),
            "{name} `{spelling}` must lex as one tag"
        );
    }
}

vocabulary! {
    /// The first tag of a type identity.
    TypeForm {
        /// Bare, with no parentheses.
        Unit = "unit",
        /// `nominal(<owner>,path(<text>))`
        Nominal = "nominal",
        /// `compiler-type(atom(<text>))`
        CompilerType = "compiler-type",
        /// `ref(<type>)`
        Reference = "ref",
        /// `ref-mut(<type>)`
        MutableReference = "ref-mut",
        /// `ref-write(<type>)`
        WriteOnlyReference = "ref-write",
        /// `slice(<type>)`
        Slice = "slice",
        /// `array(<type>,<length>)`
        Array = "array",
        /// `constrained(<type>,<constraint>*)`
        Constrained = "constrained",
        /// `generic(name(<name>),<argument>*)`
        Generic = "generic",
        /// `named(<name>)`, `named(integer-const(<text>))` or `named(canonical-const(...))`
        Named = "named",
        /// `dynamic-trait(name(<name>),<argument>*)`
        DynamicTrait = "dynamic-trait",
        /// `index-expression(<index>)`
        IndexExpression = "index-expression",
    }

    /// The text of a `name(...)` atom that this grammar reads. Any other text is
    /// a binder replacement that the consumer reads.
    NameForm {
        /// `nominal(<owner>,path(<text>))`
        Nominal = "nominal",
        /// `compiler-type(atom(<text>))`
        CompilerType = "compiler-type",
        /// `derived-application(<type>)`: a compiler-derived declaration named by
        /// the generic application that produced it.
        DerivedApplication = "derived-application",
        /// `placed-field(<type>,member(<text>))` or `placed-field(<type>,field(<text>))`
        PlacedField = "placed-field",
        /// `placed-operation(<placed-field>,operation(<text>),role(<text>))`
        PlacedOperation = "placed-operation",
        /// `invalid-derived-origin(<text>)`
        InvalidDerivedOrigin = "invalid-derived-origin",
    }

    /// The source that owns a nominal declaration.
    OwnerForm {
        /// `package-owner(<length>:<hexadecimal digest>)`
        Package = "package-owner",
        /// `toolchain-source-owner(<length>:<hexadecimal digest>)`
        ToolchainSource = "toolchain-source-owner",
        /// Bare: a toolchain declaration whose exact source is unknown.
        Toolchain = "toolchain-owner",
        /// Bare: a declaration without a source owner.
        Unresolved = "unresolved-owner",
    }

    /// One refinement of a constrained type.
    ConstraintForm {
        /// `arithmetic-domain(name(<text>))`
        ArithmeticDomain = "arithmetic-domain",
        /// `named-constraint(name(<text>))`
        NamedConstraint = "named-constraint",
        /// `declared-domain(name(<name>),<type>*)`; the name may nest the whole
        /// qualified `declared-domain(...)` application.
        DeclaredDomain = "declared-domain",
        /// `compiler-domain(family(<family>),...)`
        CompilerDomain = "compiler-domain",
    }

    /// The `family(...)` of a compiler domain.
    DomainFamily {
        /// `compiler-domain(family(carry),permission(<text>))`
        Carry = "carry",
        /// `compiler-domain(family(value),domain(<text>))`
        Value = "value",
        /// `compiler-domain(family(omega-layout),grammar(<text>),schema(<type>)*)`
        OmegaLayout = "omega-layout",
    }

    /// The extent of a fixed array.
    ArrayLengthForm {
        /// `literal(<digits>)`
        Literal = "literal",
        /// `const-parameter(<name>)`
        ConstParameter = "const-parameter",
        /// `const-call(<name>)`
        ConstCall = "const-call",
        /// `const-expression(<index>)`
        ConstExpression = "const-expression",
        /// `invalid-array-expression(<text>)`
        InvalidExpression = "invalid-array-expression",
    }

    /// A const operand, value operand or range endpoint.
    IndexForm {
        /// `const-name(<name>)`
        ConstName = "const-name",
        /// `canonical-const(type(<text>),encoding(<text>))`
        CanonicalConst = "canonical-const",
        /// `integer-const(<digits>)`
        IntegerConst = "integer-const",
        /// `integer(<digits>)`
        Integer = "integer",
        /// `boolean(<text>)`
        Boolean = "boolean",
        /// `float(<text>)`
        Float = "float",
        /// `string(<length>:<hexadecimal bytes>)`
        String = "string",
        /// `undetermined-nan(<text>)`
        UndeterminedNan = "undetermined-nan",
        /// `bitwise-not(<index>)`
        BitwiseNot = "bitwise-not",
        /// `logical-not(<index>)`
        LogicalNot = "logical-not",
        /// A binary operation: `add(<index>,<index>)`, or with a selected
        /// algebra `add(operation(...),algebra(...),<index>,<index>+)`.
        Add = "add",
        And = "and",
        BitwiseAnd = "bitwise-and",
        BitwiseOr = "bitwise-or",
        BitwiseXor = "bitwise-xor",
        Divide = "divide",
        Equal = "equal",
        Greater = "greater",
        GreaterOrEqual = "greater-or-equal",
        Less = "less",
        LessOrEqual = "less-or-equal",
        Modulo = "modulo",
        Multiply = "multiply",
        NotEqual = "not-equal",
        Or = "or",
        ShiftLeft = "shift-left",
        ShiftRight = "shift-right",
        Subtract = "subtract",
        CaseMembership = "case_membership",
        /// `call(target(<name>),static-arguments(<argument>+)?,<index>?,<index>*)`
        Call = "call",
        /// `cast(<index>,<type>,domain(<arithmetic domain>))`
        Cast = "cast",
        /// `array-literal(<index>*)`
        ArrayLiteral = "array-literal",
        /// `indexed(<index>,<index>)`
        Indexed = "indexed",
        /// `member(<index>,member(<text>),case(<text>)*)`
        Member = "member",
        /// `borrow(<index>)`
        Borrow = "borrow",
        /// `borrow-mut(<index>)`
        MutableBorrow = "borrow-mut",
        /// `borrow-write(<index>)`
        WriteOnlyBorrow = "borrow-write",
        /// `range-inclusive(<index>,<index>)`
        InclusiveRange = "range-inclusive",
        /// `range-exclusive(<index>,<index>)`
        ExclusiveRange = "range-exclusive",
        /// `struct-literal(type(<name>),case(<text>)?,field(name(<text>),<index>)*)`
        StructLiteral = "struct-literal",
        /// `type-expression(<type>)`
        TypeExpression = "type-expression",
        /// `zero-value(<type>)`
        ZeroValue = "zero-value",
        /// `atomic(<index>,<index>,ordering(<text>),result-custody(<text>))`
        Atomic = "atomic",
        /// `match(<index>,arm(<pattern>,<index>)*)`
        Match = "match",
        /// `saved-subject(depth(<digits>),<index>)`
        SavedSubject = "saved-subject",
        /// `case-component(<index>)`: the case view of the same saved value.
        CaseComponent = "case-component",
        /// `static-application(<index>,<argument or lifetime(<text>)>*)`: a
        /// selected declaration with its static operands.
        StaticApplication = "static-application",
        /// Bare: the substitution of an index operand was unsupported.
        UnsupportedConstSubstitution = "unsupported-const-substitution",
        /// Bare: a match saved subject outside its match.
        UnboundMatchSubject = "unbound-match-subject",
    }

    /// A region of an application-scoped identity.
    RegionForm {
        /// `region(owner(<text>),ordinal(<digits>))` or
        /// `region(kind(<text>),declaration(<name>),ordinal(<digits>))`
        Region = "region",
        /// Bare: a reference without an authored region.
        Elided = "elided-region",
        /// Bare: a region without a lexical declaration.
        Unresolved = "unresolved-region",
    }

    /// One pattern of a match arm.
    PatternForm {
        /// Bare.
        Wildcard = "wildcard",
        /// `destructure(<index>,fields(field(name(<name>),<action>)*),rest(<text>),<guard>,proof-selectors(...))`
        Destructure = "destructure",
    }

    /// What a destructured field does with its value.
    FieldActionForm {
        /// Bare.
        Bind = "bind",
        BindCase = "bind-case",
        /// `equal(<index>)`
        Equal = "equal",
        /// Bare.
        Waive = "waive",
        WaiveCase = "waive-case",
    }

    /// A fixed child of one production.
    Label {
        Name = "name",
        Path = "path",
        Atom = "atom",
        Type = "type",
        Encoding = "encoding",
        Family = "family",
        Permission = "permission",
        Domain = "domain",
        Grammar = "grammar",
        Schema = "schema",
        Target = "target",
        StaticArguments = "static-arguments",
        Operation = "operation",
        Algebra = "algebra",
        OpenIndexOperation = "open-index-operation",
        OpenIndexAlgebra = "open-index-algebra",
        Symbol = "symbol",
        Contract = "contract",
        Provider = "provider",
        Trait = "trait",
        Requirement = "requirement",
        Lifetime = "lifetime",
        Ordering = "ordering",
        ResultCustody = "result-custody",
        Member = "member",
        Case = "case",
        Field = "field",
        Fields = "fields",
        Arm = "arm",
        /// Bare: a destructuring arm without a guard.
        NoGuard = "no-guard",
        Rest = "rest",
        ProofSelectors = "proof-selectors",
        ProofSelector = "proof-selector",
        Binding = "binding",
        Depth = "depth",
        Role = "role",
        Owner = "owner",
        Ordinal = "ordinal",
        Kind = "kind",
        Declaration = "declaration",
    }
}
