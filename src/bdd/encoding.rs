//! Node-level encodings of the Boolean operators in terms of the primitive `ite`/`xor`.
//!
//! Both the owned [`Bdd`](super::handle::Bdd) and the by-reference [`ScopedBdd`](super::scope::ScopedBdd)
//! compose handles through the same canonical formulas. Defining them once here means the two layers cannot
//! drift; each operator method is then just a same-manager check (where applicable) plus a call here.

use crate::expression::manager::{BddManager, NodeId, FALSE_NODE, TRUE_NODE};
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
