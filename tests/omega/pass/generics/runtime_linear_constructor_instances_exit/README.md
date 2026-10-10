# Closed linear constructor instances retain their own arguments

Run `omega run --both tests/omega/pass/generics/runtime_linear_constructor_instances_exit/main.omg`.

One generic machine constructs a linear owner at two different payload types,
including an array. Each copied constructor must derive its own complete type
and infer the owner's independent backing region from the reference parameter.
The results retain both caller backings and use their selected cleanup hooks.
The array field is observed from both its owned result home and a borrowed
helper parameter; neither read copies or disposes the containing linear owner.
Native and Terminal must observe 41, 23, 73 and 74 and exit 0.
