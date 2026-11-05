//! Receiving checks repeatedly query the same immutable declarations and casts.
//! Prepare their inventories once; each walk marks and resets only touched rows.

use std::cell::{Cell, OnceCell};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TableLocalData};
use validation::CheckingInput;

struct LocalDeclaration<'program> {
    symbol: SymbolHandle,
    declaration: Option<(usize, &'program TableLocalData)>,
    alias_emitted: Cell<bool>,
}

pub(super) struct StateLocalDeclarations<'program> {
    rows: Vec<LocalDeclaration<'program>>,
}

impl<'program> StateLocalDeclarations<'program> {
    pub(super) fn new(program: &'program dyn CheckingInput, state: &State) -> Self {
        let mut rows = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
            .filter_map(|(ordinal, statement)| match statement {
                StatementNode::LocalData(local) => Some(LocalDeclaration {
                    symbol: local.symbol,
                    declaration: Some((ordinal, local)),
                    alias_emitted: Cell::new(false),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        rows.sort_unstable_by_key(|row| symbol_key(row.symbol));
        // A duplicated full symbol is malformed, even if both declarations
        // would otherwise make the same alias. Keep its refusal in the index.
        rows.dedup_by(|left, right| {
            if left.symbol != right.symbol {
                return false;
            }
            left.declaration = None;
            right.declaration = None;
            true
        });
        Self { rows }
    }

    fn position(&self, symbol: SymbolHandle) -> Option<usize> {
        self.rows
            .binary_search_by_key(&symbol_key(symbol), |row| symbol_key(row.symbol))
            .ok()
    }

    pub(super) fn declaration(
        &self,
        symbol: SymbolHandle,
    ) -> Option<(usize, &'program TableLocalData)> {
        self.rows[self.position(symbol)?].declaration
    }

    pub(super) fn alias_membership(&self) -> AliasMembership<'_> {
        AliasMembership {
            declarations: self,
            touched: Vec::new(),
        }
    }
}

fn symbol_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

/// Membership follows the existing dependency-first walk, not sorted row order.
/// Walks finish synchronously; resetting only emitted rows avoids clearing or
/// allocating storage for the whole state at each receiving obligation.
pub(super) struct AliasMembership<'declarations> {
    declarations: &'declarations StateLocalDeclarations<'declarations>,
    touched: Vec<usize>,
}

impl AliasMembership<'_> {
    pub(super) fn contains(&self, symbol: SymbolHandle) -> bool {
        self.declarations
            .position(symbol)
            .is_some_and(|position| self.declarations.rows[position].alias_emitted.get())
    }

    pub(super) fn insert(&mut self, symbol: SymbolHandle) {
        if let Some(position) = self.declarations.position(symbol)
            && !self.declarations.rows[position].alias_emitted.replace(true)
        {
            self.touched.push(position);
        }
    }
}

impl Drop for AliasMembership<'_> {
    fn drop(&mut self) {
        for &position in &self.touched {
            self.declarations.rows[position].alias_emitted.set(false);
        }
    }
}

struct CastTarget {
    expression: ExpressionHandle,
    checked: Cell<bool>,
}

/// The backing expression arena is immutable throughout this checker call.
/// Programs without a cast obligation never initialize this inventory.
#[derive(Default)]
pub(super) struct CastTargets {
    rows: OnceCell<Vec<CastTarget>>,
}

impl CastTargets {
    pub(super) fn receiving_walk<'query>(
        &'query self,
        program: &'query dyn CheckingInput,
    ) -> CastTargetChecks<'query> {
        CastTargetChecks {
            inventory: self,
            program,
            touched: Vec::new(),
        }
    }
}

/// A fresh walk retains the complete receiving application and statement epoch.
/// Only a cast's independent bare target formation is marked in this scratch.
pub(super) struct CastTargetChecks<'query> {
    inventory: &'query CastTargets,
    program: &'query dyn CheckingInput,
    touched: Vec<usize>,
}

impl CastTargetChecks<'_> {
    pub(super) fn needs_check(&mut self, expression: ExpressionHandle) -> bool {
        let rows = self.inventory.rows.get_or_init(|| {
            // Backing iteration already orders unique live arena positions;
            // filtering preserves the full-handle lookup order without sorting.
            self.program
                .expression_table
                .iter_backing_expressions()
                .filter_map(|(expression, node)| {
                    matches!(node, ExpressionNode::Cast(_)).then_some(CastTarget {
                        expression,
                        checked: Cell::new(false),
                    })
                })
                .collect::<Vec<_>>()
        });
        // The caller has just read this Cast from the same immutable backing
        // arena, including retained nodes outside the projected root roster.
        let position = rows
            .binary_search_by_key(&expression_key(expression), |row| {
                expression_key(row.expression)
            })
            .expect("a cast read from the backing arena belongs to its inventory");
        if rows[position].checked.replace(true) {
            return false;
        }
        self.touched.push(position);
        true
    }
}

fn expression_key(expression: ExpressionHandle) -> (u32, u32) {
    (expression.arena_index(), expression.generation())
}

impl Drop for CastTargetChecks<'_> {
    fn drop(&mut self) {
        if let Some(rows) = self.inventory.rows.get() {
            for &position in &self.touched {
                rows[position].checked.set(false);
            }
        }
    }
}
