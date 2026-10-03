# Contextual cleanup across generic consuming calls

Prepare the project with `omega update --project tests/omega/pass/drops/contextual_core_drop_exit --target macos_arm64`, substituting the host target as needed.
Run `omega run --both tests/omega/pass/drops/contextual_core_drop_exit/main.omg`.

Both engines must print exactly `BA` followed by a newline and return 0.
The corpus command `python3 tools/corpus_gate.py --both --filter drops/contextual_core_drop_exit`
checks both output streams as well as the exit oracle.

The entry first transfers the B owner into ordinary core `drop`. It then
transfers the A owner through `forward<T>` into the unrelated generic consuming
helper `retire<T>`. Each death edge invokes the exact `Token::drop` hook with the
current owner's `ready` prerequisite; the forwarding body must not also dispose
its moved value. Generic substitution carries the contextual cleanup obligation
through both call chains. No rule may depend on the spelling of core `drop`,
and neither generic helper assumes an undeclared Boolean prerequisite.

The hook prints its owner's byte and clears its ready flag. The entry's newline
follows both completed cleanups. Neither hook nor entry exits the process.
The fixture retains its own reviewed `Console` boundary, whose `write_byte`
explicitly promises finite termination; this external provider assumption does
not strengthen the standard library's output contract. Its unused exit leaf
retains the existing Console role nomination route. Review must admit the exact
boundary schema, provider, and output permission.

The provider and build declarations retain the existing macOS ARM64, Linux
x86-64, Linux ARM64, and Windows x86-64 routes. Run on the selected host; these
declarations do not establish execution coverage for other hosts.
