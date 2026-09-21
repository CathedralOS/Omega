//! Fixtures shared by the borrow resource tests: lowering source programs
//! into checked facts.

mod restoration_and_retirement;
mod root_and_reborrow_closures;
mod transactional_rejections;

use super::super::{
    Lexer, ResolutionRequest, lower_symbol_resolved_trees, parse_syntax_trees, resolve,
};
use crate::CheckingRequest;
use crate::lower_typed_trees;

const SYMBOLIC_ADJACENCY: &str = r#"
    data Main { items: [i32; 4]; }

    machine Main::split(&mut self) -> u64 {
        let mid: u64 = 2;
        let cut: u64 = mid;
        let left: &mut [i32] = self.items[0..cut];
        let right: &mut [i32] = self.items[mid..4];
        left.len + right.len
    }
"#;

fn lower(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize borrow resources");
    let syntax = parse_syntax_trees(&tokens).expect("parse borrow resources");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve borrow resources");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type borrow resources");
    lower_typed_trees(typed, &CheckingRequest::settled()).expect("check borrow resources")
}

fn try_lower(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize borrow resources");
    let syntax = parse_syntax_trees(&tokens).expect("parse borrow resources");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve borrow resources");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type borrow resources");
    lower_typed_trees(typed, &CheckingRequest::settled())
}

fn reborrow_access_source(parent: &str, child: &str) -> String {
    let (parent_type, parent_borrow, prefix) = match parent {
        "Read" => ("&Cell", "&self.cell", ""),
        "Mutable" => ("&mut Cell", "&mut self.cell", ""),
        "WriteOnly" => (
            "&write Cell",
            "&write root",
            "let root: &mut Cell = &mut self.cell;",
        ),
        _ => unreachable!(),
    };
    let (child_type, child_borrow, use_child) = match child {
        "Read" => ("&Cell", "&parent", "observe(child);"),
        "Mutable" => ("&mut Cell", "&mut parent", "mutate(child);"),
        "WriteOnly" => ("&write Cell", "&write parent", "replace(&write child);"),
        _ => unreachable!(),
    };
    format!(
        r#"
        data Cell {{ value: i32; }}
        data Main {{ cell: Cell; }}
        machine observe(value: &Cell) {{}}
        machine mutate(value: &mut Cell) {{ value.value = 1; }}
        machine replace(value: &write Cell) {{ value.value = 1; }}
        machine Main::exercise(&mut self) {{
            {prefix}
            let parent: {parent_type} = {parent_borrow};
            let child: {child_type} = {child_borrow};
            {use_child}
        }}
        "#,
    )
}

fn symbolic_adjacency() -> checked_trees::CheckedTrees {
    lower(SYMBOLIC_ADJACENCY)
}

fn direct_read_and_mutable_modes() -> checked_trees::CheckedTrees {
    lower(
        r#"
        data Main { readable: i32; mutable: i32; }
        data Sibling { readable: i32; }

        machine observe(value: &i32) {}
        machine mutate(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let read: &i32 = &self.readable;
            observe(read);
            let mutable_loan: &mut i32 = &mut self.mutable;
            mutate(mutable_loan);
        }

        machine Sibling::exercise(&self) {
            let read: &i32 = &self.readable;
            observe(read);
        }
        "#,
    )
}

fn direct_reborrow_chain() -> checked_trees::CheckedTrees {
    lower(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; other: Cell; }
        data Sibling { cell: Cell; }

        machine write_cell(cell: &mut Cell) { cell.value = 2; }

        machine Main::exercise(&mut self) {
            let unrelated: &mut Cell = &mut self.other;
            write_cell(unrelated);
            let first: &mut Cell = &mut self.cell;
            let second: &mut Cell = &mut first;
            let third: &mut Cell = &mut second;
            write_cell(third);
        }

        machine Sibling::exercise(&mut self) {
            let sibling: &mut Cell = &mut self.cell;
            write_cell(sibling);
        }
        "#,
    )
}

fn main_reborrow_loans(
    checked: &checked_trees::CheckedTrees,
) -> Vec<arena::Handle<checked_trees::BorrowLoanFact>> {
    let state = checked
        .facts
        .borrow
        .states
        .iter()
        .map(|(_, state)| state)
        .find(|state| checked.facts.borrow.loans.span_or_empty(state.loans).len() == 4)
        .expect("main reborrow state");
    checked
        .facts
        .borrow
        .loans
        .iter()
        .filter(|(handle, _)| checked.facts.borrow.state_owns_loan(state, *handle))
        .map(|(handle, _)| handle)
        .collect()
}

fn mutable_parent_write_only_child_restored_use() -> checked_trees::CheckedTrees {
    lower(
        r#"
        data Cell { value: i32; }
        data Main { cell: Cell; }
        machine mutate(value: &mut Cell) { value = Cell { value: 2 }; }
        machine Main::exercise(&mut self) {
            let parent: &mut Cell = &mut self.cell;
            let child: &write Cell = &write parent;
            child.value = 1;
            mutate(parent);
        }
        "#,
    )
}

fn mutable_parent_sole_shared_child_restored_use() -> checked_trees::CheckedTrees {
    lower(
        r#"
        data Main { value: i32; }
        machine observe(value: &i32) {}
        machine mutate(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let child: &i32 = &parent;
            observe(child);
            mutate(parent);
        }
        "#,
    )
}

fn mutable_parent_two_shared_children_restored_use() -> checked_trees::CheckedTrees {
    lower(
        r#"
        data Main { value: i32; }
        machine observe(left: &i32, right: &i32) {}
        machine mutate(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let left: &i32 = &parent;
            let right: &i32 = &parent;
            observe(left, right);
            mutate(parent);
        }
        "#,
    )
}

fn mutable_parent_three_shared_children_restored_use() -> checked_trees::CheckedTrees {
    lower(
        r#"
        data Main { value: i32; }
        machine observe(left: &i32, middle: &i32, right: &i32) {}
        machine mutate(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let left: &i32 = &parent;
            let middle: &i32 = &parent;
            let right: &i32 = &parent;
            observe(left, middle, right);
            mutate(parent);
        }
        "#,
    )
}

fn sequential_reborrows() -> checked_trees::CheckedTrees {
    lower(
        r#"
        data Main { value: i32; }
        machine write(value: &mut i32) { value = 1; }
        machine Main::exercise(&mut self) {
            let parent: &mut i32 = &mut self.value;
            let first: &mut i32 = &mut parent;
            write(first);
            let second: &mut i32 = &mut parent;
            write(second);
            write(parent);
        }
        "#,
    )
}
