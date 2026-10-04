//! A record construction replays against its authored struct literal: the
//! nominal carrier is the declared record, the retained field roster covers
//! every declared member exactly once in order, an omitted member is only a
//! zeroed structural leaf, and each field value is either a scalar operand
//! of its declared carrier or a structural value queued for replay.
