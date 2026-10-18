# Indexed storage address identity

For runtime u64 index < 2 into live [u64; 2], compatible references to
items[index] expose the same nonzero addr and differ from the other live
element address.

The original root probe is preserved. Each placement runs indices 0 and 1;
`match_join` also runs both arms. The query remains `address_of(&items[index])`
with the same strict bound in local, field, parameter, call_result, match_join,
const_name, other_state, other_machine, and target_body. Every positive compares
repeated exposure and the selected and other live element addresses. Expected
exit: 0 in native and Terminal execution.

Each `<place>/controls/held_element` twin retains an exclusive reference to the
selected element across the query, then writes through it. Checking must reject
shared-reference formation at the query because `held` remains active. The
bounds and initialization stay unchanged. `controls/missing_bound` independently
rejects the query's runtime selector when only `index <= 2` is known;
`controls/raw_address_to_borrow` rejects using numeric address data as a reference
with a different referent carrier.

Run `omega run --both samples/cli/facts/indexed_storage_address_identity/local/main.omg`
and the other eight placements. The target-body placement retains Linux x86_64,
Linux AArch64, macOS AArch64, and Windows x86_64 bodies. Checking covers all four;
execution evidence requires a matching host and currently covers Linux x86_64.

This bounded slice supports readable primitive scalar referents, including
borrowed fields and runtime-indexed elements. Aggregate referents, write-only
queries, and selected-machine addresses remain explicit unsupported frontiers;
the ADDRESS-EXPOSURE board item remains open.
