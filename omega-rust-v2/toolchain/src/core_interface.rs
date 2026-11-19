//! The standard interface is resolved from declarations shipped with the toolchain.
use super::TOOLCHAIN_PACKAGE;
use omega_resolver::{CompiledInterface, DeclarationId, ParsedSource, StageSource, SymbolResolvedTrees};
use std::sync::OnceLock;

/// Projection from the selected data layout, identified by its core declaration.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LayoutQuery {
    Size,
    Alignment,
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
            .find(|e| e.id.path.strip_prefix("language::core::") == Some(name))
            .expect("declared core name")
            .id
    }
    pub fn exports(&self) -> &CompiledInterface {
        &self.exports
    }
    /// Implementation identities admitted by this declaration-only library.
    pub fn supplies(
        &self,
        machine: &DeclarationId,
    ) -> bool {
        machine == self.id("access::new_plan")
            || self.ranking_views().iter().any(|(view, _)| machine == *view)
            || self.collection_primitive(machine).is_some()
            || self.layout_query(machine).is_some()
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
    pub fn ranking_views(&self) -> [(&DeclarationId, bool); 2] {
        [
            (self.id("Nat::Descending"), true),
            (self.id("Nat::BoundedDistance"), false),
        ]
    }
}
pub fn core_interface() -> &'static CoreInterface {
    static CORE: OnceLock<CoreInterface> = OnceLock::new();
    CORE.get_or_init(|| {
        let files = [
            ("indexing/collections.omg", include_str!("../core/indexing.omg")),
            ("language/core/core.omg", include_str!("../core/core.omg")),
            ("language/core/reflect/reflect.omg", include_str!("../core/reflect.omg")),
            ("language/core/access/access.omg", include_str!("../core/access.omg")),
            ("language/core/Nat/nat.omg", include_str!("../core/nat.omg")),
        ]
        .map(|(path, text)| {
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
        let exports = declarations.compiled_interface();
        CoreInterface { declarations, exports }
    })
}
