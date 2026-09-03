//! **Mesh import** — glTF, glb and obj, to triangles and a derived-default collider.
//!
//! ⚠ **What the core wants from a mesh is triangles and a bound, not a scene.** Materials, cameras,
//! animations and node hierarchies are the host's business; the generator asks *"what shape is this,
//! roughly"* and *"exactly, at L4"*. Importing more than that would mean carrying a scene graph nothing
//! reads.
//!
//! # The collider is *derived*, and derivation is a default rather than a decision
//!
//! ⚠ **A default collider is a starting point a developer overrides, never a fact.** The design's
//! fidelity ladder is `AABB ⊇ hull ⊇ mildly concave ⊇ realized`, and an import can only honestly
//! produce the first rung: it has the triangles but not the intent. So the import reports **an AABB and
//! the triangles it came from**, and anything tighter is a decision somebody makes.
//!
//! # Lazy: a mesh need not be resident to be referenced
//!
//! ⚠ **The solve reasons about a *path* and a bound**, not about vertices. [`MeshRef`] is what a
//! schematic's `Asset'…'` resolves to during generation, and it carries enough to place something —
//! which is why a hundred-megabyte mesh costs nothing until L4 asks for it.

use cv_determinism::Vec3;
use std::fmt;

/// Why a mesh did not import.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MeshError {
    /// The extension is not one that is imported.
    UnknownFormat { extension: String },
    /// The bytes are not what the extension claims.
    Malformed { detail: String },
    /// A face index points past the vertex list.
    ///
    /// ⚠ **Refused rather than clamped.** A clamped index makes a triangle nobody authored, and a mesh
    /// with one silently wrong face is worse than a mesh that did not load.
    IndexOutOfRange { face: usize, index: usize },
    /// The file imported cleanly and contains no triangles.
    Empty,
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MeshError::UnknownFormat { extension } => write!(
                f,
                "{extension:?} is not an imported mesh format — the set is .obj, .gltf and .glb"
            ),
            MeshError::Malformed { detail } => write!(f, "malformed mesh: {detail}"),
            MeshError::IndexOutOfRange { face, index } => write!(
                f,
                "face {face} names vertex {index}, which does not exist — refused rather than \
                 clamped, because a clamped index makes a triangle nobody authored"
            ),
            MeshError::Empty => write!(f, "the file imported cleanly and has no triangles"),
        }
    }
}

impl std::error::Error for MeshError {}

/// An axis-aligned bound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// The low corner.
    pub min: Vec3,
    /// The high corner.
    pub max: Vec3,
}

impl Bounds {
    /// The bound of a point set.
    pub fn of(points: &[Vec3]) -> Option<Bounds> {
        let first = *points.first()?;
        let mut b = Bounds {
            min: first,
            max: first,
        };
        for p in points {
            b.min = Vec3::new(b.min.x.min(p.x), b.min.y.min(p.y), b.min.z.min(p.z));
            b.max = Vec3::new(b.max.x.max(p.x), b.max.y.max(p.y), b.max.z.max(p.z));
        }
        Some(b)
    }

    /// Its extents.
    pub fn extents(&self) -> Vec3 {
        self.max - self.min
    }

    /// Its centre.
    pub fn centre(&self) -> Vec3 {
        (self.min + self.max).scale(0.5)
    }
}

/// An imported mesh.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// Positions, in file order.
    pub positions: Vec<Vec3>,
    /// Triangle indices, three per face.
    pub indices: Vec<usize>,
}

impl Mesh {
    /// How many triangles.
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    /// The bound of every position.
    pub fn bounds(&self) -> Option<Bounds> {
        Bounds::of(&self.positions)
    }

    /// ⚠ **The derived default collider: an AABB, and nothing tighter.**
    ///
    /// An import has the triangles and not the intent, so the first rung of the fidelity ladder is the
    /// only one it can honestly produce. A hull would look better and would be a *decision* wearing an
    /// import's clothes.
    pub fn derived_collider(&self) -> Option<Bounds> {
        self.bounds()
    }

