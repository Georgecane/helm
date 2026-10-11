//! Helm's versioned, lossless container format.
//!
//! The first implementation deliberately uses a proven general-purpose codec as
//! a baseline. Structure discovery and adaptive candidate ranking will be added
//! behind the same exact-reconstruction contract.

use std::fmt;
use std::io::{Read, Write};

const MAGIC: &[u8; 4] = b"HELM";
const VERSION: u8 = 1;
const CODEC_ZSTD: u8 = 1;
const HEADER_LEN: usize = 48;

/// Default maximum decoded size accepted by [`unpack`].
pub const DEFAULT_MAX_OUTPUT_BYTES: u64 = 1024 * 1024 * 1024;

/// Metadata stored in a Helm container header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub version: u8,
    pub codec: &'static str,
    pub original_size: u64,
    pub checksum_blake3: [u8; 32],
    pub payload_size: u64,
}

/// Errors returned by Helm container operations.
#[derive(Debug)]
pub enum HelmError {
    Io(std::io::Error),
    InvalidHeader(&'static str),
    UnsupportedVersion(u8),
    UnsupportedCodec(u8),
    OutputLimitExceeded { declared: u64, limit: u64 },
    LengthMismatch { expected: u64, actual: u64 },
    ChecksumMismatch,
}

impl fmt::Display for HelmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O or codec error: {error}"),
            Self::InvalidHeader(reason) => write!(f, "invalid Helm header: {reason}"),
            Self::UnsupportedVersion(version) => {
                write!(f, "unsupported Helm format version: {version}")
            }
            Self::UnsupportedCodec(codec) => write!(f, "unsupported Helm codec id: {codec}"),
            Self::OutputLimitExceeded { declared, limit } => write!(
                f,
                "container declares {declared} output bytes, exceeding the {limit}-byte limit"
            ),
            Self::LengthMismatch { expected, actual } => {
                write!(f, "decoded length mismatch: expected {expected}, got {actual}")
            }
            Self::ChecksumMismatch => write!(f, "decoded data failed the BLAKE3 checksum"),
        }
    }
}

impl std::error::Error for HelmError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for HelmError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

/// Compress bytes into a versioned .helm container.
///
/// Reconstruction is byte-for-byte exact. The current baseline codec is Zstandard.
pub fn pack(input: &[u8]) -> Result<Vec<u8>, HelmError> {
    let compressed = zstd::stream::encode_all(input, 9)?;
    let digest = blake3::hash(input);
    let mut output = Vec::with_capacity(HEADER_LEN + compressed.len());
    output.extend_from_slice(MAGIC);
    output.push(VERSION);
    output.push(CODEC_ZSTD);
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&(input.len() as u64).to_le_bytes());
    output.extend_from_slice(digest.as_bytes());
    output.extend_from_slice(&compressed);
    Ok(output)
}

/// Decode a .helm container while enforcing an explicit output-size limit.
pub fn unpack_with_limit(container: &[u8], max_output_bytes: u64) -> Result<Vec<u8>, HelmError> {
    let metadata = inspect(container)?;
    if metadata.original_size > max_output_bytes {
        return Err(HelmError::OutputLimitExceeded {
            declared: metadata.original_size,
            limit: max_output_bytes,
        });
    }

    let expected_len = usize::try_from(metadata.original_size)
        .map_err(|_| HelmError::OutputLimitExceeded {
            declared: metadata.original_size,
            limit: max_output_bytes,
        })?;

    let decoder = zstd::stream::read::Decoder::new(&container[HEADER_LEN..])?;
    let mut limited = decoder.take(metadata.original_size.saturating_add(1));
    let mut decoded = Vec::with_capacity(expected_len.min(8 * 1024 * 1024));
    limited.read_to_end(&mut decoded)?;

    if decoded.len() as u64 != metadata.original_size {
        return Err(HelmError::LengthMismatch {
            expected: metadata.original_size,
            actual: decoded.len() as u64,
        });
    }

    if blake3::hash(&decoded).as_bytes() != &metadata.checksum_blake3 {
        return Err(HelmError::ChecksumMismatch);
    }
    Ok(decoded)
}

