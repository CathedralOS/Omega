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

> **Settled 2026-07-18; amended in the same review: runtime cycles are
> tail-only.** Recursive call cycles are legal if and only if they carry a
> `decreases` measure (chapter 9), and in runtime code every recursive call
> must be in tail position, where the cycle lowers to loop machinery. An
> unmeasured call cycle is a compile error; a measured non-tail cycle in
> runtime code is a compile error. Transition loop-backs (chapter 4) are
> unchanged: they are jumps, not calls — unmeasured, constant-stack, free to
> run forever.

A machine may call itself, directly or through a mutual cycle, when every
cycle through the call graph strictly decreases a well-founded measure and
each recursive call is the last thing its arm does:

```omega
machine Gauss::sum(n: u64, acc: u64) -> u64
terminates { decreases n; }
{
    transition n {
        0 -> acc
        _ -> Gauss::sum(n - 1, acc + n)   // tail: the call IS the arm's result
    }
}
```

Working rules:

- **Legality is the measure, not the position.** Every runtime cycle needs
  both: `decreases` proves it terminates; tail position gives it a lowering.
  Spelling encodes intent: a transition arrow says "process — may run
  forever, constant space, no proof owed"; a recursive call says
  "terminating walk — measured, or it does not compile."
- **Every runtime cycle lowers to the loop machinery.** A tail recursive
  call compiles to the same back-edge a transition loop-back uses: zero
  stack growth, no frame accumulation, ever. Classification is strict and
  never silent: `-> 3 * Gauss::sum(...)` is not tail (the multiply runs
  after the call returns), and the error names why.
- **Non-tail recursion does not compile in runtime code.** A measured cycle
  whose call returns into more work (`1 + max(depth(l), depth(r))`) is
  rejected with the classification error. Depth belongs in data: iterate
  with explicit storage the machine declares and sizes — a fixed-capacity
  field today, a Region when the allocator arc lands. Activation frames are
  storage the author never sees or sizes; depth does not hide there.
- **The range is a termination fact, never a size.** `decreases cursor in
  lo..=hi` states where the measure lives; the floor is the well-foundedness
  bound (any start, not only zero, so a cursor walking `hi` down to `lo`
  needs no re-zeroed distance measure). Dependent endpoints are legal —
  pinned witnesses, re-proven at every back-edge, the same fact the loop
  spelling declares as a parameter range. Nothing is ever allocated from a
  range.
- **Lexicographic measures compose freely.** The measure gates legality and
  sizes nothing, so dictionary orders need no special case; bounded
  components may still flatten to a single linear measure (`m*B + n`).
- **Mutual cycles share a joint measure** (lexicographic when needed); every
  cycle through the call graph must decrease it, and at runtime every call
  along the cycle must be tail.
- **The whole program's worst-case stack is a static constant.** After
  lowering, the runtime call graph is acyclic, so the maximum live
  activation storage along any call chain is computable at build time and
  appears in the layout report.

Proof-stratum machines (chapter 10) follow the same legality rule with no
tail restriction: non-tail shapes — `1 + max(Tree::depth(node.left),
Tree::depth(node.right))`, induction over a tree — are legal there, because
fact-only machines evaluate at compile time under the checker's fuel budget
and never lower. No frame ever materializes.

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
