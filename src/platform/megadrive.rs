use anyhow::{Context, Result};
use binread::{io::Cursor, io::Read, io::Seek, BinRead};
use encoding::codec::japanese::Windows31JEncoding;
use encoding::{DecoderTrap, Encoding};
use log::debug;
use phf::phf_map;
use regex::Regex;
use serde::Serialize;
use std::fs::File;
use std::path::PathBuf;

#[derive(Serialize, Debug)]
pub enum Region {
    Japan,
    Americas,
    Europe,
}

#[derive(Serialize, Debug)]
struct SoftwareTitle {
    domestic: String,
    overseas: String,
}

#[derive(Serialize, Debug)]
struct ReleaseDate {
    month: u8,
    year: u16,
}

#[derive(Serialize, Debug)]
pub struct Rom {
    software_title: SoftwareTitle,
    software_type: String,
    supported_devices: Vec<&'static str>,
    supported_regions: Vec<Region>,
    system_type: String,
    release_date: ReleaseDate,
    serial_number: String,
    revision: String,
}

#[derive(BinRead, Debug)]
#[br(big)]
#[allow(dead_code)]
pub struct RomHeader {
    #[br(count = 16, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    pub system_type: String,

    #[br(pad_before = 3, count = 4, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    pub publisher: String,

    #[br(pad_before = 1, count = 4, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    release_year: String,

    #[br(pad_before = 1, count = 3, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    release_month: String,

    #[br(count = 48, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    pub game_title_domestic: String,

    #[br(count = 48, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    pub game_title_overseas: String,

    #[br(count = 2, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    software_type: String,

    #[br(pad_before = 1, count = 8, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    pub serial_number: String,

    #[br(pad_before = 1, count = 2, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    pub revision: String,

    checksum: u16,

    #[br(count = 16, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    supported_devices: String,

    rom_start_address: u32,
    rom_end_address: u32,
    ram_start_address: u32,
    ram_end_address: u32,

    #[br(count = 12)]
    extra_memory: Vec<u8>,

    #[br(count = 12, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    pub modem_support: String,

    // We don't want to strip this string. The spaces can be helpful when figuring out
    // if it's the old or new type of region identifier.
    #[br(pad_before = 40, count = 3, try_map = |c: Vec<u8>| String::from_utf8(c))]
    supported_regions: String,
}

// Converts a series of bytes to a string.
// Header fields are fixed-length and padded with strings, so trim those off.
// Some values have internal padding as well, like
fn bytes_to_stripped_string(bytes: &[u8]) -> Result<String> {
    let s = Windows31JEncoding
        .decode(&bytes, DecoderTrap::Ignore)
        .unwrap();
    let trimmed = s.trim_end().to_string();
    let squished = Regex::new(r"\s{2,}").unwrap().replace_all(&trimmed, " ");

    Ok(squished.to_string())
}

pub fn rom_from_file(path: &PathBuf) -> Result<Rom> {
    let mut f = File::open(&path)?;
    let mut buffer = [0; 255];
    f.seek(std::io::SeekFrom::Start(0x100))?;
    f.read(&mut buffer)?;

    debug!("Read header bytes: {:?}", buffer);
    let mut cursor = Cursor::new(&mut buffer);

    let header = RomHeader::read(&mut cursor).context("Failed to parse ROM header")?;
    debug!("Read ROM header: {:?}", header);

    Ok(rom_from_header(&header))
}

fn rom_from_header(header: &RomHeader) -> Rom {
    Rom {
        release_date: ReleaseDate {
            year: header.release_year(),
            month: header.release_month(),
        },
        software_title: SoftwareTitle {
            domestic: header.game_title_domestic.to_string(),
            overseas: header.game_title_overseas.to_string(),
        },
        revision: header.revision.to_string(),
        serial_number: header.serial_number.to_string(),
        software_type: header.software_type(),
        supported_devices: header.supported_devices(),
        supported_regions: header.supported_regions(),
        system_type: header.system_type.to_string(),
    }
}

impl RomHeader {
    pub fn supported_devices(&self) -> Vec<&'static str> {
        static DEVICES: phf::Map<char, &'static str> = phf_map! {
            'J' => "3-button controller",
            '6' => "6-button controller",
            '0' => "Master System controller",
            'A' => "Analog joystick",
            '4' => "Multitap",
            'G' => "Lightgun",
            'L' => "Activator",
            'M' => "Mouse",
            'B' => "Trackball",
            'T' => "Tablet",
            'V' => "Paddle",
            'K' => "Keyboard",
            'R' => "RS-232 (Serial)",
            'P' => "Printer",
            'C' => "CD-ROM (Sega CD)",
            'F' => "Floppy drive",
            'D' => "Download",
        };

        let mut result = Vec::new();
        let device_codes: Vec<char> = self.supported_devices.chars().collect();

        for code in device_codes {
            match DEVICES.get(&code) {
                Some(&desc) => result.push(desc),
                _ => (),
            }
        }

        result
    }

    // List of regions supported by the ROM.
    //
    // There are 3 bytes used to store the values and two different formats.
    // The "old" format is the characters 'J', 'U' and 'E' for Japan, US, Europe.
    // The "new" format is a single char as a hex digit, e.g. "F" and bitmasking
    // reveals the regions supported.
    pub fn supported_regions(&self) -> Vec<Region> {
        const HEX_CHARS: [char; 16] = [
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F',
        ];

        // unique case where it might be the old or new format, but it's probably the old.
        // if it were actually the new format, you would just be missing the Americas.
        if self.supported_regions == "E  " {
            return vec![Region::Europe];
        }

        let chars: Vec<char> = self.supported_regions.chars().collect();
        let first_char = chars[0];

        // all-in-one way to see if it's a hex code, and if so, convert to its numeric value
        match HEX_CHARS.iter().position(|&c| c == first_char) {
            Some(pos) => new_region_code(pos as u8),
            None => old_region_code(&chars),
        }
    }

    pub fn software_type(&self) -> String {
        match self.software_type.as_str() {
            "GM" => "Game".to_string(),
            "AI" => "Aid".to_string(),
            "OS" => "Boot ROM (TMSS)".to_string(),
            "BR" => "Boot ROM (Sega CD)".to_string(),
            other => format!("Unknown '{}'", other),
        }
    }

    pub fn release_year(&self) -> u16 {
        self.release_year.parse::<u16>().unwrap()
    }

    pub fn release_month(&self) -> u8 {
        const MONTHS: [&'static str; 12] = [
            "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
        ];

        match MONTHS.iter().position(|m| m == &self.release_month) {
            Some(pos) => pos as u8,
            None => 0,
        }
    }
}

// The "old" region format is 3 chars in any order: J, E, U
fn old_region_code(codes: &Vec<char>) -> Vec<Region> {
    let mut result = Vec::new();

    if codes.contains(&'J') {
        result.push(Region::Japan)
    }

    if codes.contains(&'U') {
        result.push(Region::Americas)
    }

    if codes.contains(&'E') {
        result.push(Region::Europe)
    }

    result
}

// The "new" region coding system is derived from a single byte that uses bitmasking.
// Multiple regions are represented with 1 = JP, 4 = US, 8 = EU. 2 is unused.
fn new_region_code(code: u8) -> Vec<Region> {
    let mut result = Vec::new();

    if code & 0x01 != 0 {
        result.push(Region::Japan)
    }

    if code & 0x04 != 0 {
        result.push(Region::Americas)
    }

    if code & 0x08 != 0 {
        result.push(Region::Europe)
    }

    result
}
