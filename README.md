# Helm

Helm is an experimental lossless data-compression system focused on exact reconstruction, computational structure discovery, and adaptive optimization.

**Status:** early prototype. The current implementation is a versioned container using Zstandard as a baseline codec. It does not yet implement SMDR, Gaia, Multivariable Mean, streaming, or adaptive candidate selection, and no extreme-compression ratio is claimed.

## Build

Requires a stable Rust toolchain and a C toolchain supported by the zstd crate.

    cargo build --release
    cargo test --all-targets

## CLI

    # Compress a file into a Helm container
    cargo run --release -- pack input.bin output.helm

    # Restore the original bytes exactly
    cargo run --release -- unpack output.helm restored.bin

    # Inspect the container header
    cargo run --release -- inspect output.helm

Existing output files are not overwritten unless --force is passed to pack or unpack.

## Container v1

The fixed 48-byte header is little-endian and consists of:

| Offset | Length | Meaning |
| ---: | ---: | --- |
| 0 | 4 | Magic bytes HELM |
| 4 | 1 | Format version (1) |
| 5 | 1 | Codec ID (1 = Zstandard) |
| 6 | 2 | Reserved flags (must be zero) |
| 8 | 8 | Original byte length |
| 16 | 32 | BLAKE3 checksum of original bytes |

The header is followed by the Zstandard payload. Decoding validates the header, enforces a default 1 GiB output limit, checks the decoded length, and verifies the checksum. Library users can set a different limit with unpack_with_limit.

## Planned discovery architecture

The intended adaptive search has four rounds:

1. **SMDR** and **Gaia** independently propose candidate structures.
2. **Multivariable Mean** analyzes the original data and discovered structures, then proposes new candidates.
3. Helm evaluates candidates using exact reconstruction, encoded size, compute cost, and speed; weak candidates may be pruned.
4. The winning path is selected only after all rounds, and its representation must reconstruct the source exactly.

These are design goals, not implemented capabilities. A candidate must never be accepted solely because it is smaller: exact reconstruction is mandatory.

## License

GPL-3.0-or-later. See LICENSE.
