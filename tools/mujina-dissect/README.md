# mujina-dissect

A comprehensive protocol dissector for captured I2C and serial data from
Bitcoin mining hardware.

## Overview

The `mujina-dissect` tool analyzes communication between the host and mining
hardware, providing detailed protocol-level insights. The tool reuses the same
protocol parsing code as the miner itself (from `mujina-miner/src/asic/` and
`mujina-miner/src/peripheral/`), ensuring consistency between analysis and
runtime behavior.

## Features

- **BM13xx ASIC Protocol**: Dissects serial commands and responses with CRC
  validation
- **PMBus/I2C Analysis**: Decodes power management operations with Linear11/
  Linear16 format conversion
- **TPS546 Power Controller**: Context-aware state tracking including
  VOUT_MODE interpretation
- **Human-Readable Output**: Color-coded, formatted display of protocol
  transactions

## Supported Input Formats

The dissector reads CSV files exported from Saleae Logic analyzers:
- Digital serial captures (TX/RX pins)
- I2C protocol analyzer exports

## Usage

```bash
# Analyze a capture file
cargo run --bin mujina-dissect -- path/to/capture.csv

# Filter by protocol
cargo run --bin mujina-dissect -- path/to/capture.csv -p bm13xx
cargo run --bin mujina-dissect -- path/to/capture.csv -f I2C

# Show hexdump alongside decoded output
cargo run --bin mujina-dissect -- path/to/capture.csv -x

# Force color output when piping
cargo run --bin mujina-dissect -- path/to/capture.csv --force-color | less -R
```

## Architecture

### Input Processing (`csv.rs`)

Parses Saleae Logic analyzer CSV exports, handling both digital (serial) and
I2C protocol exports with timestamped samples for accurate timing analysis.

### Protocol Parsers

- **`bm13xx.rs`**: BM13xx serial protocol dissector (calls into
  `mujina-miner/src/asic/bm13xx/protocol.rs`)
- **`i2c.rs`**: I2C transaction assembler and PMBus parser (calls into
  `mujina-miner/src/peripheral/` modules)

### Output Formatting (`main.rs`)

- Color-coded output using `colored` crate
- Optional hex dump display with `-x` flag
- Timestamp-aligned presentation

## Testing

Tests for the dissector are in each module:
- `csv.rs`: CSV parsing and sample extraction
- `i2c.rs`: Transaction assembly, PMBus parsing, context tracking
- `bm13xx.rs`: Frame detection, command/response parsing

Run tests:
```bash
cargo test --package mujina-dissect
```

**Important:** The dissector binary itself has no unit tests (it's in
`src/main.rs`). Test the parsing logic in the module-level tests instead.

## Development Guidelines

### When Modifying the Dissector

- Prefer using existing parser code from `mujina-miner/src/` over duplicating
  parsing logic
- If you need to modify how something is decoded, consider whether the change
  belongs in the miner code (where it will be shared)
- The dissector should call into the miner's codec/parser implementations, not
  reimplement them
- Add tests for new protocol features in both the miner and dissector

### Common Tasks

- **Adding a new BM13xx command**: Update
  `mujina-miner/src/asic/bm13xx/protocol.rs` first, then dissector will
  automatically decode it
- **Adding a new I2C peripheral**: Add parser to
  `mujina-miner/src/peripheral/<device>.rs`, then integrate into
  `tools/mujina-dissect/src/i2c.rs`
- **Fixing decode issues**: Likely needs fixing in the miner code, not the
  dissector