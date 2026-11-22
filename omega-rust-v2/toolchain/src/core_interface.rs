//! The standard interface is resolved from declarations shipped with the toolchain.
use super::TOOLCHAIN_PACKAGE;
use omega_resolver::{CompiledInterface, DeclarationId, ParsedSource, StageSource, SymbolResolvedTrees};
use std::sync::OnceLock;

pub use omega_target_contract::LayoutQuery;

/// The argument names a declaration. Neither query has an executable call signature.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationQuery {
    Field,
    Callable,
}

impl DeclarationQuery {
    pub fn identify(
        package: &str,
        path: &str,
    ) -> Option<Self> {
        if package != TOOLCHAIN_PACKAGE {
            return None;
        }
        [Self::Field, Self::Callable]
            .into_iter()
            .find(|query| query.path() == path)
    }

    fn path(self) -> &'static str {
        match self {
            Self::Field => r"language\core\reflect\field",
            Self::Callable => r"language\core\reflect\callable",
        }
    }
}

pub struct CoreInterface {
    pub declarations: SymbolResolvedTrees<'static>,
    exports: CompiledInterface,
}

impl CoreInterface {
    pub fn id(
        &self,
        name: &str,
    ) -> &DeclarationId {
        &self
            .exports
            .declarations
            .iter()
            .find(|e| e.id.path.strip_prefix("language\\core\\") == Some(name))
            .expect("declared core name")
            .id
    }
    pub fn arithmetic_policy_domains(&self) -> impl Iterator<Item = &DeclarationId> {
        self.exports.declarations.iter().map(|decl| &decl.id).filter(|id| {
            matches!(id.path.as_str(), r"core\u::Wrapping" | r"core\i::Wrapping"
                | r"core\u::Saturating" | r"core\i::Saturating"
                | r"core\u::Trapping" | r"core\i::Trapping")
        })
    }
    pub fn sources(&self) -> &'static [(&'static str, &'static str)] {
        &PRODUCT_CORE
    }
    pub fn exports(&self) -> &CompiledInterface {
        &self.exports
    }
    /// The closed catalog: bodyless core machines whose execution the compiler
    /// supplies under a fixed meaning (language/machines.md#supply). Every other
    /// core machine, including the `access` plan constructors, has a checked body.
    pub fn supplies(
        &self,
        machine: &DeclarationId,
    ) -> bool {
        self.ranking_views().iter().any(|(view, _)| machine == *view) || self.collection_primitive(machine).is_some() || self.layout_query(machine).is_some()
    }
    pub fn layout_query(
        &self,
        machine: &DeclarationId,
    ) -> Option<LayoutQuery> {
        if machine == self.id("size_of") {
            Some(LayoutQuery::Size)
        } else if machine == self.id("align_of") {
            Some(LayoutQuery::Alignment)
        } else {
            None
        }
    }
    pub fn collection_primitive(
        &self,
        machine: &DeclarationId,
    ) -> Option<crate::CollectionPrimitive> {
        crate::CollectionPrimitive::ALL
            .into_iter()
            .find(|primitive| machine == self.collection(*primitive))
    }
    pub fn collection(
        &self,
        primitive: crate::CollectionPrimitive,
    ) -> &DeclarationId {
        &self
            .exports
            .declarations
            .iter()
            .find(|e| e.id.path == primitive.path())
            .expect("bundled collection declaration")
            .id
    }
    pub fn access_plan_requirement(&self) -> &DeclarationId {
        self.id("AccessPolicy::plan")
    }
    /// The canonical boundary trait and its exact process-exit requirement.
    /// Identity includes this package, so a same-spelled declaration elsewhere never matches.
    pub fn process_exit_requirement(&self) -> (&DeclarationId, &DeclarationId) {
        (self.id("ProcessExit"), self.id("ProcessExit::exit_process"))
    }
    pub fn ranking_views(&self) -> [(&DeclarationId, bool); 2] {
        [
            (self.id("Nat\\Descending"), true),
            (self.id("Nat\\BoundedDistance"), false),
        ]
    }
}
pub(crate) const PRODUCT_CORE: [(&str, &str); 11] = [
    ("core/drop.omg", include_str!("../core/drop.omg")),
    ("core/integer/comparisons.omg", include_str!("../core/integer_comparisons.omg")),
    ("core/integer/arithmetic.omg", include_str!("../core/integer_arithmetic.omg")),
    ("core/integer_policies.omg", include_str!("../core/integer_policies.omg")),
    ("indexing/collections.omg", include_str!("../core/indexing.omg")),
    ("language/core/core.omg", include_str!("../core/core.omg")),
    ("language/core/reflect/reflect.omg", include_str!("../core/reflect.omg")),
    ("language/core/access/access.omg", include_str!("../core/access.omg")),
    ("language/core/Nat/nat.omg", include_str!("../core/nat.omg")),
    ("language/core/never.omg", include_str!("../core/never.omg")),
    ("language/core/process_exit.omg", include_str!("../core/process_exit.omg")),
];

pub fn core_interface() -> &'static CoreInterface {
    static CORE: OnceLock<CoreInterface> = OnceLock::new();
    CORE.get_or_init(|| {
        let files = PRODUCT_CORE.map(|(path, text)| {
            let tokens = omega_lexer::lex(text.as_bytes()).expect("bundled core tokens");
            let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap_or_else(|e| panic!("{path}: {e:?}"));
            (path, text, tokens, tree)
        });
        let sources = files.each_ref().map(|(path, text, tokens, tree)| StageSource {
            package: TOOLCHAIN_PACKAGE,
            source: ParsedSource {
                logical_path: path,
                text,
                tokens,
                tree,
            },
        });
        let declarations = omega_resolver::resolve_stage(&sources, &[]).expect("bundled core declarations");
        let mut exports = declarations.compiled_interface();
        for query in [DeclarationQuery::Field, DeclarationQuery::Callable] {
            exports
                .declaration_queries
                .insert((TOOLCHAIN_PACKAGE.into(), query.path().into()));
        }
        CoreInterface { declarations, exports }
    })
}
