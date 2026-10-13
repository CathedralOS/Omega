# later_arm_requires

A call in a later transition arm passes arguments the refuted earlier arms have already bounded: `sum`'s `requires index <= 4` holds for `index + 1` after `index >= 4 ->` is refuted; `step`'s conjunction holds once `count == 0` and `count >= 10` are refuted.

Compile-checked only: no standard-library dependency and
no \`Main::exit\` — run a check target, not native.
