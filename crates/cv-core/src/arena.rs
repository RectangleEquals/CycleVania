//! The generational arena every generated object lives in, and the typed handles that address it.
//!
//! # Why handles rather than references
//!
//! Generation builds a *graph*: a `Space` refers to its `Actor`s, an `Actor` refers to the `Node` it
//! sits in, a `Puzzle` refers to elements scattered across `Space`s. Rust references cannot express
//! that (cycles, no single owner), and `Rc<RefCell<_>>` would trade compile-time safety for runtime
//! panics while making serialization miserable.
//!
//! A [`Handle`] is instead a plain `(index, generation)` pair — an integer. That buys four things the
//! engine specifically needs:
//!
//! * **Cycles are free.** Handles are not ownership, so `A → B → A` is unremarkable.
//! * **Serialization is trivial and stable.** A handle is two `u32`s; an arena round-trips without any
//!   pointer fix-up, so reproduction bundles and editor round-trips are cheap.
//! * **It is safe to hand to the VM.** A script never receives a pointer, only an opaque integer that
//!   the core validates on every access ([`Handle::to_raw`]).
//! * **Stale access is caught, not undefined.** Reusing a slot bumps its generation, so a handle to a
//!   removed object fails a *check* rather than silently aliasing whatever took its place — the classic
//!   dangling-index bug.
//!
//! # Determinism
//!
//! Slots are allocated from a LIFO free list and iteration is in slot-index order, so given the same
//! sequence of operations an arena has the same layout, the same handles, and the same iteration order
//! on every run and every target. Nothing here consults an address, a clock, or a hash-map ordering.

use std::marker::PhantomData;
use std::num::NonZeroU32;

/// A typed reference to a value in an [`Arena`].
///
/// `Copy`, cheap, and comparable. The type parameter is phantom — it exists so a `Handle<Actor>`
/// cannot be passed to an `Arena<Node>`, a mistake that would otherwise be an index typo.
pub struct Handle<T> {
    index: u32,
    generation: NonZeroU32,
    /// `fn() -> T` keeps `Handle<T>` `Send`/`Sync`/`Copy` regardless of what `T` is.
    _marker: PhantomData<fn() -> T>,
}

impl<T> Handle<T> {
    /// Construct from raw parts. Only meaningful for a handle this arena actually issued.
    fn new(index: u32, generation: NonZeroU32) -> Self {
        Handle {
            index,
            generation,
            _marker: PhantomData,
        }
    }

    /// The slot this handle addresses.
    #[inline]
    pub fn index(self) -> u32 {
        self.index
    }

    /// The generation this handle was issued at.
    #[inline]
    pub fn generation(self) -> u32 {
        self.generation.get()
    }

    /// Pack into a single opaque `u64` — the form handed across the VM boundary, where a script sees
    /// only an integer it cannot forge into a pointer.
    #[inline]
    pub fn to_raw(self) -> u64 {
        ((self.index as u64) << 32) | (self.generation.get() as u64)
    }

    /// Unpack a handle produced by [`Handle::to_raw`]. Returns `None` for a malformed value, so a
    /// hostile or buggy script cannot conjure a handle that skips validation.
    #[inline]
    pub fn from_raw(raw: u64) -> Option<Self> {
        let generation = NonZeroU32::new(raw as u32)?;
        Some(Handle::new((raw >> 32) as u32, generation))
    }
}

// Manual impls throughout: deriving would wrongly require `T` to implement each trait, even though a
// handle holds no `T`.
impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Handle<T> {}
impl<T> PartialEq for Handle<T> {
    fn eq(&self, o: &Self) -> bool {
        self.index == o.index && self.generation == o.generation
    }
}
impl<T> Eq for Handle<T> {}
impl<T> PartialOrd for Handle<T> {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl<T> Ord for Handle<T> {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        // Index first, so sorting handles gives arena order — a deterministic, meaningful sequence.
        self.index
            .cmp(&o.index)
            .then(self.generation.cmp(&o.generation))
    }
}
impl<T> std::hash::Hash for Handle<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.index.hash(state);
        self.generation.hash(state);
    }
}
impl<T> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Handle<{}>({}v{})",
            short_type_name::<T>(),
            self.index,
            self.generation
        )
    }
}

