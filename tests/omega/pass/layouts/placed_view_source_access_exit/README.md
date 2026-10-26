# Installed placed-view source access

This is the executable customer for **PLAN-LAID-VIEWS**. Its application entry
receives the exact externally installed `Registers` allocation, forwards the
exclusive placed loan to ordinary helpers, reads status 41, writes transmit 9,
and returns normally so that the entry loan retires.

`entry_ram.txt` supplies `2900000003000000`; `expected_entry_ram.txt` demands
`2900000009000000` from the original backing after return. The native and
Terminal observations are checked independently, in addition to both exits.

```text
python3 tools/corpus_gate.py --both --filter placed_view_source_access
```

The [permission twin](../../../fail/layouts/placed_view_source_access_denied_write/main.omg)
removes transmit write permission while retaining a readable External field,
so plan formation remains valid and the authored write itself must refuse. Existing
`placed_reference_results` controls reject substituted backing, stale borrowed
storage and overlapping loans; `placed_policy_requires_exact_attached_owner`
rejects a substituted plan owner. The `installed_entry_storage` fact and its
controls exercise occurrence identity, missing backing, initialized extent,
padding, overlapping exclusive uses, and independent original-backing readback.
The `placed_stable_write` row exercises all nine subordinate transports and
their permission twins. These exercise private hosted RAM, not device-provider
or Atomic establishment.
