# mujina-miner

Open source Bitcoin mining software written in Rust for ASIC mining hardware.

> **Developer Preview**: This software is under heavy development and not ready
> for production use. The code is made available for developers interested in
> contributing, learning about Bitcoin mining protocols, or evaluating the
> architecture. APIs, protocols, and features are subject to change without
> notice. Documentation is incomplete and may be inaccurate. Use at your own
> risk.

## Overview

mujina-miner is a modern, async Rust implementation of Bitcoin mining software
designed to communicate with various Bitcoin mining hash boards via USB serial
interfaces. Part of the larger Mujina OS project, an open source, Debian-based
embedded Linux distribution optimized for Bitcoin mining hardware.

## Features

- **Heterogeneous Multi-Board Support**: Mix and match different hash board
  types in a single deployment; hot-swappable, no need to restart when adding
  or removing boards
- **Hackable & Extensible**: Clear, modular architecture with well-documented
  internals - designed for modification, experimentation, and custom extensions
- **Reference-Grade Documentation**: Thorough documentation at every layer,
  from chip protocols to system architecture, serving as both implementation
  guide and educational resource
- **API-Driven Control**: REST API for all operations---implement your own
  control strategies, automate operations, or build custom interfaces on top
- **Open-Source, Open-Contribution**: Active development with open
  contribution; not code dumps or abandonware, a living project built by
  the entire community
- **Accessible Development**: Start developing with minimal hardware; a laptop
  and a single [Bitaxe](mujina-miner/src/board/bitaxe_gamma.md) board is enough
  to contribute meaningfully

## Supported Hardware

Currently supported:
- [**Bitaxe Gamma**](mujina-miner/src/board/bitaxe_gamma.md) with BM1370 ASIC

Planned support:
- **EmberOne** with BM1362 ASIC
- **EmberOne** with Intel BZM2 ASICs
- Antminer S19j Pro hash boards
- Any and all ASIC mining hardware

## Documentation

### Project Documentation

- [Architecture Overview](docs/architecture.md) - System design and component
  interaction
- [Contribution Guide](CONTRIBUTING.md) - How to contribute to the project
- [Code Style Guide](CODE_STYLE.md) - Formatting and style rules
- [Coding Guidelines](CODING_GUIDELINES.md) - Best practices and design
  patterns

### Protocol Documentation

- [BM13xx ASIC Protocol](mujina-miner/src/asic/bm13xx/PROTOCOL.md) - Serial
  protocol for BM13xx series mining chips
- [Bitaxe-Raw Control Protocol](mujina-miner/src/mgmt_protocol/bitaxe_raw/PROTOCOL.md) -
  Management protocol for Bitaxe board peripherals

### Hardware Documentation

- [Bitaxe Gamma Board Guide](mujina-miner/src/board/bitaxe_gamma.md) - Hardware
  and software interface documentation for Bitaxe Gamma

## Build Requirements

### Linux

On Debian/Ubuntu systems:

```bash
sudo apt-get install libudev-dev
```

The `libudev-dev` package provides header files for building, and depends on
the `libudev` package which provides the library for runtime.

### macOS

macOS support is planned, but USB discovery using IOKit is not yet implemented.

## Building

```bash
# Build the workspace
cargo build

# Run unit tests (no hardware required)
cargo test
```

## Running

```bash
# Run with default logging
cargo run

# Run with debug logging
RUST_LOG=mujina_miner=debug cargo run

# Run with trace logging (shows all hardware and network communication)
RUST_LOG=mujina_miner=trace cargo run
```

## Protocol Analysis Tool

The `mujina-dissect` tool analyzes captured communication between the host and
mining hardware, providing detailed protocol-level insights for BM13xx serial
commands, PMBus/I2C power management, and fan control.

See [tools/mujina-dissect/README.md](tools/mujina-dissect/README.md) for
detailed usage and documentation.

## License

This project is licensed under the GNU General Public License v3.0 or later.
See the [LICENSE](LICENSE) file for details.

## Contributing

We welcome contributions! Whether you're fixing bugs, adding features, improving
documentation, or simply exploring the codebase to learn about Bitcoin mining
protocols and hardware, your involvement is valued.

Please see our [Contribution Guide](CONTRIBUTING.md) for details on how to get
started.

## Related Projects

- [Bitaxe](https://github.com/bitaxeorg) - Open-source Bitcoin mining
  hardware designs
- [bitaxe-raw](https://github.com/bitaxeorg/bitaxe-raw) - Firmware for Bitaxe
  boards
- [EmberOne](https://github.com/256foundation/emberone00-pcb) - Open-source
  Bitcoin mining hashboard
- [emberone-usbserial-fw](https://github.com/256foundation/emberone-usbserial-fw) -
  Firmware for EmberOne boards