/// Decode using the conservative default limit of 1 GiB.
pub fn unpack(container: &[u8]) -> Result<Vec<u8>, HelmError> {
    unpack_with_limit(container, DEFAULT_MAX_OUTPUT_BYTES)
}

/// Parse and validate the fixed header without decompressing the payload.
pub fn inspect(container: &[u8]) -> Result<Metadata, HelmError> {
    if container.len() < HEADER_LEN {
        return Err(HelmError::InvalidHeader("container is shorter than 48 bytes"));
    }
    if &container[0..4] != MAGIC {
        return Err(HelmError::InvalidHeader("bad magic bytes"));
    }

    let version = container[4];
    if version != VERSION {
        return Err(HelmError::UnsupportedVersion(version));
    }
    let codec_id = container[5];
    if codec_id != CODEC_ZSTD {
        return Err(HelmError::UnsupportedCodec(codec_id));
    }
    if container[6] != 0 || container[7] != 0 {
        return Err(HelmError::InvalidHeader("reserved flags must be zero"));
    }

    let original_size = u64::from_le_bytes(
        container[8..16]
            .try_into()
            .expect("header length was checked"),
    );
    let mut checksum_blake3 = [0_u8; 32];
    checksum_blake3.copy_from_slice(&container[16..48]);

    Ok(Metadata {
        version,
        codec: "zstd",
        original_size,
        checksum_blake3,
        payload_size: (container.len() - HEADER_LEN) as u64,
    })
}

/// Write all bytes to a writer, exposed for consumers that stream their own output.
pub fn write_all<W: Write>(writer: &mut W, bytes: &[u8]) -> Result<(), HelmError> {
    writer.write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_empty_input() {
        let packed = pack(&[]).unwrap();
        assert_eq!(unpack(&packed).unwrap(), b"");
    }

    #[test]
    fn round_trips_repetitive_input() {
        let input = b"Helm discovers structure. ".repeat(20_000);
        let packed = pack(&input).unwrap();
        assert_eq!(unpack(&packed).unwrap(), input);
        assert!(packed.len() < input.len());
    }

    #[test]
    fn round_trips_all_byte_values() {
        let input: Vec<u8> = (0..=255).cycle().take(16_384).collect();
        assert_eq!(unpack(&pack(&input).unwrap()).unwrap(), input);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut packed = pack(b"hello").unwrap();
        packed[0] = b'X';
        assert!(matches!(unpack(&packed), Err(HelmError::InvalidHeader(_))));
    }

    #[test]
    fn rejects_truncated_header() {
        assert!(matches!(unpack(b"HELM"), Err(HelmError::InvalidHeader(_))));
    }

    #[test]
    fn enforces_output_limit_before_decoding() {
        let packed = pack(b"123456789").unwrap();
        assert!(matches!(
            unpack_with_limit(&packed, 8),
            Err(HelmError::OutputLimitExceeded { declared: 9, limit: 8 })
        ));
    }

    #[test]
    fn reports_header_metadata() {
        let input = b"metadata test";
        let packed = pack(input).unwrap();
        let metadata = inspect(&packed).unwrap();
        assert_eq!(metadata.version, 1);
        assert_eq!(metadata.codec, "zstd");
        assert_eq!(metadata.original_size, input.len() as u64);
        assert_eq!(metadata.payload_size, (packed.len() - HEADER_LEN) as u64);
    }

    #[test]
    fn detects_payload_corruption() {
        let mut packed = pack(&vec![b'A'; 4096]).unwrap();
        let last = packed.len() - 1;
        packed[last] ^= 0x40;
        assert!(unpack(&packed).is_err());
    }
}
