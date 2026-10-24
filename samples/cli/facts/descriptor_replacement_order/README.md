# Descriptor replacement after RHS effects

Replacing one input-carrier reference retains its new backing and the untouched sibling.

Parent: **STATE-LOCAL-VALUE-FRONTIER**, aggregate construction, replacement
and reads. Assignment captures storage before evaluating its RHS; old-value
cleanup and the final store use that captured slot and its actual current
descriptor, not the displaced descriptor's stale ingress identity.
See [assignment order](../../../../wiki/spec/language/expressions.md#evaluation-schedule),
[normal exclusive completion](../../../../wiki/spec/terminal-psi/ownership.md#borrowed-storage-restoration)
and [carrier lifecycle](../../../../wiki/spec/terminal-psi/loans.md#reborrow-lineage-and-access).

The unchanged original root from the preceding repair's RHS falsifier must
exit 0 in both engines, preserve backing 17 and sibling 19, and observe the
installed reference's backing 41. The stronger
[intermediate control](controls/intermediate/main.omg) first installs a
different reference to 31 during the RHS, then returns the reference to 41;
omitting the outer store must fail its oracle. These are runtime identity and
carrier-lifecycle requirements, not permission to turn type shape into loans.

The original optimized baseline `0dce4dc599e2596206d3ae5ce209729ec3800b6b`
checks the original source but refuses native publication:

    replace::entry, statement 0:
    statement sequence: assignment: structural destination window

Acceptance is `omega run --both <place>/main.omg` with authored exit 0.
Every place changes the first descriptor from backing 17 through an RHS write
to backing 31, then installs the returned reference to 41. The untouched second
descriptor still reads 19. The caller does not reconstruct this identity with
a guard before using the returned carrier.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [use after transfer](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [use after transfer](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [use after transfer](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [use after transfer](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [use after transfer](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [use after transfer](const_name/invalid/main.omg) |
| Another state | [program](other_state/main.omg) | [use after transfer](other_state/invalid/main.omg) |
| Another machine | [program](other_machine/main.omg) | [use after transfer](other_machine/invalid/main.omg) |
| Target body | [program](target_body/main.omg) | [use after transfer](target_body/invalid/main.omg) |

The const-name place carries the selected slot through a constant index; it
does not claim a constant can own a runtime reference. Each twin changes only
the final reference argument to read the carrier after its earlier argument
has transferred it. It must reject at that use, not at package preparation.
The nine positives and twins are pinned under
`tests/omega/{pass,fail}/facts/descriptor_replacement_order_through_*`.

Controls challenge the operation rather than its spelling:

- [Captured selector](controls/selector/main.omg): the RHS changes the index
  from 0 to 1, but the outer store still replaces element 0.
- [Independent copies](controls/copies/main.omg): copies retain 17 and 31
  while the replaced slot reads 41; [removing copy permission](controls/copies/invalid/main.omg)
  must reject a second use.
- [Local owner](controls/local_owner/main.omg): the same replacement works
  in local storage; [a moved parent](controls/local_owner/invalid/main.omg)
  cannot be resurrected by writing one descendant.
- [Borrowed-first array](controls/borrow_first_array/main.omg) and
  [owned-first array](controls/owned_first_array/main.omg): copy permission is
  independent of which access first demands the canonical array type.
- [Reordered state inputs](controls/reordered_state/main.omg): returned
  descriptor origins use the actual input leaf, not a formal ordinal or the
  first same-typed sibling. [Escaping local backing](controls/reordered_state/invalid/main.omg)
  must still reject.
- [Exclusive descriptor arrays](controls/exclusive_arrays/main.omg): thin
  references remain readable; both thin-reference and slice-view arrays are
  materialized and retired, but their exact emitted declarations retain no
  copy permission. This is an authority control, not a nested-byte-read test. The
  `fail/borrows/exclusive_reference_array_copy` fixture cannot export an
  exclusive array through shared input.
- [Qualified referent](controls/qualified_retarget/main.omg): replacing a local
  record's shared reference retires its old descriptor, not either backing.
  The original program observes 73 from the old backing after the replacement
  and retains the call through the new descriptor. Optimized clean main
  `7a7f0a6cd808` checked this unchanged source but refused publication at
  `observed_exit::entry` with `sequence cleanup actions`; diagnostic-only tracing
  on `85a91d4c22f6` isolated a comparison of qualified custody identity with
  unqualified referent storage identity. The join must authenticate both through
  the exact assigned declaration, preserving access and the Referent path.
- [Predicate-qualified replacement](controls/qualified_predicate/main.omg):
  the same layout/custody join also works when the referent has a real predicate.
  Its [invalid control](controls/qualified_predicate/invalid/main.omg) changes
  only the replacement's value from 11 to 1 and must fail the field's required
  `Cell::Controlled` membership. Equal layouts do not discharge that obligation.
  These controls are pinned by `pass/borrows/{qualified,predicate}_reference_field_retarget_exit`
  and `fail/borrows/predicate_reference_retarget_requires_membership`.

Requirement evidence and boundaries:

1. **Establishment:** explicit record/array construction supplies both input
   descriptors; completed RHS writes and the final store establish the new one.
2. **Identity and lifetime:** independent 17/31/41 backings and sibling 19
   distinguish stale, intermediate, final and unrelated identities. Copies
   retain their own carriers. No cleanup owns an external referent.
3. **Composition:** the same replacement survives all nine places, including
   state return joins and scalar observations of an owned predicate receiver.
4. **Soundness boundaries:** only admitted ingress leaves and owned strict
   prefixes authorize descriptor retirement. Noncopy second use, moved-parent
   writes and local escapes remain invalid; no source affine claim is invented.
5. **Psi 05–07:** checked post-RHS cleanup disposes the selected descriptor,
   retains its home and installs at captured coordinates. Terminal execution
   must agree with native; encoded copy permission is retained explicitly.
6. **Omega 00–02:** nested fixed-array/record layouts and structural call/state
   placements carry descriptor addresses without reopening source.
7. **Omega 03–06:** selected structural copies, allocated homes and call/frame
   preservation retain both addresses through the observable reads.
8. **Omega 07–09 and emission:** canonical Terminal decoding and direct
   macOS arm64 image execution exercise the complete path, not check-only
   acceptance. This row does not independently establish every branch,
   relocation, ABI or stack-bound mechanism.

Run identities, actual engine observations, accepted locks and artifact hashes
belong to the qualification/claim record for this repair. Native evidence is
macOS arm64 only; four hosted bindings do not establish execution on other
hosts. Windows native execution is explicitly deferred to a Windows agent.
The parent remains open for runtime/case projections, opaque incoming referents
and other stored-reference composition outside this row. Clean baseline
`0dce4dc599e2` already fails nested byte observations below an exclusive
descriptor array: byte indexing refuses at Psi 05's declared projection walk,
and length reads execute natively but fail Terminal interpretation with
`VerifiedOperationMalformed`. The broader failed probe and both baseline and
candidate streams remain in the repair's evidence; this row does not fix them.
