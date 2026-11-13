# Package-qualified memory operations

A program can name an imported operation by its full package address when a nested helper has the same name. This program writes 29 into a field through `omega::language::write`, reads it through `omega::language::read`, and checks that both neighboring fields retain their values.

The import loads `core/layout.omg`; that file declares its functions at the `omega::language` package root. The package prefix has no declaration symbols of its own. The compiler must record the selected call without inventing unresolved declarations for those prefix tokens.

The write and read pass borrowed fields into library calls. Those calls must keep the record of where the memory view was created, so reading after the write still reaches the original storage. The compiler must not replace that record with an unknown pointer while passing the borrowed field.

The [reject twin](../../../fail/modules/package_qualified_scalar_access_denied/main.omg) removes only the field's write permission. Qualification must still select the same checked operation and reject that write.

This regression supports Cathedral's UEFI boot migration. It establishes qualified call recording, not UEFI boot acceptance.
