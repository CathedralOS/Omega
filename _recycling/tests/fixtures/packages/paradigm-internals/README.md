# paradigm-internals

Target-filter fixture package. `Helper` carries a portable `shared` member
plus `uefi_x86_64`-only paradigm internals, including a same-target caller of
one of them, so the `tests/omega/{fail,pass}/targets` inert-sibling canaries
exercise the rule across a dependency-package boundary.

