The original arithmetic refinement still calls `cap(100)` and requires 90. The entry calls three additional probe functions. Together they check two independent public contracts at 7 and 9, that an inner call still receives 7 when the public caller passes 42, an inherited equality with no local promise, and an exact generic application at 7 and 13. Every check must complete for exit 0. Four target-keyed Build rows select each public boundary provider.

The false type application remains in `fail/traits/exact_application_keeps_parameter_types`. `fail/providers/provider_type_slot_unknown` keeps another valid candidate while selecting a map missing the exact edge. `fail/traits/member_omits_requirement_ensures` retains the original Grow guarantee failure. `fail/traits/inline_requirement_inherited_false_ensures` separately checks the inline member’s false inherited guarantee, so neither failure can mask the other. The bodyless inline member control is in `fail/proofs/quotient_representative_admitted_closure_rejected`; its later quotient check remains unverified.

These programs consolidate the earlier focused inputs after the corpus freeze in #1841. The `69ec9f80599b` compiler on Linux and `3b8603164e96` on Windows, Mac and Linux ARM each observed native/Terminal exit 0 for this program, in both the corpus and ordinary CLI. The missing edge, false type application, false inherited guarantee, omitted Grow promise and bodyless inline member each reached its intended refusal. The last control establishes the missing-body check; its later quotient check remains unverified. Other ownership timeouts and the whole task remain open.

The original `8dc` observations and the old `83e` forwarding failure retain their own programs and producers. This fixture does not close `BUILD-OWNED-TARGET-SELECTION`.

The integrated `6b1c338126fa` Linux compiler again produces native/Terminal exit 0
for this complete entry and reaches each intended independent-edge, exact-type,
inherited-guarantee, omitted-promise and bodyless-member refusal in the
[corpus replay](https://github.com/CathedralOS/Omega/actions/runs/42033929209).
The quotient control still establishes only missing-body formation; its later
quotient-closure obligation remains unverified. This current observation does
not replace the recorded identities of earlier host runs.
