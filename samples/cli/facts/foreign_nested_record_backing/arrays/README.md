# Retained array fields in original nested backing

Parent: [NORMALIZED-ABI-LOWERING](../../../../../TASKS.md); continuation of
[the nested record fact](../README.md).

A mutable foreign registration retains the exact original nested array Buffer backing until completion consumes Pending; completion writes its middle cell to 73 while marker 91, neighbors 305419896 and 1515863100, and an earlier copy 37 remain unchanged.

`Buffer` owns three copyable `Cell` records in a fixed array. The actual C
provider receives the original complete allocation, writes the middle element
to 41 during registration, retains its pointer, and writes 73 at completion.
The receiver checks both adjacent same-width cells and marker 91 after completion,
while the earlier scalar copy remains 37. `Pending<'source>` retains the exclusive
loan until completion; each invalid twin reads that same middle cell while the
registration is live and must reject.

The nine places transport the same pending claim and its original array backing:
[local](local/main.omg), [field](field/main.omg),
[parameter](parameter/main.omg), [call result](call_result/main.omg),
[match join](match_join/main.omg), [const name](const_name/main.omg),
[nested machine](other_state/main.omg), [another machine](other_machine/main.omg),
and [target body](target_body/main.omg). Every place has an `invalid/main.omg`
twin and pinned copies under `tests/omega/{pass,fail}/facts/foreign_nested_array_backing_*`.
The const place materializes a copyable array, then registers its owned runtime
copy; the retained pending occurrence is not a constant. The match place invokes
both incoming branches with distinct route tags, each checked after completion.
All positive builds declare the four hosted targets.

At fixed source `8fcac034aeb21b0c476a3e76deed4de471797f9b`, optimized producer
SHA256 `3bdb9da9c546e7f1bae412e43806d807bbfdb59564990b27eeeeb1ae1166029d`
checks the local witness and executes it natively at zero. Public `run --both`
returns zero while explicitly reporting Terminal `DECLINED`:
`native embedded field needs a supported materialized record codec`.
That observation establishes the missing codec, not engine agreement.

The repair walks the finite semantic record/array graph and joins every scalar
occurrence to the existing retained repeated-layout coordinates. It preserves
original allocation and loan identity; geometry supplies no ownership or domain
authority. Existing primitive codec validation, full roster matching, overlap
checks and rejection of tags, opaque regions, references and other unsupported
leaf kinds remain in force. Provider lifecycle traps and C layout assertions
exercise actual foreign behavior, not an interpreter token model.

Optimized macOS producer `4d336559156` (SHA256
`111d3e944aed98fbb3855790680e2fcbb555d5a95643d3fbdef8ad73ab08424e`)
executes all nine corrected sample places and pinned copies at native/Terminal
zero; all 18 live-loan observations reject for their authored reason. The const
place declares its scalar leaf record `[copy]`; its earlier affine-default
constant was invalid source, not a compiler refusal of a valid constant.

The runtime match additionally required the ordinary owned qualification transfer
at the selected-value join. Each covered arm rebases the qualification on its
actual completed value, and only identical subject, domain instance and authority
survive. Branch-private contexts publish nothing for an incomplete result roster;
no receiving declaration establishes membership. This follows
[domain transport](../../../../../wiki/spec/language/domains.md) and
[pattern joins](../../../../../wiki/spec/language/patterns.md).
An independent [issued-record control](../../../../../tests/omega/pass/domains/runtime_completed_routed_record_join_exit/main.omg)
executes both runtime arms with distinct values. Its
[unissued-arm twin](../../../../../tests/omega/fail/domains/completed_routed_record_join_unissued_arm/main.omg)
changes one issuer call to an equal-valued bare constructor and rejects at the
qualified consuming call, without a type or ownership error.

Other-host execution of this continuation remains pending. Raw baseline sources,
producer/profile receipts, failed setups, provider self-tests and outputs are retained in
`/Users/zcanann/Omega-build/retained-array-backing-2026-10-27/`.
This row does not establish pointer graphs, tagged/dynamic carriers,
independently qualified descendants or owned aggregate result publication.
