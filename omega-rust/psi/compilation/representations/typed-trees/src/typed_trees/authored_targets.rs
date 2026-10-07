//! Authored deployment scope survives selection and machine specialization.
//!
//! Selection may clear a body's mutable target marker. A specialization may
//! instead hold a fresh machine symbol outside the family roster. Both readers
//! need the same exact family/origin join, never a target guessed from names.

use super::TypedTrees;
use symbols::SymbolHandle;

impl TypedTrees {
    /// Return the authored tag, or absence for a target-independent declaration.
    /// Malformed family/origin associations reject instead of becoming portable.
    pub fn authored_machine_target(
        &self,
        mut symbol: SymbolHandle,
    ) -> Result<Option<&str>, &'static str> {
        // Each nonterminal step follows one retained specialization row. This
        // bound catches cycles without allocating another compiler lookup map.
        for _ in 0..=self.machine_specializations.len() {
            let mut machines = self
                .machines()
                .iter()
                .filter(|machine| machine.symbol == symbol);
            let machine = machines
                .next()
                .ok_or("authored target has no exact machine")?;
            if machines.next().is_some() {
                return Err("authored target has repeated machine coordinates");
            }
            let mut members = self
                .target_machine_families
                .iter()
                .flat_map(|family| &family.members)
                .filter(|member| member.machine == symbol);
            if let Some(member) = members.next() {
                if members.next().is_some() || member.target.as_str().is_empty() {
                    return Err("machine has no unique authored target member");
                }
                return Ok(Some(member.target.as_str()));
            }
            let mut origins = self
                .machine_specializations
                .iter()
                .filter(|specialization| specialization.instance == symbol);
            if let Some(origin) = origins.next() {
                if origins.next().is_some() {
                    return Err("machine has repeated specialization origins");
                }
                // The first specialization may reuse the authored template's
                // symbol. That is a terminal origin, not a cycle.
                if origin.template != symbol {
                    symbol = origin.template;
                    continue;
                }
            }
            if machine.target.is_some() {
                return Err("target-tagged machine has no authored family member");
            }
            return Ok(None);
        }
        Err("authored target specialization origins contain a cycle")
    }
}
