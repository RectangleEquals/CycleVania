//! The deterministic binary serialization spine — reproduction bundles, editor round-trips, and
//! (from M06) the descriptor.
//!
//! # Why hand-rolled
//!
//! Serialization sits directly on the determinism guarantee: a reproduction bundle that does not
//! round-trip byte-for-byte is not a reproduction. A hand-written format buys explicit control of
//! every byte — fixed endianness, fixed widths, fixed ordering — with no dependency whose next release
//! could quietly change a representation. It is also small: the whole format is the primitives below.
//!
//! # The rules that keep it target-independent
//!
//! * **Little-endian, fixed width, always.** Every integer is written at a declared size.
//! * **No `usize` or `isize`, anywhere.** This is enforced *structurally* — [`Writer`] simply has no
//!   method for them. `usize` is 64-bit on native and **32-bit on wasm32**, so serializing one would
//!   silently produce different bytes on the two targets and break the cross-target guarantee. Lengths
//!   go out as `u32`.
//! * **`f64` is written as its raw bit pattern**, never as text: exact, NaN-payload-preserving, and
//!   free of any decimal-formatting rounding.
//! * **Collections serialize in iteration order**, and every collection the engine serializes has a
//!   deterministic one (see [`Arena`](crate::Arena)). No `HashMap` is ever written directly.
//!
//! # Envelope
//!
//! A stored artifact starts with [`MAGIC`] and a [`FORMAT_VERSION`], so a stale or foreign file is
//! rejected with a clear error instead of being misparsed into plausible garbage.

use crate::arena::{Arena, Handle};
use crate::object::{IdAllocator, ObjectHeader, ObjectId};
use std::fmt;

/// Magic bytes beginning every serialized artifact: "CycleVania Data Stream".
pub const MAGIC: [u8; 4] = *b"CVDS";

/// The format version. Bump on any incompatible change to the encoding.
pub const FORMAT_VERSION: u32 = 1;

/// Everything that can go wrong while reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SerError {
    /// Ran out of bytes mid-value.
    UnexpectedEof { needed: usize, remaining: usize },
    /// The stream did not begin with [`MAGIC`].
    BadMagic { found: [u8; 4] },
    /// The stream's version is not one this build understands.
    UnsupportedVersion { found: u32, supported: u32 },
    /// A string was not valid UTF-8.
    InvalidUtf8,
    /// A value was structurally impossible (a zero generation, an out-of-range tag, …).
    InvalidValue(&'static str),
    /// Bytes remained after the value was fully read — a sign of a length or layout mismatch.
    TrailingBytes(usize),
}

impl fmt::Display for SerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SerError::UnexpectedEof { needed, remaining } => {
                write!(
                    f,
                    "unexpected end of stream: needed {needed} bytes, {remaining} remain"
                )
            }
            SerError::BadMagic { found } => {
                write!(
                    f,
                    "bad magic {found:?}; expected {MAGIC:?} — not a CycleVania stream"
                )
            }
            SerError::UnsupportedVersion { found, supported } => {
                write!(
                    f,
                    "format version {found} is not supported (this build reads {supported})"
                )
            }
            SerError::InvalidUtf8 => write!(f, "string was not valid UTF-8"),
            SerError::InvalidValue(what) => write!(f, "invalid value: {what}"),
            SerError::TrailingBytes(n) => write!(f, "{n} trailing bytes after the value"),
        }
    }
}

impl std::error::Error for SerError {}

/// Result alias for reads.
pub type SerResult<T> = Result<T, SerError>;

// ---------------------------------------------------------------------------------------------
// Writer
// ---------------------------------------------------------------------------------------------

/// Appends values to a byte buffer. Infallible — writing only grows a `Vec`.
///
/// Note the deliberate absence of `usize`/`isize` methods; see the module docs.
#[derive(Clone, Debug, Default)]
pub struct Writer {
    buf: Vec<u8>,
}

impl Writer {
    /// A new, empty writer.
    pub fn new() -> Self {
        Writer { buf: Vec::new() }
    }

    /// A writer that begins with the magic + version envelope.
    pub fn with_envelope() -> Self {
        let mut w = Writer::new();
        w.bytes_raw(&MAGIC);
        w.u32(FORMAT_VERSION);
        w
    }

    /// The bytes written so far.
    pub fn finish(self) -> Vec<u8> {
        self.buf
    }

