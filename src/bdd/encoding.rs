//! Node-level encodings of the Boolean operators in terms of the primitive `ite`/`xor`.
//!
//! Both the owned [`Bdd`](super::handle::Bdd) and the by-reference [`ScopedBdd`](super::scope::ScopedBdd)
//! compose handles through the same canonical formulas. Defining them once here means the two layers cannot
//! drift; each operator method is then just a same-manager check (where applicable) plus a call here.

use std::collections::HashMap;

use crate::expression::manager::{BddManager, NodeId, VarId, FALSE_NODE, TRUE_NODE};
use crate::expression::manager_cell::ManagerCell;

/// `f ∧ g`, encoded as `ite(f, g, 0)`.
pub(super) fn and<C: ManagerCell>(cell: &C, f: NodeId, g: NodeId) -> NodeId {
    BddManager::ite(cell, f, g, FALSE_NODE)
}

/// `f ∨ g`, encoded as `ite(f, 1, g)`.
pub(super) fn or<C: ManagerCell>(cell: &C, f: NodeId, g: NodeId) -> NodeId {
    BddManager::ite(cell, f, TRUE_NODE, g)
}

/// `f ⊕ g`.
pub(super) fn xor<C: ManagerCell>(cell: &C, f: NodeId, g: NodeId) -> NodeId {
    BddManager::xor(cell, f, g)
}

/// `¬f`, encoded as `ite(f, 0, 1)`.
pub(super) fn not<C: ManagerCell>(cell: &C, f: NodeId) -> NodeId {
    BddManager::ite(cell, f, FALSE_NODE, TRUE_NODE)
}

/// `f[var := g]` — resolve the name (absent ⇒ no-op) and run the fused compose engine.
pub(super) fn compose<C: ManagerCell>(cell: &C, f: NodeId, var: &str, g: NodeId) -> NodeId {
    let var_id = cell.read().var_id(var);
    match var_id {
        None => f,
        Some(v) => BddManager::compose(cell, f, v, g),
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
    BddManager::compose_map(cell, f, &map)
}

/// `f|var=value` — resolve the name (absent ⇒ no-op) and run the restrict engine.
pub(super) fn restrict<C: ManagerCell>(cell: &C, f: NodeId, var: &str, value: bool) -> NodeId {
    let var_id = cell.read().var_id(var);
    match var_id {
        None => f,
        Some(v) => BddManager::restrict(cell, f, v, value),
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
    BddManager::restrict_many(cell, f, &map)
}
