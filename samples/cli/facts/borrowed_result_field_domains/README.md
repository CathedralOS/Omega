# Borrowed result field domains

Owning task: `NESTED-MACHINE-UNIFICATION`, complete normal-return contracts through borrowed results. [Invocation contracts](../../../../wiki/spec/language/state_contracts.md) require each normal completion to supply its complete result contract; [byte views](../../../../wiki/spec/terminal-psi/byte_views.md) distinguish a returned descriptor from its backing footprint.

Borrowed results retain their checked field domains and selected element, so inspection reads that element.

The nine places are [local](local/main.omg), [field](field/main.omg), [parameter](parameter/main.omg), [call result](call_result/main.omg), [match join](match_join/main.omg), [const name](const_name/main.omg), [nested machine](other_state/main.omg), [another machine](other_machine/main.omg), and [selected implementation](target_body/main.omg). Each reads both selected elements, whose first bytes differ. Wrong selection or data traps. Every invalid twin changes only the producer to write 255 into the selected ASCII field before returning it; the checked callee must reject that broken result obligation.

The producer owes its field-domain roster as well as exact descriptor/caller backing and lifetime. A conservative array footprint is not the selected element's logical value. Returning, storing or joining the descriptor must retain the field promise, while later overlapping writes must invalidate observations. No receiving annotation, physical overlap, nominal type equality, or caller equality guard establishes the promised field domain.

Current baseline: optimized producer `31ef02ce48700b2fb3752e2c5c05a8edf83fc04a` wrongly refuses all nine positives for missing forwarded ASCII field guarantees; all nine invalid controls reject the producer's broken field obligation. The witness and oracle hashes are frozen before the result-publication repair. All four hosted entries are authored. The row remains unfinished until native/Terminal agreement and intended refusals are observed and pinned; no runtime success is claimed here.
