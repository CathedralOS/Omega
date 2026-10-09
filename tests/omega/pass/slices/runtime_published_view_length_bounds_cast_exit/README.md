# Published view length bounds

The helper returns the middle two bytes of a four-byte array and proves its
published length equation. The caller uses that equation to form an Exact
`u64` to `u8` cast, then checks the observed length. Native and Terminal
execution must both exit 0.

Run `python3 tools/corpus_gate.py --both --filter runtime_published_view_length_bounds_cast_exit`.
The companion `fail/slices/result_length_contract_requires_returned_view`
rejects an unproved length promise. `unbounded_length_increment_requires_fit`
keeps an unknown descriptor length from evading `u64` overflow checking.
