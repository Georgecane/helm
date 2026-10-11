//! Helm's native lossless container and codec.
//!
//! Format v2 uses Helm's own greedy LZ77 match finder and token stream. No
//! third-party compression codec is used. The format is experimental.

mod codec;
use std::fmt;
const MAGIC: &[u8; 4] = b"HELM";
const VERSION: u8 = 2;
const CODEC_HELM_LZ77: u8 = 1;
const HEADER_LEN: usize = 48;
pub const DEFAULT_MAX_OUTPUT_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    pub version: u8,
    pub codec: &'static str,
    pub original_size: u64,
    pub checksum_blake3: [u8; 32],
    pub payload_size: u64,
}

#[derive(Debug)]
pub enum HelmError {
    Io(std::io::Error),
    InvalidHeader(&'static str),
    InvalidPayload(&'static str),
    UnsupportedVersion(u8),
    UnsupportedCodec(u8),
    OutputLimitExceeded { declared: u64, limit: u64 },
    LengthMismatch { expected: u64, actual: u64 },
    ChecksumMismatch,
}

impl fmt::Display for HelmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::InvalidHeader(reason) => write!(f, "invalid Helm header: {reason}"),
            Self::InvalidPayload(reason) => write!(f, "invalid Helm payload: {reason}"),
            Self::UnsupportedVersion(version) => write!(f, "unsupported Helm format version: {version}"),
            Self::UnsupportedCodec(codec) => write!(f, "unsupported Helm codec id: {codec}"),
            Self::OutputLimitExceeded { declared, limit } => write!(f, "container declares {declared} output bytes, exceeding the {limit}-byte limit"),
            Self::LengthMismatch { expected, actual } => write!(f, "decoded length mismatch: expected {expected}, got {actual}"),
            Self::ChecksumMismatch => write!(f, "decoded data failed the BLAKE3 checksum"),
        }
    }
}
impl std::error::Error for HelmError {}
impl From<std::io::Error> for HelmError {
    fn from(value: std::io::Error) -> Self { Self::Io(value) }
}

pub fn pack(input: &[u8]) -> Result<Vec<u8>, HelmError> {
    let compressed = codec::encode(input);
    let digest = blake3::hash(input);
    let mut output = Vec::with_capacity(HEADER_LEN + compressed.len());
    output.extend_from_slice(MAGIC);
    output.push(VERSION);
    output.push(CODEC_HELM_LZ77);
    output.extend_from_slice(&0_u16.to_le_bytes());
    output.extend_from_slice(&(input.len() as u64).to_le_bytes());
    output.extend_from_slice(digest.as_bytes());
    output.extend_from_slice(&compressed);
    Ok(output)
}

pub fn unpack_with_limit(container: &[u8], max_output_bytes: u64) -> Result<Vec<u8>, HelmError> {
    let metadata = inspect(container)?;
    if metadata.original_size > max_output_bytes {
        return Err(HelmError::OutputLimitExceeded { declared: metadata.original_size, limit: max_output_bytes });
    }
    let decoded = codec::decode(&container[HEADER_LEN..], metadata.original_size)?;
    if decoded.len() as u64 != metadata.original_size {
        return Err(HelmError::LengthMismatch { expected: metadata.original_size, actual: decoded.len() as u64 });
    }
    if blake3::hash(&decoded).as_bytes() != &metadata.checksum_blake3 {
        return Err(HelmError::ChecksumMismatch);
    }
    Ok(decoded)
}
pub fn unpack(container: &[u8]) -> Result<Vec<u8>, HelmError> {
    unpack_with_limit(container, DEFAULT_MAX_OUTPUT_BYTES)
}

pub fn inspect(container: &[u8]) -> Result<Metadata, HelmError> {
    if container.len() < HEADER_LEN { return Err(HelmError::InvalidHeader("container is shorter than 48 bytes")); }
    if &container[0..4] != MAGIC { return Err(HelmError::InvalidHeader("bad magic bytes")); }
    if container[4] != VERSION { return Err(HelmError::UnsupportedVersion(container[4])); }
    if container[5] != CODEC_HELM_LZ77 { return Err(HelmError::UnsupportedCodec(container[5])); }
    if container[6] != 0 || container[7] != 0 { return Err(HelmError::InvalidHeader("reserved flags must be zero")); }
    let original_size = u64::from_le_bytes(container[8..16].try_into().expect("header length checked"));
    let mut checksum_blake3 = [0_u8; 32];
    checksum_blake3.copy_from_slice(&container[16..48]);
    Ok(Metadata { version: VERSION, codec: "helm-lz77-v1", original_size, checksum_blake3, payload_size: (container.len() - HEADER_LEN) as u64 })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn empty_roundtrip() { let p = pack(&[]).unwrap(); assert_eq!(unpack(&p).unwrap(), b""); }
    #[test] fn repetitive_roundtrip_and_compression() { let x = b"Helm discovers structure. ".repeat(20_000); let p = pack(&x).unwrap(); assert_eq!(unpack(&p).unwrap(), x); assert!(p.len() < x.len() / 2); }
    #[test] fn all_bytes_roundtrip() { let x: Vec<u8> = (0..=255).cycle().take(16_384).collect(); assert_eq!(unpack(&pack(&x).unwrap()).unwrap(), x); }
    #[test] fn randomish_roundtrip() { let mut s = 0x1234_5678_u32; let x: Vec<u8> = (0..32_000).map(|_| { s ^= s << 13; s ^= s >> 17; s ^= s << 5; s as u8 }).collect(); assert_eq!(unpack(&pack(&x).unwrap()).unwrap(), x); }
    #[test] fn bad_magic_rejected() { let mut p = pack(b"hello").unwrap(); p[0] = b'X'; assert!(matches!(unpack(&p), Err(HelmError::InvalidHeader(_)))); }
    #[test] fn truncated_header_rejected() { assert!(matches!(unpack(b"HELM"), Err(HelmError::InvalidHeader(_)))); }
    #[test] fn output_limit_enforced() { let p = pack(b"123456789").unwrap(); assert!(matches!(unpack_with_limit(&p, 8), Err(HelmError::OutputLimitExceeded { declared: 9, limit: 8 }))); }
    #[test] fn metadata_reports_native_codec() { let x = b"metadata test"; let m = inspect(&pack(x).unwrap()).unwrap(); assert_eq!(m.version, 2); assert_eq!(m.codec, "helm-lz77-v1"); assert_eq!(m.original_size, x.len() as u64); }
    #[test] fn corruption_detected() { let mut p = pack(&[b'A'; 4096]).unwrap(); let i = p.len()-1; p[i] ^= 0x40; assert!(unpack(&p).is_err()); }
    #[test] fn unknown_codec_rejected() { let mut p = pack(b"hello").unwrap(); p[5] = 99; assert!(matches!(unpack(&p), Err(HelmError::UnsupportedCodec(99)))); }
}
