//! Track the context of each written file qualifier while parsing one file.

use language_core::build_operations::BuildOperation;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use syntax_trees::identifier::Identifier;
use syntax_trees::item::{GenericConformanceBound, Item, TypeParameter, TypeParameterKind};
use syntax_trees::types::TypeReferenceNode;
use syntax_trees::{ParsedRootItems, SyntaxTrees};

#[derive(Default)]
pub(crate) struct FileQualifiers<'source> {
    // Parsing can visit a name more than once. Its opening bar identifies one occurrence.
    uses: RefCell<BTreeMap<usize, (&'source str, QualifierUse)>>,
    // The first `asm` instruction that needs core's packed-integer contracts.
    packed_instruction: std::cell::Cell<Option<source_text::SourceSpan>>,
}

enum QualifierUse {
    Host,
    Product,
    RootOperand(Option<String>),
}

impl<'source> FileQualifiers<'source> {
    pub(crate) fn record_packed_instruction(&self, site: source_text::SourceSpan) {
        if self.packed_instruction.get().is_none() {
            self.packed_instruction.set(Some(site));
        }
    }

    pub(crate) fn packed_instruction(&self) -> Option<source_text::SourceSpan> {
        self.packed_instruction.get()
    }

    pub(crate) fn record(&self, start: usize, alias: &'source str) {
        self.uses
            .borrow_mut()
            .entry(start)
            .or_insert((alias, QualifierUse::Host));
    }

    fn mark(&self, path: &[Identifier], role: QualifierUse) {
        if let Some(head) = path.first()
            && let Some((_, product)) = self
                .uses
                .borrow_mut()
                .get_mut(&head.source_span().span.start)
        {
            *product = role;
        }
    }

    pub(crate) fn mark_root_operand(&self, path: &[Identifier]) {
        self.mark(path, QualifierUse::RootOperand(None));
    }

    /// After a machine body closes, its inner machines already recorded their own binders.
    pub(crate) fn record_build_binder(
        &self,
        trees: &SyntaxTrees,
        body: source_text::Span,
        binders: arena::HandleSpan<TypeParameter>,
        bounds: &[GenericConformanceBound],
    ) {
        let explicit = bounds
            .iter()
            .any(|bound| bound.binder.as_str(trees) == "Build");
        let schema = trees
            .items
            .type_parameters(binders)
            .iter()
            .find_map(|parameter| {
                if parameter.name.as_str(trees) == "Build"
                    && let TypeParameterKind::Const { type_reference } = parameter.kind
                {
                    match trees.type_references.type_reference(type_reference) {
                        TypeReferenceNode::Named(name)
                        | TypeReferenceNode::Generic {
                            base_name: name, ..
                        } => Some(name.as_str(trees)),
                        _ => None,
                    }
                } else {
                    None
                }
            });
        if explicit || schema.is_some() {
            for (_, (_, role)) in self.uses.borrow_mut().range_mut(body.start..body.end) {
                if matches!(role, QualifierUse::RootOperand(None)) {
                    *role = if explicit {
                        QualifierUse::Host
                    } else {
                        QualifierUse::RootOperand(schema.map(str::to_owned))
                    };
                }
            }
        }
    }

    /// A static Build call can select a declaration by filename. Its ordinary
    /// value arguments and any other use of that file still belong to the host.
    pub(crate) fn mark_build_call(&self, trees: &SyntaxTrees, expression: ExpressionHandle) {
        let ExpressionNode::Call(call) = trees.expressions.expression(expression) else {
            return;
        };
        if call.machine_arguments.is_empty() || !call.target_is_static || !call.receiver.is_valid()
        {
            return;
        }
        let ExpressionNode::Name {
            members,
            application: None,
        } = trees.expressions.expression(call.receiver)
        else {
            return;
        };
        let [owner] = trees.expressions.identifier_path_members(*members) else {
            return;
        };
        if owner.as_str(trees) != "Build" {
            return;
        }
        let Some(operation) = BuildOperation::from_call_target(call.target.as_str(trees)) else {
            return;
        };
        if operation.uses_tests_facet() {
            return;
        }
        for (position, argument) in call.machine_arguments.iter().enumerate() {
            if operation.product_operand_role(position).is_some() {
                self.mark(&argument.path, QualifierUse::Product);
            }
        }
    }

    pub(crate) fn occurrences(
        &self,
        trees: &SyntaxTrees,
        roots: &ParsedRootItems,
    ) -> BTreeMap<&'source str, Vec<syntax_trees::item::FileQualifier>> {
        let mut aliases = BTreeMap::new();
        let uses = self.uses.borrow();
        if uses.is_empty() {
            return aliases;
        }
        let mut traits = BTreeSet::new();
        let mut named_record = false;
        for root in &roots.items {
            match trees.root_item(*root) {
                Item::Trait(schema) => {
                    let name = schema.name.as_str(trees);
                    traits.insert(name);
                    named_record |= name == "Build";
                }
                Item::Conformance(record) => {
                    named_record |= record
                        .alias
                        .as_ref()
                        .is_some_and(|name| name.as_str(trees) == "Build");
                }
                _ => {}
            }
        }
        for (&offset, (alias, role)) in uses.iter() {
            let product = match role {
                QualifierUse::Product => true,
                QualifierUse::Host => false,
                QualifierUse::RootOperand(schema) => {
                    !named_record && !schema.as_deref().is_some_and(|name| traits.contains(name))
                }
            };
            aliases
                .entry(*alias)
                .or_default()
                .push(syntax_trees::item::FileQualifier {
                    offset,
                    product_operand: product,
                });
        }
        aliases
    }
}
