# folded_actual_requires

`scale <= 254` on a u8 is NOT vacuous (carrier reaches 255), but a call actual whose result constant-folds instantiates the premise `4 <= 254` decided-true — the call discharges.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
