# range_requires_discharge

An authored `requires` on a range-endpoint callee discharges at the concrete argument: `u64 [0..=bounded(256)]` re-checks `bounded`'s `requires value <= 512` as `256 <= 512` decided-true.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
