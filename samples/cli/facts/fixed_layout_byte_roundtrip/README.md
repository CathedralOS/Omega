# Guarded fixed-layout byte roundtrip

A byte roundtrip through an explicit fixed layout lends a decoded view only after its payload bound is established, preserving every field and neighboring storage.

This row belongs to [LIBRARY-REPRESENTATION-FAMILIES](../../../../TASKS.md).
The [local program](local/main.omg) initializes a sparse 40-byte
`LayoutStorage<Payload, Spread>` inside five words of a seven-word array, copies
the complete byte region, and lends the decoded view after proving `second >= 42`.
Its consumer checks all three fields and both source/destination canaries.
The [static reject twin](invalid_local/main.omg) changes only the guard to `true`;
it rejects the attempt to publish an unestablished valid view.

The [rejected-payload control](local/rejected_payload/main.omg) writes 41 with the
same bound, layout and byte operation. It must take `None` and exit 0; the
original program must take `Some` and exit 0. Together they detect a missing or
inverted guard, while the field values and canaries detect incorrect storage
selection. An exclusive initial view exposes independently valid fields without
assuming its record predicate; escaping mutable authority also owes the original
occupied storage, including padding.

The local pair and rejected-payload control are qualified on Linux x86_64 with
native/Terminal agreement. [Run 41387515883](https://github.com/CathedralOS/Omega/actions/runs/41387515883)
also checks seven intended rejection controls and reloads the retained Terminal
artifact using the matching optimized Rust producer at `70ba9ee572b4`.
The [typed interior-region prerequisite](../fixed_region_byte_roundtrip/README.md)
is landed.

The [field program](field/main.omg) moves the decoded optional into `Saved.value`
before matching and consumption. It runs both payloads 42 and 41 and checks 1 and 0
plus every field and canary; its twin removes only the decoder guard.
[Run 41412055228](https://github.com/CathedralOS/Omega/actions/runs/41412055228)
qualifies the field pair and ordinary samples on Linux x86_64, checks the local
roundtrip and nested-result loan control, and records native/Terminal agreement
with optimized producer `cfcadebbcc85`.

The [parameter](parameter/main.omg), [call-result](call_result/main.omg), and
[match-join](match_join/main.omg) programs carry the same guarded view through a
typed consumer, a returned `Saved` record, and a join of direct construction with
a helper result. Each runs payloads 42 and 41, checks results 1 and 0 and every
field/canary, and has the same guard-only rejection twin.
[Run 41422238131](https://github.com/CathedralOS/Omega/actions/runs/41422238131)
qualifies those three pairs and ordinary samples on Linux x86_64, with three
native/Terminal exit-0 agreements and the nested-result loan rejection control.
The optimized Rust producer is `dd84cfd469ed`; its returned-loan joins use the
same exact applied-owner correspondence as local stored descriptors, preserving
root, site, access, lineage and complete-capture requirements.

The [const-name](const_name/main.omg), [nested-machine](other_state/main.omg),
[other-machine](other_machine/main.omg) and [selected-target](target_body/main.omg)
places complete the row. Constants carry fully initialized source representations
as ordinary copied values; runtime materialization owns their backing. A real
terminal handoff carries `Saved` and the independently read canaries into the
nested machine. Build chooses the named decoder, with marker17/29 independently
checking the selected body.
[Run 41436633328](https://github.com/CathedralOS/Omega/actions/runs/41436633328)
qualifies those four pairs and ordinary samples with optimized producer
`e1c62c271d9e`: eleven checking observations, four native/Terminal exit-0
agreements, and eight ordinary sample observations. The three existing escape
controls still reject. Retained-home analysis follows the common concrete
invocation graph and each callee's actual machine, preserving complete operand
captures, escape checks and the ban on re-establishing a still-borrowed home.

All nine places have Rust-reference Linux x86_64 observations at the recorded
producer epochs above. Earlier places retain their original accepted records;
this run adds only the final four. Other hosted execution, the Omega-written
producer and the broader representation-family requirements remain open.

[Run 41442508227](https://github.com/CathedralOS/Omega/actions/runs/41442508227)
qualifies all nine places and the rejected-payload control on matching Mac
ARM64 with optimized Rust producer `fddd05d57572`. All ten canonical positives
agree at native/Terminal exit 0; all nineteen source checks and nineteen
ordinary sample observations meet their original oracles. The named ARM decoder
reports marker29, exercising the selected body and ARM calling convention with
the unchanged backing, fields and canaries.

[Run 41450602544](https://github.com/CathedralOS/Omega/actions/runs/41450602544)
qualifies the same nine places and rejected-payload control on matching Linux
ARM64 with optimized Rust producer `463f7229129b`. Ten native/Terminal exit-0
agreements and nine guard-only refusals meet the original oracles. The exact
canonical inputs match the previously qualified ordinary samples, so this host
run does not repeat the sample sweep. Producer preparation took 1,095.736 seconds;
the selected source checks and native/Terminal executions took 182.139 seconds.

Linux x86_64, Linux ARM64 and Mac ARM64 records retain their actual producer
epochs. The product producer and broader representation-family acceptance
remain open. This run selects this single fact row on one additional host.

[Run 41480419172](https://github.com/CathedralOS/Omega/actions/runs/41480419172)
requalifies the Linux x86_64 row with optimized Rust producer
`dcd2509c66fa4717e28a0b781c10f44fe6a3100d` after repairing target-local loan
retirement. A descriptor read inside a selected transition target used to retain
one activation without its exit weakening after restoring the guard context.
The selected exit now retires only its newly activated, state-owned statement
resources through the existing execution-order expiry rule; captured exit facts
and guard continuations retain their original meaning. The strict resource
verifier remains unchanged.

All 22 selected source checks meet their original oracles, including the nine
guard-only twins, nested-result loan control and overlapping typed-region
control. All eleven positives agree at native/Terminal exit 0: the ten guarded
row programs and the typed-region parameter prerequisite. Source programs,
bounds, storage and runtime oracles are unchanged. Producer preparation took
174.151 seconds; checking took 160.249 seconds and execution 103.964 seconds.
The CLI and runner hashes match the preceding single-fixture acceptance, and
unchanged ordinary sample observations are retained rather than repeated.

[Run 41481468044](https://github.com/CathedralOS/Omega/actions/runs/41481468044)
qualifies the unchanged nine-place row and rejected-payload control on matching
Windows x86_64 with optimized Rust producer
`5ee8ffe84a9c497e989f9016f01f2536da4bfabf`. All ten positives agree at
native/Terminal exit 0, and all nine guard-only twins reject for the original
unestablished recast target. The 112 frozen input files match the prior row;
the producer includes the selected-exit retirement repair now published in
[#1478](https://github.com/CathedralOS/Omega/pull/1478). Actual Windows producer
identity is retained rather than relabeled as the Linux qualification.

Producer preparation took 785.359 seconds, source checking 135.125 seconds and
native/Terminal execution 97.000 seconds. This matching-host qualification
records only its ten Windows observations and reuses the unchanged ordinary
samples. The Rust-reference row now has observations on all four hosted targets;
the Omega-written producer and broader representation-family requirements
remain open.
