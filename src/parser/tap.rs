use anyhow::{Result, anyhow};
use std::convert::TryInto;

// Constants for TAP format
const TAP_HEADER_SIZE: usize = 20;
const TAP_SIGNATURE: &[u8] = b"C64-TAPE-RAW";

// Constants for Pulse decoding (PAL C64 clock ~985248 Hz)
const THRESHOLD_SHORT_MEDIUM: u32 = 512;
const THRESHOLD_MEDIUM_LONG: u32 = 672;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Pulse {
    Short,
    Medium,
    Long,
    KeepAlive, // Very short pulses
}

struct TapReader<'a> {
    data: &'a [u8],
    pos: usize,
    version: u8,
    cycles_per_unit: u32, // Usually 8
}

impl<'a> TapReader<'a> {
    fn new(data: &'a [u8]) -> Result<Self> {
        if data.len() < TAP_HEADER_SIZE {
            return Err(anyhow!("TAP file too small"));
        }

        if &data[0..12] != TAP_SIGNATURE {
            return Err(anyhow!("Invalid TAP signature"));
        }

        let version = data[12];
        let size = u32::from_le_bytes(data[16..20].try_into()?);

        if size as usize + TAP_HEADER_SIZE > data.len() {
            return Err(anyhow!("TAP file truncated"));
        }

        Ok(Self {
            data,
            pos: TAP_HEADER_SIZE,
            version,
            cycles_per_unit: 8,
        })
    }

    fn read_pulse_cycles(&mut self) -> Option<u32> {
        if self.pos >= self.data.len() {
            return None;
        }

        let byte = self.data[self.pos];
        self.pos += 1;

        if byte == 0 {
            if self.version == 0 {
                // Version 0: 0 means overflow / very long. Usually treated as 256*8.
                Some(256 * self.cycles_per_unit)
            } else {
                // Version 1+: 0 means read next 3 bytes (24-bit size)
                if self.pos + 3 > self.data.len() {
                    return None;
                }
                let val = u32::from_le_bytes([
                    self.data[self.pos],
                    self.data[self.pos + 1],
                    self.data[self.pos + 2],
                    0,
                ]);
                self.pos += 3;
                Some(val)
            }
        } else {
            Some(byte as u32 * self.cycles_per_unit)
        }
    }

    fn read_next_pulse(&mut self) -> Option<Pulse> {
        let cycles = self.read_pulse_cycles()?;
        if cycles < 200 {
            Some(Pulse::KeepAlive)
        } else if cycles < THRESHOLD_SHORT_MEDIUM {
            Some(Pulse::Short)
        } else if cycles < THRESHOLD_MEDIUM_LONG {
            Some(Pulse::Medium)
        } else {
            Some(Pulse::Long)
        }
    }
}

struct TapBitReader<'a> {
    reader: TapReader<'a>,
    // Simple 1-step undo buffer
    last_pulse_pos: usize,
}

impl<'a> TapBitReader<'a> {
    fn new(data: &'a [u8]) -> Result<Self> {
        Ok(Self {
            reader: TapReader::new(data)?,
            last_pulse_pos: TAP_HEADER_SIZE,
        })
    }

    fn next_pulse(&mut self) -> Option<Pulse> {
        self.last_pulse_pos = self.reader.pos;
        self.reader.read_next_pulse()
    }

    fn next_valid_pulse(&mut self) -> Option<Pulse> {
        loop {
            let p = self.next_pulse()?;
            if p != Pulse::KeepAlive {
                return Some(p);
            }
        }
    }

    fn undo_pulse(&mut self) {
        self.reader.pos = self.last_pulse_pos;
    }

    // Try to sync to the next block using KERNAL logic
    fn sync(&mut self) -> bool {
        loop {
            // 1. Find a Long Pulse
            let p1 = match self.next_valid_pulse() {
                Some(p) => p,
                None => return false, // End of tape
            };

            if p1 == Pulse::Long {
                // 2. Check the pulse immediately following
                let p2 = match self.next_valid_pulse() {
                    Some(p) => p,
                    None => return false,
                };

                match p2 {
                    Pulse::Short => {
                        // Found L, S pair (Sync sequence).
                        // KERNAL loops back to find next Long.
                        continue;
                    }
                    Pulse::Medium => {
                        // Found L, M pair (Data Start Marker).
                        // Ready to read bits.
                        return true;
                    }
                    Pulse::Long => {
                        // Found L, L.
                        // The second L might be start of a valid sequence.
                        // Backtrack so next iter sees p2 as p1.
                        self.undo_pulse();
                        continue;
                    }
                    _ => {
                        // L followed by something else.
                        // Keep searching.
                        continue;
                    }
                }
            }
            // If not Long, keep searching.
        }
    }

