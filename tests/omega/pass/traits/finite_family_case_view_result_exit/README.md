A finite-family call returns a tagged byte view into caller-owned storage.
The case must match the selected width, and reading the returned view must keep
both endpoint bytes from the selected caller input. Both widths and both
caller arrays execute.

Owning task: **FINITE-GENERIC-DISPATCH** in [TASKS.md](../../../../../TASKS.md).
This is a focused probe for the remaining returned-case requirement; it does
not close the parent task or replace the nine transport places in
[the descriptor fact](../../../../../samples/cli/facts/finite_family_returned_descriptor/README.md).

The published Linux compiler `3fdbcaba2c8fea5695bbb0336adb0ce473e0eaef`
refuses this program during lowering because separate calls to the same method
instance have different private symbols. Neither engine runs in that
[baseline](https://github.com/CathedralOS/Omega/actions/runs/41967472108).

The repair compares each call's complete public contract before sharing a
runtime dispatch row. With compiler `44f1471e7e5f9e96599b8d0f68dd8b11d053f687`,
[this program checks, completes ordinary package review and runs at native/Terminal exit 0](https://github.com/CathedralOS/Omega/actions/runs/41968764734).
That whole report remains failed because two unchanged Std-based controls time
out; they supply no rejection evidence. Other-host qualification is pending.
