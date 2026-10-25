# payload_less_case_equality

A qualified payload-LESS case member stays a legal `==`/`!=` operand even when a sibling case carries a payload — `cmd == Command::None` beside `Move(dx: i32)`, tag identity is total equality. Requires `CommandEquality satisfies Equatable<Command> { }` (synthesized structural equals). Learnings: no `if` expression — `match bool` arms instead; nested machines cannot see enclosing-machine locals.

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