    /// How many bytes have been written.
    pub fn position(&self) -> usize {
        self.buf.len()
    }

    /// Raw bytes, with no length prefix.
    pub fn bytes_raw(&mut self, b: &[u8]) {
        self.buf.extend_from_slice(b);
    }

    /// A single byte.
    pub fn u8(&mut self, v: u8) {
        self.buf.push(v);
    }

    /// A `bool`, as `0` or `1`.
    pub fn bool(&mut self, v: bool) {
        self.u8(u8::from(v));
    }

    /// A little-endian `u16`.
    pub fn u16(&mut self, v: u16) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// A little-endian `u32`.
    pub fn u32(&mut self, v: u32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// A little-endian `u64`.
    pub fn u64(&mut self, v: u64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// A little-endian `i32`.
    pub fn i32(&mut self, v: i32) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// A little-endian `i64`.
    pub fn i64(&mut self, v: i64) {
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// An `f64`, as its exact IEEE-754 bit pattern.
    pub fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }

    /// A length, always as `u32`.
    ///
    /// # Panics
    /// If the length exceeds `u32::MAX`, which no engine collection should ever reach and which would
    /// otherwise truncate silently.
    pub fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("collection longer than u32::MAX cannot be serialized"));
    }

    /// A UTF-8 string, length-prefixed.
    pub fn str(&mut self, s: &str) {
        self.len(s.len());
        self.bytes_raw(s.as_bytes());
    }

    /// A length-prefixed byte slice.
    pub fn bytes(&mut self, b: &[u8]) {
        self.len(b.len());
        self.bytes_raw(b);
    }

    /// A value implementing [`Serialize`].
    pub fn write<T: Serialize + ?Sized>(&mut self, v: &T) {
        v.serialize(self);
    }
}

// ---------------------------------------------------------------------------------------------
// Reader
// ---------------------------------------------------------------------------------------------

