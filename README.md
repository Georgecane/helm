# Helm

Helm is an experimental lossless compression system focused on exact reconstruction, computational structure discovery, and adaptive optimization.

**Status:** early prototype. The current codec is Helm's own native LZ77 token engine. It does not use Zstandard or another third-party compression codec. SMDR, Gaia, Multivariable Mean, manifold atlases, streaming, and adaptive candidate selection remain planned work. No extreme compression ratio is claimed.

## Build and test

Requires a stable Rust toolchain.

    cargo build --release
    cargo fmt --all -- --check
    cargo clippy --all-targets --all-features -- -D warnings
    cargo test --all-targets

## CLI

    cargo run --release -- pack input.bin output.helm
    cargo run --release -- unpack output.helm restored.bin
    cargo run --release -- inspect output.helm

Existing output files are not overwritten unless --force is passed to pack or unpack.

## Native container format v2

The version was incremented when the payload changed from the original prototype's Zstandard stream to Helm's native codec, so old prototype files are not silently misread.

The 48-byte little-endian header contains:

| Offset | Length | Meaning |
| ---: | ---: | --- |
| 0 | 4 | Magic bytes HELM |
| 4 | 1 | Format version (2) |
| 5 | 1 | Codec ID (1 = Helm LZ77 v1) |
| 6 | 2 | Reserved flags (must be zero) |
| 8 | 8 | Original byte length |
| 16 | 32 | BLAKE3 checksum of original bytes |

The payload is a Helm-owned token stream:

- Literal token: tag 0, a 16-bit little-endian length, then literal bytes.
- Match token: tag 1, a 16-bit little-endian backward distance, then a 16-bit little-endian match length.

The current greedy matcher uses a 3-byte key and retains the most recent position for each key. Matches are limited to a 65,535-byte backward window. The decoder validates token boundaries, distances, output length, and checksum, and enforces a default 1 GiB output limit.

This is a deliberately small first native engine, not the final compression architecture. The current format is experimental and may change before stable releases.

## Planned structure-discovery architecture

The intended adaptive search has four rounds:

1. SMDR and Gaia independently propose candidate structures.
2. Multivariable Mean analyzes the original data and discovered structures, then proposes new candidates.
3. Helm evaluates candidate representations using exact reconstruction, encoded size, compute cost, and speed; weak candidates may be pruned.
4. The best verified representation is selected only after the rounds, and must reconstruct the source exactly.

These components are design goals, not yet implemented. A candidate must never be accepted solely because it is smaller: exact reconstruction is mandatory.

## License

GPL-3.0-or-later. See LICENSE.