    /// Check every index and report the first that is out of range.
    pub fn validate(&self) -> Result<(), MeshError> {
        for (face, chunk) in self.indices.chunks(3).enumerate() {
            for i in chunk {
                if *i >= self.positions.len() {
                    return Err(MeshError::IndexOutOfRange { face, index: *i });
                }
            }
        }
        if self.triangles() == 0 {
            return Err(MeshError::Empty);
        }
        Ok(())
    }
}

/// What a schematic's `Asset'…'` resolves to during the solve.
///
/// ⚠ **This is the whole of a mesh at L1.** A path and a bound are enough to place something, so a
/// hundred-megabyte file costs nothing until L4 asks for its triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshRef {
    /// Where it lives.
    pub path: String,
    /// Its bound, which the import recorded.
    pub bounds: Bounds,
    /// How many triangles it has, for the cook's accounting.
    pub triangles: usize,
}

impl MeshRef {
    /// A reference to a mesh whose triangles are not loaded.
    pub fn of(path: impl Into<String>, mesh: &Mesh) -> Option<MeshRef> {
        Some(MeshRef {
            path: path.into(),
            bounds: mesh.bounds()?,
            triangles: mesh.triangles(),
        })
    }
}

/// Import a mesh, choosing the reader by extension.
pub fn import(path: &str, bytes: &[u8]) -> Result<Mesh, MeshError> {
    let ext = path
        .rsplit('.')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mesh = match ext.as_str() {
        "obj" => obj(
            std::str::from_utf8(bytes).map_err(|_| MeshError::Malformed {
                detail: "an .obj must be UTF-8 text".into(),
            })?,
        )?,
        "gltf" => gltf(
            std::str::from_utf8(bytes).map_err(|_| MeshError::Malformed {
                detail: "a .gltf must be UTF-8 JSON".into(),
            })?,
        )?,
        "glb" => gltf_binary(bytes)?,
        other => {
            return Err(MeshError::UnknownFormat {
                extension: format!(".{other}"),
            })
        }
    };
    mesh.validate()?;
    Ok(mesh)
}

/// Wavefront OBJ.
///
/// ⚠ **`v` and `f` only.** Normals, texture coordinates, materials and groups are read past rather than
/// rejected: an importer that refused a file for carrying a `vt` line would refuse most real exports,
/// and none of it is data the generator reads.
pub fn obj(src: &str) -> Result<Mesh, MeshError> {
    let mut positions = Vec::new();
    let mut indices = Vec::new();

    for line in src.lines() {
        let line = line.trim();
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let coords: Vec<f64> = parts.filter_map(|p| p.parse::<f64>().ok()).collect();
                if coords.len() < 3 {
                    return Err(MeshError::Malformed {
                        detail: format!("a v line needs three coordinates: {line:?}"),
                    });
                }
                positions.push(Vec3::new(coords[0], coords[1], coords[2]));
            }
            Some("f") => {
                // A face is `i`, `i/t`, `i//n` or `i/t/n`; only the first field is a position.
                let corners: Vec<i64> = parts
                    .filter_map(|p| p.split('/').next().and_then(|i| i.parse::<i64>().ok()))
                    .collect();
                if corners.len() < 3 {
                    return Err(MeshError::Malformed {
                        detail: format!("a face needs at least three corners: {line:?}"),
                    });
                }
                // ⚠ **A fan, and OBJ indices are 1-based and may be negative** — a negative index
                // counts back from the end, which is legal and which a naive reader turns into a
                // gigantic positive one.
                let resolve = |i: i64| -> usize {
                    if i < 0 {
                        (positions.len() as i64 + i) as usize
                    } else {
                        (i - 1).max(0) as usize
                    }
                };
                for w in 1..corners.len() - 1 {
                    indices.push(resolve(corners[0]));
                    indices.push(resolve(corners[w]));
                    indices.push(resolve(corners[w + 1]));
                }
            }
            _ => {}
        }
    }
    Ok(Mesh { positions, indices })
}

