//! [`Symbol`]: a compact, interned string for variable names.
//!
//! Variable names flow through every layer of the crate (BDD manager, cover labels, cubes). `Symbol`
//! is the storage we use for them, tuned for that workload:
//!
//! - **Small-string optimised.** A name of up to `INLINE_CAP` bytes — the inline capacity, derived from
//!   the platform's `String` size — lives **inline**; this is virtually every real variable name
//!   (`a`, `x0`, `carry_in`, …), with no heap allocation and an O(1),
//!   `memcpy`-cheap [`Clone`]. An `Arc<str>` would heap-allocate a refcount header plus the bytes even
//!   for `"a"`.
//! - **Interned.** Longer names are deduplicated through a process-global pool, so equal names share
//!   one allocation and compare in O(1) by pointer. The pool holds **weak** references, so it never
//!   keeps a name alive: when the last `Symbol` for a name drops, its allocation is freed and the dead
//!   pool entry is pruned. The pool is bounded to the long names currently live — it cannot leak.
//!
//! `Symbol`'s `Ord`, `Eq` and `Hash` all act on the string **content** and agree with `str`'s, so a
//! `BTreeMap<Symbol, _>` / `HashMap<Symbol, _>` can be looked up with a `&str` key (via [`Borrow`]).

use std::borrow::{Borrow, Cow};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, LazyLock, Mutex, Weak};
use weak_table::WeakHashSet;

/// Maximum byte length stored inline (without heap allocation). Derived from the platform's
/// `String` size — `size_of::<String>() - 2` (1 enum-tag byte + 1 `len: u8`) — rather than a
/// hardcoded constant, so `size_of::<Symbol>()` stays in the `Arc<str>`/`String` class on every
/// target: 22 bytes on 64-bit, 10 bytes on 32-bit (where `String` is 12 bytes; a hardcoded 22
/// would bloat `Symbol` to 24 bytes there). Guarded by the compile-time size-class assert below.
pub(crate) const INLINE_CAP: usize = std::mem::size_of::<String>() - 2;

/// The inline length is stored as a `u8` (`Repr::Inline { len: u8, .. }`), so the inline capacity
/// must fit in a `u8` for the `len = bytes.len() as u8` cast to be lossless.
const _: () = assert!(INLINE_CAP <= u8::MAX as usize);

/// A compact, interned variable-name string. See the [module docs](self).
#[derive(Clone)]
pub struct Symbol(Repr);

#[derive(Clone)]
enum Repr {
    /// Up to `INLINE_CAP` bytes stored inline (`buf[..len]` is valid UTF-8).
    Inline { len: u8, buf: [u8; INLINE_CAP] },
    /// A longer name, interned and shared.
    Heap(Arc<str>),
}

/// `Symbol`'s tag/niche layout is expected to keep it in the `String`/`Arc<str>` size class on
/// every target; this turns that assumption into a build error rather than a silent regression,
/// mirroring the `BPI` guard in `src/espresso/mod.rs`.
const _: () = assert!(std::mem::size_of::<Symbol>() <= std::mem::size_of::<String>());

