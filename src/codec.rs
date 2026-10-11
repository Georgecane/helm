//! Native Helm LZ77 token codec.
//! Tokens: 0 + u16 length + literal bytes; 1 + u16 backward distance + u16 match length.
use std::collections::HashMap;
use crate::HelmError;

const LITERAL: u8 = 0;
const MATCH: u8 = 1;
const MIN_MATCH: usize = 4;
const MAX_TOKEN: usize = u16::MAX as usize;
const WINDOW: usize = u16::MAX as usize;

fn key(input: &[u8], at: usize) -> u32 {
    u32::from(input[at]) | (u32::from(input[at + 1]) << 8) | (u32::from(input[at + 2]) << 16)
}
fn put_u16(output: &mut Vec<u8>, value: usize) { output.extend_from_slice(&(value as u16).to_le_bytes()); }
fn emit_literals(output: &mut Vec<u8>, input: &[u8]) {
    for chunk in input.chunks(MAX_TOKEN) {
        output.push(LITERAL);
        put_u16(output, chunk.len());
        output.extend_from_slice(chunk);
    }
}

pub(super) fn encode(input: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(input.len());
    let mut latest: HashMap<u32, usize> = HashMap::new();
    let (mut position, mut literal_start) = (0, 0);
    while position + 2 < input.len() {
        let k = key(input, position);
        let candidate = latest.insert(k, position);
        let mut best_len = 0;
        let mut best_distance = 0;
        if let Some(previous) = candidate {
            let distance = position - previous;
            if distance <= WINDOW {
                let max_len = (input.len() - position).min(MAX_TOKEN);
                let mut length = 0;
                while length < max_len && input[previous + (length % distance)] == input[position + length] {
                    length += 1;
                }
                if length >= MIN_MATCH { best_len = length; best_distance = distance; }
            }
        }
        if best_len >= MIN_MATCH {
            emit_literals(&mut output, &input[literal_start..position]);
            output.push(MATCH);
            put_u16(&mut output, best_distance);
            put_u16(&mut output, best_len);
            let end = position + best_len;
            let mut i = position + 1;
            while i + 2 < end { latest.insert(key(input, i), i); i += 1; }
            position = end;
            literal_start = position;
        } else {
            position += 1;
        }
    }
    emit_literals(&mut output, &input[literal_start..]);
    output
}

pub(super) fn decode(input: &[u8], expected_size: u64) -> Result<Vec<u8>, HelmError> {
    let capacity = usize::try_from(expected_size).map_err(|_| HelmError::InvalidPayload("declared output size does not fit this platform"))?;
    let mut output = Vec::with_capacity(capacity.min(8 * 1024 * 1024));
    let mut cursor = 0;
    while cursor < input.len() {
        let tag = input[cursor];
        cursor += 1;
        match tag {
            LITERAL => {
                if input.len().saturating_sub(cursor) < 2 { return Err(HelmError::InvalidPayload("truncated literal length")); }
                let len = u16::from_le_bytes([input[cursor], input[cursor + 1]]) as usize;
                cursor += 2;
                if len == 0 { return Err(HelmError::InvalidPayload("empty literal token")); }
                if input.len().saturating_sub(cursor) < len { return Err(HelmError::InvalidPayload("truncated literal bytes")); }
                if (output.len() as u64).saturating_add(len as u64) > expected_size { return Err(HelmError::InvalidPayload("token stream exceeds declared output size")); }
                output.extend_from_slice(&input[cursor..cursor + len]);
                cursor += len;
            }
            MATCH => {
                if input.len().saturating_sub(cursor) < 4 { return Err(HelmError::InvalidPayload("truncated match token")); }
                let distance = u16::from_le_bytes([input[cursor], input[cursor + 1]]) as usize;
                let len = u16::from_le_bytes([input[cursor + 2], input[cursor + 3]]) as usize;
                cursor += 4;
                if distance == 0 || distance > output.len() { return Err(HelmError::InvalidPayload("match distance is outside decoded history")); }
                if len < MIN_MATCH { return Err(HelmError::InvalidPayload("match length is below the minimum")); }
                if (output.len() as u64).saturating_add(len as u64) > expected_size { return Err(HelmError::InvalidPayload("token stream exceeds declared output size")); }
                for _ in 0..len { let byte = output[output.len() - distance]; output.push(byte); }
            }
            _ => return Err(HelmError::InvalidPayload("unknown token type")),
        }
    }
    if output.len() as u64 != expected_size {
        return Err(HelmError::LengthMismatch { expected: expected_size, actual: output.len() as u64 });
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn overlapping_match_roundtrip() { let x = b"abcabcabcabcabcabc"; assert_eq!(decode(&encode(x), x.len() as u64).unwrap(), x); }
    #[test] fn invalid_distance_rejected() { assert!(matches!(decode(&[MATCH, 1, 0, 4, 0], 4), Err(HelmError::InvalidPayload(_)))); }
    #[test] fn truncated_literal_rejected() { assert!(matches!(decode(&[LITERAL, 5, 0, b'a'], 5), Err(HelmError::InvalidPayload(_)))); }
}