/// The last path segment of a type name, for readable `Debug` output.
fn short_type_name<T>() -> &'static str {
    let full = std::any::type_name::<T>();
    full.rsplit("::").next().unwrap_or(full)
}

/// One arena slot: a generation counter plus an optional value.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Slot<T> {
    /// Bumped every time the slot is vacated, invalidating handles issued against the old value.
    generation: NonZeroU32,
    value: Option<T>,
}

/// A generational arena — a `Vec` of slots addressed by [`Handle`], with safe reuse.
///
/// Contains no `unsafe` (the crate forbids it), so the "stale handle" guarantee is enforced by an
/// explicit generation check rather than by hoping callers behave.
///
/// Equality compares the **whole layout** — generations and vacant slots included — not just the live
/// values, because two arenas holding equal values at different indices are *not* interchangeable:
/// handles into one would not address the same things in the other.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arena<T> {
    slots: Vec<Slot<T>>,
    /// Vacant slot indices, used LIFO. A `Vec` (not a `HashSet`) so reuse order is deterministic.
    free: Vec<u32>,
    len: usize,
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena::new()
    }
}

impl<T> Arena<T> {
    /// An empty arena.
    pub fn new() -> Self {
        Arena {
            slots: Vec::new(),
            free: Vec::new(),
            len: 0,
        }
    }

    /// An empty arena with room for `capacity` values.
    pub fn with_capacity(capacity: usize) -> Self {
        Arena {
            slots: Vec::with_capacity(capacity),
            free: Vec::new(),
            len: 0,
        }
    }

    /// How many values are currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Are there no values stored?
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// How many slots exist, live or vacant. Handles only ever address `< slot_count()`.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Insert a value, returning a handle to it.
    ///
    /// # Panics
    /// If the arena would exceed `u32::MAX` slots.
    pub fn insert(&mut self, value: T) -> Handle<T> {
        self.len += 1;
        if let Some(index) = self.free.pop() {
            let slot = &mut self.slots[index as usize];
            debug_assert!(
                slot.value.is_none(),
                "free list pointed at an occupied slot"
            );
            slot.value = Some(value);
            return Handle::new(index, slot.generation);
        }
        let index = u32::try_from(self.slots.len()).expect("arena exceeded u32::MAX slots");
        let generation = NonZeroU32::new(1).expect("1 is non-zero");
        self.slots.push(Slot {
            generation,
            value: Some(value),
        });
        Handle::new(index, generation)
    }

    /// Borrow a value, or `None` if the handle is stale or out of range.
    pub fn get(&self, handle: Handle<T>) -> Option<&T> {
        let slot = self.slots.get(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None; // the slot was reused; this handle refers to a dead value
        }
        slot.value.as_ref()
    }