    fn read_bit(&mut self) -> Result<u8> {
        // Standard KERNAL bit reading:
        // Pass 1: Read Pulse. If Long -> End of Block.
        // Pass 2: Read Pulse. Short -> 0, Medium -> 1.

        let p1 = match self.next_valid_pulse() {
            Some(p) => p,
            None => return Err(anyhow!("EOF reading bit")),
        };

        if p1 == Pulse::Long {
            return Err(anyhow!("Block terminated by Long pulse"));
        }

        let p2 = match self.next_valid_pulse() {
            Some(p) => p,
            None => return Err(anyhow!("EOF reading bit")),
        };

        // Pass 2 check
        match p2 {
            Pulse::Short => Ok(0),
            Pulse::Medium => Ok(1),
            Pulse::Long => Err(anyhow!("Block terminated by Long pulse in 2nd pass")),
            Pulse::KeepAlive => Err(anyhow!("KeepAlive pulse inside bit data")),
        }
    }

    fn read_byte(&mut self) -> Result<u8> {
        let mut byte = 0u8;
        // Standard C64 ROM writes LSB first
        for i in 0..8 {
            let bit = self.read_bit()?;
            byte |= bit << i;
        }

        // Read Checkbit (Parity)
        // We consume it but don't strictly enforce it
        let _ = self.read_bit();

        Ok(byte)
    }
}

