# Helm

Helm is an experimental lossless compression system focused on exact reconstruction, computational structure discovery, and adaptive optimization.

**Status:** early prototype. The encoder now evaluates native reversible structure candidates in four rounds before selecting a representation. It does not use Zstandard or another third-party compression codec. Gaia is currently a small adaptive byte-context predictor, not a general-purpose trained neural model. Continuous manifold atlases and generalized symbolic equation discovery remain future work. No extreme compression ratio is claimed.

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

The payload begins with a one-byte transform identifier, followed by Helm's token stream. IDs: 0 identity; 1, 2, 3, 4 byte-stride residuals at strides 1, 2, 4, and 8; 5 adaptive Gaia context residual; 6 Multivariable Mean residual. The token stream is:

- Literal token: tag 0, a 16-bit little-endian length, then literal bytes.
- Match token: tag 1, a 16-bit little-endian backward distance, then a 16-bit little-endian match length.

The four-round candidate search runs (1) SMDR-style reversible symbolic residuals across byte strides, (2) Gaia's adaptive successor model, (3) a two-symbol Multivariable Mean predictor, and (4) encoded-cost comparison with exact reconstruction mandatory. The smallest verified token stream wins. The native token coder uses a 3-byte match key and a 65,535-byte backward window. The decoder validates token boundaries, distances, transform IDs, output length, and checksum, and enforces a default 1 GiB output limit.

This is a deliberately small first native engine, not the final compression architecture. The current format is experimental and may change before stable releases.

## Planned structure-discovery architecture

The intended adaptive search has four rounds:

1. SMDR and Gaia independently propose candidate structures.
2. Multivariable Mean analyzes the original data and discovered structures, then proposes new candidates.
3. Helm evaluates candidate representations using exact reconstruction, encoded size, compute cost, and speed; weak candidates may be pruned.
4. The best verified representation is selected only after the rounds, and must reconstruct the source exactly.

The current implementation is an executable first skeleton, not the finished mathematical architecture: SMDR currently proposes byte-stride residuals; Gaia is an adaptive context predictor rather than a full trained ML discovery engine; Multivariable Mean is a two-symbol predictor; and continuous manifold atlases plus generalized symbolic equation discovery are not implemented yet. A candidate must never be accepted solely because it is smaller: exact reconstruction is mandatory.

## License

GPL-3.0-or-later. See LICENSE.
