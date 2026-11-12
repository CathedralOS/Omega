# carry_contract_decl

Carry contracts: `[carry(suspension: allowed, cpu: any, thread: any,
address: movable)]` on `data Empty`, the same contract as a bound on
`Portable<T [carry(...)]>`'s type parameter, a zero-extent array
`[&i32; 0]` initialized by `[]`, and `Portable<Empty>` nesting.

Compile-checked only: freestanding — run a check target, not native.
