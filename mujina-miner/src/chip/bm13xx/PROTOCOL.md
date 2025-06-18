# BM13xx Protocol Documentation

This document describes the BM13xx family ASIC protocol as implemented in this codebase.

## Overview

The BM13xx family (BM1366, BM1370, etc.) uses a frame-based serial protocol for communication between the host and mining ASICs. The protocol supports both command/response patterns and asynchronous nonce reporting.

## Frame Format

All frames follow this basic structure:
```
| Preamble | Type/Flags | Length | Payload | CRC |
```

### Command Frames (Host → ASIC)
- **Preamble**: `0x55 0xAA` (2 bytes)
- **Type/Flags**: 1 byte encoding type, broadcast flag, and command
- **Length**: 1 byte total frame length
- **Payload**: Variable length data
- **CRC**: CRC5 for commands, CRC16 for jobs

### Response Frames (ASIC → Host)
- **Preamble**: `0xAA 0x55` (2 bytes, reversed from commands)
- **Payload**: Response-specific data
- **CRC**: CRC5 in last byte (bits 0-4), with response type in bits 5-7

## Byte Order (Endianness)

**All multi-byte values in the BM13xx protocol use little-endian byte order.**

This means for multi-byte values:
- The least significant byte (LSB) is transmitted first
- The most significant byte (MSB) is transmitted last

Examples:
- 16-bit value `0x1234` → transmitted as `[0x34, 0x12]`
- 32-bit value `0x12345678` → transmitted as `[0x78, 0x56, 0x34, 0x12]`

Affected fields:
- **16-bit values**: version, chip_id, CRC16
- **32-bit values**: nonce, nbits, ntime, register values

Special cases:
- **chip_id field**: This 2-byte field is transmitted in big-endian order (e.g., BM1370 = `0x13 0x70`). It may be better interpreted as a fixed byte sequence `[0x13, 0x70]` rather than as an integer value.
- **Hash values** (merkle_root, prev_block_hash): These are byte arrays that should be transmitted as-is without endianness conversion
- **Single bytes**: No endianness applies (job_id, midstate_num, etc.)

## Command Types

### Read Register (CMD=2)
Reads a 4-byte register from the ASIC.

**Request Format:**
```
| 0x55 0xAA | Flags | Length | Chip_Addr | Reg_Addr | CRC5 |
```
- Flags: `0x52` for broadcast, `0x42` for specific chip
- Example: `55 AA 52 05 00 00 0A` (broadcast read of register 0x00)

### Write Register (CMD=1)
Writes a 4-byte value to a register.

**Request Format:**
```
| 0x55 0xAA | Flags | Length | Chip_Addr | Reg_Addr | Data[4] | CRC5 |
```

### Mining Job (TYPE=1, CMD=1)

BM13xx supports two job formats:

#### Full Format (BM1370/BM1366)
The ASIC calculates SHA256 midstates internally.

**Request Format:**
```
| 0x55 0xAA | 0x21 | Length | Job_Data | CRC16 |
```

**Job_Data Structure (82 bytes):**
```
| job_id | num_midstates | starting_nonce[4] | nbits[4] | ntime[4] | merkle_root[32] | prev_block_hash[32] | version[4] |
```

#### Midstate Format (BM1397)
The host pre-calculates SHA256 midstates.

**Request Format:**
```
| 0x55 0xAA | 0x21 | Length | Job_Data | CRC16 |
```

**Job_Data Structure (variable):**
```
| job_id | num_midstates | starting_nonce[4] | nbits[4] | ntime[4] | merkle4[4] | midstate0[32] | [midstate1-3[32]] |
```

## Response Types

### Read Register Response (TYPE=0)
**Format (9 bytes total):**
```
| 0xAA 0x55 | Register_Value[4] | Chip_Addr | Reg_Addr | CRC5+Type |
```

### Nonce Response (TYPE=4)

**BM1370 Format (11 bytes total):**
```
| 0xAA 0x55 | Nonce[4] | Midstate_Num | Job_ID | Version[2] | CRC5+Type |
```

**Field Encoding:**
- **Nonce**: 32-bit nonce value
  - Bits 31-25: Core ID (which of 128 cores found it)
  - Bits 24-0: Actual nonce value
- **Job_ID**: Encodes both job ID and small core ID
  - Bits 7-1: Actual job ID (shifted left by 1)
  - Bits 0-3: Small core ID (0-15)
- **Version**: 16-bit version for version rolling

## Key Implementation Details

### Job ID Management
- Job IDs cycle through values: 0, 24, 48, 72, 96, 120, then back to 0
- This ensures job IDs are well-distributed across the 7-bit space

### CRC Calculation
- **CRC5**: Used for command/response frames
  - Polynomial: 0x05
  - Init: 0x1F
  - Calculated over all bytes after preamble
- **CRC16**: Used for job packets only
  - Polynomial: 0x1021 (CRC-16-CCITT-FALSE)
  - Init: 0xFFFF
  - Calculated over all bytes after preamble, before CRC

### Version Rolling
When enabled, ASICs can modify the version field in the block header to expand their search space beyond the 32-bit nonce range.

## References
- ESP-miner BM1370 implementation
- BM1397 protocol documentation
- CGMiner driver implementations