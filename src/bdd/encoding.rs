//! Name-resolution adapters over [`BddOps`]: the variable-name-keyed compose and restrict operations
//! shared by the owned [`Bdd`](super::handle::Bdd) and the by-reference
//! [`ScopedBdd`](super::scope::ScopedBdd).
//!
//! Each resolves a name (or names) to a [`VarId`] under a read borrow — an absent name is a no-op, an
//! empty post-resolution map short-circuits — then dispatches to the corresponding [`BddOps`]
//! primitive. Defining them once here means the two handle layers cannot drift. The purely node-level
//! Boolean operators (`and`/`or`/`not`/`xor`) are [`BddOps`] methods and need no adapter.

use std::collections::HashMap;

use crate::bdd::manager::{BddOps, NodeId, VarId};
use crate::bdd::manager_cell::ManagerCell;
use crate::cover::{Minterm, StringLabel};

/// `f[var := g]` — a single-entry [`compose_map`]: the sole substitution routes through the same
/// simultaneous-substitution engine, so an absent name resolves to an empty map (a no-op) exactly as
/// it does for `compose_map`.
pub(super) fn compose<C: ManagerCell>(cell: &C, f: NodeId, var: &str, g: NodeId) -> NodeId {
    compose_map(cell, f, std::iter::once((var, g)))
}

/// Simultaneous `f[v := g_v]` over name-keyed entries; absent names dropped, a repeated
/// name takes its LAST entry; empty (post-resolution) map ⇒ no-op.
pub(super) fn compose_map<C: ManagerCell, S: AsRef<str>>(
    cell: &C,
    f: NodeId,
    entries: impl IntoIterator<Item = (S, NodeId)>,
) -> NodeId {
    let entries: Vec<(S, NodeId)> = entries.into_iter().collect();
    let map: HashMap<VarId, NodeId> = {
        let mgr = cell.read();
        entries
            .iter()
            .filter_map(|(name, g)| mgr.var_id(name.as_ref()).map(|v| (v, *g)))
            .collect()
    };
    if map.is_empty() {
        return f;
    }
    cell.compose_map(f, &map)
}

/// Restrict `f` to the subspace pinned by `minterm`, re-homing the (label-keyed) minterm onto this
/// manager's variable order first: a name the manager knows lands at its VarId slot, an absent name is
/// left free, an unknown name is dropped. This is the single entry point the name-keyed shims and the
/// public `restrict_to` both funnel through.
pub(super) fn restrict_to<C: ManagerCell, L: StringLabel>(
    cell: &C,
    f: NodeId,
    minterm: &Minterm<L>,
) -> NodeId {
    let values: Vec<Option<bool>> = {
        let mgr = cell.read();
        // Project onto the manager's variable names in VarId order: a target-only var becomes
        // don't-care (free), an input-only name is dropped. The result is positional over VarId.
        let names: Vec<&str> = mgr.id_to_var.iter().map(|s| s.as_ref()).collect();
        minterm.project_to(names).iter().collect()
    };
    cell.restrict_to(f, &Minterm::anonymous(&values))
}

/// `f|var=value` — resolve the name (absent ⇒ no-op) and run the restrict engine.
pub(super) fn restrict<C: ManagerCell>(cell: &C, f: NodeId, var: &str, value: bool) -> NodeId {
    restrict_many(cell, f, std::iter::once((var, value)))
}

/// Simultaneous `f|{v=value}` over name-keyed entries; absent names dropped, a repeated name takes its
/// LAST entry; an all-free (or empty) assignment ⇒ no-op.
pub(super) fn restrict_many<C: ManagerCell, S: AsRef<str>>(
    cell: &C,
    f: NodeId,
    entries: impl IntoIterator<Item = (S, bool)>,
) -> NodeId {
    let values = {
        let mgr = cell.read();
        let mut vals = vec![None; mgr.id_to_var.len()];
        for (name, value) in entries {
            if let Some(v) = mgr.var_id(name.as_ref()) {
                vals[v] = Some(value); // a later entry for the same name wins
            }
        }
        vals
    };
    if values.iter().all(Option::is_none) {
        return f; // empty or all-unknown assignment is a no-op
    }
    cell.restrict_to(f, &Minterm::anonymous(&values))
}
