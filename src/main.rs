use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use helm::{inspect, pack, unpack};

fn usage() -> &'static str {
    "Usage:
  helm pack <input> <output.helm> [--force]
  helm unpack <input.helm> <output> [--force]
  helm inspect <input.helm>"
}

fn write_output(path: &Path, bytes: &[u8], force: bool) -> Result<(), Box<dyn std::error::Error>> {
    if force {
        fs::write(path, bytes)?;
        return Ok(());
    }

    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("pack") | Some("unpack") if args.len() == 3 || args.len() == 4 => {
            let force = args.get(3).is_some_and(|flag| flag == "--force");
            if args.len() == 4 && !force {
                return Err(format!("unknown option: {}\n{}", args[3], usage()).into());
            }
            let input = fs::read(&args[1])?;
            let output = match args[0].as_str() {
                "pack" => pack(&input)?,
                "unpack" => unpack(&input)?,
                _ => unreachable!(),
            };
            write_output(Path::new(&args[2]), &output, force)?;
            println!(
                "{} {} bytes -> {} bytes",
                if args[0] == "pack" { "packed" } else { "unpacked" },
                input.len(),
                output.len()
            );
            Ok(())
        }
        Some("inspect") if args.len() == 2 => {
            let container = fs::read(&args[1])?;
            let metadata = inspect(&container)?;
            println!("Format version: {}", metadata.version);
            println!("Codec: {}", metadata.codec);
            println!("Original size: {} bytes", metadata.original_size);
            println!("Payload size: {} bytes", metadata.payload_size);
            println!(
                "BLAKE3: {}",
                metadata
                    .checksum_blake3
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            );
            Ok(())
        }
        _ => Err(usage().into()),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("helm: {error}");
        std::process::exit(1);
    }
}