/// glTF, as JSON with an embedded base64 buffer.
///
/// ⚠ **Positions and indices, through the accessor tables.** A glTF buffer is bytes; what makes them
/// meaningful is the accessor's component type and count, so reading the bytes without the accessor is
/// the mistake that produces a mesh shaped like noise.
pub fn gltf(src: &str) -> Result<Mesh, MeshError> {
    let doc = crate::json::parse(src).map_err(|e| MeshError::Malformed {
        detail: e.to_string(),
    })?;
    let buffers = decode_buffers(&doc)?;
    read_gltf(&doc, &buffers)
}

/// glb — a container whose first chunk is the glTF JSON and whose second is the binary buffer.
pub fn gltf_binary(bytes: &[u8]) -> Result<Mesh, MeshError> {
    if bytes.len() < 12 || &bytes[0..4] != b"glTF" {
        return Err(MeshError::Malformed {
            detail: "a .glb starts with the magic \"glTF\"".into(),
        });
    }
    let mut at = 12usize;
    let mut json: Option<&str> = None;
    let mut bin: Option<&[u8]> = None;
    while at + 8 <= bytes.len() {
        let len = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap_or([0; 4])) as usize;
        let kind = &bytes[at + 4..at + 8];
        let start = at + 8;
        let end = start.saturating_add(len);
        if end > bytes.len() {
            return Err(MeshError::Malformed {
                detail: "a chunk runs past the end of the file".into(),
            });
        }
        match kind {
            b"JSON" => {
                json = std::str::from_utf8(&bytes[start..end]).ok();
            }
            b"BIN\0" => bin = Some(&bytes[start..end]),
            _ => {}
        }
        at = end;
    }
    let Some(json) = json else {
        return Err(MeshError::Malformed {
            detail: "a .glb has no JSON chunk".into(),
        });
    };
    let doc = crate::json::parse(json).map_err(|e| MeshError::Malformed {
        detail: e.to_string(),
    })?;
    let mut buffers = decode_buffers(&doc)?;
    if let Some(bin) = bin {
        // The BIN chunk is buffer 0 when its `uri` is absent.
        if buffers.is_empty() {
            buffers.push(bin.to_vec());
        } else if buffers[0].is_empty() {
            buffers[0] = bin.to_vec();
        }
    }
    read_gltf(&doc, &buffers)
}

fn decode_buffers(doc: &crate::json::Json) -> Result<Vec<Vec<u8>>, MeshError> {
    let mut out = Vec::new();
    for b in doc
        .get("buffers")
        .and_then(crate::json::Json::as_array)
        .unwrap_or(&[])
    {
        match b.get("uri").and_then(crate::json::Json::as_str) {
            Some(uri) => {
                let Some((_, payload)) = uri.split_once(";base64,") else {
                    // An external file: the caller supplies it, and an importer that guessed a path
                    // would be inventing filesystem policy.
                    out.push(Vec::new());
                    continue;
                };
                out.push(base64(payload)?);
            }
            None => out.push(Vec::new()),
        }
    }
    Ok(out)
}

