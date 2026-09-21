# RC-REPRESENTATIVE-PROGRAMS-CLOSURE — re-verification ledger

Status: narrow re-verification of the representative-programs matrix row —
most recent probe at `dd4e53e91a` (linux x86-64, cargo — no mbx). Gate
remains OPEN (red); the two probed single-test legs confirm the recorded
failure families are unchanged.

## Probe at `dd4e53e91a`

```text
cargo nextest run -p compiler --test samples_compile \
  -E 'test(=sample_entry_exceptions_are_explicit_and_non_runnable) \
      or test(=named_integer_conversion_samples_reach_checked_trees)'
```

| leg | result | notes |
| --- | --- | --- |
| `sample_entry_exceptions_are_explicit_and_non_runnable` | PASS | stays green — the `cli__device__device_extent_access` authored-root pin still holds |
| `named_integer_conversion_samples_reach_checked_trees` | FAIL (111.1s) | verbatim residual — `cli/basics/print_number` at statement 19: "cannot prove default-domain field requirement for return from Main::main: self.out requires [u8; N]::Utf8" — unchanged from `20a7d1332c`/`0f75a052f0` |

## Prior probe at `0f75a052f0`

## Probe

```text
cargo nextest run -p compiler --test samples_compile \
  -E 'test(~named_integer_conversion_samples) or test(~sample_entry_exceptions)'
```

| leg | result | notes |
| --- | --- | --- |
| `sample_entry_exceptions_are_explicit_and_non_runnable` | PASS | stays green — the `cli__device__device_extent_access` authored-root pin (closed via ENTRY-CONTENT-ROOTS) still holds |
| `named_integer_conversion_samples_reach_checked_trees` | FAIL (105.9s) | verbatim residual — `cli/basics/print_number`: "cannot prove default-domain field requirement for return from Main::main at statement 19: self.out requires [u8; N]::Utf8" — unchanged from the `20a7d1332c` reading |

## Residual lanes (unchanged)

- print_number `[u8; N]::Utf8` domain-field leg — re-witnessed verbatim above
- windows_x86_64 `named-callable(WindowsProcessEntry::enter)` schema rejection
  of std `targets/windows_x86_64/entry.omg`
- ProgramEntry establishment "rejoins 0 Terminal attachment identities"
  (fletcher_checksum + algorithms cohort; same moved-failure family recorded
  under DIVISION-CANARY-ENTRY-BINDING at `f501d377d8`)
- `CheckedOperator` occurrence audit residual (t2c `authored_selections` —
  PROOF-SAMPLES-CHECKED-CALL-SELECTION holds that surface this wave)
- GUI `Fused provider for boundary {Clock,Input,Gui,FilesystemHost}` legs —
  the same compiler-side Fused provider-join regression recorded by
  `geometry_native_f7212dc016.md` this wave
- non-linux host legs host-gated (macOS/Windows/QEMU unavailable here)

Closure stays gated on those lanes; the full-suite re-run is bounded out
(aggregate legs measured >47min at `9e3edc7be9`, >130min for the
documented-exit run leg at `edc77c21480`). Sibling re-mines:
RC-REPRESENTATIVE-PROGRAMS-GATE, -GREEN, -PER-HOST.
