# Checked predicate membership on an array copy

Copying a four-u16 array carries its checked zero tail, so exact addition to
65535 stays 65535.

This is a prerequisite of `FILESYSTEM-PLATFORM-SERVICES`: the Windows raw
metadata provider terminates a wide-character buffer before passing its
qualified copy to another state. It is not a Windows-specific language rule.

The same fact and consuming operation travel through `local`, `field`,
`parameter`, `call_result`, `match_join`, `const_name`, `other_state`,
`other_machine`, and `target_body`. Each place has an `invalid/` twin whose
tail changes from zero to one. No caller guard re-establishes the fact before
the exact addition. Each positive binds all four hosted targets and expects
exit 0 from both native and Terminal execution.

Run a place with `omega run --both --keep <place>/main.omg`, then independently
consume its saved `program.oterm` using `omega inspect-terminal --artifact
<saved>/program.oterm`. The second command matters: execution alone did not
detect the canonical reader's stricter copy-membership rule.

| Requirement | Evidence |
| --- | --- |
| Establishment | A completed initialization call is followed by the exact tail store; the qualified capture must establish `ZeroTail`. |
| Identity and lifetime | The copy is a fresh unrestricted occurrence, not a qualification added retroactively to the initializer's contract. Separate fixtures check mutation before capture rejects and mutation after capture preserves the copy. |
| Composition | Nine transport places retain the same predicate and exact u16 addition. |
| Soundness boundaries | Each one-tail twin rejects; routed origin is not obtained from equal bytes. |
| Psi 05–07 | Checked memberships survive proof erasure, canonical serialization and separate consumption; Terminal execution returns the authored result. |
| Omega 00–02 | Hosted Build bindings select the entry; fixed-array geometry and qualified aggregate argument/result placement remain ordinary plans. |
| Omega 03–06 | Native array copies, calls and state transfers preserve payloads and independent storage; the exact addition is realized in u16. |
| Omega 07–09 and emission | The host image executes with exit 0 and agrees with Terminal execution; this does not establish native execution on an unavailable host. |

The regression fixtures mirror this row under
`tests/omega/{pass,fail}/facts/predicate_array_copy_through_*`.
Additional controls under `tests/omega/{pass,fail}/domains/copied_array_domain_*`
challenge source mutation and issuer provenance. These programs establish only
the copy/predicate mechanism, not the complete Windows filesystem provider or
its owner-deferred Windows native acceptance.
