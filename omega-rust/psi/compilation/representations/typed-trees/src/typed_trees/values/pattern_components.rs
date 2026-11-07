//! A case view keeps its decomposition contract when a reference aliases it.
//!
//! Carrier geometry does not turn that view into an owned copy of the whole
//! data. Locals retain the component established by their initializer, and
//! receiving checks enforce it on later writes. This is a binding contract,
//! not a claim that the initializer's contents or old referents remain live.
//! Ordinary loan and storage-version checks still authenticate each use.

use crate::expression::{ExpressionHandle, ExpressionNode};
use language_core::PatternComponent;
use std::collections::BTreeMap;

impl crate::TypedTrees {
    pub fn pattern_component(&self, expression: ExpressionHandle) -> Option<PatternComponent> {
        Query {
            program: self,
            visited: BTreeMap::new(),
        }
        .read(expression)
    }
}

struct Query<'a> {
    program: &'a crate::TypedTrees,
    // Only actual transparent dependencies enter this request-local map.
    // The initial None also refuses cycles, and shared aliases are read once.
    visited: BTreeMap<ExpressionHandle, Option<PatternComponent>>,
}

impl Query<'_> {
    fn read(&mut self, expression: ExpressionHandle) -> Option<PatternComponent> {
        let program = self.program;
        let table = &program.expression_table;
        if !table.expression_is_valid(expression) {
            return None;
        }
        let node = table.expression(expression);
        let direct = table.direct_pattern_component(expression)?;
        if let ExpressionNode::SavedSubject(saved) = node {
            self.program.saved_value_source(saved)?;
            return Some(if direct == PatternComponent::Case {
                direct
            } else {
                saved.component
            });
        }
        if direct == PatternComponent::Case {
            return Some(direct);
        }
        let source = match node {
            ExpressionNode::Name(name) => {
                if table.names_reserved_result(expression) {
                    return Some(PatternComponent::Fields);
                }
                let symbol = name.resolved_symbol()?;
                if self.program.symbols.get(symbol).kind != symbols::SymbolKind::Local {
                    return Some(PatternComponent::Fields);
                }
                let local = self.program.unique_local_data_for_symbol(symbol)?;
                if !local.role.is_runtime_binding() {
                    return None;
                }
                if !local.initial_value.is_valid() {
                    return Some(PatternComponent::Fields);
                }
                Some(local.initial_value)
            }
            ExpressionNode::Borrow(borrow) => Some(borrow.target),
            ExpressionNode::Cast(cast) => Some(cast.value),
            ExpressionNode::Match(_) => None,
            _ => return Some(PatternComponent::Fields),
        };
        if let Some(source) = source {
            if !table.expression_is_valid(source) {
                return None;
            }
            let leaf = match table.expression(source) {
                ExpressionNode::Borrow(_) | ExpressionNode::Cast(_) | ExpressionNode::Match(_) => {
                    false
                }
                ExpressionNode::Name(name) => {
                    name.resolved_symbol().is_none_or(|symbol| {
                        self.program.symbols.get(symbol).kind != symbols::SymbolKind::Local
                    }) || table.direct_pattern_component(source) == Some(PatternComponent::Case)
                }
                _ => true,
            };
            if leaf {
                return self.read(source);
            }
        }
        if let Some(component) = self.visited.get(&expression) {
            return *component;
        }
        self.visited.insert(expression, None);
        let result = if let Some(source) = source {
            self.read(source)
        } else {
            let ExpressionNode::Match(dispatch) = node else {
                return None;
            };
            let mut selected = None;
            for arm in table.match_arms(dispatch.arms) {
                let component = self.read(arm.value)?;
                if selected.is_some_and(|prior| prior != component) {
                    return None;
                }
                selected = Some(component);
            }
            selected
        };
        self.visited.insert(expression, result);
        result
    }
}
