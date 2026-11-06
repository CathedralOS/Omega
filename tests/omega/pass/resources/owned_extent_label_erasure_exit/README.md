# Removing a label preserves owned storage

Removing a non-owning label from an extent must preserve its owned storage
claims. This program adds `Marked`, then casts away that label while retaining
the same granted, vacant extent. The returned extent must still support
partitioning, record initialization, mutation and extraction.

Both windows check the original value 17, updated value 29, neighboring values
101/103 and returned length 32. The program binds all four hosted targets and
requires native and Terminal exit 0. Execution acceptance is pending.

The [reject control](../../../fail/resources/owned_extent_erased_label_rejected/main.omg)
changes only the cast's return promise to include the removed `Marked` label.
It must reject that promise while retaining the ordinary owned-claim duties.

Parent: `BUMP-ALLOCATOR-CANARY`. The existing
[resident-backing row](../../../../../samples/cli/facts/resident_backing/README.md)
retains the broader transport acceptance, including its unfinished constant
place. This focused pair adds no new fact row.
