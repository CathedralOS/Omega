//! Mathematical definitions and proof propositions in the resolved program.

pub mod mathematical {
    use crate::source_resolved::data::TypeParameter;
    use crate::source_resolved::expression::ExpressionHandle;
    use crate::source_resolved::name::DiagnosticName;
    use crate::source_resolved::types::TypeReference;
    use arena::{Handle, HandleSpan};
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct MathematicalDefinition {
        pub symbol: SymbolHandle,
        pub name: DiagnosticName,
        pub is_public: bool,
        pub binders: HandleSpan<TypeParameter>,
        pub parameters: HandleSpan<MathematicalParameter>,
        pub result: MathematicalTypeHandle,
        pub body: MathematicalBody,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct MathematicalParameter {
        pub symbol: SymbolHandle,
        pub name: DiagnosticName,
        pub relevance: language_core::BindingRelevance,
        pub ty: MathematicalTypeHandle,
    }

    pub type MathematicalTypeHandle = Handle<MathematicalType>;

    #[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
    pub enum MathematicalType {
        Ordinary(TypeReference),
        Arrow {
            binder: Option<DiagnosticName>,
            domain: MathematicalTypeHandle,
            codomain: MathematicalTypeHandle,
        },
        Application {
            callee: MathematicalTypeHandle,
            arguments: HandleSpan<ExpressionHandle>,
        },
    }

    impl Default for MathematicalType {
        fn default() -> Self {
            Self::Ordinary(TypeReference::default())
        }
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub enum MathematicalBody {
        #[default]
        Assumption,
        Definition(ExpressionHandle),
        DependentArrow {
            binders: HandleSpan<MathematicalParameter>,
            body: ExpressionHandle,
        },
    }
}
pub mod proposition {
    use crate::source_resolved::data::Binder;
    use crate::source_resolved::expression::ExpressionHandle;
    use crate::source_resolved::name::DiagnosticName;
    use crate::source_resolved::types::TypeReference;
    use arena::HandleSpan;
    use symbols::SymbolHandle;

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub struct PropositionDefinition {
        pub symbol: SymbolHandle,
        pub name: DiagnosticName,
        pub is_public: bool,
        pub binders: HandleSpan<PropositionBinder>,
        pub parameters: HandleSpan<crate::source_resolved::signature::StateParameter>,
        pub transparent_formula_source_span: Option<source_text::SourceSpan>,
        pub body: PropositionBody,
    }

    pub type PropositionBinder = Binder<PropositionBinderKind>;

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub enum PropositionBinderKind {
        #[default]
        Type,
        Const {
            type_reference: TypeReference,
        },
        Machine,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq, record_storage::Record)]
    pub enum PropositionBody {
        #[default]
        Primitive,
        Witness {
            evidence: TypeReference,
        },
        Transparent {
            proposition: ExpressionHandle,
        },
    }
}
