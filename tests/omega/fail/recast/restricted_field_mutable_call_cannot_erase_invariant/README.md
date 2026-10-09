# Restricted field mutable call

A shared call may observe the original restricted field through its scalar
carrier. An unrestricted mutable scalar formal must not acquire the same
presentation: its body can assign a value that violates the owning record's
standing invariant.
