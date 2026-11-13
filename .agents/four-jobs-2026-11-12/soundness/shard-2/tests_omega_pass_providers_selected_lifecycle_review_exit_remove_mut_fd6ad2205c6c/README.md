# Review an implementation that delegates memory access

A selected implementation initializes a word by calling another implementation
directly. Package review must keep those two calls distinct: selecting the outer
implementation does not select it for every checked application of the helper.

The program allocates real memory, reads the initial value 17 through a borrowed
view, writes 29, and extracts the value. It checks that adjacent fields retain
101 and 103. Both native execution and the Terminal Psi interpreter must exit 0.

The baseline compiler at `5c37ebfec51ea57cd758fd92e57481c706b531da` already runs
this program successfully. However, ordinary `omega update --project <this
directory> --target macos_arm64` rejects it because review attaches both
implementations to the same closed requirement. Validate that update command
as well as `omega run --both main.omg`; execution alone cannot detect this bug.

The selected receiver includes all its type and lifetime arguments. Review uses
the same exact match as native selection before counting competing implementations.
The existing [ambiguous provider control](../../../fail/providers/ordinary_provider_call_graph_ambiguous/main.omg)
and [duplicate selection control](../../../fail/build/provider_package_default_duplicate_rejected/main.omg)
must continue to reject.
