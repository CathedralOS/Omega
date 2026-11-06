# Removing a label preserves owned storage

Removing a non-owning label from an extent must preserve its owned storage
claims. This program adds `Marked`, then casts away that label while retaining
the same granted, vacant extent. The returned extent must still support
partitioning, record initialization, mutation and extraction.

Both windows check the original value 17, updated value 29, neighboring values
101/103 and returned length 32. The program binds all four hosted targets and
requires native and Terminal exit 0. Both engines return 0 at compiler source
`e9afae5fccb734c20c4a84ea4431368cb9c3a790` on each hosted target:

| Host | Execution record |
| --- | --- |
| Linux x64 | [Passed](https://github.com/CathedralOS/Omega/actions/runs/42238416266) |
| Linux ARM64 | [Passed](https://github.com/CathedralOS/Omega/actions/runs/42238426089) |
| Windows x64 | [Passed](https://github.com/CathedralOS/Omega/actions/runs/42238430957) |
| macOS ARM64 | [Passed](https://github.com/CathedralOS/Omega/actions/runs/42238421280) |

The [reject control](../../../fail/resources/owned_extent_erased_label_rejected/main.omg)
changes only the cast's return promise to include the removed `Marked` label.
It must reject that promise while retaining the ordinary owned-claim duties.

Parent: `BUMP-ALLOCATOR-CANARY`. The existing
[resident-backing row](../../../../../samples/cli/facts/resident_backing/README.md)
retains the broader transport acceptance, including its unfinished constant
place. This focused pair adds no new fact row.