/// Reads values from a byte slice, tracking position and reporting precise errors.
#[derive(Clone, Debug)]
pub struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    /// A reader over `buf`, starting at the beginning.
    pub fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    /// A reader positioned past a validated magic + version envelope.
    pub fn with_envelope(buf: &'a [u8]) -> SerResult<Self> {
        let mut r = Reader::new(buf);
        let magic = r.take(4)?;
        let found = [magic[0], magic[1], magic[2], magic[3]];
        if found != MAGIC {
            return Err(SerError::BadMagic { found });
        }
        let version = r.u32()?;
        if version != FORMAT_VERSION {
            return Err(SerError::UnsupportedVersion {
                found: version,
                supported: FORMAT_VERSION,
            });
        }
        Ok(r)
    }

    /// How many bytes have been consumed.
    pub fn position(&self) -> usize {
        self.pos
    }

    /// How many bytes remain.
    pub fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    /// Is the stream fully consumed?
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Error unless the stream is fully consumed — call after reading a whole artifact to catch
    /// length or layout mismatches that would otherwise pass silently.
    pub fn expect_end(&self) -> SerResult<()> {
        if self.is_empty() {
            Ok(())
        } else {
            Err(SerError::TrailingBytes(self.remaining()))
        }
    }

    /// Consume exactly `n` bytes.
    pub fn take(&mut self, n: usize) -> SerResult<&'a [u8]> {
        if self.remaining() < n {
            return Err(SerError::UnexpectedEof {
                needed: n,
                remaining: self.remaining(),
            });
        }
        let out = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(out)
    }

    /// Read a `u8`.
    pub fn u8(&mut self) -> SerResult<u8> {
        Ok(self.take(1)?[0])
    }

    /// Read a `bool`; any value other than `0`/`1` is an error.
    pub fn bool(&mut self) -> SerResult<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(SerError::InvalidValue("bool must be 0 or 1")),
        }
    }

    /// Read a little-endian `u16`.
    pub fn u16(&mut self) -> SerResult<u16> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    /// Read a little-endian `u32`.
    pub fn u32(&mut self) -> SerResult<u32> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Read a little-endian `u64`.
    pub fn u64(&mut self) -> SerResult<u64> {
        let b = self.take(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// Read a little-endian `i32`.
    pub fn i32(&mut self) -> SerResult<i32> {
        Ok(self.u32()? as i32)
    }

    /// Read a little-endian `i64`.
    pub fn i64(&mut self) -> SerResult<i64> {
        Ok(self.u64()? as i64)
    }

    /// Read an `f64` from its exact bit pattern.
    pub fn f64(&mut self) -> SerResult<f64> {
        Ok(f64::from_bits(self.u64()?))
    }

    /// Read a `u32` length prefix.
    ///
    /// Named `read_len` rather than `len` because it *consumes* bytes — it is not this reader's own
    /// length (see [`Reader::remaining`]). Rejects lengths exceeding the bytes actually remaining, so
    /// a corrupt header cannot make the reader attempt a huge allocation.
    pub fn read_len(&mut self) -> SerResult<usize> {
        let n = self.u32()? as usize;
        if n > self.remaining() {
            return Err(SerError::UnexpectedEof {
                needed: n,
                remaining: self.remaining(),
            });
        }
        Ok(n)
    }

    /// Read a length-prefixed UTF-8 string.
    pub fn str(&mut self) -> SerResult<String> {
        let n = self.read_len()?;
        let b = self.take(n)?;
        std::str::from_utf8(b)
            .map(str::to_owned)
            .map_err(|_| SerError::InvalidUtf8)
    }

    /// Read a length-prefixed byte vector.
    pub fn bytes(&mut self) -> SerResult<Vec<u8>> {
        let n = self.read_len()?;
        Ok(self.take(n)?.to_vec())
    }

    /// Read a value implementing [`Deserialize`].
    pub fn read<T: Deserialize>(&mut self) -> SerResult<T> {
        T::deserialize(self)
    }
}

// ---------------------------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------------------------

/// A value that can be written deterministically.
pub trait Serialize {
    /// Append `self` to `w`.
    fn serialize(&self, w: &mut Writer);
}

/// A value that can be read back.
pub trait Deserialize: Sized {
    /// Read one `Self` from `r`.
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self>;
}

/// Serialize a value into a complete artifact, envelope included.
pub fn to_bytes<T: Serialize + ?Sized>(value: &T) -> Vec<u8> {
    let mut w = Writer::with_envelope();
    w.write(value);
    w.finish()
}

/// Read a complete artifact written by [`to_bytes`], validating the envelope and requiring that the
/// whole stream is consumed.
pub fn from_bytes<T: Deserialize>(buf: &[u8]) -> SerResult<T> {
    let mut r = Reader::with_envelope(buf)?;
    let value = r.read::<T>()?;
    r.expect_end()?;
    Ok(value)
}

// ---------------------------------------------------------------------------------------------
// Primitive impls
// ---------------------------------------------------------------------------------------------

macro_rules! impl_primitive {
    ($t:ty, $write:ident, $read:ident) => {
        impl Serialize for $t {
            fn serialize(&self, w: &mut Writer) {
                w.$write(*self);
            }
        }
        impl Deserialize for $t {
            fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
                r.$read()
            }
        }
    };
}

impl_primitive!(u8, u8, u8);
impl_primitive!(u16, u16, u16);
impl_primitive!(u32, u32, u32);
impl_primitive!(u64, u64, u64);
impl_primitive!(i32, i32, i32);
impl_primitive!(i64, i64, i64);
impl_primitive!(f64, f64, f64);
impl_primitive!(bool, bool, bool);

impl Serialize for str {
    fn serialize(&self, w: &mut Writer) {
        w.str(self);
    }
}

impl Serialize for String {
    fn serialize(&self, w: &mut Writer) {
        w.str(self);
    }
}

impl Deserialize for String {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        r.str()
    }
}

impl<T: Serialize> Serialize for Option<T> {
    fn serialize(&self, w: &mut Writer) {
        match self {
            Some(v) => {
                w.bool(true);
                w.write(v);
            }
            None => w.bool(false),
        }
    }
}

impl<T: Deserialize> Deserialize for Option<T> {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        if r.bool()? {
            Ok(Some(r.read()?))
        } else {
            Ok(None)
        }
    }
}

impl<T: Serialize> Serialize for Vec<T> {
    fn serialize(&self, w: &mut Writer) {
        w.len(self.len());
        for v in self {
            w.write(v);
        }
    }
}

