# Keep reading the same data after a call that may pause

The compiler now accepts these programs, which keep reading the same data after
a function call that may pause. Each changes how it views the data without
making a copy; data forbidden from surviving a pause still rejects.

The program reads a single-number buffer as a record with one field, without
copying the data. It saves a reference—a way to access that existing data—before
the call. Changing that reference to another compatible view is called a recast.
The compiler must remember that both views still use the original storage and
its restrictions.

Each example stores 73, makes the call, and checks that reading the data still
gives 73. The compiler also follows the helper's final call before releasing its
local data. When another return leaves the data unused, cleanup identifies the
value still occupying its storage, even after its last read has finished.

This is another production of the parent fact, under
`STATIC-MACHINE-MIGRATION` in [TASKS.md](../../../../../TASKS.md). It uses the
same fact sentence, `Message` fields, consuming read, canaries, and rejection
obligation as the [direct-loan production](../README.md). Each cast starts from
an already stored reference, rather than an explicit borrow expression. The
cast changes the checked view of that storage; it grants no new lifetime,
access, or suspension permission.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [source](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| constant | [source](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [source](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| other machine | [source](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| target body | [source](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each positive binds all four hosted targets and declares exit 0. Every twin
changes only `Backing`'s suspension policy to forbidden. The backing stays live
through the recast even though no source statement reads it after suspension.
The match place executes both choices, observing 73 and 11; the target-body
place checks the distinct selected implementation tag. `read` returns zero if
either neighboring field changes, and the entry traps on any wrong observation.

The [recast contract](../../../../../wiki/spec/layouts/recasts.md) requires
compatible geometry and representation validity while preserving address,
authority, provenance, and lifetime. The [carry contract](../../../../../wiki/spec/resources/carry.md#borrows-across-suspension)
requires the original referent, parent loans, access restrictions, and live
resource demands to survive the call. These are established separately: equal
sizes or the target record's default carry policy supply no authority.

At fixed base `3009cab5a41` on macOS ARM64, the optimized compiler accepts the
direct-loan local place and an explicit-borrow recast. The otherwise equivalent
stored-reference local place rejects at `park`: "the loan lifetime resource is
unavailable". Its twin also reports the original `backing`'s forbidden carry
policy. These results were captured before the compiler repair.

On macOS ARM64, the optimized candidate runs all nine positive places with
matching native and Terminal Psi results. All nine twins reject for the
original backing's forbidden suspension policy. A [separate regression fixture](../../../../../tests/omega/pass/borrows/recast_suspension_branch_cleanup_exit/main.omg)
executes both returning branches: one reads 73 through the saved reference, and
the other leaves the record unused and returns 19.

The recorded producer is source `fedee10e523` plus compiler patch SHA-256
`1807520ca9f0be4e3d3fddf3b8a40183f64055d9a4e36f6934e7635ba345f8c8`,
binary SHA-256 `3aa99c3a3a3c7df9dc3fcf7eb2549e63d17a6fdd47568462720226840bea288f`.
The 18-project sample gate took 72 seconds. Source, producer/profile records,
the patch, independent read-only review and raw branch observations are retained
under `build/recast-loan/completion-4/` and `build/recast-loan/control-evidence/`.
Later comment changes clarify the examples without changing their operations or
oracles.

Copied or projected source descriptors still fail to supply the required
retained permission; joined source permissions and other derived lineages remain
under the parent task. This row does not establish product-compiler parity or
complete that task's acceptance.

The inline scheduler can establish retention across a possibly suspending call;
it does not demonstrate an actual parked activation or another host's runtime.
