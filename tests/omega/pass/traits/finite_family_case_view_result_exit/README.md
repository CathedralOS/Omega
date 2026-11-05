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
out; they supply no rejection evidence.

[The final Linux source](https://github.com/CathedralOS/Omega/actions/runs/41975158564)
at `f37c9cfc83e25c784206b0b60fb5d829f5295499` also checks and runs this program,
including ordinary package review and native/Terminal exit 0.
[Linux ARM](https://github.com/CathedralOS/Omega/actions/runs/41973539242) passes
with its original compiler and program source `555bdbb56368426ed961fd76f613a32315ab44f1`.
[Windows](https://github.com/CathedralOS/Omega/actions/runs/41973546386) and
[macOS](https://github.com/CathedralOS/Omega/actions/runs/41973543038) pass at that
same original source. Independent review accepted the raw evidence and found
that the final refactor preserves these admitted programs' emitted contracts.
No other-host execution at the final Linux source is claimed.
