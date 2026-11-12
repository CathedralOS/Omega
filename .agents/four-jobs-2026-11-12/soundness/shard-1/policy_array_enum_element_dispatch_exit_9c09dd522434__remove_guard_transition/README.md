# Enum element dispatch through a policy-qualified array

`Table::weigh` dispatches on `table.modes[index]`, an element of
`[Mode; 4] in Trapping` selected by a runtime index. The array's arithmetic
policy qualifies the element's declared type (`Mode in Trapping`) without
changing its storage, so the subject loan roots the plain `Mode` structural
plan, as the plan registry's identity rule requires. Busy, idle and
out-of-range rows weigh 2, 1 and 0, and the program exits 0.

```sh
omega update --project tests/omega/pass/collections/policy_array_enum_element_dispatch_exit --target macos_arm64
omega run --both tests/omega/pass/collections/policy_array_enum_element_dispatch_exit/main.omg
```

Native execution and the checked-tree interpreter both exit 0 on macOS ARM64.
Before the fix, lowering refused the dispatch: the subject loan's root kept
the qualification and named no registered plan.
