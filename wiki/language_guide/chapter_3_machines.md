# Chapter 3: Machines

A machine is the callable boundary.

Machines may be attached to data, or free-standing when there is no natural
owning data type.

## Attached Machines

Attached machines operate on a named data type.

```omega
data Player {
    health: i32;
}

machine Player::take_damage(
    &mut self,
    amount: i32
) {
    self.health = self.health - amount;
}
```

`self` is explicit. If the machine mutates the receiver, it takes `&mut self`.

## Free-Standing Machines

Free-standing machines are ordinary machines without a data receiver.

```omega
machine add_i32(
    left: i32,
    right: i32
) -> i32 {
    left + right
}
```

Use a free-standing machine for math helpers, proof helpers, and operations
that are not naturally owned by one data type.

## Program Entry

Executable programs should use an explicit root data type.

```omega
data Main {
    total: i32;
}

machine Main::main(&mut self) -> i32 {
    self.total = add_i32(3, 4);
    self.total
}
```

The process entry is the `main` machine on the `Main` root data object. Startup
allocates the root object, then enters `Main::main(&mut root)`.

This keeps process-owned state under one explicit owner.

## Parameters And Returns

Machine parameters are entry data. A machine return type is the value shape its
body or internal state graph eventually produces.

```omega
machine Parser::resolve(
    &self,
    line: &String
) -> Command {
    Command::Invalid
}
```

Every reachable terminal path in a typed machine must produce a compatible
return value.

## Calls

Ordinary call syntax enters a machine and creates a call frame.

```omega
let command: Command = self.parser.resolve(&self.line);
```

Calls and transitions are different. A call enters another machine. A transition
jumps to a state inside the current machine. Chapter 4 introduces states and
transitions directly.

## Measured Recursion

> **Settled 2026-07-18.** Recursive call cycles are legal if and only if they
> carry a `decreases` measure (chapter 9). An unmeasured call cycle is a
> compile error. Transition loop-backs (chapter 4) are unchanged: they are
> jumps, not calls — unmeasured, constant-stack, free to run forever.

A machine may call itself, directly or through a mutual cycle, when every
cycle through the call graph strictly decreases a well-founded measure:

```omega
machine Tree::depth(node: &Node) -> u64
terminates { decreases node -> Tree::Height; }
{
    transition node.is_leaf {
        true  -> 0
        false -> 1 + max(Tree::depth(node.left), Tree::depth(node.right))
        //       ^ non-tail: the calls return into the max — see the space rule
    }
}
```

Working rules:

- **Legality is the measure, not the position.** Tail and non-tail recursion
  are both gated by `decreases`. Spelling encodes intent: a transition arrow
  says "process — may run forever, constant space, no proof owed"; a
  recursive call says "terminating walk — measured, or it does not compile."
- **Tail position lowers to the loop machinery.** A recursive call in tail
  position compiles to the same back-edge a transition loop-back uses: zero
  stack growth. Classification is strict and never silent:
  `-> 3 * Tree::depth(...)` is not tail (the multiply runs after the call
  returns), and the error names why.
- **Non-tail recursion carries a space obligation.** The machine declares a
  compile-time depth budget and proves the initial measure fits it
  (spelling provisional). The activation region is an ordinary
  machine-storage field — `[Frame; BUDGET]` plus a depth witness — sized at
  layout time and reported like any other field (chapter 20). There is no
  operating-system stack to overflow: an over-generous budget fails loudly
  as a visibly large layout.
- **Runtime-unbounded depth does not compile.** An unranged runtime measure
  cannot discharge the budget obligation; bounding the witness — a declared
  range, a dominating guard — is the fix.
- **Mutual cycles share a joint measure** (lexicographic when needed); every
  cycle through the call graph must decrease it.
- **The whole program's worst-case stack is a static constant.** Every call
  cycle is budget-bounded, so the maximum live activation storage along any
  call chain is computable at build time and appears in the layout report.

Proof-stratum machines (chapter 10) follow the same legality rule with no
space obligation: they evaluate at compile time under the checker's fuel
budget and never lower.

## Contracts

Machines may declare requirements and guarantees.

```omega
machine Player::enter_combat(&mut self)
requires
    self in Player::Alive
ensures
    self in Player::InCombat
{
}
```

The caller must satisfy `requires`. The machine body must establish `ensures`.

## Machine Graph Compatibility

Internal states participate in the machine's graph, but they are not public
machine entries.

Working rules:

- State-transition arguments must match the target state's parameters.
- Terminal values must satisfy the active machine's return type.
- Every reachable terminal path in a typed machine graph must produce the
  declared return type.
- Transition dispatch arms add proof assumptions for the target edge.
