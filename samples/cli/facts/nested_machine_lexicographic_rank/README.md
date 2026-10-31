# Joint lexicographic ranks

Owner: [NESTED-MACHINE-UNIFICATION](../../../../TASKS.md).

Fact: joint private arrivals retain a lexicographic rank while lower components
reset after a strict higher-component decrease. The same bound and operation
cross all nine places: `Progress { outer: 1, inner: 2, canary: 641 }` visits four
inner steps, preserving canary641 and advancing the caller counter3→7. The
already-complete0,0 input leaves it unchanged. Each zero-step twin changes only
the inner decrement from1to0; a strict edge elsewhere cannot excuse that
preserving cycle.

Current fixed base `a91a8de86514f5b0ba71509b3af4d656916d6062` refuses the
[original frozen probe](parameter/probe/main.omg): a write, call or alias
invalidates the entry-relative ranking. The baseline is a compiler defect:
[termination](../../../../wiki/spec/language/termination.md#ranking) admits
lexicographic products and requires strict descent on every complete cycle.
Same-shaped participants share the same exact declaration; the open syntax
question for differently shaped mutual participants is not involved.

Checking and source-to-Terminal/native acceptance are unfinished. Source hashes,
exact baseline producer and raw refusal are in `build/joint-lexicographic/`.