pub fn parse_tap(data: &[u8]) -> Result<(u16, Vec<u8>)> {
    let mut reader = TapBitReader::new(data)?;

    // We search for a Header (Type 1 or 3), then the following Data (Type 2 or 4).

    loop {
        if !reader.sync() {
            return Err(anyhow!("End of tape or sync not found"));
        }

        // Read Block Type
        let block_type = match reader.read_byte() {
            Ok(b) => b,
            Err(_) => continue, // Sync found but read failed, retry sync
        };

        if block_type == 1 || block_type == 3 {
            // Found Header
            // Header format:
            // Byte 0: Start Address LO
            // Byte 1: Start Address HI
            // Byte 2: End Address LO
            // Byte 3: End Address HI
            // Byte 4-19: Filename

            let start_lo = match reader.read_byte() {
                Ok(b) => b,
                Err(_) => continue,
            };
            let start_hi = match reader.read_byte() {
                Ok(b) => b,
                Err(_) => continue,
            };
            let end_lo = match reader.read_byte() {
                Ok(b) => b,
                Err(_) => continue,
            };
            let end_hi = match reader.read_byte() {
                Ok(b) => b,
                Err(_) => continue,
            };

            let start_addr = (start_hi as u16) << 8 | (start_lo as u16);
            let end_addr = (end_hi as u16) << 8 | (end_lo as u16);

            // Consume filename (16 bytes)
            let mut valid_header = true;
            for _ in 0..16 {
                if reader.read_byte().is_err() {
                    valid_header = false;
                    break;
                }
            }
            if !valid_header {
                continue;
            }

            // Now we need to find the Data block (Type 2 or 4)
            // It usually follows immediately.
            // We loop until we find it or fail.

            loop {
                if !reader.sync() {
                    return Err(anyhow!("Found header but data block missing"));
                }

                let next_type = match reader.read_byte() {
                    Ok(b) => b,
                    Err(_) => continue,
                };

                if next_type == 2 || next_type == 4 {
                    // Found Data Block!
                    // Calculate expected length from Header info
                    let len = (end_addr.saturating_sub(start_addr)) as usize;

                    if len == 0 || len > 65536 {
                        // If invalid length from header, maybe try to read until sync fail?
                        // For now return error
                        return Err(anyhow!("Invalid program length from header: {}", len));
                    }

                    let mut program_data = Vec::with_capacity(len);
                    // We need to be careful: the data block might have MORE bytes than expected (checksum, etc.)
                    // But we only want the program data.
                    // We read exactly 'len' bytes.

                    for i in 0..len {
                        match reader.read_byte() {
                            Ok(b) => program_data.push(b),
                            Err(_) => {
                                return Err(anyhow!("Failed to read data byte {}/{}", i, len));
                            }
                        }
                    }

                    return Ok((start_addr, program_data));
                } else if next_type == 1 || next_type == 3 {
                    // Found another header (likely redundant copy).
                    // We consume the critical fields to advance the stream.
                    // Standard length of header is 21 bytes (1 type + 20 payload).
                    // We already read type. Read 20 more.
                    // If this fails, we just continue loop, maybe next block is data?
                    for _ in 0..20 {
                        let _ = reader.read_byte();
                    }

                    // We ignore the content of this redundant header and rely on the first one we found.
                    // This is safe because redundant headers are typically identical.
                    continue;
                }
            }
        }
        // If block type was not 1 or 3, loop again to find next sync
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to encode a bit as pulses
    fn encode_bit(bit: u8, data: &mut Vec<u8>) {
        if bit == 0 {
            // Short, Short
            data.push(0x30); // ~384 cycles / 8 = 48 (0x30)
            data.push(0x30);
        } else {
            // Medium, Medium
            // Use 0x42 (528) to be comfortably above the 512 threshold
            data.push(0x42);
            data.push(0x42);
        }
    }

    // Helper to encode a byte with parity
    fn encode_byte(byte: u8, data: &mut Vec<u8>) {
        // Encode LSB first
        for i in 0..8 {
            let bit = (byte >> i) & 1;
            encode_bit(bit, data);
        }
        // Parity bit (odd parity used in KERNAL? check bit 1 if sum is odd?
        // Actually KERNAL parity logic: checkbit = 1 if # of 1s in byte is odd?
        // Let's just output a 0 or 1. My parser ignores the value.
        encode_bit(1, data); // Checkbit
    }

    // Helper to encode sync sequence
    fn encode_sync(count: usize, data: &mut Vec<u8>) {
        for _ in 0..count {
            data.push(0x60); // Long (0x60*8 = 768 > 672)
            data.push(0x30); // Short
        }
    }

    // Helper to encode start marker (Long, Medium)
    fn encode_marker(data: &mut Vec<u8>) {
        data.push(0x60); // Long
        data.push(0x42); // Medium
    }

    #[test]
    fn test_parse_tap_synthetic() {
        let mut tap_data = Vec::new();

        // 1. Header
        tap_data.extend_from_slice(b"C64-TAPE-RAW"); // Signature
        tap_data.push(0); // Version 0
        tap_data.push(0);
        tap_data.push(0);
        tap_data.push(0); // Future
        tap_data.extend_from_slice(&(10000u32).to_le_bytes()); // Size (dummy)

        // 2. Header Block
        encode_sync(20, &mut tap_data);
        encode_marker(&mut tap_data);

        // Block Type 1 (Header)
        encode_byte(1, &mut tap_data);

        // Start Address (0x0801)
        encode_byte(0x01, &mut tap_data);
        encode_byte(0x08, &mut tap_data);

        // End Address (0x0804 = Start + 3 bytes)
        encode_byte(0x04, &mut tap_data);
        encode_byte(0x08, &mut tap_data);

        // Filename (16 bytes) - dummy
        for _ in 0..16 {
            encode_byte(0, &mut tap_data);
        }

        // 3. Data Block
        encode_sync(20, &mut tap_data);
        encode_marker(&mut tap_data);

        // Block Type 2 (Data)
        encode_byte(2, &mut tap_data);

        // Data bytes (3 bytes: 0xA9, 0x00, 0x00)
        encode_byte(0xA9, &mut tap_data);
        encode_byte(0x00, &mut tap_data);
        encode_byte(0x00, &mut tap_data);

        // Add some trailing pulses
        encode_sync(5, &mut tap_data);

        // Update size in header
        let size = (tap_data.len() - TAP_HEADER_SIZE) as u32;
        let size_bytes = size.to_le_bytes();
        tap_data[16] = size_bytes[0];
        tap_data[17] = size_bytes[1];
        tap_data[18] = size_bytes[2];
        tap_data[19] = size_bytes[3];

        // Parse
        let result = parse_tap(&tap_data);

        assert!(result.is_ok(), "Parse failed: {:?}", result.err());
        let (addr, data) = result.unwrap();

        assert_eq!(addr, 0x0801);
        assert_eq!(data, vec![0xA9, 0x00, 0x00]);
    }
}