    /// Mutably borrow a value, or `None` if the handle is stale or out of range.
    pub fn get_mut(&mut self, handle: Handle<T>) -> Option<&mut T> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        slot.value.as_mut()
    }

    /// Is this handle still live?
    pub fn contains(&self, handle: Handle<T>) -> bool {
        self.get(handle).is_some()
    }

    /// Remove a value, returning it. A stale handle yields `None` and changes nothing.
    ///
    /// The slot's generation is bumped so every outstanding handle to it becomes stale. If the
    /// generation would overflow, the slot is **retired** (never reused) rather than wrapping around
    /// and silently revalidating ancient handles.
    pub fn remove(&mut self, handle: Handle<T>) -> Option<T> {
        let slot = self.slots.get_mut(handle.index as usize)?;
        if slot.generation != handle.generation {
            return None;
        }
        let value = slot.value.take()?;
        self.len -= 1;
        match NonZeroU32::new(slot.generation.get().wrapping_add(1)) {
            Some(next) if next.get() > slot.generation.get() => {
                slot.generation = next;
                self.free.push(handle.index);
            }
            // Saturated: leave the generation pinned and never hand this slot out again.
            _ => {}
        }
        Some(value)
    }

    /// Remove every value. Generations are bumped, so all existing handles become stale.
    pub fn clear(&mut self) {
        let live: Vec<Handle<T>> = self.handles().collect();
        for h in live {
            self.remove(h);
        }
    }

    /// Iterate live `(handle, &value)` pairs in **slot-index order** — deterministic.
    pub fn iter(&self) -> impl Iterator<Item = (Handle<T>, &T)> + '_ {
        self.slots.iter().enumerate().filter_map(|(i, slot)| {
            slot.value
                .as_ref()
                .map(|v| (Handle::new(i as u32, slot.generation), v))
        })
    }

    /// Iterate live `(handle, &mut value)` pairs in slot-index order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Handle<T>, &mut T)> + '_ {
        self.slots.iter_mut().enumerate().filter_map(|(i, slot)| {
            let generation = slot.generation;
            slot.value
                .as_mut()
                .map(move |v| (Handle::new(i as u32, generation), v))
        })
    }

    /// Iterate live values in slot-index order.
    pub fn values(&self) -> impl Iterator<Item = &T> + '_ {
        self.slots.iter().filter_map(|slot| slot.value.as_ref())
    }

    /// Iterate live handles in slot-index order.
    pub fn handles(&self) -> impl Iterator<Item = Handle<T>> + '_ {
        self.iter().map(|(h, _)| h)
    }

    // --- Serialization support (see `serialize.rs`) ------------------------------------------

    /// The raw slot table: `(generation, Option<&value>)` in index order.
    pub(crate) fn raw_slots(&self) -> impl Iterator<Item = (u32, Option<&T>)> + '_ {
        self.slots
            .iter()
            .map(|s| (s.generation.get(), s.value.as_ref()))
    }

    /// The free list, in order.
    pub(crate) fn raw_free(&self) -> &[u32] {
        &self.free
    }

    /// Rebuild an arena from a raw slot table and free list, exactly preserving handle validity.
    pub(crate) fn from_raw_parts(slots: Vec<(u32, Option<T>)>, free: Vec<u32>) -> Option<Self> {
        let mut out = Vec::with_capacity(slots.len());
        let mut len = 0usize;
        for (generation, value) in slots {
            let generation = NonZeroU32::new(generation)?; // generation 0 is never valid
            if value.is_some() {
                len += 1;
            }
            out.push(Slot { generation, value });
        }
        // A free index must be in range and actually vacant, or handle validity is compromised.
        for &i in &free {
            match out.get(i as usize) {
                Some(slot) if slot.value.is_none() => {}
                _ => return None,
            }
        }
        Some(Arena {
            slots: out,
            free,
            len,
        })
    }
}

impl<T> std::ops::Index<Handle<T>> for Arena<T> {
    type Output = T;
    /// # Panics
    /// If the handle is stale. Use [`Arena::get`] when that is a possibility.
    fn index(&self, handle: Handle<T>) -> &T {
        self.get(handle).expect("stale or out-of-range handle")
    }
}

impl<T> std::ops::IndexMut<Handle<T>> for Arena<T> {
    fn index_mut(&mut self, handle: Handle<T>) -> &mut T {
        self.get_mut(handle).expect("stale or out-of-range handle")
    }
}

