use anyhow::{Context, Result};
use binread::{io::Cursor, io::Read, BinRead};
use log::debug;
use serde::Serialize;
use std::fs::File;
use std::path::PathBuf;

#[derive(BinRead, Debug)]
#[br(big)]
#[allow(dead_code)]
pub struct RomHeader {
    #[br(count = 12, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    game_title: String,

    #[br(count = 4, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    game_code: String,

    #[br(count = 2, try_map = |c: Vec<u8>| bytes_to_stripped_string(&c))]
    maker_code: String,

    unit_code: u8,

    device_type: u8,

    // 2^(20 + VAL)
    card_size: u8,

    #[br(pad_before = 8)]
    flags: u8,
}

#[derive(Serialize, Debug)]
pub enum Device {
    DS,
    DSi,
}

#[derive(Serialize, Debug)]
pub struct Rom {
    pub software_title: String,
    pub game_code: String,
    pub maker_code: String,
    pub supported_devices: Vec<Device>,
}

fn bytes_to_stripped_string(bytes: &Vec<u8>) -> Result<String> {
    let s = String::from_utf8(bytes.to_vec())?;

    Ok(s.trim_end().trim_matches(char::from(0x00)).to_string())
}

impl RomHeader {
    fn supported_devices(&self) -> Vec<Device> {
        if self.unit_code == 3 {
            return vec![Device::DSi];
        }

        if self.unit_code == 2 {
            return vec![Device::DS, Device::DSi];
        }

        vec![Device::DS]
    }
}

pub fn rom_from_file(path: &PathBuf) -> Result<Rom> {
    let mut f = File::open(&path)?;
    let mut buffer = [0; 512];
    f.read(&mut buffer)?;

    debug!("Read header bytes: {:?}", buffer);
    let mut cursor = Cursor::new(&mut buffer);

    let header = RomHeader::read(&mut cursor).context("Failed to parse ROM header")?;
    debug!("Read ROM header: {:?}", header);

    Ok(Rom {
        software_title: header.game_title.to_string(),
        game_code: header.game_code.to_string(),
        maker_code: header.maker_code.to_string(),
        supported_devices: header.supported_devices(),
    })
}
