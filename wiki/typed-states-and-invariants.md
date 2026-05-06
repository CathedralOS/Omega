# Typed States And Invariants

This note captures a newer direction for Omega: states may become typed, callable graph nodes, and type annotations may carry proof obligations.

This is not final syntax. It is a pressure-test document so the compiler and samples can evolve without losing the idea.

## Typed State Signatures

Early Omega states were mostly named graph nodes:

```omega
state Running {
    Tick();

    -> self;
}
```

The newer sketch allows states to accept explicit parameters and produce a typed value:

```omega
state Clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
    -> self.ClampLow(min) when value < min;
    -> self.ClampHigh(max) when value > max;
    -> self.ClampDone(value);
}

state ClampLow(&mut self, min: f32) -> f32 {
    -> self.ClampDone(min);
}

state ClampHigh(&mut self, max: f32) -> f32 {
    -> self.ClampDone(max);
}

state ClampDone(&mut self, value: f32) -> f32 {
    value
}
```

This makes a state feel callable, but it is still not just a normal function. It is still a graph node with explicit outgoing edges.

Working interpretation:

- State parameters are local state-entry data.
- `&mut self` means the state may mutate the current machine.
- The return type is the type that must be yielded by terminal state completion.
- A state body may end in transitions or a final expression.
- Transitions can forward values into another state.
- A transition to another state is a typed goto, not a stack return.

The implementation should be careful not to accidentally reintroduce arbitrary call-stack semantics everywhere.

## Stack Semantics Versus Goto Semantics

Omega should distinguish two control-flow worlds.

Inside one machine, transitions are gotos. They do not push a stack frame, remember a return address, or resume the source state. The source state deactivates, and the target state activates with explicit arguments.

```omega
state Clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
    -> self.ClampLow(min) when value < min;
    -> self.ClampHigh(max) when value > max;
    -> self.ClampDone(value);
}
```

Those edges are not calls. `Clamp` never resumes after `ClampLow`, `ClampHigh`, or `ClampDone`. It hands control away.

Across machines, nested machine flow may be stack-like:

```omega
state Running {
    -> dungeon.Main -> Running;
}
```

The parent enters a child machine and records or otherwise carries the continuation to use when the child machine terminates. That continuation stack belongs to machine composition, not ordinary intra-machine branching.

Typed states are therefore function-shaped graph nodes, not classic functions. Their signatures constrain which gotos are legal.

## Transition Return Value Compatibility

A typed transition is legal only when the graph handoff lines up. The most important compatibility check is return value compatibility: the target graph must be able to produce the value shape the source graph is obligated to produce.

Likely checks:

- The target state exists in the current machine or explicitly addressed machine.
- The provided arguments match the target state's parameters.
- The target state's return value type can satisfy the current state's return value obligation.
- Every reachable terminal expression in a typed state graph produces the declared result type.
- A guarded transition may add proof assumptions for the target edge, but it does not create a caller frame.

For example:

```omega
state ClampLow(&mut self, min: f32) -> f32 {
    -> self.ClampDone(min);
}
```

`ClampLow` can jump to `ClampDone` because `ClampDone` accepts the forwarded `f32` and produces the same `f32` result expected by the `Clamp` graph.

This is the key distinction from classic functions: result compatibility is a graph invariant, not a return path.

## State Return Values

Omega has resisted general return values because transitions are handoffs, not function calls.

Typed states soften that stance in a constrained way:

```omega
state ClampDone(&mut self, value: f32) -> f32 {
    value
}
```

This should mean:

- The state graph eventually produces a value.
- The value is produced by a terminal expression or equivalent terminal state.
- Intermediate transitions do not "return" in the C/Rust sense.
- The compiler can model the whole state cluster as a transition graph with a result.
- The produced value flows to the enclosing graph expectation, not back to an intra-machine caller frame.

Open question:

Is a typed state cluster allowed to suspend across ticks, or must it complete in one scheduling turn?

## Bounded Types

Omega should allow data types to carry proof-friendly refinements:

```omega
state clamp(
    value: i32,
    min: const i32,
    max: const i32
) -> i32[range<min, max>] {
}
```

And owned data may carry invariants:

```omega
owns mass: i32[range<1, 100>];
```

Working interpretation:

- `i32[range<1, 100>]` is an `i32` refined by a range invariant.
- `i32[range<min, max>]` is an `i32` refined by compile-time or proof-visible bounds.
- `const` parameters may be used in type-level constraints.
- The compiler emits proof obligations anywhere a value is assigned, returned, or transitioned into a bounded slot.

For example:

```omega
state Clamp(&mut self, value: f32, min: f32, max: f32) -> f32 {
    -> self.ClampLow(min) when value < min;
    -> self.ClampHigh(max) when value > max;
    -> self.ClampDone(value);
}
```

