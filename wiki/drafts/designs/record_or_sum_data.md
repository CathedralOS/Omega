# Record or sum data

Ratified 2026-11-10. A `data` declaration could hold fields and cases
together. That third shape needed its own decomposition selector
(`case as name`), a positional rule (fields, then one case component) and a
type that existed only behind a reference (`Command::case`), and it still left
open whether the case part could be owned, stored, built separately or passed
as a generic argument. An ordinary field holding an ordinary sum already
answers all of those. A `data` declaration is now a record or a sum, and
`data` declarations nest:

```omega
// before
data Command {
    command_id: u64;
    case GetUser(user_id: u64);
    case CountUsers;
}

machine inspect(command: &Command) -> bool {
    let Command { command_id as _, case as operation } = command;
    targets_one_user(operation)                     // takes &Command::case
}

// after
data Command {
    command_id: u64;

    op: data Operation {
        case GetUser(user_id: u64);
        case CountUsers;
    };
}

machine inspect(command: &Command) -> bool {
    targets_one_user(&command.op)                   // takes &Command::Operation
}
```

The normative owners are
[records, sums and nested declarations](../../spec/language/data_and_literals.md#records-sums-and-nested-declarations)
and [data and case decomposition](../../spec/language/patterns.md#data-and-case-decomposition).
Track implementation under `RECORD-OR-SUM-DATA` in [TASKS.md](../../../TASKS.md).
Delete this record when the implementation and acceptance are complete.

## Decisions

1. A data body declares fields or cases. Declaring both rejects.
2. Common fields and alternatives are combined by holding a sum in a record's
   field. The field and the sum follow the ordinary rules for fields and sums.
3. A data body may contain `data` declarations after its members, to any
   depth, in a record or a sum. A nested declaration introduces the name
   `Owner::Name` and no field, case, constructor argument, pattern component
   or cleanup action. It has its own invariants and cleanup and captures no
   value of its owner.
4. `name: data Type { ... };` in a field or case-payload position means the
   member `name: Type` plus the nested declaration. A type declared in a case
   payload is named under the enclosing data. The inline form introduces no
   parameters of its own, and each name is declared once.
5. Nesting is optional. A sum that several declarations hold is declared at
   module scope and held in a field.
6. A nested declaration inherits its owner's visibility, recursively, and
   takes no `public` of its own.
7. A nested declaration may use its owner's ordinary static parameters, and
   they are part of its identity whether or not its body mentions them:
   `Pool<File>::Handle` and `Pool<Socket>::Handle` are distinct types. Identity
   selects no representation. A nested alias, `data Header = SharedHeader;`,
   gives one shared type.
8. An owner's `some` binder is not inherited. A field passes the witness to a
   parameter the nested type declares.
9. Nested data shares its owner's member namespace with cases and attached
   domains. A collision rejects.
10. A case predicate takes any place as its subject. `self.op in
    Operation::GetUser` in a record's `requires` relates the held sum's case
    to the record's fields and parameters, and matching `command.op` of an
    established record combines the selected case with the record's
    predicates.
11. A helper receiving only the sum has the sum's contract. Lending the field
    mutably leaves the lender owing the record's predicates at handback.
12. Patterns stay one level deep. A held sum is matched as its own subject,
    `match command.op`, or as a tuple component beside other fields,
    `match (command.command_id, command.op)`. A record pattern does not
    contain a case pattern.
13. A sum held in a field has the same representation freedoms and
    obligations as any other field. A reference to it denotes the field's
    original storage, and an optional packing that cannot preserve a valid
    use is not selected. No size cost is specified.
14. `case as name`, the case-only component of positional patterns and
    `Data::case` are retired. This replaces
    `separate-data-and-case-decomposition` (2026-10-29) and
    `case-projection-type` (2026-11-09).

## What changes for source

| Before | After |
| --- | --- |
| Fields and cases in one body | A record with a field holding a sum |
| `Command::GetUser { command_id: 17, user_id: 42 }` | `Command { command_id: 17, op: Command::Operation::GetUser { user_id: 42 } }` |
| `Command { command_id as _, case as operation }` | `Command { command_id as _, op as operation }`, or `&command.op` |
| `&Command::case` | `&Command::Operation` |
| `command in Command::GetUser` | `command.op in Command::Operation::GetUser` |
| `self in Command::GetUser` in `requires` | `self.op in Operation::GetUser` |

A text scan for declarations with both a top-level field and a top-level case:

| Tree | `data` declarations | Fields and cases together | Of those, under `facts/` |
| --- | --- | --- | --- |
| `source` | 438 | 1 | 0 |
| `samples` | 9,438 | 52 | 43 |
| `tests` | 21,584 | 100 | 21 |

The one declaration in `source` is `Token` in `source/psi/tokens/tokens.omg`.
`case as` appears 56 times in `.omg` files and `::case` in type position does
not appear.

## Acceptance

- A body with a field and a case rejects with a diagnostic that prints the
  record-holding-a-sum form.
- Nested and inline declarations at two depths construct, match, move out of
  their field and pass by value and by reference under `--both`; a nested
  declaration without a field adds no member.
- A type declared in a case payload resolves under the enclosing data, an
  inline declaration with its own parameter rejects, and a nested name that
  collides with a case or a second declaration rejects.
- A nested type of a public owner is nameable by a consumer and one of a
  private owner is not.
- `Envelope<u32>::Body` does not unify with `Envelope<bool>::Body`, and a
  nested alias does.
- The `Command<T>` fixture: `self.op in ...` in `requires` rejects a
  mismatched construction, and matching `command.op` yields the type equality
  in the arm.
- A helper mutating through `&mut command.op` leaves the caller to prove the
  record's predicates again, and replacing the held sum through that
  reference leaves sibling fields and their loans intact.
- Tuple dispatch over a field and the held sum, with a payload binding in an
  arm, passes `--both`; a case pattern inside a record pattern rejects.
- Named and positional record decomposition and `let Pattern = expr;` pass
  `omega run --both` across the nine placement rows with rejecting twins. This
  carries the acceptance of the retired `DATA-DECOMPOSITION` item.
- `case as name`, `Data::case` and flattened case patterns reject.
- `Token` and the affected samples and tests are migrated.
