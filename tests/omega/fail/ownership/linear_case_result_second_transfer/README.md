# A returned case owner cannot transfer its selected linear payload twice

Forwarding preserves the active payload's linear custody. Consuming the
returned owner discharges that custody; a second call cannot use it again.
