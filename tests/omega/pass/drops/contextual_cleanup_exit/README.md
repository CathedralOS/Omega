# Contextual cleanup through an ordinary helper

Prepare the project with `omega update --project tests/omega/pass/drops/contextual_cleanup_exit --target macos_arm64`, substituting the host target as needed.
Run `omega run --both tests/omega/pass/drops/contextual_cleanup_exit/main.omg`.

Both engines must print exactly `BA` followed by a newline and return 0.
The corpus command `python3 tools/corpus_gate.py --both --filter drops/contextual_cleanup_exit`
checks both output streams; the CLI's exit-code agreement alone is insufficient.
The consuming helper disposes two owners in reverse parameter order. Each hook
requires the current owner's ready flag, prints its own byte, and clears the flag.
The caller's newline occurs after both hooks return. No hook exits the process.

The fixture owns its `Console` boundary and explicitly declares `terminates`
for `write_byte`: finite, normal completion of each byte write is an external
provider assumption in this test. Package review must admit the exact owned
boundary schema, provider plan, and output permission. The assumption does not
come from the name `Console`, and the standard library's unqualified output
contract remains unchanged. An ordinary output requirement without that
finite-progress promise cannot justify automatic cleanup.

The unused `exit_process` requirement and provider leaf retain the existing
Console role nomination and review route. Neither cleanup nor the entry calls
it; successful execution returns normally after the newline. Provider source
uses the existing macOS ARM64, Linux x86-64, Linux ARM64, and Windows x86-64
routes. Validate on the selected host; target declarations are not evidence
that every host has been executed.
