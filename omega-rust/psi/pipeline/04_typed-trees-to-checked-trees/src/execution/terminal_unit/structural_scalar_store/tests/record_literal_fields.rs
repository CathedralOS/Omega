//! A record literal assigned through a field path (`self.h = Handle { .. }`)
//! decomposes into the ordered scalar member stores an authored nested-field
//! sequence would carry: each member keeps its `RecordField` computation
//! coordinate and each store names the target's full carrier path. The same
//! admission covers `addr` members — an address rides the pointer-width
//! integer lane like any other scalar leaf.
