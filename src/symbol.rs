//! [`Symbol`]: a compact, interned string for variable names.
//!
//! Variable names flow through every layer of the crate (BDD manager, cover labels, cubes). `Symbol`
//! is the storage we use for them, tuned for that workload:
//!
//! - **Small-string optimised.** A name of up to [`INLINE_CAP`] bytes (which is virtually every real
//!   variable name — `a`, `x0`, `carry_in`, …) lives **inline**, with no heap allocation and an O(1),
//!   `memcpy`-cheap [`Clone`]. An `Arc<str>` would heap-allocate a refcount header plus the bytes even
//!   for `"a"`.
//! - **Interned.** Longer names are deduplicated through a process-global pool, so equal names share
//!   one allocation and compare in O(1) by pointer. The pool holds **weak** references, so it never
//!   keeps a name alive: when the last `Symbol` for a name drops, its allocation is freed and the dead
//!   pool entry is pruned. The pool is bounded to the long names currently live — it cannot leak.
//!
//! `Symbol`'s `Ord`, `Eq` and `Hash` all act on the string **content** and agree with `str`'s, so a
//! `BTreeMap<Symbol, _>` / `HashMap<Symbol, _>` can be looked up with a `&str` key (via [`Borrow`]).

use std::borrow::Borrow;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, LazyLock, Mutex, Weak};
use weak_table::WeakHashSet;

/// Maximum byte length stored inline (without heap allocation). Chosen so `size_of::<Symbol>()` stays
/// in the `Arc<str>`/`String` class (24 bytes on 64-bit).
pub const INLINE_CAP: usize = 22;

/// A compact, interned variable-name string. See the [module docs](self).
#[derive(Clone)]
pub struct Symbol(Repr);

#[derive(Clone)]
enum Repr {
    /// Up to [`INLINE_CAP`] bytes stored inline (`buf[..len]` is valid UTF-8).
    Inline { len: u8, buf: [u8; INLINE_CAP] },
    /// A longer name, interned and shared.
    Heap(Arc<str>),
}

impl Symbol {
    /// Intern a string as a `Symbol` (inline if short, pooled otherwise).
    pub fn new(s: &str) -> Symbol {
        let bytes = s.as_bytes();
        if bytes.len() <= INLINE_CAP {
            let mut buf = [0u8; INLINE_CAP];
            buf[..bytes.len()].copy_from_slice(bytes);
            Symbol(Repr::Inline {
                len: bytes.len() as u8,
                buf,
            })
        } else {
            Symbol(Repr::Heap(intern(s)))
        }
    }

    /// The string content.
    #[inline]
    pub fn as_str(&self) -> &str {
        match &self.0 {
            // SAFETY: `buf[..len]` was copied from a `&str` in `new`, so it is valid UTF-8.
            Repr::Inline { len, buf } => unsafe {
                std::str::from_utf8_unchecked(&buf[..*len as usize])
            },
            Repr::Heap(s) => s,
        }
    }
}

// ===== The interning pool =====
//
// A weak hash set of the heap-stored names, hashed by content (through the upgraded `Arc<str>`). It
// holds only weak references, so it never keeps a name alive and prunes expired entries itself — the
// pool stays bounded to the long names currently live.

static POOL: LazyLock<Mutex<WeakHashSet<Weak<str>>>> =
    LazyLock::new(|| Mutex::new(WeakHashSet::new()));

/// Return the shared `Arc<str>` for `s`, allocating and registering one if not already live.
fn intern(s: &str) -> Arc<str> {
    let mut pool = POOL.lock().unwrap();
    if let Some(arc) = pool.get(s) {
        return arc;
    }
    let arc: Arc<str> = Arc::from(s);
    pool.insert(Arc::clone(&arc));
    arc
}

// ===== Trait surface — all content-based and `str`-consistent =====

