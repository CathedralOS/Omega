# Chapter 10: Relax Scopes

Some transformations temporarily violate invariants but restore them before the value escapes.

```omega
data Body {
    mass: i32[range<1, 100>];
}

machine Body::whatever(&mut self) {
    relax self.mass {
        self.mass -= 50000;
        self.mass += 50001;
    }
}
```

`relax self.mass { ... }` means the compiler is allowed to weaken the invariant on `self.mass` inside the block, but it creates proof debt.

The intended rules:

- The invariant on `self.mass` may be temporarily relaxed inside the block.
- The compiler must prove the invariant holds at the end of the block.
- The relaxed value must not be observed by unrelated states, platform calls, or escaped references.
- The relax block is a proof boundary, not a safety-off switch.

Potential constraints:

- Only explicitly named targets may be relaxed.
- Relaxed values cannot be passed to calls unless the callee accepts the relaxed type.
- Transitions inside relax blocks are only allowed if the relax obligation is carried onto each outgoing edge.
- Nested relax scopes need a clear proof stack.
