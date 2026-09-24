mod exif;
mod jpeg;
mod pretty;

use std::env;
use std::fs::File;
use std::io::BufReader;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let path = match args.next() {
        Some(p) => p,
        None => {
            eprintln!("usage: streaming-exif-reader <path-to-jpeg>");
            return ExitCode::FAILURE;
        }
    };

    let file = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("could not open {}: {e}", path.to_string_lossy());
            return ExitCode::FAILURE;
        }
    };

    let mut segments = match jpeg::SegmentReader::new(BufReader::new(file)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let payload = match segments.next_exif_payload() {
        Ok(Some(p)) => p,
        Ok(None) => {
            println!("no EXIF metadata found before the image data started");
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    match exif::parse(&payload) {
        Ok(data) => {
            pretty::print_report(&data);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("invalid EXIF data: {e}");
            ExitCode::FAILURE
        }
    }
}
