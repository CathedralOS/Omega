//! Match local disposal to exact outgoing custody, not a lexical drop count.
//! Establishment and transfers determine whether each selected normal edge
//! still owns the root. Drop and disposition rows must cover that independent
//! remainder; deleting both rows cannot erase a live local's obligation.
