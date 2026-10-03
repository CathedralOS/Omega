# Mutable local array snapshot

After ordinary package preparation:

```sh
cargo run -p omega -- run --both tests/omega/pass/structs/runtime_local_array_snapshot_exit/main.omg
```

The record's first operand captures the owned `[30, 40]` array. Its second
operand borrows the original local and changes element 0 to 90. Exit 70 requires
both the earlier record snapshot and the later source mutation, including the
unchanged neighboring element. It must not alias the copied array to its source
or reorder the effect ahead of capture.

The focused corpus acceptance is:

```sh
python3 tools/corpus_gate.py --both --filter structs/runtime_local_array_snapshot_exit
```

On macOS ARM64 with compiler base `a0af6064e4c`, native and checked-interpreter
execution both return 70 and agree. The field-backed control
`structs/runtime_array_member_snapshot_exit` retains the same result. This
capability was already implemented; the fixture pins its composed behavior.

The ordinary CLI command above returns exit 200 in the fresh checkout because
package acceptance is missing or current requirements need review. Complete
the generated review through `omega update --project
tests/omega/pass/structs/runtime_local_array_snapshot_exit --target macos_arm64`
before retrying. Corpus fixture admission does not approve those decisions.

`fail/borrows/local_array_borrow_operand_read` keeps the independent call
operand fence: an earlier completed array copy does not permit reading the
original through a later operand while its exclusive borrow is held.
