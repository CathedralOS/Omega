# Returned write destination

A returned write-only loan permits replacing caller storage 7 with 42 while
preserving neighboring 19. Every place repeats that fact and exits 0 only
when the caller observes both values after the loan ends.

The original `main.omg` remains the starting witness. The nine places are:

| Place | Project | Transport |
| --- | --- | --- |
| local | `local/` | returned descriptor bound locally |
| field | `field/` | returned descriptor stored in a record field |
| parameter | `parameter/` | returned descriptor passed as a parameter |
| call result | `call_result/` | returned descriptor forwarded by another call |
| match join | `match_join/` | either arm returns the same caller destination |
| const name | `const_name/` | named constant selects the caller array element |
| another state | `other_state/` | returned descriptor becomes a successor parameter |
| another machine | `other_machine/` | a forwarding machine calls the replacement machine |
| another target body | `target_body/` | each target body returns the caller destination |

Run `omega run --both samples/cli/facts/returned_write_destination/<place>/main.omg`
with finite stdin. Each place has a `controls/read_destination/` reject twin.
The twin changes only the store from `destination = 42` to
`destination = destination` (the field case uses `destination.loan`),
requiring prohibited observation of the same write-only referent.
Check it with `omega --check <control>/main.omg`; it must reject for write-only
access, rather than a package, provider, or native-production failure.

The const name identifies a destination projection, not a permission carrier:
constants have no stable address and grant no authority. Its returned loan
still originates in caller storage. All target bodies are source checked;
native execution establishes only the matching host target.

The initial Linux x86-64 check found two compiler gaps before repair: the
match-join ran natively but Terminal reported `AffineFrontierMismatch`, and
the target-body family was incorrectly treated as opaque write-only supply.
The positive programs retain their original shape across those repairs.

Linux x86-64 validation runs every place with `--both`: native and Terminal
exit 0, and every twin rejects with one write-only observation diagnostic.
The matching fact fixtures are under `tests/omega/{pass,fail}/facts/`.
Focused corpus selectors are `returned_write_destination` for the row and
`write_only_target_family,write_only_checked_adapter` for non-host checking,
opaque-member rejection, and checked-adapter call-closure rejection.
