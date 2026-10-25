//! ROM loading and LoROM address mapping.

use crate::crc32::crc32;
use std::fs;
use std::path::Path;

const COPIER_HEADER_LEN: usize = 0x200;
const LOROM_HEADER: usize = 0x7FC0;

pub struct Rom {
    data: Vec<u8>,
    pub had_copier_header: bool,
}

pub struct Header {
    pub title: String,
    pub map_mode: u8,
    pub rom_type: u8,
    pub country: u8,
    pub version: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct Vectors {
    pub native_cop: u16,
    pub native_brk: u16,
    pub native_nmi: u16,
    pub native_irq: u16,
    pub emu_cop: u16,
    pub emu_nmi: u16,
    pub emu_reset: u16,
    pub emu_irq: u16,
}

impl Rom {
    pub fn load(path: impl AsRef<Path>) -> Result<Rom, String> {
        let path = path.as_ref();
        let data = fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Rom::from_bytes(data)
    }

    /// Accepts headered (.smc) and unheadered (.sfc) dumps.
    pub fn from_bytes(mut data: Vec<u8>) -> Result<Rom, String> {
        let had_copier_header = data.len() % 0x8000 == COPIER_HEADER_LEN;
        if had_copier_header {
            data.drain(..COPIER_HEADER_LEN);
        }
        if data.len() < 0x8000 {
            return Err(format!("ROM too small ({} bytes)", data.len()));
        }
        Ok(Rom { data, had_copier_header })
    }

    /// The image without the copier header.
    pub fn bytes(&self) -> &[u8] {
        &self.data
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn crc32(&self) -> u32 {
        crc32(&self.data)
    }

    /// File offset for a 24-bit LoROM address, or None for RAM / hardware / unmapped regions.
    pub fn lorom_offset(&self, addr: u32) -> Option<usize> {
        let bank = ((addr >> 16) & 0xFF) as usize;
        let off = (addr & 0xFFFF) as usize;
        if off < 0x8000 || bank == 0x7E || bank == 0x7F {
            return None;
        }
        let file_off = ((bank & 0x7F) << 15) | (off & 0x7FFF);
        Some(file_off % self.data.len())
    }

    pub fn read8(&self, addr: u32) -> Option<u8> {
        self.lorom_offset(addr).map(|o| self.data[o])
    }

    pub fn read16(&self, addr: u32) -> Option<u16> {
        let lo = self.read8(addr)? as u16;
        let hi = self.read8(addr.wrapping_add(1) & 0xFF_FFFF)? as u16;
        Some(lo | (hi << 8))
    }

    pub fn header(&self) -> Header {
        let h = &self.data[LOROM_HEADER..LOROM_HEADER + 0x20];
        let title = h[..21]
            .iter()
            .map(|&b| if (0x20..0x7F).contains(&b) { b as char } else { '?' })
            .collect::<String>()
            .trim_end()
            .to_string();
        Header { title, map_mode: h[0x15], rom_type: h[0x16], country: h[0x19], version: h[0x1B] }
    }

    pub fn vectors(&self) -> Vectors {
        let v = |off: usize| u16::from_le_bytes([self.data[0x7F00 + off], self.data[0x7F00 + off + 1]]);
        Vectors {
            native_cop: v(0xE4),
            native_brk: v(0xE6),
            native_nmi: v(0xEA),
            native_irq: v(0xEE),
            emu_cop: v(0xF4),
            emu_nmi: v(0xFA),
            emu_reset: v(0xFC),
            emu_irq: v(0xFE),
        }
    }
}

#[cfg(test)]
pub(crate) fn test_rom(code: &[(u32, &[u8])]) -> Rom {
    let mut data = vec![0u8; 0x10000];
    for &(addr, bytes) in code {
        let off = ((addr as usize >> 16) & 0x7F) << 15 | (addr as usize & 0x7FFF);
        data[off..off + bytes.len()].copy_from_slice(bytes);
    }
    Rom::from_bytes(data).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lorom_mapping() {
        let rom = Rom::from_bytes(vec![0; 0x10000]).unwrap();
        assert_eq!(rom.lorom_offset(0x00_8000), Some(0));
        assert_eq!(rom.lorom_offset(0x80_8000), Some(0));
        assert_eq!(rom.lorom_offset(0x01_8000), Some(0x8000));
        assert_eq!(rom.lorom_offset(0x00_2100), None);
        assert_eq!(rom.lorom_offset(0x7E_8000), None);
    }

    #[test]
    fn strips_copier_header() {
        let rom = Rom::from_bytes(vec![0; 0x8000 + 0x200]).unwrap();
        assert!(rom.had_copier_header);
        assert_eq!(rom.len(), 0x8000);
    }
}
