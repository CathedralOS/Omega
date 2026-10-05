Run `omega run --both tests/omega/pass/borrows/runtime_mutable_primitive_reference_exit/main.omg`.
Both engines return 0. The program captures mutable u32, i64 and Boolean
storage, writes through each reference, and observes the original homes. The
u32 reference also crosses an ordinary helper call before the owner reads it.
An incorrect home or stale initializer reaches the authored Trap branch.