impl Symbol {
    /// Intern a string as a `Symbol` (inline if short, pooled otherwise).
    ///
    /// Accepts any `&str`-like type with no privilege — `&str`, `String`, `Arc<str>`, `Box<str>`,
    /// `Cow<str>`, another `Symbol`, … all work through the single [`AsRef<str>`] bound, the same way
    /// [`Path::new`](std::path::Path::new) accepts any `AsRef<OsStr>`.
    #[must_use]
    pub fn new<S: AsRef<str>>(s: S) -> Symbol {
        let s = s.as_ref();
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
    #[must_use]
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

/// Like [`intern`], but reuses the caller's existing `Arc<str>` on a pool miss instead of allocating a
/// fresh copy. Lets `From<Arc<str>>` move a long name's allocation straight into the heap repr.
fn intern_arc(arc: Arc<str>) -> Arc<str> {
    let mut pool = POOL.lock().unwrap();
    if let Some(existing) = pool.get(&*arc) {
        return existing;
    }
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

impl From<&mut str> for Symbol {
    fn from(s: &mut str) -> Symbol {
        Symbol::new(s)
    }
}

impl From<Box<str>> for Symbol {
    fn from(s: Box<str>) -> Symbol {
        Symbol::new(s)
    }
}

impl From<Cow<'_, str>> for Symbol {
    fn from(s: Cow<'_, str>) -> Symbol {
        Symbol::new(s)
    }
}

/// Consumes the `Arc<str>`, reusing its allocation for a heap-stored (long) name rather than copying —
/// `Symbol`'s heap representation *is* an `Arc<str>`, so a long name interns by moving the incoming
/// allocation into the pool. Short names still go inline (the `Arc` is dropped).
impl From<Arc<str>> for Symbol {
    fn from(s: Arc<str>) -> Symbol {
        // `str::len()` is the UTF-8 **byte** length (not chars), which is what `INLINE_CAP`'s byte
        // capacity (`buf: [u8; INLINE_CAP]`) wants — so multibyte names split inline/heap by bytes,
        // exactly as in `Symbol::new`.
        if s.len() <= INLINE_CAP {
            Symbol::new(&*s)
        } else {
            Symbol(Repr::Heap(intern_arc(s)))
        }
    }
}

impl std::str::FromStr for Symbol {
    type Err = std::convert::Infallible;

    /// Interning a string into a [`Symbol`] never fails, so `"x".parse::<Symbol>()` is infallible.
    fn from_str(s: &str) -> Result<Symbol, Self::Err> {
        Ok(Symbol::new(s))
    }
}

impl Default for Symbol {
    /// The default `Symbol` is the **empty name** (`""`), stored inline. This is a valid, comparable
    /// `Symbol` — handy as a placeholder — but note it is not a meaningful variable name; an all-empty
    /// label table would make distinct positions compare equal.
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

// String comparison is symmetric and works against both `str` and `&str` in either order, mirroring
// the way `std`'s `String` compares against string slices.
impl PartialEq<str> for Symbol {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Symbol {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<Symbol> for str {
    fn eq(&self, other: &Symbol) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<Symbol> for &str {
    fn eq(&self, other: &Symbol) -> bool {
        *self == other.as_str()
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

impl PartialOrd<str> for Symbol {
    fn partial_cmp(&self, other: &str) -> Option<std::cmp::Ordering> {
        self.as_str().partial_cmp(other)
    }
}

impl PartialOrd<&str> for Symbol {
    fn partial_cmp(&self, other: &&str) -> Option<std::cmp::Ordering> {
        self.as_str().partial_cmp(*other)
    }
}

impl PartialOrd<Symbol> for str {
    fn partial_cmp(&self, other: &Symbol) -> Option<std::cmp::Ordering> {
        self.partial_cmp(other.as_str())
    }
}

impl PartialOrd<Symbol> for &str {
    fn partial_cmp(&self, other: &Symbol) -> Option<std::cmp::Ordering> {
        (*self).partial_cmp(other.as_str())
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
    fn new_accepts_any_str_type_without_privilege() {
        use std::borrow::Cow;
        // Every `&str`-like type constructs a `Symbol` directly — no `String` detour, no privilege.
        assert_eq!(Symbol::new("a"), Symbol::new(String::from("a")));
        assert_eq!(Symbol::new("a"), Symbol::new(Arc::<str>::from("a")));
        assert_eq!(Symbol::new("a"), Symbol::new(Box::<str>::from("a")));
        assert_eq!(Symbol::new("a"), Symbol::new(Cow::Borrowed("a")));
        assert_eq!(Symbol::new("a"), Symbol::new(Symbol::new("a")));
        // Long (heap-interned) names too.
        assert_eq!(Symbol::new(LONG), Symbol::new(Arc::<str>::from(LONG)));
    }

    #[test]
    fn from_accepts_common_string_types() {
        // Every common owned/shared/borrowed string type converts via `From`/`.into()` with no privilege.
        let mut owned = String::from("a");
        assert_eq!(Symbol::from("a"), Symbol::from(String::from("a")));
        assert_eq!(Symbol::from("a"), Symbol::from(&String::from("a")));
        assert_eq!(Symbol::from("a"), Symbol::from(owned.as_mut_str()));
        assert_eq!(Symbol::from("a"), Symbol::from(Box::<str>::from("a")));
        assert_eq!(Symbol::from("a"), Symbol::from(Arc::<str>::from("a")));
        assert_eq!(Symbol::from("a"), Symbol::from(Cow::Borrowed("a")));
        assert_eq!(
            Symbol::from("a"),
            Symbol::from(Cow::<str>::Owned("a".into()))
        );
    }

    #[test]
    fn from_arc_preserves_interning() {
        // A long name built from an `Arc<str>` shares one interned allocation with the same name built
        // any other way — `From<Arc<str>>` must go through the pool, not stash a private allocation.
        let from_arc = Symbol::from(Arc::<str>::from(LONG));
        let from_str = Symbol::new(LONG);
        match (&from_arc.0, &from_str.0) {
            (Repr::Heap(a), Repr::Heap(b)) => {
                assert!(Arc::ptr_eq(a, b), "must share one interned Arc")
            }
            _ => panic!("long names must be heap-interned"),
        }
        // A short name from an `Arc<str>` goes inline (the Arc is dropped), like any short name.
        assert!(matches!(
            Symbol::from(Arc::<str>::from("a")).0,
            Repr::Inline { .. }
        ));
    }

    #[test]
    fn from_arc_splits_inline_heap_by_bytes_not_chars() {
        // 'é' is 2 bytes. `size_of::<String>()` is 3 words, so INLINE_CAP is even on real targets:
        // INLINE_CAP / 2 copies of 'é' exactly fill it (bytes == INLINE_CAP) → inline; one byte more
        // → heap. A char-count split would wrongly inline a name that overflows the buffer.
        let mut fits = "é".repeat(INLINE_CAP / 2);
        if INLINE_CAP % 2 == 1 {
            fits.push('a');
        }
        assert_eq!(fits.len(), INLINE_CAP); // str::len() is bytes
        let over = format!("{fits}a");
        assert!(matches!(
            Symbol::from(Arc::<str>::from(fits.as_str())).0,
            Repr::Inline { .. }
        ));
        assert!(matches!(
            Symbol::from(Arc::<str>::from(over.as_str())).0,
            Repr::Heap(_)
        ));
        // Content is preserved across the split either way.
        assert_eq!(Symbol::from(Arc::<str>::from(over.as_str())).as_str(), over);
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
    fn compares_against_str_in_both_directions() {
        use std::cmp::Ordering;
        let sym = Symbol::new("m");

        // Equality, both orders, against `&str` and `str`.
        assert!(sym == "m");
        assert!("m" == sym);
        assert!(*"m" == sym);
        assert!(sym != "n");
        assert!("n" != sym);

        // Ordering, both orders, against `&str`.
        assert!(sym < "n");
        assert!(sym > "a");
        assert!("a" < sym);
        assert!("n" > sym);
        assert_eq!(sym.partial_cmp("m"), Some(Ordering::Equal));
        assert_eq!("m".partial_cmp(&sym), Some(Ordering::Equal));
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

    #[test]
    fn concurrent_distinct_intern_and_drop() {
        use std::thread;
        // Many threads interning DISTINCT long names while others drop theirs — exercises the weak
        // set's prune-on-drop path racing against the insert path. Invariants: a name never aliases
        // the wrong `Arc` (checked inline), and once every `Symbol` has dropped the pool keeps none
        // of them alive.
        let threads = 8;
        let per_thread = 200;
        let handles: Vec<_> = (0..threads)
            .map(|t| {
                thread::spawn(move || {
                    for i in 0..per_thread {
                        let name = format!("{LONG}_distinct_{t}_{i}");
                        let s = Symbol::new(&name);
                        assert_eq!(s.as_str(), name); // no cross-thread aliasing
                        drop(s); // immediate drop prunes this weak entry
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        for t in 0..threads {
            for i in [0, per_thread - 1] {
                let name = format!("{LONG}_distinct_{t}_{i}");
                assert!(!pool_has_live(&name), "pool must not keep {name} alive");
            }
        }
    }
}
