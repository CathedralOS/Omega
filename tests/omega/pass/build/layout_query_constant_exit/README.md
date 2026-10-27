# Build-time layout constant

The Build declaration consumes a named constant whose initializer needs the
admitted execution profile's layout. The constant must resume before Build
checks its value and publishes entry bindings. A wrong answer follows a branch
that cannot publish those bindings. The resulting Product runs with exit 0 in
both native and Terminal execution.

This exercises the Build consumer of Psi's selected constant evaluation.
Product-side query closure is covered by
`../../layouts/runtime_constant_layout_contract_exit`; an uninstantiated layout
policy is refused by `../../../fail/layouts/size_of_uninstantiated_plan_laid`.
It does not establish target-independent portable layout-query publication.
