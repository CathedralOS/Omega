# Borrowed result field domains

Owning task: `NESTED-MACHINE-UNIFICATION`, complete normal-return contracts through borrowed results. [Invocation contracts](../../../../wiki/spec/language/state_contracts.md) require each normal completion to supply its complete result contract; [byte views](../../../../wiki/spec/terminal-psi/byte_views.md) distinguish a returned descriptor from its backing footprint.

Borrowed results retain their checked field domains and selected element, so inspection reads that element.

The nine places are [local](local/main.omg), [field](field/main.omg), [parameter](parameter/main.omg), [call result](call_result/main.omg), [match join](match_join/main.omg), [const name](const_name/main.omg), [nested machine](other_state/main.omg), [another machine](other_machine/main.omg), and [selected implementation](target_body/main.omg). Each reads both selected elements, whose first bytes differ. Wrong selection or data traps. Every invalid twin changes only the producer to write 255 into the selected ASCII field before returning it; the checked callee must reject that broken result obligation.

The producer owes its field-domain roster as well as exact descriptor/caller backing and lifetime. A conservative array footprint is not the selected element's logical value. Returning, storing or joining the descriptor must retain the field promise, while later overlapping writes must invalidate observations. No receiving annotation, physical overlap, nominal type equality, or caller equality guard establishes the promised field domain.

Producer `d58b8ab0f8fed8e2cd06c6fe4197fd20013334e2` executes all nine
original positives with native and Terminal exit 0 on Linux x64, Linux ARM64,
macOS ARM64 and Windows x64. All nine invalid twins reject the broken `choose`
result. The [PR 1880 validation records](https://github.com/CathedralOS/Omega/pull/1880)
identify each host run. Matching fixtures remain under
`tests/omega/{pass,fail}/facts/borrowed_result_field_domains_<place>`.
The sibling-write productions below have their own witness scope and records.

The callee establishes each field promise; the completed descriptor carries it independently of a conservative alias footprint. Captured copies and joins consume each actual producer on every alternative. Promise dependencies use the existing storage-write history; a write of 255 through the returned reference rejects the later `inspect` obligation, while a write to an unrelated local preserves it. Root/descriptor lifetimes and issuer authority remain independently checked.

The [sibling-write production](sibling_write/README.md) carries this same fact through all nine places after writing a disjoint field of the selected Cell. Its completed storage footprints preserve the field promise without claiming an exact index or projecting through a coarse enclosure. Its own twins and runtime canaries pin overlap invalidation and the selected write; the parent task remains open.

The [indexed sibling-write production](indexed_sibling_write/README.md) carries the same fact through all nine places with independent cell and stamp selectors. Builtin indexing retains the completed collection's possible storage footprint; omitted and misdirected stores trap, while an overlapping payload write still rejects.
