//! M03 exit criteria: a realistic object graph — cyclic, cross-arena — round-trips through
//! serialization with **identity**, and every handle still resolves to the same object afterwards.
//!
//! The shape here is deliberately the one the pipeline will actually build: rooms referring to each
//! other (a loop, as any metroidvania map has), actors referring back to the room that contains them,
//! and vacant slots left by removals. If handles or generations shifted during a round-trip, the
//! back-references would silently repoint — the exact failure a reproduction bundle must never have.

use cv_core::serialize::{from_bytes, to_bytes, Deserialize, Reader, SerResult, Serialize, Writer};
use cv_core::{Arena, Handle, IdAllocator, Object, ObjectHeader, ObjectId};

// ---------------------------------------------------------------------------------------------
// A small two-type graph with cycles in it
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct Room {
    header: ObjectHeader,
    /// Rooms this one connects to — freely cyclic.
    exits: Vec<Handle<Room>>,
    /// The actor standing here, if any (a cross-arena reference).
    occupant: Option<Handle<Actor>>,
    volume: f64,
}

#[derive(Clone, Debug, PartialEq)]
struct Actor {
    header: ObjectHeader,
    /// Back-reference into the other arena, completing a cycle.
    home: Handle<Room>,
}

impl Object for Room {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut ObjectHeader {
        &mut self.header
    }
    fn type_name(&self) -> &'static str {
        "Room"
    }
}

impl Object for Actor {
    fn header(&self) -> &ObjectHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut ObjectHeader {
        &mut self.header
    }
    fn type_name(&self) -> &'static str {
        "Actor"
    }
}

impl Serialize for Room {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.header);
        w.write(&self.exits);
        w.write(&self.occupant);
        w.f64(self.volume);
    }
}

impl Deserialize for Room {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Room {
            header: r.read()?,
            exits: r.read()?,
            occupant: r.read()?,
            volume: r.f64()?,
        })
    }
}

impl Serialize for Actor {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.header);
        w.write(&self.home);
    }
}

impl Deserialize for Actor {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(Actor {
            header: r.read()?,
            home: r.read()?,
        })
    }
}

/// Everything a generated world owns, as the real `World` will.
#[derive(Clone, Debug, PartialEq)]
struct World {
    ids: IdAllocator,
    rooms: Arena<Room>,
    actors: Arena<Actor>,
}

impl Serialize for World {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.ids);
        w.write(&self.rooms);
        w.write(&self.actors);
    }
}

impl Deserialize for World {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(World {
            ids: r.read()?,
            rooms: r.read()?,
            actors: r.read()?,
        })
    }
}

