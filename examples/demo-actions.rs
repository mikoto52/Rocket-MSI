//! Test-only action probe. Appends a diagnostic line to the explicitly supplied log.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        std::process::exit(2);
    }
    let operation = (|| -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new(&args[0]);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(
            file,
            "{}\targs={:?}\tcwd={}\texe_exists={}",
            args[1],
            &args[3..],
            std::env::current_dir()?.display(),
            std::env::current_exe()?.is_file()
        )?;
        if let Some(ms) = args[3..].iter().find_map(|a| a.strip_prefix("--sleep-ms=")) {
            std::thread::sleep(std::time::Duration::from_millis(ms.parse()?));
        }
        Ok(())
    })();
    if operation.is_err() {
        std::process::exit(3);
    }
    std::process::exit(args[2].parse().unwrap_or(2));
}
