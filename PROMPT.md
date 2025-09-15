# Plan: Enhance TPS546 Protocol Dissection for Human-Readable Output

## Problem Analysis
The current TPS546 dissector shows many operations as "UNKNOWN" with raw hex values, making it hard to understand what's happening without consulting datasheets. Example issues from esp-miner-boot.csv at 50+ seconds:
- `WRITE UNKNOWN=[04, ff]` instead of meaningful register names
- `READ UNKNOWN=[66, ca]` instead of decoded voltage/current values
- Missing PMBus Linear11/Linear16 format decoding for measurements
- Limited command coverage despite extensive PMBus infrastructure available

## Implementation Plan

### Phase 1: Expand Command Coverage
1. **Update TPS546 command_name() function** to include all PMBus commands found in the existing pmbus.rs module
   - Add 40+ missing command codes (VIN_ON, VIN_OFF, IOUT_OC_FAULT_LIMIT, etc.)
   - Reference the comprehensive commands module already defined

### Phase 2: Add Value Decoding
2. **Implement PMBus data format decoding** in format_transaction()
   - Linear11 format for voltages, currents, temperatures (READ_VIN, READ_IOUT, READ_TEMPERATURE_1)
   - Linear16 format for VOUT_COMMAND using VOUT_MODE context
   - OPERATION command value interpretation (OFF_IMMEDIATE, SOFT_OFF, ON, etc.)
   - Bit field decoding for configuration registers (ON_OFF_CONFIG, VOUT_MODE)

### Phase 3: Add Context-Aware Decoding
3. **Track register state for context-dependent decoding**
   - Cache VOUT_MODE value to decode subsequent VOUT_COMMAND and VOUT readings
   - Track OPERATION state to understand power sequencing operations
   - Add unit conversions (voltages in V, currents in A, temperatures in °C)

### Phase 4: Enhance Output Format
4. **Improve human-readable formatting**
   - Show engineering units: "READ_VIN=12.15V" instead of "[8a, ca]"
   - Decode status flags: "STATUS_WORD=PGOOD | ON | VOUT_UV_WARN"
   - Add operation descriptions: "OPERATION=ON (0x80)" instead of raw hex

## Testing Plan
Test the enhanced dissector by analyzing the esp-miner-boot.csv capture around 50 seconds and onward:

```bash
# Before enhancement (current output):
cargo run --bin mujina-dissect -- /home/rkuester/mujina/captures/bitaxe-gamma-logic/esp-miner-boot.csv -f I2C | grep -A20 "50\."

# Expected after enhancement:
[ 50.460072] I2C: TPS546@0x24 READ IC_DEVICE_ID="TPS546D24A"
[ 50.469645] I2C: TPS546@0x24 WRITE VIN_OFF=65.535V
[ 50.474829] I2C: TPS546@0x24 WRITE ON_OFF_CONFIG=SOFT_START_EN | OUTPUT_EN
[ 50.479077] I2C: TPS546@0x24 READ MFR_ID="TI" (0x0003)
[ 50.496383] I2C: TPS546@0x24 READ VOUT_MODE=Linear16, exp=-9
[ 50.544996] I2C: TPS546@0x24 WRITE VOUT_COMMAND=1.15V
[ 50.737312] I2C: TPS546@0x24 READ STATUS_WORD=PGOOD | OFF | CML_FAULT
[ 50.748465] I2C: TPS546@0x24 READ READ_VIN=12.42V
[ 50.757361] I2C: TPS546@0x24 READ READ_VOUT=0.012V
```

## Expected Results
Transform unreadable output like:
```
WRITE UNKNOWN=[04, ff]
READ UNKNOWN=[66, ca]
```

Into human-readable format:
```
WRITE VIN_OFF=65.535V
READ READ_TEMPERATURE_1=75.2°C
```

This leverages the existing PMBus infrastructure (Linear11/Linear16 decoders, command definitions, status bit definitions) that's already implemented but not used by the dissector.

## Success Criteria
- All TPS546 register names are human-readable (no more UNKNOWN commands)
- Voltage/current/temperature values show engineering units
- Status flags are decoded to meaningful names
- Power sequencing operations are clearly described
- Test capture at 50+ seconds shows dramatic improvement in readability