impl<T> FromIterator<T> for Arena<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut arena = Arena::new();
        for v in iter {
            arena.insert(v);
        }
        arena
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_get_remove() {
        let mut a: Arena<&str> = Arena::new();
        let x = a.insert("x");
        let y = a.insert("y");
        assert_eq!(a.len(), 2);
        assert_eq!(a.get(x), Some(&"x"));
        assert_eq!(a[y], "y");
        assert_eq!(a.remove(x), Some("x"));
        assert_eq!(a.len(), 1);
        assert_eq!(a.get(x), None);
        assert!(!a.contains(x));
        assert!(a.contains(y));
        // Removing twice is a no-op, not a panic or a double-free.
        assert_eq!(a.remove(x), None);
    }

    #[test]
    fn stale_handles_are_rejected_after_slot_reuse() {
        let mut a: Arena<u32> = Arena::new();
        let old = a.insert(1);
        a.remove(old);
        let new = a.insert(2);
        // The slot was reused...
        assert_eq!(old.index(), new.index(), "expected LIFO slot reuse");
        // ...but the generation moved on, so the old handle does not alias the new value.
        assert_ne!(old.generation(), new.generation());
        assert_eq!(
            a.get(old),
            None,
            "stale handle must not read the new occupant"
        );
        assert_eq!(a.get(new), Some(&2));
        assert_eq!(
            a.remove(old),
            None,
            "stale handle must not remove the new occupant"
        );
        assert_eq!(a.get(new), Some(&2));
    }

    #[test]
    fn mutation_through_handles() {
        let mut a: Arena<i32> = Arena::new();
        let h = a.insert(10);
        *a.get_mut(h).unwrap() += 5;
        a[h] *= 2;
        assert_eq!(a[h], 30);
    }

    #[test]
    fn iteration_is_slot_ordered_and_deterministic() {
        let mut a: Arena<u32> = Arena::new();
        let handles: Vec<_> = (0..8).map(|i| a.insert(i)).collect();
        a.remove(handles[2]);
        a.remove(handles[5]);
        let seen: Vec<u32> = a.values().copied().collect();
        assert_eq!(seen, vec![0, 1, 3, 4, 6, 7]);
        // Repeated iteration is identical, and handles come back in index order.
        assert_eq!(seen, a.values().copied().collect::<Vec<_>>());
        let idx: Vec<u32> = a.handles().map(|h| h.index()).collect();
        assert!(idx.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn free_list_reuse_is_deterministic() {
        // The same operation sequence must produce the same handles every time — this is what makes
        // a generated world reproducible.
        fn run() -> Vec<u64> {
            let mut a: Arena<u32> = Arena::new();
            let hs: Vec<_> = (0..6).map(|i| a.insert(i)).collect();
            a.remove(hs[1]);
            a.remove(hs[4]);
            a.remove(hs[0]);
            let more: Vec<_> = (100..104).map(|i| a.insert(i)).collect();
            more.iter().map(|h| h.to_raw()).collect()
        }
        assert_eq!(run(), run());
    }

    #[test]
    fn raw_round_trip_is_lossless() {
        let mut a: Arena<u8> = Arena::new();
        let h = a.insert(7);
        let raw = h.to_raw();
        assert_eq!(Handle::<u8>::from_raw(raw), Some(h));
        assert_eq!(a.get(Handle::from_raw(raw).unwrap()), Some(&7));
        // Generation 0 is never valid, so a zeroed/forged value cannot become a handle.
        assert_eq!(Handle::<u8>::from_raw(0), None);
        assert_eq!(Handle::<u8>::from_raw(1u64 << 32), None);
    }

    #[test]
    fn clear_invalidates_every_handle() {
        let mut a: Arena<u32> = Arena::new();
        let hs: Vec<_> = (0..4).map(|i| a.insert(i)).collect();
        a.clear();
        assert!(a.is_empty());
        assert_eq!(a.len(), 0);
        for h in hs {
            assert_eq!(a.get(h), None);
        }
        // Slots remain for reuse.
        assert_eq!(a.slot_count(), 4);
    }

    #[test]
    fn handles_sort_in_arena_order() {
        let mut a: Arena<u32> = Arena::new();
        let mut hs: Vec<_> = (0..5).map(|i| a.insert(i)).collect();
        hs.reverse();
        hs.sort();
        assert_eq!(
            hs.iter().map(|h| h.index()).collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
    }

    #[test]
    fn equality_compares_layout_not_just_contents() {
        // Same values, but one arena reached them through a removal — so handles differ, and the
        // arenas are correctly *not* equal.
        let mut a: Arena<u32> = Arena::new();
        a.insert(1);
        a.insert(2);

        let mut b: Arena<u32> = Arena::new();
        let tmp = b.insert(99);
        b.remove(tmp);
        b.insert(1);
        b.insert(2);

        assert_eq!(
            a.values().copied().collect::<Vec<_>>(),
            b.values().copied().collect::<Vec<_>>()
        );
        assert_ne!(a, b, "differing generations must not compare equal");

        // Identically-built arenas are equal.
        let mut c: Arena<u32> = Arena::new();
        c.insert(1);
        c.insert(2);
        assert_eq!(a, c);
    }

    #[test]
    fn collects_from_iterator() {
        let a: Arena<u32> = (0..5).collect();
        assert_eq!(a.len(), 5);
        assert_eq!(a.values().copied().collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
    }
}