/// Build a world with a cycle, a cross-arena back-reference, and holes from removals.
fn build_world() -> (World, Vec<Handle<Room>>, Handle<Actor>) {
    let mut ids = IdAllocator::new();
    let mut rooms: Arena<Room> = Arena::new();
    let mut actors: Arena<Actor> = Arena::new();

    let handles: Vec<Handle<Room>> = (0..5)
        .map(|i| {
            rooms.insert(Room {
                header: ObjectHeader::new(ids.allocate(), format!("room_{i}")),
                exits: Vec::new(),
                occupant: None,
                volume: (i as f64) * 12.5,
            })
        })
        .collect();

    // Leave holes so the free list and vacant slots are exercised too.
    rooms.remove(handles[3]);

    // A cycle: 0 → 1 → 2 → 0, plus a self-loop on 4.
    rooms[handles[0]].exits = vec![handles[1]];
    rooms[handles[1]].exits = vec![handles[2]];
    rooms[handles[2]].exits = vec![handles[0], handles[4]];
    rooms[handles[4]].exits = vec![handles[4]];

    // Cross-arena cycle: room 2 holds an actor whose home is room 2.
    let actor = actors.insert(Actor {
        header: ObjectHeader::new(ids.allocate(), "wanderer"),
        home: handles[2],
    });
    rooms[handles[2]].occupant = Some(actor);

    (World { ids, rooms, actors }, handles, actor)
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[test]
fn world_round_trips_to_identity() {
    let (world, _, _) = build_world();
    let bytes = to_bytes(&world);
    let back: World = from_bytes(&bytes).expect("world deserializes");
    assert_eq!(back, world, "round-trip must be identity");
    // And re-serializing yields the identical bytes — the property a reproduction bundle needs.
    assert_eq!(to_bytes(&back), bytes);
}

#[test]
fn handles_still_resolve_after_a_round_trip() {
    let (world, rooms, actor) = build_world();
    let back: World = from_bytes(&to_bytes(&world)).unwrap();

    // Every handle taken *before* serialization still addresses the same object afterwards.
    for (i, h) in rooms.iter().enumerate() {
        match world.rooms.get(*h) {
            Some(original) => {
                let restored = back.rooms.get(*h).expect("live handle must survive");
                assert_eq!(restored, original, "room {i} changed across the round-trip");
            }
            None => {
                // The removed room must still be absent — its slot must not have been refilled.
                assert!(
                    back.rooms.get(*h).is_none(),
                    "removed room {i} came back to life"
                );
            }
        }
    }
    assert_eq!(back.actors.get(actor).unwrap().home, rooms[2]);
}

#[test]
fn cycles_survive_and_remain_traversable() {
    let (world, rooms, _) = build_world();
    let back: World = from_bytes(&to_bytes(&world)).unwrap();

    // Walk the 0 → 1 → 2 → 0 loop and confirm it closes.
    let mut at = rooms[0];
    for _ in 0..3 {
        at = back.rooms[at].exits[0];
    }
    assert_eq!(at, rooms[0], "the cycle must close back on itself");

    // The self-loop is intact.
    assert_eq!(back.rooms[rooms[4]].exits[0], rooms[4]);

    // The cross-arena cycle (room → actor → room) is intact.
    let occupant = back.rooms[rooms[2]]
        .occupant
        .expect("room 2 has an occupant");
    assert_eq!(back.actors[occupant].home, rooms[2]);
}

#[test]
fn arena_layout_is_preserved_not_compacted() {
    // Compacting away the vacant slot would renumber later rooms and silently repoint every handle.
    let (world, rooms, _) = build_world();
    let back: World = from_bytes(&to_bytes(&world)).unwrap();
    assert_eq!(back.rooms.slot_count(), world.rooms.slot_count());
    assert_eq!(back.rooms.len(), world.rooms.len());
    assert_eq!(back.rooms.len(), 4, "one of five rooms was removed");

    // The freed slot is still free, and reuse continues from where it left off — identically.
    let mut a = world.clone();
    let mut b = back;
    let new_a = a.rooms.insert(a.rooms[rooms[0]].clone());
    let new_b = b.rooms.insert(b.rooms[rooms[0]].clone());
    assert_eq!(
        new_a, new_b,
        "post-round-trip allocation must match the original"
    );
    assert_eq!(
        new_a.index(),
        rooms[3].index(),
        "the vacated slot should be reused"
    );
    assert_ne!(
        new_a.generation(),
        rooms[3].generation(),
        "with a bumped generation"
    );
}

#[test]
fn id_allocator_does_not_reissue_after_a_round_trip() {
    let (world, _, _) = build_world();
    let mut back: World = from_bytes(&to_bytes(&world)).unwrap();
    let next = back.ids.allocate();
    assert_eq!(
        next.to_raw(),
        world.ids.peek(),
        "resumed allocation must continue, not restart"
    );
    // The new id collides with nothing already in the world.
    let existing: Vec<ObjectId> = world.rooms.values().map(|r| r.id()).collect();
    assert!(!existing.contains(&next));
}

#[test]
fn serialization_is_byte_stable_across_rebuilds() {
    // Two independently constructed but identical worlds must serialize identically — no address,
    // iteration-order, or hash-seed dependence anywhere in the data model.
    let (a, _, _) = build_world();
    let (b, _, _) = build_world();
    assert_eq!(to_bytes(&a), to_bytes(&b));
}

#[test]
fn truncated_worlds_fail_cleanly() {
    let (world, _, _) = build_world();
    let bytes = to_bytes(&world);
    // Every truncation must be an error, never a panic or a partially-formed world.
    for cut in 0..bytes.len() {
        let result = from_bytes::<World>(&bytes[..cut]);
        assert!(result.is_err(), "truncation at {cut} should not parse");
    }
}