/// Standard base64, no line breaks.
fn base64(src: &str) -> Result<Vec<u8>, MeshError> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(src.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in src.bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let Some(v) = ALPHABET.iter().position(|a| *a == c) else {
            return Err(MeshError::Malformed {
                detail: "a base64 payload has a character outside the alphabet".into(),
            });
        };
        acc = (acc << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Ok(out)
}

fn read_gltf(doc: &crate::json::Json, buffers: &[Vec<u8>]) -> Result<Mesh, MeshError> {
    use crate::json::Json;
    let accessors = doc.get("accessors").and_then(Json::as_array).unwrap_or(&[]);
    let views = doc
        .get("bufferViews")
        .and_then(Json::as_array)
        .unwrap_or(&[]);

    let mut positions = Vec::new();
    let mut indices = Vec::new();

    for mesh in doc.get("meshes").and_then(Json::as_array).unwrap_or(&[]) {
        for prim in mesh
            .get("primitives")
            .and_then(Json::as_array)
            .unwrap_or(&[])
        {
            let base = positions.len();
            if let Some(i) = prim
                .get("attributes")
                .and_then(|a| a.get("POSITION"))
                .and_then(Json::as_f64)
            {
                let floats = read_accessor(accessors, views, buffers, i as usize, 4)?;
                for xyz in floats.chunks(3) {
                    if xyz.len() == 3 {
                        positions.push(Vec3::new(xyz[0], xyz[1], xyz[2]));
                    }
                }
            }
            if let Some(i) = prim.get("indices").and_then(Json::as_f64) {
                let values = read_accessor(accessors, views, buffers, i as usize, 0)?;
                indices.extend(values.into_iter().map(|v| base + v as usize));
            }
        }
    }
    Ok(Mesh { positions, indices })
}

/// Read one accessor as `f64`s, honouring its component type.
///
/// ⚠ **The component type is what makes the bytes mean anything.** A reader that assumed `f32` would
/// turn an index buffer of `u16`s into a mesh shaped like noise, and it would do so without an error.
fn read_accessor(
    accessors: &[crate::json::Json],
    views: &[crate::json::Json],
    buffers: &[Vec<u8>],
    index: usize,
    _hint: usize,
) -> Result<Vec<f64>, MeshError> {
    use crate::json::Json;
    let bad = |what: &str| MeshError::Malformed {
        detail: what.to_string(),
    };
    let acc = accessors
        .get(index)
        .ok_or_else(|| bad("no such accessor"))?;
    let count = acc.get("count").and_then(Json::as_f64).unwrap_or(0.0) as usize;
    let component = acc
        .get("componentType")
        .and_then(Json::as_f64)
        .unwrap_or(5126.0) as u32;
    let per = match acc.get("type").and_then(Json::as_str).unwrap_or("SCALAR") {
        "SCALAR" => 1,
        "VEC2" => 2,
        "VEC3" => 3,
        "VEC4" => 4,
        other => return Err(bad(&format!("accessor type {other:?} is not imported"))),
    };
    let width = match component {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        other => return Err(bad(&format!("component type {other} is not imported"))),
    };

    let view_i = acc.get("bufferView").and_then(Json::as_f64).unwrap_or(0.0) as usize;
    let view = views.get(view_i).ok_or_else(|| bad("no such bufferView"))?;
    let buffer_i = view.get("buffer").and_then(Json::as_f64).unwrap_or(0.0) as usize;
    let offset = view.get("byteOffset").and_then(Json::as_f64).unwrap_or(0.0) as usize
        + acc.get("byteOffset").and_then(Json::as_f64).unwrap_or(0.0) as usize;
    let data = buffers.get(buffer_i).ok_or_else(|| bad("no such buffer"))?;

    let mut out = Vec::with_capacity(count * per);
    for n in 0..count * per {
        let at = offset + n * width;
        let Some(slice) = data.get(at..at + width) else {
            return Err(bad("an accessor runs past its buffer"));
        };
        out.push(match component {
            5120 => slice[0] as i8 as f64,
            5121 => slice[0] as f64,
            5122 => i16::from_le_bytes([slice[0], slice[1]]) as f64,
            5123 => u16::from_le_bytes([slice[0], slice[1]]) as f64,
            5125 => u32::from_le_bytes(slice.try_into().unwrap_or([0; 4])) as f64,
            _ => f32::from_le_bytes(slice.try_into().unwrap_or([0; 4])) as f64,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CUBE_OBJ: &str = "\
# a unit cube corner
v 0.0 0.0 0.0
v 1.0 0.0 0.0
v 1.0 1.0 0.0
v 0.0 1.0 0.0
vt 0.0 0.0
vn 0.0 0.0 1.0
f 1/1/1 2/1/1 3/1/1
f 1/1/1 3/1/1 4/1/1
";

    #[test]
    fn an_obj_imports_to_triangles() {
        let mesh = import("/Content/Meshes/quad.obj", CUBE_OBJ.as_bytes()).unwrap();
        assert_eq!(mesh.positions.len(), 4);
        assert_eq!(mesh.triangles(), 2);
        assert_eq!(mesh.indices, vec![0, 1, 2, 0, 2, 3]);
    }

    #[test]
    fn face_lines_with_texture_and_normal_fields_read_only_the_position() {
        let mesh = obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1//1 2//2 3//3\n").unwrap();
        assert_eq!(mesh.indices, vec![0, 1, 2]);
    }

    #[test]
    fn a_polygon_face_becomes_a_fan() {
        let mesh = obj("v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3 4\n").unwrap();
        assert_eq!(mesh.triangles(), 2);
    }

    #[test]
    fn a_negative_obj_index_counts_back_from_the_end() {
        // ⚠ Legal OBJ, and a naive reader turns it into a gigantic positive index.
        let mesh = obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nf -3 -2 -1\n").unwrap();
        assert_eq!(mesh.indices, vec![0, 1, 2]);
    }

    #[test]
    fn unrecognised_obj_lines_are_read_past_rather_than_refused() {
        // ⚠ An importer that refused a file for carrying a `usemtl` would refuse most real exports.
        let mesh =
            obj("mtllib x.mtl\nusemtl steel\no group\ns off\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n")
                .unwrap();
        assert_eq!(mesh.triangles(), 1);
    }

    #[test]
    fn an_index_past_the_vertex_list_is_refused_rather_than_clamped() {
        // ⚠ A clamped index makes a triangle nobody authored.
        let err = import("x.obj", b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 9\n").unwrap_err();
        assert_eq!(err, MeshError::IndexOutOfRange { face: 0, index: 8 });
        assert!(err.to_string().contains("rather than clamped"));
    }

    #[test]
    fn a_file_with_no_triangles_is_an_error_rather_than_an_empty_mesh() {
        assert_eq!(
            import("x.obj", b"v 0 0 0\nv 1 0 0\n").unwrap_err(),
            MeshError::Empty
        );
    }

    #[test]
    fn an_unknown_extension_names_the_set() {
        let err = import("x.fbx", b"").unwrap_err();
        assert_eq!(
            err,
            MeshError::UnknownFormat {
                extension: ".fbx".into()
            }
        );
        assert!(err.to_string().contains(".obj, .gltf and .glb"));
    }

    #[test]
    fn the_derived_collider_is_an_aabb_and_nothing_tighter() {
        // ⚠ An import has the triangles and not the intent; a hull would be a decision wearing an
        // import's clothes.
        let mesh = obj("v 0 0 0\nv 2 0 0\nv 0 3 0\nv 0 0 4\nf 1 2 3\nf 1 3 4\n").unwrap();
        let b = mesh.derived_collider().unwrap();
        assert_eq!(b.min, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(b.max, Vec3::new(2.0, 3.0, 4.0));
        assert_eq!(b.extents(), Vec3::new(2.0, 3.0, 4.0));
    }

    #[test]
    fn a_mesh_ref_carries_enough_to_place_without_the_triangles() {
        // ⚠ A hundred-megabyte mesh costs nothing until L4 asks.
        let mesh = obj("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n").unwrap();
        let r = MeshRef::of("/Content/Meshes/x.obj", &mesh).unwrap();
        assert_eq!(r.triangles, 1);
        assert_eq!(r.bounds.max, Vec3::new(1.0, 1.0, 0.0));
        assert!(r.path.ends_with(".obj"));
    }

    /// A minimal glTF: one triangle, positions as f32 and indices as u16, base64-embedded.
    fn gltf_triangle() -> String {
        let mut bytes: Vec<u8> = Vec::new();
        for v in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for c in v {
                bytes.extend_from_slice(&c.to_le_bytes());
            }
        }
        let index_at = bytes.len();
        for i in [0u16, 1, 2] {
            bytes.extend_from_slice(&i.to_le_bytes());
        }
        let b64 = encode_base64(&bytes);
        format!(
            r#"{{ "buffers": [ {{ "uri": "data:application/octet-stream;base64,{b64}" }} ],
                 "bufferViews": [ {{ "buffer": 0, "byteOffset": 0, "byteLength": {index_at} }},
                                  {{ "buffer": 0, "byteOffset": {index_at}, "byteLength": 6 }} ],
                 "accessors": [ {{ "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3" }},
                                {{ "bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR" }} ],
                 "meshes": [ {{ "primitives": [ {{ "attributes": {{ "POSITION": 0 }}, "indices": 1 }} ] }} ] }}"#
        )
    }

    fn encode_base64(bytes: &[u8]) -> String {
        const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(A[((n >> (18 - i * 6)) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    #[test]
    fn a_gltf_imports_through_its_accessor_tables() {
        let mesh = import("/Content/Meshes/tri.gltf", gltf_triangle().as_bytes()).unwrap();
        assert_eq!(mesh.positions.len(), 3);
        assert_eq!(mesh.triangles(), 1);
        assert_eq!(mesh.positions[1], Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(mesh.indices, vec![0, 1, 2]);
    }

    #[test]
    fn the_component_type_decides_how_the_bytes_are_read() {
        // ⚠ A reader that assumed f32 would turn a u16 index buffer into noise, without an error.
        let as_u16 = import("t.gltf", gltf_triangle().as_bytes()).unwrap();
        let claimed_f32 =
            gltf_triangle().replace("\"componentType\": 5123", "\"componentType\": 5126");
        let differently = import("t.gltf", claimed_f32.as_bytes());
        assert!(
            differently.is_err() || differently.unwrap().indices != as_u16.indices,
            "the component type must change what the same bytes mean"
        );
    }

    #[test]
    fn a_glb_splits_its_chunks_and_finds_the_json() {
        let json = gltf_triangle();
        let mut glb: Vec<u8> = b"glTF".to_vec();
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&0u32.to_le_bytes()); // total length, unread
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(json.as_bytes());

        let mesh = import("/Content/Meshes/tri.glb", &glb).unwrap();
        assert_eq!(mesh.triangles(), 1);
    }

    #[test]
    fn a_glb_without_the_magic_is_refused() {
        assert!(matches!(
            import("x.glb", b"not a glb at all"),
            Err(MeshError::Malformed { .. })
        ));
    }

    #[test]
    fn a_chunk_running_past_the_end_is_refused_rather_than_read() {
        let mut glb: Vec<u8> = b"glTF".to_vec();
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&0u32.to_le_bytes());
        glb.extend_from_slice(&9999u32.to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(b"{}");
        assert!(matches!(
            import("x.glb", &glb),
            Err(MeshError::Malformed { .. })
        ));
    }

    #[test]
    fn bounds_of_nothing_is_none_rather_than_a_zero_box() {
        // ⚠ A zero box at the origin is a real bound that happens to be wrong.
        assert!(Bounds::of(&[]).is_none());
        let empty = Mesh {
            positions: vec![],
            indices: vec![],
        };
        assert!(empty.derived_collider().is_none());
        assert!(MeshRef::of("x", &empty).is_none());
    }

    #[test]
    fn a_bounds_centre_is_between_its_corners() {
        let b = Bounds::of(&[Vec3::new(-1.0, -2.0, -3.0), Vec3::new(1.0, 2.0, 3.0)]).unwrap();
        assert_eq!(b.centre(), Vec3::new(0.0, 0.0, 0.0));
    }
}
