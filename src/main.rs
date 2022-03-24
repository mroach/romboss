use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use env_logger;
use log::{debug, info};
use serde::Serialize;
use serde_json::json;
use std::fs::File;

mod snes;

#[derive(Parser)]
#[clap(name = "rombo")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Info {
        #[clap(required = true, parse(from_os_str))]
        path: PathBuf,

        #[clap(long = "output", short = 'o', default_value = "json", possible_values = ["json", "xml"])]
        output: String,
    },
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Cli::parse();

    match &args.command {
        Commands::Info { path, output: _ } => {
            let metadata = std::fs::metadata(&path)
                .with_context(|| format!("failed to read file metadata of `{:?}`", &path))?;

            let mut offset = 0x00;
            let mut has_smc_header = false;

            match metadata.len() % 1024 {
                0 => info!("No SMC header present"),
                512 => {
                    info!("SMC header present");
                    offset = 0x0200;
                    has_smc_header = true;
                }
                x => panic!("Invalid file? rem 1024 is {}", x),
            }

            debug!("reading file {:?}", &path);

            let mut f = File::open(&path).unwrap();
            let detected = snes::find_rom_header(&mut f, metadata.len(), offset)?;
            let rom = detected.0;
            let romtype = detected.1;
            let file_size_bytes = metadata.len();

            let as_json = json!({
                "file": {
                    "has_smc_header": has_smc_header,
                    "size": bytes_to_storage(file_size_bytes),
                },
                "rom": {
                    "name": rom.name.trim_end_matches(" "),
                    "type": romtype,
                    "map_mode": rom.map_mode_description(),
                    "cartridge_type": rom.cartridge_type_description(),
                    "destination": rom.destination_code_description(),
                    "rom_size": bytes_to_storage(2u64.pow(rom.rom_size.into()) * 1024),
                    "sram_size": bytes_to_storage(2u64.pow(rom.sram_size.into()) * 1024),
                }
            });

            println!("{}", serde_json::to_string_pretty(&as_json)?);
        }
    }

    Ok(())
}

#[derive(Serialize)]
struct StorageSize {
    bytes: u64,
    kilobits: u64,
    kilobytes: u64,
}

fn bytes_to_storage(byte_len: u64) -> StorageSize {
    const KBIT: u64 = 128;
    const KBYTE: u64 = 1024;

    StorageSize {
        bytes: byte_len,
        kilobits: byte_len / KBIT,
        kilobytes: byte_len / KBYTE,
    }
}
