The authored result is exit 0 in native and Terminal execution. The affine
owner's hook mutates the values held by its linear children before their hooks.
It must retain their claim identities and receiver storage until those hooks
retire them. A parent hook is not whole-subtree destruction.

This challenges cleanup's assumption that a nominal parent carries no live
linear descendants; it does not introduce a separate lowering route.

Each child's hook requires its value to equal 9. The parent's checked guarantee
establishes both prerequisites before the children retire in reverse declaration
order. Cleaning the second child must preserve the guarantee about the untouched
first child, together with both children's exact claims and receiver storage.

Run `omega update --project <fixture> --target linux_x86_64`, then
`omega run --both <fixture>/main.omg` on the matching host. The unchanged
program must produce exit 0 in both engines.
