# Reference record completion

From the repository root, prepare the fixture with `omega update --project
tests/omega/pass/borrows/runtime_reference_record_call_completion_exit`, reviewing
local package changes if requested. Then run:

```text
omega run --both tests/omega/pass/borrows/runtime_reference_record_call_completion_exit/main.omg
```

Both engines must return `70`. The stored mutable reference is used through a
shared record argument, then retired before the original owner is mutated. Both
direct completion and a selected successor must preserve scalar siblings until
frame completion. Exhausting resource custody does not erase initialized storage.