The proof shape is:

- If `value < min`, `ClampLow(min)` produces `min`.
- If `value > max`, `ClampHigh(max)` produces `max`.
- Otherwise, `ClampDone(value)` is only reachable when `min <= value <= max`.

The ordered transition list becomes an implicit proof partition.

## Repair Scopes

Some transformations temporarily violate invariants but restore them before the value escapes.

Sketch:

```omega
owns mass: i32[range<1, 100>];

state Whatever() {
    repair self.mass {
        self.mass -= 50000;
        self.mass += 50001;
    }
}
```

`repair self.mass { ... }` means:

- The invariant on `self.mass` may be temporarily relaxed inside the block.
- The compiler must prove the invariant holds at the end of the block.
- The relaxed value must not be observed by other states, transitions, platform calls, or escaped references.
- The repair block is a proof boundary.

This should not become a general "turn off safety" block. It is closer to a proof-local transaction.

Potential constraints:

- Only explicitly named targets may be repaired.
- Repaired values cannot be passed to commands unless the command accepts the relaxed type.
- Transitions inside repair blocks are only allowed if the repair obligation is carried onto each outgoing edge.
- Nested repair scopes need a clear proof stack.

## Invariant Propagation Across Transitions

Omega should be able to weaken an invariant temporarily, then prove that each transition either restores the invariant or transfers a narrower proof obligation to the next state.

Sketch:

```omega
owns health: i32[range<1, 100>] = 100;

state TakeDamage(&mut self, amount: i32[range<1, 100>]) {
    repair self.health {
        self.health -= amount;

        -> Revive when self.health <= 0;
        -> Bloodied(amount) when self.health > 25 && amount <= 50;
        -> StillAlive;
    }
}

state Bloodied(amount: i32[range<1, 50>]) {
}

state Revive(&mut self) {
    self.health = 100;
}
```

The useful idea is not that `repair` means "anything goes." It means the compiler has a proof debt. If `self.health` normally has `range<1, 100>`, then `self.health -= amount` may temporarily widen the known type to something like `i32[range<-98, 99>]`.

Each outgoing transition must account for that debt:

- `-> Revive when self.health <= 0` is valid if `Revive` re-establishes `self.health: i32[range<1, 100>]`.
- `-> Bloodied(amount) when self.health > 25 && amount <= 50` is valid only if the guard plus the current proof context implies the target argument bounds.
- `-> StillAlive` is valid only if the remaining ordered-transition context proves `self.health` is back inside `range<1, 100>` or `StillAlive` accepts the weakened invariant.

Because transitions are ordered, the final bare transition inherits the negation of earlier guards. In the sketch above, `StillAlive` sees `self.health > 0` and the negation of the `Bloodied` guard if the earlier edges did not fire.

This gives Omega a way to model controlled damage, recovery, saturation, clamping, retry state, and other real programs without pretending every intermediate instruction preserves every invariant.

Open design pressure:

- A repair block may need an explicit target list, such as `repair self.health`, so the compiler knows which invariants are weakened.
- A transition that leaves a repair scope may need to carry hidden proof state, not hidden runtime state.
- A target state may need to declare that it accepts a weakened machine invariant, otherwise it must receive repaired data.
- Proof weakening across transitions must remain visible in diagnostics and tooling, or it becomes the exact kind of magic Omega is trying to avoid.

## Proof Obligations

Typed states and bounded values imply compiler-generated obligations.

Likely obligations:

- Every assignment into a bounded location preserves the bound.
- Every terminal expression of a typed state satisfies the declared return type.
- Every transition into a typed state provides compatible arguments.
- Every typed transition satisfies return value compatibility.
- Every guarded transition establishes the assumptions needed by its target.
- Every `repair` scope restores all relaxed invariants before exit.
- Every transition leaving a `repair` scope either repairs the invariant or carries an explicit proof obligation into a compatible target.

This maps well onto TLA+ style action checking:

- Machine fields are variables.
- State parameters are action inputs.
- Transitions are guarded next-state relations.
- Bounded types are invariants or pre/postconditions.
- Repair scopes are local invariant relaxation with mandatory restoration.

## Syntax Questions

Open questions to settle later:

- Should the refinement syntax be `i32[range<1, 100>]`, `i32 where range<1, 100>`, or something else?
- Are `const` parameters compile-time values, proof constants, or both?
- Is `&mut self` the right spelling, or should Omega avoid Rust-looking receiver syntax?
- Can typed states be called from commands, or only transitioned into?
- Does a final expression make a state too function-like?
- How does this interact with branch-free states and ordered transitions?
- Can the compiler infer result bounds from ordered transitions without explicit annotations?
- Can repair obligations cross arbitrary transitions, or only transitions to states that opt in?
