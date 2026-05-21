# Chapter 10: Relax Scopes

Most invariant-preserving updates should use temporary values.

```omega
machine Body::set_mass(
    &mut self,
    delta: i32
) {
    let next_mass: i32[range<1, 100>] = self.mass + delta;
    self.mass = next_mass;
}
```

The compiler can usually lower temporary values into registers or SSA values.
Temps keep invalid states out of memory, simplify proofs, and make optimization
easier.

`relax` exists for the cases where a real location must be mutated in place
while temporarily violating its invariant.

```omega
data Tree {
    root: NodeId;
}

machine Tree::rotate_left(&mut self) {
    relax self.root {
        Tree::rotate_left_raw(&mut relaxed self.root);
        Tree::restore_balance(&mut relaxed self.root);
    }
}
```

The exact spelling for relaxed parameters is provisional. The semantic point is
that `self.root` is in a weakened invariant state inside the block.

## Core Rule

A relax scope is lexical, local, and non-transitioning.

Working rules:

- `relax target { ... }` creates an exclusive relaxed borrow of `target`.
- The normal invariant on `target` is weakened inside the block.
- The compiler must prove the normal invariant is restored at the end of the
  block.
- No transition may occur inside a relax scope.
- The relaxed borrow cannot escape.
- Normal calls cannot observe the relaxed target.
- Nested machine calls are allowed only when their signatures explicitly accept
  the relaxed view, or when the checker can prove they cannot observe the
  relaxed target.

The first implementation should use the stricter version of the last rule:
calls inside relax must explicitly accept the relaxed target.

## Why Transitions Are Rejected

Transitions are jumps through the machine graph. Letting relaxed proof debt
cross transition edges would require weakened state signatures, graph-wide debt
tracking, and diagnostics across cycles.

That is too much complexity for the core feature.

Instead:

```omega
machine Tree::rebalance(&mut self) {
    transition {
        _ -> rotate()
    }

    state rotate(&mut self) {
        relax self.tree {
            Tree::rotate_raw(&mut relaxed self.tree);
        }

        transition {
            _ -> done()
        }
    }

    state done(&mut self) {
    }
}
```

The relaxed region completes before control moves elsewhere.

## Calls Inside Relax

Nested machine calls are fine when the callee contract matches the relaxed
state.

```omega
machine Tree::rotate_raw(root: &mut relaxed NodeId) {
}

machine Tree::rebalance(&mut self) {
    relax self.root {
        Tree::rotate_raw(&mut relaxed self.root);
    }
}
```

But this is rejected:

```omega
machine Tree::rebalance(&mut self) {
    relax self.root {
        self.inspect(); // illegal if inspect expects normal Tree invariants
    }
}
```

Inside the relax scope, `self.root` is both weakened and exclusively borrowed.
Any call that can observe `self.root` under normal invariants is not valid.

## Multiple Relaxed Targets

Multiple targets may be relaxed only when borrow checking proves they are
disjoint.

```omega
machine Tree::swap_children(&mut self) {
    relax (self.left, self.right) {
        Tree::swap_raw(&mut relaxed self.left, &mut relaxed self.right);
    }
}
```

If the compiler cannot prove disjointness, the relax is rejected.

## When To Use Relax

Use temps first.

Relax is for exclusive in-place mutation:

- Large structures where rebuilding a temporary is not reasonable.
- Tree/list/hash-map rotations that temporarily break representation
  invariants.
- Allocator internals.
- Partial initialization of buffers or aggregate storage.
- Platform calls that fill a structure in phases.
- Low-level algorithms where stable identity or address matters.

If relax feels necessary for ordinary arithmetic, scalar field updates, or
simple branching, the code should probably use a temporary value instead.
