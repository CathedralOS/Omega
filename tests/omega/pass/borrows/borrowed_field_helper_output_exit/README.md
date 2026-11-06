# Helpers can save references read from a borrowed table

A caller reads either of two reference fields, then a selected helper saves that
reference in its output record. Both outputs must still point to the selected
objects. The program reverses the two fields for its second call and checks the
fixed values surrounding the table fields and both output references. It also
passes a mutable input to the same shared-reference helper.

This reduces Cathedral's loaded-image discovery call. The former compiler
rejected the reference field's existing input permission while accepting only
the permission for the whole borrowed parameter. The repair selects the exact
unchanged field and its own reference type; it does not grant access through the
containing table.

The existing [replacement rejection](../../../fail/borrow/write_only_reference_aggregate_conditional_replacement_loan/main.omg)
keeps the original backing borrowed when a conditional slot replacement does
not execute. The [shared-carrier rejection](../../../fail/borrows/shared_reference_carrier_writable_call/main.omg)
keeps writable access unavailable through shared inputs. Neither geometry nor a
matching type can replace those borrowing obligations.

Expected exit: 0 on both engines. Build binds all four hosted targets.