impl<T: Deserialize> Deserialize for Vec<T> {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        // `len()` is bounded by the bytes remaining, so this cannot over-allocate on corrupt input.
        let n = r.u32()? as usize;
        let mut out = Vec::with_capacity(n.min(r.remaining()));
        for _ in 0..n {
            out.push(r.read()?);
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------------------------
// Engine type impls
// ---------------------------------------------------------------------------------------------

impl<T> Serialize for Handle<T> {
    fn serialize(&self, w: &mut Writer) {
        w.u32(self.index());
        w.u32(self.generation());
    }
}

impl<T> Deserialize for Handle<T> {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let index = r.u32()?;
        let generation = r.u32()?;
        Handle::from_raw(((index as u64) << 32) | generation as u64)
            .ok_or(SerError::InvalidValue("handle generation must be non-zero"))
    }
}

impl Serialize for ObjectId {
    fn serialize(&self, w: &mut Writer) {
        w.u64(self.to_raw());
    }
}

impl Deserialize for ObjectId {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(ObjectId::from_raw(r.u64()?))
    }
}

impl Serialize for ObjectHeader {
    fn serialize(&self, w: &mut Writer) {
        w.write(&self.id);
        w.str(&self.name);
    }
}

impl Deserialize for ObjectHeader {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let id = r.read::<ObjectId>()?;
        let name = r.str()?;
        Ok(ObjectHeader { id, name })
    }
}

impl Serialize for IdAllocator {
    fn serialize(&self, w: &mut Writer) {
        w.u64(self.peek());
    }
}

impl Deserialize for IdAllocator {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        Ok(IdAllocator::resuming_from(r.u64()?))
    }
}

/// Arenas serialize their **whole slot table**, vacancies and generations included, so every
/// outstanding handle stays valid across a round-trip. Storing only the live values would compact the
/// indices and silently repoint every handle in the graph.
impl<T: Serialize> Serialize for Arena<T> {
    fn serialize(&self, w: &mut Writer) {
        w.len(self.slot_count());
        for (generation, value) in self.raw_slots() {
            w.u32(generation);
            match value {
                Some(v) => {
                    w.bool(true);
                    w.write(v);
                }
                None => w.bool(false),
            }
        }
        let free = self.raw_free();
        w.len(free.len());
        for &i in free {
            w.u32(i);
        }
    }
}

