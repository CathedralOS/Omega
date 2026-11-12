# proof_data_cycle

Free machines over recursive proof-only data may form a non-tail
call SCC because no runtime frame is emitted: each cross-machine
edge consumes the case-payload subterm selected by the shared
witness (`ProofNat::Succ { prev } -> ... { prev: right(prev) }`).

Compile-checked only: no standard-library dependency and
no `Main::exit` — run a check target, not native.
