# drop_lifetime_owner

A drop hook on a lifetime-parameterized owner: `data Owner<'first, 'second>` holds two borrowed `i32` fields with distinct lifetime binders, and `Owner::drop(owner: &mut Owner<'first, 'second>)` re-declares them so the cleanup edge selects the hook for any instantiation.

## Learnings

- The drop hook re-declares the owner's lifetime binders in its signature — the hook is generic over the same lifetimes as the type.
- Borrowing locals into a record and `drop`-ing the record works: the borrows die with the record.

Compile-checked only: run a check target, not native.
