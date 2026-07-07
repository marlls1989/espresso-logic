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

/// `f[var := g]` — resolve the name (absent ⇒ no-op) and run the fused compose engine.
pub(super) fn compose<C: ManagerCell>(cell: &C, f: NodeId, var: &str, g: NodeId) -> NodeId {
    let var_id = cell.read().var_id(var);
    match var_id {
        None => f,
        Some(v) => cell.compose(f, v, g),
    }
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

/// `f|var=value` — resolve the name (absent ⇒ no-op) and run the restrict engine.
pub(super) fn restrict<C: ManagerCell>(cell: &C, f: NodeId, var: &str, value: bool) -> NodeId {
    let var_id = cell.read().var_id(var);
    match var_id {
        None => f,
        Some(v) => cell.restrict(f, v, value),
    }
}

/// Simultaneous `f|{v=value}` over name-keyed entries; absent names dropped, a repeated
/// name takes its LAST entry; empty (post-resolution) map ⇒ no-op.
pub(super) fn restrict_many<C: ManagerCell, S: AsRef<str>>(
    cell: &C,
    f: NodeId,
    entries: impl IntoIterator<Item = (S, bool)>,
) -> NodeId {
    let entries: Vec<(S, bool)> = entries.into_iter().collect();
    let map: HashMap<VarId, bool> = {
        let mgr = cell.read();
        entries
            .iter()
            .filter_map(|(name, value)| mgr.var_id(name.as_ref()).map(|v| (v, *value)))
            .collect()
    };
    if map.is_empty() {
        return f;
    }
    cell.restrict_many(f, &map)
}
