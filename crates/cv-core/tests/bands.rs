//! **Elevation bands** — the gap M20 P03 was told to close.
//!
//! ⚠ **The design named neither the algorithm nor the parameter**, only the two ways it goes wrong:
//! *too tight and every room gets its own band; too loose and a mezzanine merges with a ground floor.*
//! Both are asserted here, along with the third failure the design did not name — chaining.

use cv_core::floor::{band_of, bands, FloorSurface};
use cv_core::object::ObjectId;
use cv_determinism::geom::{Aabb, Vec3};

const STANDING: f64 = 1.9;

fn at(owner: &str, y: f64) -> FloorSurface {
    FloorSurface {
        owner: ObjectId::derived("space", owner),
        patch: Aabb {
            min: Vec3 { x: 0.0, y, z: 0.0 },
            max: Vec3 { x: 4.0, y, z: 4.0 },
        },
        slope: 0.0,
    }
}

#[test]
fn an_uneven_floor_is_one_band() {
    // ⚠ **Too tight and every room gets its own band.** A floor with a step in it is still one floor.
    let out = bands(&[at("a", 0.0), at("b", 0.12), at("c", 0.4)], STANDING);
    assert_eq!(out.len(), 1, "{out:?}");
    assert_eq!(out[0].members.len(), 3);
}

#[test]
fn a_mezzanine_does_not_merge_with_the_ground_floor() {
    // ⚠ **Too loose and a mezzanine merges with a ground floor** — the other named failure.
    let out = bands(&[at("ground", 0.0), at("mezzanine", 3.0)], STANDING);
    assert_eq!(out.len(), 2, "{out:?}");
    assert!(out[1].low > out[0].high);
}

#[test]
fn a_gradient_does_not_chain_three_storeys_into_one_band() {
    // ⚠ **The failure the design did not name.** Surfaces at 0, 1.8, 3.6, 5.4 are each within a
    // tolerance of the last, so gap-based clustering alone merges three storeys — and a
    // layer-isolation control that cannot separate storeys is useless in exactly the buildings it
    // matters most for.
    let out = bands(
        &[at("a", 0.0), at("b", 1.8), at("c", 3.6), at("d", 5.4)],
        STANDING,
    );
    assert!(out.len() > 1, "a gradient chained into one band: {out:?}");
    for band in &out {
        assert!(
            band.span() <= STANDING * 2.0 + f64::EPSILON,
            "a band spans more than two standing heights: {band:?}"
        );
    }
}

#[test]
fn the_tolerance_is_the_players_own_height() {
    // ⚠ Two surfaces you cannot stand between are one level; two you can are two. Scaling the world
    // scales the banding, with no dial of its own.
    let tight = bands(&[at("a", 0.0), at("b", 1.5)], STANDING);
    assert_eq!(
        tight.len(),
        1,
        "1.5 apart is under a standing height: {tight:?}"
    );

    let scaled = bands(&[at("a", 0.0), at("b", 1.5)], 1.0);
    assert_eq!(
        scaled.len(),
        2,
        "at a smaller stature the same gap is two levels: {scaled:?}"
    );
}

#[test]
fn bands_are_numbered_from_the_bottom() {
    let out = bands(
        &[at("top", 6.0), at("bottom", 0.0), at("middle", 3.0)],
        STANDING,
    );
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].index, 0);
    assert!(out[0].low < out[2].low, "index 0 is the lowest");
}

#[test]
fn the_same_input_bands_the_same_way_whatever_order_it_arrives_in() {
    // ⚠ **Determinism.** Two surfaces at one height must land in the same order on every machine, or
    // the band a room belongs to depends on a hash.
    let forward = bands(&[at("a", 0.0), at("b", 0.0), at("c", 4.0)], STANDING);
    let backward = bands(&[at("c", 4.0), at("b", 0.0), at("a", 0.0)], STANDING);
    assert_eq!(forward, backward);
}

#[test]
fn an_off_band_elevation_answers_none_rather_than_guessing() {
    // ⚠ **`None` is a real answer.** The view dims an off-band node rather than hiding it, so spatial
    // context survives — a nearest-band guess would put it somewhere it is not.
    let out = bands(&[at("a", 0.0), at("b", 4.0)], STANDING);
    assert_eq!(band_of(&out, 0.0), Some(0));
    assert_eq!(band_of(&out, 4.0), Some(1));
    assert_eq!(band_of(&out, 2.0), None, "between bands is between bands");
}

#[test]
fn no_surfaces_is_no_bands() {
    assert!(bands(&[], STANDING).is_empty());
}
