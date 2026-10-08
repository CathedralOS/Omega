A named mutable descriptor writes the original constructed linear owner.
After its last use, owner and materialized field observations see that write;
the owner still requires explicit disposal. Transferring it to an owning helper
retains its entry claim while that helper forms another named shared descriptor,
observes the same backing, and explicitly disposes the owner.

Run: `omega run --both tests/omega/pass/borrows/linear_owner_materialized_mutable_projection_exit/main.omg`.
