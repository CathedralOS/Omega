# Hosted-extent occupancy for a receiver's nominal cleanup

Prepare the project with `omega update --project tests/omega/pass/drops/contextual_receiver_drop_exit --target macos_arm64`, substituting the host target as needed.
Run `omega run --both tests/omega/pass/drops/contextual_receiver_drop_exit/main.omg`.

The entry context `main: &mut Main` carries `machine Main::drop`, so its data
requires nominal cleanup. A retained receiver with that obligation stays
eligible: its cleanup is classified as hosted-extent occupancy and tracked
through the binding, rather than refusing the entry. The program prints a
newline and returns 0.

The occupancy marker is ledger custody, not emitted cleanup: the bridge does
not yet invoke the receiver's drop on normal return, so `main.byte` (67, `C`)
is not written today. When cleanup execution lands, this fixture's expected
stdout becomes `\nC` and the drop's byte must appear.

The fixture owns its `Console` boundary under the same reviewed provider
assumption as `contextual_cleanup_exit`: `write_byte` declares `terminates`,
and the unused `exit_process` leaf retains the existing role nomination.
