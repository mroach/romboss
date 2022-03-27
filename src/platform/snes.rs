use anyhow::bail;
use anyhow::Result;
use binread::{io::Cursor, io::Read, io::Seek, BinRead};
use encoding::codec::japanese::EUCJPEncoding;
use encoding::{DecoderTrap, Encoding};
use log::debug;
use phf::phf_map;
use serde::Serialize;
use std::fs::File;
use std::path::PathBuf;

#[derive(Serialize, Debug)]
pub struct Rom {
    map_mode: String,
    cartridge_type: String,
    target_market: String,
    title: String,
    has_smc_header: bool,
    rom_size: StorageSize,
    sram_size: StorageSize,
}

#[derive(Serialize, Debug)]
pub struct StorageSize {
    bytes: u32,
    kilobytes: u32,
    kilobits: u32,
}

#[derive(BinRead, Debug)]
#[br(big)]
#[allow(dead_code)]
pub struct RomHeader {
    #[br(count = 2)]
    maker_code: Vec<u8>,

    #[br(count = 4)]
    game_code: Vec<u8>,

    #[br(count = 7)]
    fixed_value: Vec<u8>, // should be all 0x00

    exapnsion_ram_size: u8,
    special_version: u8,
    cartridge_type: u8,

    #[br(count = 21, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c[..]))]
    name: String,

    map_mode: u8,
    rom_type: u8,
    rom_size: u8,
    sram_size: u8,

    // 00: JP, 01: NA, 02: EU, 03: NORDIC, 04: FI, 05: DK, 06: FR, 07: NL, 08: ES
    // 09: DE, 0A: IT, 0B: CN, 0C: ID, 0D: KR, 0E: ?, 0F: CA, 10: BR, 11: AU, 12-14: ?
    destination_code: u8,

    // should always be 0x33 (51)
    fixed_value_2: u8,

    version: u8,

    complement_check: u16,

    checksum: u16,
}

// Converts a series of bytes to a string using EUC-JP encoding and stripping trailing spaces.
fn bytes_to_stripped_string(bytes: &[u8]) -> Result<String> {
    let s = EUCJPEncoding.decode(&bytes, DecoderTrap::Ignore).unwrap();
    Ok(s.trim_end().to_string())
}

impl RomHeader {
    pub fn map_mode_description(&self) -> String {
        static MAP_MODES: phf::Map<u8, &'static str> = phf_map! {
            0x20u8 => "2.68MHz LoROM",
            0x21u8 => "2.68MHz HiROM",
            0x23u8 => "SA-1",
            0x25u8 => "2.68MHz ExHiROM",
            0x30u8 => "3.58MHz LoROM",
            0x31u8 => "3.58MHz HiROM",
            0x35u8 => "3.58MHz ExHiROM",
        };

        lookup_description(self.map_mode, &MAP_MODES)
    }

    pub fn cartridge_type_description(&self) -> String {
        static CARTRIDGE_TYPES: phf::Map<u8, &'static str> = phf_map! {
            0x00u8 => "ROM only",
            0x01u8 => "ROM and RAM",
            0x02u8 => "ROM, RAM and battery",
            0x33u8 => "ROM and SA-1",
            0x34u8 => "ROM, SA-1 and RAM",
            0x35u8 => "ROM, SA-1, RAM and battery",
        };

        lookup_description(self.cartridge_type, &CARTRIDGE_TYPES)
    }

    pub fn destination_code_description(&self) -> String {
        static DESTINATION_CODES: phf::Map<u8, &'static str> = phf_map! {
            0x00u8 => "Japan",
            0x01u8 => "North America",
            0x02u8 => "Europe",
            0x03u8 => "Nordic",
            0x04u8 => "Finland",
            0x05u8 => "Denmark",
            0x06u8 => "France",
            0x07u8 => "Netherlands",
            0x08u8 => "Spain",
            0x09u8 => "Germany",
            0x0Au8 => "Italy",
            0x0Bu8 => "China",
            0x0Cu8 => "Indonesia",
            0x0Du8 => "Korea",
            0x0Fu8 => "Canada",
            0x10u8 => "Brazil",
            0x11u8 => "Australia",
        };

        lookup_description(self.destination_code, &DESTINATION_CODES)
    }

    pub fn rom_size(&self) -> StorageSize {
        kilobytes_to_storage(2u32.pow(self.rom_size as u32))
    }

    pub fn sram_size(&self) -> StorageSize {
        kilobytes_to_storage(2u32.pow(self.sram_size as u32))
    }
}

