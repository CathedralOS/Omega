# Published allocation

`AllocatedBody` is the immutable handoff from register allocation to machine
lowering. It owns shared plans and the facts those plans were admitted with.
It contains no allocation procedure or replay callback. `facts()` lends the
records the next stage consumes; cloning shares the underlying plan storage.

Only the allocation stage and this representation may link `issuer/`.
`AllocationParts` alone is not an admitted body. Repository boundary checks
keep the issuing capability out of consumers and the native driver.
