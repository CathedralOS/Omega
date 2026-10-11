# Dependent state index contracts

The native and Terminal programs must exit 0 after reading the sixth element
through a guarded state arrival, an ordinary scalar capture, and a second state
arrival. No scalar range suffix or generated interval domain participates.

After ordinary disposable-fixture package review:

```text
omega run --both tests/omega/pass/dependent/runtime_state_dependent_index_forward_exit/main.omg
```

## Migrated declaration inventory

The corresponding negative controls previously stopped at declaration checking
of a synthesized `__hoist` local, before checking the incoming state obligation.
Their historical declarations are in `f7baf32326a` (unrelated argument) and
`352f9d029ae` (forward after write), under `canaries/fail/dependent/`, with
endpoints named `self.count`. The static-receiver migration at `a4559733265`
named that same explicit context `main`. The table records the declarations
retained at `fd8672848bf` and their canonical replacements.

| Control and declaration | Previous spelling | Canonical replacement |
| --- | --- | --- |
| Both negative controls, common field `Main.count` | `count: u32 [0..=11]` | `count: u32`, data `where count <= 11` |
| Both negative controls, state `read` parameter `i` | `i: u32 [0..=main.count]` | `i: u32`, state `requires i <= main.count` |
| Forward-after-write control, state `deeper` parameter `k` | `k: u32 [0..=main.count]` | `k: u32`, state `requires k <= main.count` |

The unsigned carrier retains every lower bound of zero. The field's upper bound
remains a containing-value invariant; state requirements constrain each arrival,
not the scalar's type identity. Ordinary flow facts govern captured locals.

The unrelated-argument control now sets `count = 3` and passes six. Its old
`count = 8` also passed six, which the current checker correctly proves valid
after migration; that was not a negative contract witness. The forward-after-write
control retains its original eight-to-three mutation and rejects at `deeper`,
not at `read` or a temporary declaration. Neither control accepts mere integer
representability as proof of the dependent requirement.
