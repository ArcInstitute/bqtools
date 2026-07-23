use std::hash::Hasher;
use std::num::Wrapping;
use std::sync::Arc;

use binseq::{BinseqRecord, ParallelProcessor};
use parking_lot::Mutex;
use xxhash_rust::xxh3::Xxh3;

use crate::cli::Mate;

/// Which record fields feed into the per-record hash.
// Each field is an independent inclusion toggle, not a state machine.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct FieldMask {
    pub seq: bool,
    pub qual: bool,
    pub headers: bool,
    pub flags: bool,
}

/// Writes a length-prefixed byte field into the hasher.
///
/// The length prefix keeps adjacent fields from being confused for one
/// another (e.g. hashing "AB" then "C" must not collide with "A" then "BC").
///
/// The length is written via an explicit `to_le_bytes()`, not
/// `Hasher::write_u64` - that default method serializes via `to_ne_bytes()`,
/// so the bytes actually fed into the hasher (and thus the checksum) would
/// differ between little- and big-endian hosts for the exact same file.
fn write_field<H: Hasher>(hasher: &mut H, data: &[u8]) {
    hasher.write(&(data.len() as u64).to_le_bytes());
    hasher.write(data);
}

/// See [`write_field`] for why this avoids `Hasher::write_u64`.
fn write_flag<H: Hasher>(hasher: &mut H, value: u64) {
    hasher.write(&value.to_le_bytes());
}

/// Hashes the user-selected fields of a single record.
///
/// Per-record hashes are combined by [`VerifyProcessor`] with a commutative
/// wrapping sum, so the resulting checksum does not depend on record order -
/// required because parallel BINSEQ writers make no guarantee that output
/// order matches input order.
///
/// `fields.headers` must already be forced off by the caller for files that
/// don't actually store headers: `sheader()`/`xheader()` fall back to a
/// synthesized string derived from the record's position in that case, and
/// hashing it would leak record order into the checksum. See
/// [`super::reader_has_headers`].
fn hash_record<R: BinseqRecord>(record: &R, fields: FieldMask, mate: Mate) -> u64 {
    let mut hasher = Xxh3::new();

    let include_primary = matches!(mate, Mate::One | Mate::Both);
    let include_extended = record.is_paired() && matches!(mate, Mate::Two | Mate::Both);

    if include_primary {
        if fields.seq {
            write_field(&mut hasher, record.sseq());
        }
        if fields.qual && record.has_quality() {
            write_field(&mut hasher, record.squal());
        }
        if fields.headers {
            write_field(&mut hasher, record.sheader());
        }
    }
    if include_extended {
        if fields.seq {
            write_field(&mut hasher, record.xseq());
        }
        if fields.qual && record.has_quality() {
            write_field(&mut hasher, record.xqual());
        }
        if fields.headers {
            write_field(&mut hasher, record.xheader());
        }
    }
    if fields.flags {
        if let Some(value) = record.flag() {
            write_flag(&mut hasher, value);
        }
    }

    hasher.finish()
}

pub struct VerifyProcessor {
    /// Which fields (and mate(s)) feed into each record's hash.
    fields: FieldMask,
    mate: Mate,

    /// Thread-local partial sum/count, merged into the shared totals on each batch.
    t_checksum: Wrapping<u64>,
    t_count: usize,

    /// Shared totals across all threads.
    checksum: Arc<Mutex<Wrapping<u64>>>,
    count: Arc<Mutex<usize>>,
}

impl Clone for VerifyProcessor {
    fn clone(&self) -> Self {
        Self {
            fields: self.fields,
            mate: self.mate,
            t_checksum: Wrapping(0),
            t_count: 0,
            checksum: self.checksum.clone(),
            count: self.count.clone(),
        }
    }
}

impl VerifyProcessor {
    pub fn new(fields: FieldMask, mate: Mate) -> Self {
        Self {
            fields,
            mate,
            t_checksum: Wrapping(0),
            t_count: 0,
            checksum: Arc::new(Mutex::new(Wrapping(0))),
            count: Arc::new(Mutex::new(0)),
        }
    }

    pub fn checksum(&self) -> u64 {
        self.checksum.lock().0
    }

    pub fn num_records(&self) -> usize {
        *self.count.lock()
    }
}

impl ParallelProcessor for VerifyProcessor {
    fn process_record<R: BinseqRecord>(&mut self, record: R) -> binseq::Result<()> {
        self.t_checksum += Wrapping(hash_record(&record, self.fields, self.mate));
        self.t_count += 1;
        Ok(())
    }

