# Runtime subjects through transparent aliases

The imported `Captured<Index: u64> = Indexed<Index>` family keeps a runtime-capable Value binder. `make` accepts an unknown ordinary runtime parameter, captures it in the underlying data's existing `Subject` descriptor, and returns through the alias. The caller changes its original variable from 7 to 9 before assigning the saved value to explicit `Indexed<7>`. The descriptor and payload remain 7 and 42, while the source variable is 9.

The retagged-subject twin changes only the receiving explicit application to the mutated variable. The carrier twin keeps the erased alias's `u64` contract unchanged but changes the ordinary function parameter and matching call operand to `u32`; erasing the RHS must preserve that argument obligation. No dynamic layout, static promotion, alias wrapper storage or extra authority is introduced.

Source `0487b72ae30a8613ad2b36932164ac11553cfdc4` passed on Linux x86_64 in [pilot 41271474633](https://github.com/CathedralOS/Omega/actions/runs/41271474633): native and Terminal both exited 0, and both intended-invalid children matched their authored refusals. The opt1 compiler SHA-256 was `33acca96bd1320420545967377456670259df5142345ee8909c7bd837a1a7ab2`. Preparation took 342.298 seconds; the three-project gate took 30.863 seconds.

The first `288d8b3e` producer failed before execution because source subject binding read only nominal data telescopes and left the retained alias application's Value argument unresolved. The repair lends the same alias headers to the existing Const/Value subject resolver, preserving lexical frontiers and all later carrier/subject obligations. The original false/wrong-reason observations remain retained.

Full binder/case/Product/nine-place/other-host acceptance remains separate.
