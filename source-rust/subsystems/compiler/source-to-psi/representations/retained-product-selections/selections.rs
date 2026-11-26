//! A dependency package records the providers and native bindings chosen by Build,
//! including their exact source declarations and the arguments fixed by each choice.
#![forbid(unsafe_code)]

use semantic_vocabulary::source_selections::{CompositionMode, ProductTarget};
use source_admission::RetainedDeclaration;
use source_declarations::declaration_selection::{
    BuildSelectionOrigin, NativeBindingSource, product_operands::ProductStaticOperandGraph,
};
use source_symbol_table::source_inputs::SourceMap;
use std::sync::Arc;

/// The provider Build chose for a declaration or operator family and its optional target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedProviderSelection {
    pub subject: RetainedProviderSubject,
    pub provider: RetainedDeclaration,
    pub provider_identity: String,
    pub provider_application: Option<RetainedProductApplication>,
    pub target: Option<ProductTarget>,
    pub composition_mode: CompositionMode,
    pub origin: BuildSelectionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetainedProviderSubject {
    Declaration {
        declaration: RetainedDeclaration,
        identity: String,
        application: Option<RetainedProductApplication>,
    },
    OperatorFamily(RetainedOperatorFamily),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedOperatorCoordinate {
    pub declaration: RetainedDeclaration,
    pub requirement_identity: String,
    pub static_parameter_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedOperatorFamily {
    pub canonical_path: String,
    coordinates: Vec<RetainedOperatorCoordinate>,
}

impl RetainedOperatorFamily {
    pub fn new(
        canonical_path: String,
        coordinates: Vec<RetainedOperatorCoordinate>,
    ) -> Result<Self, &'static str> {
        if canonical_path.is_empty()
            || coordinates.is_empty()
            || coordinates.iter().any(|member| {
                member.requirement_identity.is_empty()
                    || !matches!(
                        member.declaration.kind(),
                        symbols::SymbolKind::Machine | symbols::SymbolKind::Operator
                    )
            })
            || coordinates
                .windows(2)
                .any(|pair| pair[0].requirement_identity >= pair[1].requirement_identity)
        {
            return Err("retained operator family needs its complete canonical coordinate roster");
        }
        Ok(Self {
            canonical_path,
            coordinates,
        })
    }

    pub fn coordinates(&self) -> &[RetainedOperatorCoordinate] {
        &self.coordinates
    }
}

impl RetainedProviderSubject {
    /// All files containing original declarations belong to the selected family.
    pub fn declarations(&self) -> impl Iterator<Item = &RetainedDeclaration> {
        let (single, family): (_, &[RetainedOperatorCoordinate]) = match self {
            Self::Declaration { declaration, .. } => (Some(declaration), &[]),
            Self::OperatorFamily(family) => (None, family.coordinates()),
        };
        single
            .into_iter()
            .chain(family.iter().map(|member| &member.declaration))
    }

    pub fn application(&self) -> Option<&RetainedProductApplication> {
        match self {
            Self::Declaration { application, .. } => application.as_ref(),
            Self::OperatorFamily(_) => None,
        }
    }
}

/// The exact Product requirement named by Build's native binding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedNativeDeclaration {
    pub application: Option<RetainedProductApplication>,
    pub source: NativeBindingSource,
    pub requirement: RetainedDeclaration,
    pub requirement_identity: String,
    pub service_declaration: [u8; 32],
    pub dependency_alias: Option<String>,
}

/// The static arguments and source locations needed to rebuild the same application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedProductApplication {
    pub graph: ProductStaticOperandGraph,
    pub sources: Arc<SourceMap>,
    pub identity: String,
}
impl RetainedProductApplication {
    pub fn declaration_spans(&self) -> impl Iterator<Item = source_text::SourceSpan> + '_ {
        self.graph
            .names
            .iter()
            .filter(|(_, name)| name.builtin_type.is_none() && name.builtin_function.is_none())
            .filter_map(|(_, name)| name.declaration)
            .chain(self.graph.operands.iter().filter_map(|(_, operand)| {
                let declaration = operand.declaration?;
                // Written paths already contribute their non-builtin declarations above.
                // A closed operand can also refer to a declaration without spelling its name.
                let written = self.graph.names.span(operand.path).and_then(|path| path.last());
                (written.and_then(|name| name.declaration) != Some(declaration))
                    .then_some(declaration)
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use source_declarations::declaration_selection::product_operands::{
        ProductOperandName, ProductStaticOperand,
    };
    use source_text::{SourceId, SourceSpan, Span};

    #[test]
    fn mounted_declarations_include_an_operand_with_no_written_name() {
        let anchored = SourceSpan::new(SourceId(23), Span::new(2, 6));
        let mut graph = ProductStaticOperandGraph::default();
        graph.root = graph.operands.insert(ProductStaticOperand {
            declaration: Some(anchored),
            ..Default::default()
        });
        let builtin = SourceSpan::new(SourceId(7), Span::new(0, 4));
        let path = graph.names.insert_many([ProductOperandName {
            text: "bool".into(),
            declaration: Some(builtin),
            builtin_type: Some(symbols::BuiltinTypeAtom::Bool),
            ..Default::default()
        }]);
        graph.operands.insert(ProductStaticOperand {
            path,
            declaration: Some(builtin),
            ..Default::default()
        });
        let application = RetainedProductApplication {
            graph,
            identity: String::new(),
            sources: Arc::default(),
        };
        assert_eq!(
            application.declaration_spans().collect::<Vec<_>>(),
            [anchored]
        );
    }
}
