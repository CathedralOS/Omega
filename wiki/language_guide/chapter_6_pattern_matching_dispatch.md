# Chapter 6: Pattern Matching And Dispatch

Patterns select an arm and contribute facts about the saved subject. Expression
matching returns a value; transition matching chooses a control edge. The
[pattern specification](../spec/language/patterns.md) owns their shared rules.

## Match Expressions

A value-position match selects one result:

```omega
let category: i32 = match code {
    2 -> 10
    13 -> 20
    _ -> 0
};
```

The intended semantics evaluate the subject once, test arms in order, and
evaluate only the selected arm. Arm values need compatible types and the
dispatch must be exhaustive.

Scalar `match` implements that schedule today. The supported pattern and
owned/borrowed custody frontier is recorded in the
[source processing note](../../omega-rust/psi/compilation/pipeline/README.md#lexing-and-parsing).
The data/case-view forms below state the accepted language; their implementation
and source migration remain `DATA-DECOMPOSITION` work in [TASKS.md](../../TASKS.md).
An accepted scalar example is not evidence that every pattern form or owned
result is implemented.

## Data And Case Patterns

Common fields belong to the whole value, not to each case's payload. Match them
once in an outer data pattern, then match the case-only view:

```omega
data Command {
    command_id: u64;
    case GetUser(user_id: u64);
    case CountUsers;
}

machine targets_one_user(command: &Command) -> bool {
    match command {
        Command { command_id as _, case as operation } ->
            match operation {
                Command::GetUser { user_id as _ } -> true
                Command::CountUsers -> false
            }
    }
}
```

All inline cases follow the common fields. The equivalent positional outer
pattern is `(_, operation)`: common fields in authored order, then one case
component. Binding names are arbitrary. A named pattern uses `case as name` for
that final component; `case` is a pattern selector, not a stored field. Ordinary
field renaming still uses `as`, while `field: value` tests a value.

Patterns account for all components unless `..` explicitly waives the remaining
ones. A case pattern mentions payload fields only; common-field bindings inside
case patterns are retired. Pure sums still support direct case matching.
Nested matches retain outer bindings and still-valid facts, and each dispatch
must be exhaustive. The case view preserves the original subject's type
constraints, identity, lifetime, access and custody; ignoring a binding does not
waive cleanup. Physical layout and case-literal construction are unchanged.
The [pattern specification](../spec/language/patterns.md#data-and-case-decomposition)
owns the full rule.

## Irrefutable Bindings

When a pattern only decomposes the value, a local binding avoids a one-arm outer
match. Inside the example machine above, bind the same view before matching it:

```omega
let Command { command_id as _, case as operation } = command;
```

The positional equivalent is `let (_, operation) = command;`. Evaluate the
initializer once. The checked type and live facts must prove the pattern cannot
fail; otherwise use ordinary match arms. A binding has no hidden trap, assertion
or fallback. The original nested-match form remains valid, and there is no
special `match expression as Pattern` header.

Copies, moves, borrows and ignored-value cleanup keep their normal obligations.
A borrowed source does not permit implicit moves or copies of non-copy fields;
the case view retains its existing backing loan and lifetime. See
[irrefutable data bindings](../spec/language/patterns.md#irrefutable-data-bindings).
These forms remain part of the pending `DATA-DECOMPOSITION` implementation.

## Transition Dispatch

A transition can choose the next machine to invoke terminally:

```omega
transition navigation.choice {
    NavigationChoice::Quit -> finished()
    NavigationChoice::Look -> look()
    NavigationChoice::Invalid -> invalid_command()
}
```

Only the chosen edge runs. Its pattern facts help establish the target machine's
requirements, but do not waive its ordinary type, ownership, or authority checks.

## Tuple Patterns

Tuple dispatch combines explicit conditions:

```omega
transition (player_defeated, enemy_defeated) {
    (true, _) -> player_died()
    (false, true) -> enemy_died()
    (false, false) -> exchange_blows()
}
```

The first arm deliberately wins when both conditions hold. A component wildcard
ignores that component without asserting anything about its value.

Positional data patterns use the same parenthesized style on a data subject,
but their components come from its declared fields and optional final case view,
not an implicit conversion to unrelated tuple storage. Named and positional data
patterns do not change representation or add runtime fields.

## Named Facts Before Dispatch

Name complicated conditions before branching:

```omega
let empty: bool = items.len == 0;
transition empty {
    true -> no_items()
    false -> inspect_first()
}
```

The false arm establishes nonempty input. A target using that fact must receive
the corresponding view or value through its machine parameters. Naming a fact
does not make the source binding ambient in the target machine.

## Exhaustiveness

A complete case dispatch fails when an added case is not covered:

```omega
transition command {
    Command::Look -> look()
    Command::Quit -> finished()
    Command::Invalid -> invalid_command()
}
```

A wildcard opts into handling other cases. Pure case-union domains can contribute
finite case coverage; arbitrary predicate-domain tests do not establish coverage
merely because every known example matches. Prior facts can exclude impossible
subjects, but a failed proof cannot be treated as an unreachable arm.

A matched [case constraint](chapter_1_data_values_literals.md#constraints-on-individual-cases)
supplies both type equations and payload facts to its arm. For `Value<i32>`, the
Boolean case's `T == bool` contradicts the subject type, so a proved contradiction
can exclude it from coverage. Harder case predicates require established proof,
not a guess: otherwise cover the case or explicitly discharge its dead arm.
An arm's facts do not leak into other arms or survive invalidating mutation.

## Tail Dispatch

A transition ends the current straight-line segment. A machine target, nested
or otherwise, is a final call whose result completes the current invocation.
A value arm completes directly. Suspending/blocking calls retain their exact
acknowledgement markers; lexical nesting grants no exemption.
[Chapter 4](chapter_4_nested_machines_transitions.md#final-calls-including-suspension)
explains result forwarding and the retained loan/cleanup obligations.

## Domain Patterns

A runtime domain pattern needs a finite, pure executable test and any required
establishment provenance. Proof visibility alone does not make a test executable.

```omega
transition player {
    Player::Dead -> game_over()
    _ -> continue_game()
}
```

The selected domain fact belongs to the saved subject and its current revision.
Mutation can invalidate it. Overlapping domains use ordinary first-match order;
testing predicates cannot mint a routed qualification. See
[domains](chapter_8_domains.md).
