use super::{assignment_receiver_polarity, carried_atomic_operation, place_is_atomic_storage};
use access_plans::{AtomicAccessOperation, BorrowPolarity};
use language_core::atomic::MemoryOrdering;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

/// The polarity and carried operation of every assignment in `publish`.
fn publish_assignments(
    program: &TypedTrees,
) -> Vec<(BorrowPolarity, Option<AtomicAccessOperation>, bool)> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Cell::publish")
        .expect("Cell::publish is declared");
    let state = &program.machine_states(machine)[0];
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| {
            let StatementNode::Assignment(assignment) = statement else {
                return None;
            };
            let carried = match program.expression_table.expression(assignment.value) {
                ExpressionNode::Atomic(atomic) => carried_atomic_operation(program, atomic),
                _ => None,
            };
            Some((
                assignment_receiver_polarity(program, machine, Some(state), assignment),
                carried,
                place_is_atomic_storage(program, machine, Some(state), assignment.target),
            ))
        })
        .collect()
}

fn validation_messages(source: &str) -> Vec<String> {
    match crate::validate_program(&typed(source)) {
        Ok(()) => Vec::new(),
        Err(diagnostics) => diagnostics
            .into_iter()
            .map(|diagnostic| diagnostic.message)
            .collect(),
    }
}

const NOT_MUTABLE: &str = "assignment cannot write `value` because it is not mutable in this state";

#[test]
fn store_carrier_on_atomic_storage_demands_a_shared_receiver() {
    let program = typed(
        "data Cell { value: AtomicU32; }
         machine Cell::publish(&self, value: u32) { self.value.store(value, NoOrdering); }",
    );
    assert_eq!(
        publish_assignments(&program),
        [(
            BorrowPolarity::Shared,
            Some(AtomicAccessOperation::Store(MemoryOrdering::NoOrdering)),
            true
        )]
    );
}

#[test]
fn fetch_carrier_recovers_its_family_from_the_interpreter_model() {
    let program = typed(
        "data Cell { value: AtomicU32; }
         machine Cell::publish(&self) {
             let prior: u32 = self.value.fetch_or(1, Publish);
             let before: u32 = self.value.fetch_add(2, ReceivePublish);
             let mask: u32 = self.value.fetch_and(3, NoOrdering);
             let flipped: u32 = self.value.fetch_xor(4, Receive);
             let less: u32 = self.value.fetch_sub(5, GlobalOrder);
         }",
    );
    let carried: Vec<_> = publish_assignments(&program)
        .into_iter()
        .map(|(polarity, carried, atomic_storage)| {
            assert_eq!(polarity, BorrowPolarity::Shared);
            assert!(atomic_storage);
            carried
        })
        .collect();
    assert_eq!(
        carried,
        [
            Some(AtomicAccessOperation::FetchOr(MemoryOrdering::Publish)),
            Some(AtomicAccessOperation::FetchAdd(
                MemoryOrdering::ReceivePublish
            )),
            Some(AtomicAccessOperation::FetchAnd(MemoryOrdering::NoOrdering)),
            Some(AtomicAccessOperation::FetchXor(MemoryOrdering::Receive)),
            Some(AtomicAccessOperation::FetchSub(MemoryOrdering::GlobalOrder)),
        ]
    );
}

#[test]
fn swap_and_compare_exchange_carriers_share_the_receiver() {
    let program = typed(
        "data Cell { value: AtomicU32; }
         machine Cell::publish(&self) {
             let displaced: u32 = self.value.swap(7, ReceivePublish);
             let observed: u32 = self.value.compare_exchange(7, 9, ReceivePublish, Receive);
         }",
    );
    let carried: Vec<_> = publish_assignments(&program)
        .into_iter()
        .map(|(polarity, carried, _)| {
            assert_eq!(polarity, BorrowPolarity::Shared);
            carried
        })
        .collect();
    assert_eq!(
        carried,
        [
            Some(AtomicAccessOperation::Swap(MemoryOrdering::ReceivePublish)),
            Some(AtomicAccessOperation::CompareExchange {
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
            }),
        ]
    );
}

#[test]
fn direct_assignment_to_atomic_storage_keeps_the_exclusive_demand() {
    let program = typed(
        "data Cell { value: AtomicU32; }
         machine Cell::publish(&self, value: u32) { self.value = value; }",
    );
    assert_eq!(
        publish_assignments(&program),
        [(BorrowPolarity::Exclusive, None, true)]
    );
}

#[test]
fn store_carrier_on_plain_storage_keeps_the_exclusive_demand() {
    let program = typed(
        "data Cell { value: u32; }
         machine Cell::publish(&self, value: u32) { self.value.store(value, NoOrdering); }",
    );
    assert_eq!(
        publish_assignments(&program),
        [(
            BorrowPolarity::Exclusive,
            Some(AtomicAccessOperation::Store(MemoryOrdering::NoOrdering)),
            false
        )]
    );
}

#[test]
fn shared_receiver_admits_atomic_store_and_fetch_but_not_plain_writes() {
    assert_eq!(
        validation_messages(
            "data Cell { value: AtomicU32; }
             machine Cell::publish(&self, value: u32) {
                 self.value.store(value, NoOrdering);
                 let prior: u32 = self.value.fetch_or(1, Publish);
             }"
        ),
        Vec::<String>::new()
    );
    let plain = validation_messages(
        "data Cell { value: u32; }
         machine Cell::publish(&self, value: u32) { self.value = value; }",
    );
    assert!(
        plain.iter().any(|message| message.contains(NOT_MUTABLE)),
        "{plain:#?}"
    );
    let plain_store = validation_messages(
        "data Cell { value: u32; }
         machine Cell::publish(&self, value: u32) { self.value.store(value, NoOrdering); }",
    );
    assert!(
        plain_store
            .iter()
            .any(|message| message.contains(NOT_MUTABLE)),
        "{plain_store:#?}"
    );
    let direct = validation_messages(
        "data Cell { value: AtomicU32; }
         machine Cell::publish(&self, value: u32) { self.value = value; }",
    );
    assert!(
        direct.iter().any(|message| message.contains(NOT_MUTABLE)),
        "{direct:#?}"
    );
}

#[test]
fn shared_receiver_atomic_store_still_reports_an_unknown_field() {
    let messages = validation_messages(
        "data Cell { value: AtomicU32; }
         machine Cell::publish(&self, value: u32) { self.valeu.store(value, NoOrdering); }",
    );
    assert!(
        messages
            .iter()
            .any(|message| message.contains("data `Cell` has no field `valeu`")),
        "{messages:#?}"
    );
}
