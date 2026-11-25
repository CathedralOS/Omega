//! Stage 03 asks the operand-owner table which declarations hold an
//! expression or type several times per typed cohort. The table's walk skips
//! the name lookups whose only result is a symbol it would discard; these
//! tests pin that its rows equal those of a walk that still performs them.

use crate::source_state::TypedTrees;
use crate::source_state::expression::ExpressionHandle;
use crate::source_state::mathematical::MathematicalTypeHandle;
use crate::source_state::references::{
    TypedReferences, collect_program_operand_owners, visit_program,
};
use crate::source_state::types::TypeReferenceHandle;
use symbols::SymbolHandle;

type Rows<Element> = Vec<Vec<(SymbolHandle, arena::Handle<Element>)>>;

/// The owner walk as it was before it skipped symbol-only lookups: it
/// reads every symbol, so the walk performs operator, family and
/// transition-family resolution for each call it reaches.
#[derive(Default)]
struct SymbolReadingOwners {
    owner: SymbolHandle,
    outer_owners: Vec<SymbolHandle>,
    symbols: usize,
    types: Rows<crate::source_state::types::TypeReferenceNode>,
    expressions: Rows<crate::source_state::expression::ExpressionNode>,
    mathematical_types: Rows<crate::source_state::mathematical::MathematicalType>,
}

fn retain<Element>(
    seen: &mut Rows<Element>,
    owner: SymbolHandle,
    reference: arena::Handle<Element>,
) -> bool {
    let slot = reference.arena_index() as usize;
    if seen.len() <= slot {
        seen.resize_with(slot + 1, Vec::new);
    }
    if seen[slot].contains(&(owner, reference)) {
        return false;
    }
    seen[slot].push((owner, reference));
    true
}

fn owners_of<Element>(
    seen: &Rows<Element>,
    reference: arena::Handle<Element>,
) -> Vec<SymbolHandle> {
    seen.get(reference.arena_index() as usize)
        .into_iter()
        .flatten()
        .filter(|(_, retained)| *retained == reference)
        .map(|(owner, _)| *owner)
        .collect()
}

impl Extend<SymbolHandle> for SymbolReadingOwners {
    fn extend<Symbols: IntoIterator<Item = SymbolHandle>>(&mut self, symbols: Symbols) {
        self.symbols += symbols.into_iter().count();
    }
}

impl TypedReferences for SymbolReadingOwners {
    fn declaration(&mut self, symbol: SymbolHandle) -> bool {
        self.owner = symbol;
        true
    }
    fn enter_lexical_scope(&mut self, owner: SymbolHandle) {
        self.outer_owners.push(self.owner);
        if owner.is_valid() {
            self.owner = owner;
        }
    }
    fn leave_lexical_scope(&mut self) {
        if let Some(owner) = self.outer_owners.pop() {
            self.owner = owner;
        }
    }
    fn type_reference(&mut self, reference: TypeReferenceHandle) -> bool {
        retain(&mut self.types, self.owner, reference)
    }
    fn expression(&mut self, expression: ExpressionHandle) -> bool {
        retain(&mut self.expressions, self.owner, expression)
    }
    fn mathematical_type(&mut self, reference: MathematicalTypeHandle) -> bool {
        retain(&mut self.mathematical_types, self.owner, reference)
    }
}

fn assert_owners_match_symbol_reading_walk(program: &TypedTrees) {
    let owners = collect_program_operand_owners(program);
    let mut reference = SymbolReadingOwners::default();
    visit_program(program, &mut reference);
    assert!(reference.symbols > 0, "the reference walk reads symbols");
    let mut compared = 0;
    for (handle, node) in program.expression_table.iter_expressions() {
        assert_eq!(
            owners.expression_owners(handle).collect::<Vec<_>>(),
            owners_of(&reference.expressions, handle),
            "owners of {node:?}"
        );
        compared += 1;
    }
    for handle in reference.types.iter().flatten().map(|(_, handle)| *handle) {
        assert_eq!(
            owners.type_owners(handle).collect::<Vec<_>>(),
            owners_of(&reference.types, handle),
            "owners of type {handle:?}"
        );
        compared += 1;
    }
    assert!(compared > 0);
}

#[test]
fn operand_owners_equal_a_walk_that_reads_every_symbol() {
    use crate::source_state::expression::ExpressionNode;

    let program = crate::qualified_operator_home_tests::checked(&[(
        "owners.omg",
        r#"
data Counter { }
machine add(left: u64, right: u64) -> u64 { left + right }
machine twice(value: u64) -> u64 { add(value, value) }
machine run(counter: @mutable Counter) {
    transition {
        _ -> block step(1, 0)
    }
    machine step(value: u64, total: u64) {
        let next: u64 = twice(value);
        transition value == 0 {
            true -> block done(total),
            _ -> block step(0, add(total, next))
        }
    }
    machine done(total: u64) {
        let kept: u64 = total;
    }
}
"#,
    )])
    .expect("the owner fixture types");
    let calls = program
        .expression_table
        .iter_expressions()
        .filter(|(_, node)| matches!(node, ExpressionNode::Call(_)))
        .count();
    assert!(calls >= 3, "the fixture reaches named calls: {calls}");
    assert_owners_match_symbol_reading_walk(&program);
}