fn lookup_description(code: u8, map: &phf::Map<u8, &'static str>) -> String {
    match map.get(&code) {
        Some(desc) => desc.to_string(),
        _ => format!("Unknown {:#x}", code),
    }
}

pub fn rom_from_file(path: &PathBuf) -> Result<Rom> {
    let metadata = std::fs::metadata(&path)?;
    let mut offset = 0x00;
    let mut has_smc_header = false;

    match metadata.len() % 1024 {
        0 => debug!("No SMC header present"),
        512 => {
            debug!("SMC header present");
            offset = 0x0200;
            has_smc_header = true;
        }
        x => panic!("Invalid file? rem 1024 is {}", x),
    }

    debug!("reading rom from file {:?}", &path);

    let mut f = File::open(&path).unwrap();
    let header = find_rom_header(&mut f, metadata.len(), offset)?;

    Ok(rom_from_header(&header, has_smc_header))
}

fn rom_from_header(header: &RomHeader, has_smc_header: bool) -> Rom {
    Rom {
        map_mode: header.map_mode_description(),
        cartridge_type: header.cartridge_type_description(),
        target_market: header.destination_code_description(),
        title: header.name.to_string(),
        has_smc_header: has_smc_header,
        rom_size: header.rom_size(),
        sram_size: header.sram_size(),
    }
}

// The header stores values in kilobytes, so "8" is 8 kB, or 8192 bytes.
fn kilobytes_to_storage(kilobyte_len: u32) -> StorageSize {
    StorageSize {
        bytes: kilobyte_len * 1024,
        kilobits: kilobyte_len * 8,
        kilobytes: kilobyte_len,
    }
}

// Find a ROM header in the beginning of the file.
// To avoid reading the file multiple times, wh
pub fn find_rom_header(file: &mut File, size: u64, offset: u64) -> Result<RomHeader> {
    const HEADER_START_LOROM: u32 = 0x7FB0;
    const HEADER_START_HIROM: u32 = 0xFFB0;
    const HEADER_SIZE: u32 = 48;
    const HEADER_BUFFER_SIZE: usize =
        ((HEADER_START_HIROM - HEADER_START_LOROM) + HEADER_SIZE) as usize;

    let real_size = size - offset;

    let start_looking_at = HEADER_START_LOROM as u64;
    let mut buffer = [0; HEADER_BUFFER_SIZE];

    file.seek(std::io::SeekFrom::Start(offset + start_looking_at))?;
    file.read(&mut buffer).expect("failed to read buffer");

    let mut rom = read_header_at(&buffer, HEADER_START_LOROM as u64 - start_looking_at)?;
    if header_checks_out(&rom, real_size) {
        return Ok(rom);
    }
    debug!("Does not appear to be a LoRom: {:?}", rom);

    rom = read_header_at(&buffer, HEADER_START_HIROM as u64 - start_looking_at)?;
    if header_checks_out(&rom, real_size) {
        return Ok(rom);
    }

    debug!("Does not appear to be a HiRom: {:?}", rom);

    bail!("Could not detect a valid header. This may not be a valid Super Nintendo ROM.")
}

// Determines if the parsed header appears legitimate.
//
// The header tends to be in one of two places in the ROM file. A decent way to
// check if you've read the right spot is by checking that the "fixed value"
// gives you what you expect and that the "rom size" value matches the actual
// size of the ROM on disk.
fn header_checks_out(rom: &RomHeader, real_size: u64) -> bool {
    const FIXED_VALUE_1: [u8; 7] = [0, 0, 0, 0, 0, 0, 0];

    if rom.fixed_value != FIXED_VALUE_1 {
        debug!("fixed value was {:?}", rom.fixed_value);
        return false;
    }

    let calculated_size = 2u64.pow(rom.rom_size.into()) * 1024;

    if real_size == calculated_size {
        return true;
    }

    debug!(
        "calculated_size of {} does not match real size {}",
        calculated_size, real_size
    );

    false
}

fn read_header_at(mut buffer: &[u8], offset: u64) -> Result<RomHeader> {
    let mut cursor = Cursor::new(&mut buffer);
    cursor.seek(binread::io::SeekFrom::Start(offset))?;
    let rom = RomHeader::read(&mut cursor)?;

    Ok(rom)
}
