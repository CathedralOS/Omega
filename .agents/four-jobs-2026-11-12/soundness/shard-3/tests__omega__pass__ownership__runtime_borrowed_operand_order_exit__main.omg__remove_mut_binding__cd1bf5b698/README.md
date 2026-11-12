# Scalar evaluation before borrowed extraction

```sh
omega run --both tests/omega/pass/ownership/runtime_borrowed_operand_order_exit/main.omg
```

Both engines exit 0. The first scalar argument observes 41 before the later
owned argument vacates that field. The returned token restores the same field,
and its marker sibling remains 19. The token's drop hook clears its old value,
so replacement cannot dispose the installed result as the displaced owner.
The reversed argument order is rejected by `borrowed_operand_later_read`.