impl Deref for Symbol {
    type Target = str;
    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for Symbol {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for Symbol {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&str> for Symbol {
    fn from(s: &str) -> Symbol {
        Symbol::new(s)
    }
}

impl From<String> for Symbol {
    fn from(s: String) -> Symbol {
        Symbol::new(&s)
    }
}

impl From<&String> for Symbol {
    fn from(s: &String) -> Symbol {
        Symbol::new(s)
    }
}

impl Default for Symbol {
    fn default() -> Symbol {
        Symbol::new("")
    }
}

impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            // Interning makes equal content share one `Arc`, so pointer equality is the fast path.
            (Repr::Heap(a), Repr::Heap(b)) => Arc::ptr_eq(a, b) || a == b,
            _ => self.as_str() == other.as_str(),
        }
    }
}

impl Eq for Symbol {}

impl PartialEq<str> for Symbol {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl Hash for Symbol {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl Ord for Symbol {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for Symbol {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl fmt::Debug for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Quote like `&str`/`Arc<str>` so `Minterm`/`Cube` Debug output is unchanged.
        fmt::Debug::fmt(self.as_str(), f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether a live `Symbol` for `s` is currently registered in the pool (test-only inspection).
    fn pool_has_live(s: &str) -> bool {
        POOL.lock().unwrap().contains(s)
    }

    const LONG: &str = "this_is_a_long_variable_name_well_over_the_inline_cap";

    #[test]
    fn roundtrip_inline_and_heap() {
        for s in ["", "a", "x0", "carry_in", LONG] {
            assert_eq!(Symbol::new(s).as_str(), s);
        }
    }

    #[test]
    fn inline_threshold() {
        let inline = "a".repeat(INLINE_CAP);
        let heap = "a".repeat(INLINE_CAP + 1);
        assert!(matches!(Symbol::new(&inline).0, Repr::Inline { .. }));
        assert!(matches!(Symbol::new(&heap).0, Repr::Heap(_)));
    }

    #[test]
    fn size_is_compact() {
        // Stay in the Arc<str>/String class — no bloat.
        assert!(std::mem::size_of::<Symbol>() <= std::mem::size_of::<String>());
    }

    #[test]
    fn long_names_are_deduplicated() {
        let a = Symbol::new(LONG);
        let b = Symbol::new(LONG);
        match (&a.0, &b.0) {
            (Repr::Heap(x), Repr::Heap(y)) => {
                assert!(Arc::ptr_eq(x, y), "interning must share one Arc")
            }
            _ => panic!("long names must be heap-interned"),
        }
        assert_eq!(a, b);
    }

    #[test]
    fn pool_prunes_on_drop() {
        let unique = format!("{LONG}_prune_probe");
        {
            let _s = Symbol::new(&unique);
            assert!(
                pool_has_live(&unique),
                "interned name should be live in the pool"
            );
        }
        // Last Symbol dropped → its Arc is freed → the weak entry no longer upgrades.
        assert!(!pool_has_live(&unique), "pool must not keep the name alive");
    }

    #[test]
    fn ord_hash_borrow_match_str() {
        assert!(Symbol::new("a") < Symbol::new("b"));
        assert_eq!(
            Symbol::new("foo").cmp(&Symbol::new("foo")),
            std::cmp::Ordering::Equal
        );

        // HashMap keyed by Symbol, queried by &str (relies on Borrow<str> + matching Hash/Eq).
        let mut map = std::collections::HashMap::new();
        map.insert(Symbol::new("key"), 42);
        assert_eq!(map.get("key"), Some(&42));

        // BTreeMap keyed by Symbol, queried by &str (relies on Borrow<str> + matching Ord).
        let mut bt = std::collections::BTreeMap::new();
        bt.insert(Symbol::new(LONG), 7);
        assert_eq!(bt.get(LONG), Some(&7));
    }

    #[test]
    fn is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Symbol>();
    }

    #[test]
    fn concurrent_interning_shares_one_arc() {
        use std::thread;
        let name = format!("{LONG}_concurrent");
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let n = name.clone();
                thread::spawn(move || Symbol::new(&n))
            })
            .collect();
        let syms: Vec<Symbol> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        for s in &syms[1..] {
            match (&syms[0].0, &s.0) {
                (Repr::Heap(a), Repr::Heap(b)) => assert!(Arc::ptr_eq(a, b)),
                _ => panic!("expected heap-interned symbols"),
            }
        }
    }
}