    fn on_batch_complete(&mut self) -> binseq::Result<()> {
        *self.checksum.lock() += self.t_checksum;
        *self.count.lock() += self.t_count;
        self.t_checksum = Wrapping(0);
        self.t_count = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use binseq::BitSize;

    use super::*;

    /// Minimal `BinseqRecord` impl for exercising `hash_record` directly.
    struct MockRecord {
        sseq: Vec<u8>,
        slen: u64,
        squal: Vec<u8>,
        sheader: Vec<u8>,
        flag: Option<u64>,
    }
    impl BinseqRecord for MockRecord {
        fn bitsize(&self) -> BitSize {
            BitSize::Two
        }
        fn index(&self) -> u64 {
            0
        }
        fn flag(&self) -> Option<u64> {
            self.flag
        }
        fn sheader(&self) -> &[u8] {
            &self.sheader
        }
        fn xheader(&self) -> &[u8] {
            b""
        }
        fn slen(&self) -> u64 {
            self.slen
        }
        fn xlen(&self) -> u64 {
            0
        }
        fn sbuf(&self) -> &[u64] {
            &[]
        }
        fn xbuf(&self) -> &[u64] {
            &[]
        }
        fn squal(&self) -> &[u8] {
            &self.squal
        }
        fn sseq(&self) -> &[u8] {
            &self.sseq
        }
    }

    fn record(seq: &[u8], header: &[u8], flag: Option<u64>) -> MockRecord {
        MockRecord {
            sseq: seq.to_vec(),
            slen: seq.len() as u64,
            squal: Vec::new(),
            sheader: header.to_vec(),
            flag,
        }
    }

    const ALL_FIELDS: FieldMask = FieldMask {
        seq: true,
        qual: true,
        headers: true,
        flags: true,
    };

    /// Records every byte handed to it via `Hasher::write`, so tests can pin
    /// down the exact byte sequence fed into the hash - specifically, that
    /// it's little-endian and not `Hasher::write_u64`'s `to_ne_bytes()`.
    /// Note this only catches a regression to `write_u64` if these tests are
    /// ever run on a big-endian host: on the little-endian hosts this CI
    /// actually runs on, `to_ne_bytes()` and `to_le_bytes()` produce
    /// byte-identical output, so a reintroduced `write_u64` call would pass
    /// this test right along with the intended `to_le_bytes()` call. Code
    /// review is the real guard against that regression; this test exists to
    /// pin the intended (portable) encoding, not to detect the mistake.
    #[derive(Default)]
    struct SpyHasher {
        bytes: Vec<u8>,
    }
    impl Hasher for SpyHasher {
        fn finish(&self) -> u64 {
            0
        }
        fn write(&mut self, bytes: &[u8]) {
            self.bytes.extend_from_slice(bytes);
        }
    }

    #[test]
    fn test_write_field_length_prefix_is_little_endian() {
        let mut spy = SpyHasher::default();
        write_field(&mut spy, b"AC");
        let mut expected = 2u64.to_le_bytes().to_vec();
        expected.extend_from_slice(b"AC");
        assert_eq!(spy.bytes, expected);
    }

    #[test]
    fn test_write_flag_value_is_little_endian() {
        let mut spy = SpyHasher::default();
        write_flag(&mut spy, 0x0102_0304_0506_0708);
        assert_eq!(spy.bytes, 0x0102_0304_0506_0708u64.to_le_bytes());
    }

    #[test]
    fn test_hash_record_is_deterministic() {
        let r1 = record(b"ACGT", b"read1", Some(3));
        let r2 = record(b"ACGT", b"read1", Some(3));
        assert_eq!(
            hash_record(&r1, ALL_FIELDS, Mate::Both),
            hash_record(&r2, ALL_FIELDS, Mate::Both)
        );
    }

    #[test]
    fn test_hash_record_differs_on_sequence() {
        let r1 = record(b"ACGT", b"read1", Some(3));
        let r2 = record(b"TTTT", b"read1", Some(3));
        assert_ne!(
            hash_record(&r1, ALL_FIELDS, Mate::Both),
            hash_record(&r2, ALL_FIELDS, Mate::Both)
        );
    }

    /// Concatenation without length-framing could let a header/sequence
    /// boundary shift produce the same bytes; length-prefixing must prevent
    /// that collision.
    #[test]
    fn test_hash_record_no_boundary_collision() {
        let a = record(b"AC", b"GT", None);
        let b = record(b"ACG", b"T", None);
        let fields = FieldMask {
            seq: true,
            qual: false,
            headers: true,
            flags: false,
        };
        assert_ne!(
            hash_record(&a, fields, Mate::Both),
            hash_record(&b, fields, Mate::Both)
        );
    }

    /// A record with no flag (`None`) contributes nothing to the hash - same
    /// as `--skip-flags` - since binseq only produces `None` for records in
    /// files that don't carry flags at all. A record that does carry a flag
    /// (even `Some(0)`) must still change the hash.
    #[test]
    fn test_hash_record_flag_none_is_noop() {
        let with_none = record(b"ACGT", b"h", None);
        let with_zero = record(b"ACGT", b"h", Some(0));
        let fields = FieldMask {
            seq: false,
            qual: false,
            headers: false,
            flags: true,
        };
        let no_flags = FieldMask {
            flags: false,
            ..fields
        };
        assert_eq!(
            hash_record(&with_none, fields, Mate::Both),
            hash_record(&with_none, no_flags, Mate::Both),
            "a record with no flag should hash the same whether or not flags are included"
        );
        assert_ne!(
            hash_record(&with_none, fields, Mate::Both),
            hash_record(&with_zero, fields, Mate::Both),
            "a record that does carry a flag must still change the hash"
        );
    }

    #[test]
    fn test_processor_checksum_is_order_independent() {
        let records = [
            record(b"AAAA", b"r1", Some(1)),
            record(b"CCCC", b"r2", Some(2)),
            record(b"GGGG", b"r3", None),
        ];

        let forward: Wrapping<u64> = records
            .iter()
            .map(|r| Wrapping(hash_record(r, ALL_FIELDS, Mate::Both)))
            .sum();
        let reversed: Wrapping<u64> = records
            .iter()
            .rev()
            .map(|r| Wrapping(hash_record(r, ALL_FIELDS, Mate::Both)))
            .sum();

        assert_eq!(forward, reversed);
    }
}
