# How L3 checks place access and runtime callable types

A new parser node can change what storage a program writes or hide a callable inside another type. L3 makes both decisions explicitly for every variant of the parser's [NodeKind](../01-parser/src/tree/node_kind.rs). Adding a variant makes the typer fail to compile until both matches handle it.

## Place access

A place is the storage an expression designates. Assignment and explicit borrowing enter the same [check_place](src/expressions/places.rs) match. It receives the requested operation before it looks through grouping or selects storage. An ordinary field read enters `project_fields` with shared access.

| Source path | Where the decision is made | Observable result |
| --- | --- | --- |
| Parentheses around a place | `check_place` visits the child with the original request. | Grouping a local, field or index preserves its access. A field loan names the selected field inside the outer parentheses. |
| Local or other value expression | `type_expression` types the value. A write or exclusive borrow also calls `require_writable`. | A mutable binding can be assigned. Rebinding a shared reference grants no authority to change its target. |
| Field selection | `project_fields` checks the requested access and `require_writable` checks the receiver. | A write-only field supports assignment and a write-only loan. Reading it or taking a readable mutable loan fails. Collection length and address remain read-only. |
| Built-in brackets | [index](src/expressions/indexing.rs) selects the operation from the requested access and the element or range selector. | Mutable indexing requires mutable authority. The typed result records the selected operation and the collection it borrows. |
| Authored brackets | [declared_operator](src/expressions/operators.rs) checks the receiver before adapting it. `index` checks the selected result against the requested access. | A mutable collection operand can supply the mutable receiver. An immutable value cannot. |
| Implicit borrowing at a call | [admit_argument](src/expressions/call_arguments.rs) can introduce a shared loan. | Passing an ordinary value cannot implicitly introduce a mutable or write-only loan. |

The place match explicitly refuses structural nodes that cannot serve as operands. The [place tests](tests/calls/data_and_places.rs) check authority errors, the selected index operation, loan access and source span. They include grouping around the receiver, between field selectors and around the complete place. [Expression rules](../../../wiki/spec/language/expressions.md) and [ownership rules](../../../wiki/spec/language/ownership.md) define the expected behavior.

## Runtime callable ceilings

A runtime callable must declare the services it may reach and the crashes it may produce. [raw_type](src/type_system/annotations.rs) checks both clauses when it forms a `MachineType`. Its exhaustive match decides which children form types.

| Source path | Where the decision is made | Observable result |
| --- | --- | --- |
| Field, parameter, result or local | The declaration or body typer requests the annotation. | Omitting either ceiling reports an error on the callable itself. |
| Reference, lifetime-qualified reference, slice or array | `raw_type` forms the target or element. Array extents and lifetimes have their own checks. | Nesting preserves the callable's ceiling requirement. |
| Named type application | [raw_argument](src/type_system/generic_arguments.rs) uses the declared binder to choose type formation or value checking. | A type argument such as the element of `Box<T>` goes through type formation. |
| Callable parameter or result, mathematical function or qualified carrier | `raw_type` forms the component types. A type-family application checks its argument subjects against the family's signature. | An enclosing callable's clauses cannot supply a missing clause on a nested callable. |
| Static callback | Only a callable directly under `ConstParam` or `KindedParam` can omit a ceiling. [resolve_callback_bounds](src/contracts/callback_bounds.rs) also constructs bounds for statically selected requirements. | The direct bound can remain open. Runtime parameter and result types inside the bound still need both clauses. |

The [callable tests](tests/calls/data_and_places.rs) compose every ordered pair of reference, array, slice and generic wrappers. For each pair they check all four combinations of the two clauses and the diagnostic span of the inner callable. The [effect specification](../../../wiki/spec/language/effects.md#open-callable-contracts) defines which positions may remain open.

## Checking a change

Run `mbx test --locked -p omega-typer --test calls` from `omega-rust-v2`. These tests lex, parse, resolve and type each example. Rust checks the constructor matches whenever it builds the typer. A new spelling that reuses an existing node kind needs a source-level test, because the enum stays the same. The compiler checks the completeness of the match arms. The source tests check what those arms actually do.
