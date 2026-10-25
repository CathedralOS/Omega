# Copyable opaque carrier exchange

Parent: [NORMALIZED-ABI-LOWERING](../../../../TASKS.md), including the retained
OPAQUE-BY-VALUE-BOUNDARY-ABI requirement. This row exercises checked semantic
copying of an owned, closed, copyable Inert opaque across an independently
compiled producer and consumer. It does not establish linear, qualified or
view-backed input support or matching-host execution on other hosts.

**Checked semantic copy of a closed copyable Inert carrier preserves libc host
66051 on the retained original and on the component input, and preserves sibling
host 513.**

Each place copies that sentence and uses the same
`AddressCarrier<AddressCarrier<AddressWord>>` selection. The producer calls the
actual classful `inet_makeaddr` and `inet_lnaof` exports; the consumer cannot
inspect or manufacture the selected carrier. It copies the first result, creates
a disjoint sibling, then observes all three by-value arguments through the
independent component. Both engines must exit zero. The comparison observes the
results and does not establish a source proof fact.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local](local/main.omg) | [copy required](local/invalid/main.omg) |
| Field | [field](field/main.omg) | [copy required](field/invalid/main.omg) |
| Parameter | [parameter](parameter/main.omg) | [copy required](parameter/invalid/main.omg) |
| Call result | [call result](call_result/main.omg) | [copy required](call_result/invalid/main.omg) |
| Match join | [both incoming arms](match_join/main.omg) | [copy required](match_join/invalid/main.omg) |
| Const name | [named construction inputs](const_name/main.omg) | [copy required](const_name/invalid/main.omg) |
| Nested machine | [owned arrivals](other_state/main.omg) | [copy required](other_state/invalid/main.omg) |
| Another machine | [owned forwarding](other_machine/main.omg) | [copy required](other_machine/invalid/main.omg) |
| Target body | [four checked bodies](target_body/main.omg) | [copy required](target_body/invalid/main.omg) |

All positives bind the four hosted `ProgramEntry` targets. The const-name place
names the same runtime construction inputs; foreign-created opaque occurrences
are not compile-time constants. The producer and [reject producer](producer_invalid/main.omg)
differ only by removing `[copy]` from `InternetAddress`. Every reject consumer
keeps its positive source byte-for-byte and fails because it uses the original
after transferring that affine value to `sent`. The expected diagnostic contains
`was transferred or consumed` and `it holds no value`. The eighteen copies are
pinned under `tests/omega/{pass,fail}/facts/opaque_copyable_carrier_through_*`.

The applicable [requirements and evidence](../../../../AGENTS.md#requirements-and-evidence)
are:

- **Establishment:** completed foreign calls establish exact runtime values.
  The selected closed representation is structurally copyable and Inert under
  [opaque lifecycle and movement](../../../../wiki/spec/build/opaque_representations.md#lifecycle-and-movement).
  Removing the opaque's copy permission makes the unchanged second use invalid.
- **Identity and lifetime:** checked semantic copying retains an original and a
  copied occurrence. Reading either leaves the other and the disjoint sibling
  available. The trusted host store retains actual immutable C values under
  exact selected-application commitments. Affine consumption still retires its
  token; physical placement does not independently mint an occurrence.
- **Composition:** independent provider arguments and results, helpers, field
  projections, match arms, nested machines and target bodies preserve the same
  selected carrier and runtime observations. Independent component descriptions
  are required by each positive corpus fixture.
- **Soundness boundaries:** copy authority comes from checked source and the
  selected structurally copyable Inert carrier. The adapter retains exact
  normalized foreign bindings, shape and selected-application checks. Loan,
  claim, qualification and metadata-bearing component inputs retain their
  existing admission boundaries.
- **Checked-to-Terminal Psi (`Psi 05–07`):** public `omega run --both` must observe
  both execution engines. Unrestricted component arguments retain caller
  custody; affine arguments still consume the exact owned frontier.
- **Provider settlement, geometry and ABI (`Omega 00–02`):** separate producer
  and consumer products agree on the exact opaque application at the composition
  edge. The four-byte record uses actual C argument/result placements, with the
  content-addressed Windows component and `omega_host.dll` imports.
- **Instruction selection and activation (`Omega 03–06`):** the same scalar
  carrier passes through ordinary call, result and field homes. This row does
  not establish stack-argument boundaries or indirect aggregate return classes.
- **Encoding and emission (`Omega 07–09`):** actual Windows PE products must run
  and agree with Terminal execution. Other hosts require their own observations;
  declarations and successful compilation do not supply those observations.

```sh
omega run --both samples/cli/facts/opaque_copyable_carrier_exchange/local/main.omg
omega --check samples/cli/facts/opaque_copyable_carrier_exchange/local/invalid/main.omg
python tools/corpus_gate.py --both --filter opaque_copyable_carrier_through
```

The recovered pre-crash candidate passed all nine Windows native/Terminal
executions and nine copy-sensitive rejects. A fresh optimized compiler with the
same compiler/library inputs as main `fedebdd58c2c` reproduces the copy defect:
the local positive runs natively at zero, while Terminal refuses the result as
requiring an affine carrier. The repaired producer at `22f864f13829` passes the
nine legacy-form positives and nine copy-sensitive rejects; the scoped corpus
also verifies nine independent native/Terminal executions and four existing
linear, borrowed, qualified-result and selection controls. These observations
retain their actual source and producer identities in `build/opaque-copy/`.

The authored row now uses canonical nested `machine` declarations, explicit
arguments and explicit nested crash/reach contracts. Its current first failure
is the parser's refusal of a nested declaration (`expected ;, found identifier`),
also reproduced on the fresh main producer. That prerequisite belongs to
NESTED-MACHINE-UNIFICATION. Neither the positive nor its reject currently reaches
the copy obligation in that canonical form. The legacy-form observations do not
close this row, and no canonical execution or other-host result is claimed.