impl<T: Deserialize> Deserialize for Arena<T> {
    fn deserialize(r: &mut Reader<'_>) -> SerResult<Self> {
        let slot_count = r.u32()? as usize;
        let mut slots = Vec::with_capacity(slot_count.min(r.remaining()));
        for _ in 0..slot_count {
            let generation = r.u32()?;
            let value = if r.bool()? {
                Some(r.read::<T>()?)
            } else {
                None
            };
            slots.push((generation, value));
        }
        let free_count = r.u32()? as usize;
        let mut free = Vec::with_capacity(free_count.min(r.remaining()));
        for _ in 0..free_count {
            free.push(r.u32()?);
        }
        Arena::from_raw_parts(slots, free)
            .ok_or(SerError::InvalidValue("arena slot table is inconsistent"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primitives_round_trip() {
        let mut w = Writer::new();
        w.u8(0xAB);
        w.u16(0x1234);
        w.u32(0xDEAD_BEEF);
        w.u64(0x0123_4567_89AB_CDEF);
        w.i32(-42);
        w.i64(-9_000_000_000);
        w.bool(true);
        w.str("héllo ☃");
        w.bytes(&[1, 2, 3]);
        let buf = w.finish();

        let mut r = Reader::new(&buf);
        assert_eq!(r.u8().unwrap(), 0xAB);
        assert_eq!(r.u16().unwrap(), 0x1234);
        assert_eq!(r.u32().unwrap(), 0xDEAD_BEEF);
        assert_eq!(r.u64().unwrap(), 0x0123_4567_89AB_CDEF);
        assert_eq!(r.i32().unwrap(), -42);
        assert_eq!(r.i64().unwrap(), -9_000_000_000);
        assert!(r.bool().unwrap());
        assert_eq!(r.str().unwrap(), "héllo ☃");
        assert_eq!(r.bytes().unwrap(), vec![1, 2, 3]);
        assert!(r.expect_end().is_ok());
    }

    #[test]
    fn floats_survive_exactly_including_edge_values() {
        // Bit-pattern encoding, so these all come back identical rather than "close".
        let cases = [
            0.0,
            -0.0,
            1.0,
            -1.0,
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::EPSILON,
            5e-324, // smallest subnormal
            0.1,    // not representable in binary
            f64::INFINITY,
            f64::NEG_INFINITY,
        ];
        let mut w = Writer::new();
        for v in cases {
            w.f64(v);
        }
        w.f64(f64::NAN);
        let buf = w.finish();
        let mut r = Reader::new(&buf);
        for v in cases {
            let got = r.f64().unwrap();
            assert_eq!(
                got.to_bits(),
                v.to_bits(),
                "f64 {v} did not round-trip bit-exactly"
            );
        }
        assert!(r.f64().unwrap().is_nan());
        // -0.0 must not have become 0.0.
        assert!(cases[1].is_sign_negative());
    }

    #[test]
    fn envelope_rejects_foreign_and_stale_streams() {
        let good = to_bytes(&42u32);
        assert_eq!(from_bytes::<u32>(&good).unwrap(), 42);

        // Wrong magic.
        let mut bad = good.clone();
        bad[0] = b'X';
        assert!(matches!(
            from_bytes::<u32>(&bad),
            Err(SerError::BadMagic { .. })
        ));

        // Wrong version.
        let mut stale = good.clone();
        stale[4] = 99;
        assert!(matches!(
            from_bytes::<u32>(&stale),
            Err(SerError::UnsupportedVersion { found: 99, .. })
        ));

        // Truncated.
        assert!(matches!(
            from_bytes::<u32>(&good[..6]),
            Err(SerError::UnexpectedEof { .. })
        ));

        // Trailing junk is caught rather than ignored.
        let mut extra = good.clone();
        extra.push(0);
        assert_eq!(from_bytes::<u32>(&extra), Err(SerError::TrailingBytes(1)));
    }

    #[test]
    fn corrupt_lengths_are_rejected_without_huge_allocations() {
        // A string claiming 4 GB in a 20-byte buffer must error, not try to allocate.
        let mut w = Writer::with_envelope();
        w.u32(u32::MAX);
        w.bytes_raw(b"short");
        let buf = w.finish();
        let mut r = Reader::with_envelope(&buf).unwrap();
        assert!(matches!(r.str(), Err(SerError::UnexpectedEof { .. })));
    }

    #[test]
    fn collections_round_trip() {
        let v: Vec<u32> = (0..100).collect();
        assert_eq!(from_bytes::<Vec<u32>>(&to_bytes(&v)).unwrap(), v);

        let opts: Vec<Option<String>> = vec![
            Some("a".into()),
            None,
            Some("".into()),
            Some("ünïcødé".into()),
        ];
        assert_eq!(
            from_bytes::<Vec<Option<String>>>(&to_bytes(&opts)).unwrap(),
            opts
        );

        let empty: Vec<u64> = Vec::new();
        assert_eq!(from_bytes::<Vec<u64>>(&to_bytes(&empty)).unwrap(), empty);
    }

    #[test]
    fn writing_is_byte_stable() {
        // The same value must always produce the same bytes — the property the whole guarantee needs.
        let v: Vec<Option<String>> = vec![Some("door".into()), None, Some("lever".into())];
        assert_eq!(to_bytes(&v), to_bytes(&v));
    }

    #[test]
    fn identity_types_round_trip() {
        let id = ObjectId::derived("actor", "door");
        assert_eq!(from_bytes::<ObjectId>(&to_bytes(&id)).unwrap(), id);

        let header = ObjectHeader::new(id, "heavy_gate");
        assert_eq!(
            from_bytes::<ObjectHeader>(&to_bytes(&header)).unwrap(),
            header
        );

        let mut alloc = IdAllocator::new();
        alloc.allocate();
        alloc.allocate();
        let back = from_bytes::<IdAllocator>(&to_bytes(&alloc)).unwrap();
        assert_eq!(back.peek(), alloc.peek());
        // A resumed allocator does not reissue.
        assert_eq!(back.clone().allocate(), ObjectId::from_raw(3));
    }

    #[test]
    fn zero_generation_handle_is_rejected() {
        let mut w = Writer::with_envelope();
        w.u32(0); // index
        w.u32(0); // generation — never valid
        let buf = w.finish();
        assert!(matches!(
            from_bytes::<Handle<u32>>(&buf),
            Err(SerError::InvalidValue(_))
        ));
    }
}
