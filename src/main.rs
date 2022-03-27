use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use env_logger;
use serde::Serialize;
use std::path::PathBuf;

mod platform;

#[derive(Parser)]
#[clap(name = "romboss")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Info {
        #[clap(required = true, parse(from_os_str))]
        path: PathBuf,

        #[clap(long = "output", short = 'o', default_value = "json", possible_values = ["json", "yaml"])]
        output_format: String,

        #[clap(long = "platform", short = 'p', default_value = "auto", possible_values = ["auto", "snes", "sfc", "megadrive", "genesis"])]
        platform: String,
    },
}

fn main() -> Result<()> {
    env_logger::init();

    let args = Cli::parse();

    match &args.command {
        Commands::Info {
            path,
            output_format,
            platform: platform_label,
        } => {
            let platform = match platform_label.as_str() {
                "auto" => detect_rom_platform(&path).context(concat!(
                    "Could not automatically determine the platform.",
                    "Use the '-p' flag to specify a platform explicitly"
                ))?,
                other => parse_platform_label(other)
                    .with_context(|| format!("Unrecognised platform label '{}'", other))?,
            };

            let rom = rom_from_file(&path, platform)?;

            // TODO: This is obviously redundant and should be solvable with generics or however
            // Rust might let you say "here's something that implements this trait" (Serialize).
            match rom {
                Rom::SuperNintendo(r) => print_serializable_rom(&r, output_format)?,
                Rom::MegaDrive(r) => print_serializable_rom(&r, output_format)?,
            };
            Ok(())
        }
    }
}

fn print_serializable_rom<T>(rom: &T, format: &str) -> Result<()>
where
    T: Serialize,
{
    match format {
        "json" => println!("{}", serde_json::to_string_pretty(&rom)?),
        "yaml" => println!("{}", serde_yaml::to_string(&rom)?),
        fmt => bail!("Unsupported format {}", fmt),
    }

    Ok(())
}

#[derive(Serialize, Debug)]
enum Rom {
    SuperNintendo(platform::snes::Rom),
    MegaDrive(platform::megadrive::Rom),
}

fn detect_rom_platform(path: &PathBuf) -> Option<Platform> {
    // For now, only detect from the path.
    // A future enhancement may be detecting based on file contents, like mime magic.
    platform_from_path(path)
}

#[derive(Debug)]
enum Platform {
    MegaDrive,
    SuperNintendo,
}

fn parse_platform_label(label: &str) -> Option<Platform> {
    match label {
        "snes" | "sfc" => return Some(Platform::SuperNintendo),
        "megadrive" | "genesis" => return Some(Platform::MegaDrive),
        _ => None,
    }
}

fn platform_from_path(path: &PathBuf) -> Option<Platform> {
    let ext = path.extension().unwrap().to_ascii_lowercase();
    let ext = ext.to_str().unwrap();

    match ext {
        "smc" | "sfc" | "swc" => return Some(Platform::SuperNintendo),
        "gen" | "md" | "smd" => return Some(Platform::MegaDrive),
        _ => None,
    }
}

fn rom_from_file(path: &PathBuf, platform: Platform) -> Result<Rom> {
    match platform {
        Platform::SuperNintendo => {
            let rom = platform::snes::rom_from_file(path)?;
            Ok(Rom::SuperNintendo(rom))
        }
        Platform::MegaDrive => {
            let rom = platform::megadrive::rom_from_file(path)?;
            Ok(Rom::MegaDrive(rom))
        }
        val => bail!("Unsupported platform {:?}", val),
    }
}
