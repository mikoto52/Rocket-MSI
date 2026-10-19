#[cfg(windows)]
use emb_msi::{finish, package, project};

#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

#[cfg(windows)]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || (args.len() == 1 && (args[0] == "--help" || args[0] == "-h")) {
        println!(
            "InstallEmblyBuilderCli\n\nUsage:\n  InstallEmblyBuilderCli.exe build <project.embproj>\n  InstallEmblyBuilderCli.exe <project.embproj>\n  InstallEmblyBuilderCli.exe validate <project.embproj>\n  InstallEmblyBuilderCli.exe init <new-project.embproj>\n  InstallEmblyBuilderCli.exe normalize <project.embproj> <new.embproj>\n  InstallEmblyBuilderCli.exe verify <package.msi>\n\nBuilds MSI without installing it. Relative paths are based on the project file.\nExit code: 0 on success, 1 on error. Existing output files are not overwritten."
        );
        return Ok(());
    }
    if args.len() == 1
        && std::path::Path::new(&args[0])
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("embproj"))
    {
        args.insert(0, "build".into());
    }
    if args.len() == 2 && args[0] == "init" {
        project::Project::new()?.save_new(std::path::Path::new(&args[1]))?;
        println!(
            "Created project {}. Set Source.directory before building.",
            std::path::Path::new(&args[1]).display()
        );
        return Ok(());
    }
    if args.len() == 2 && args[0] == "validate" {
        project::Project::load(std::path::Path::new(&args[1]))?;
        println!("Project XML and settings are valid (no build performed)");
        return Ok(());
    }
    if args.len() == 3 && args[0] == "normalize" {
        project::Project::load(std::path::Path::new(&args[1]))?
            .save_new(std::path::Path::new(&args[2]))?;
        println!("Saved normalized project without overwriting existing files");
        return Ok(());
    }
    if args.len() == 2 && args[0] == "build" {
        let path = std::path::Path::new(&args[1]);
        let output = project::Project::load(path)?.build(path)?;
        println!("Created {} (not installed)", output.display());
        return Ok(());
    }
    if args.len() == 2 && args[0] == "verify" {
        return package::verify(std::path::Path::new(&args[1]));
    }
    if args.len() < 3 || args[0] != "build" {
        return Err(
            "Usage: InstallEmblyBuilderCli.exe init <project.embproj> | validate <project.embproj> | normalize <project.embproj> <new.embproj> | build <project.embproj> | build <source-file-or-folder> <output.msi> [--license <terms.txt|terms.rtf>] [--branding-text <text>] [--cab-storage embedded|external] [--cab-size-mib 1..2047] [--launch-file <relative.exe>] [--launch-arg <value>]... | verify <package.msi>".into(),
        );
    }
    let mut license_path = None;
    let mut branding = None;
    let mut storage = None;
    let mut cab_limit = None;
    let mut launch_file = None;
    let mut launch_arguments = Vec::new();
    let (options, remainder) = args[3..].as_chunks::<2>();
    for pair in options {
        match pair[0].to_str() {
            Some("--license") if license_path.is_none() => {
                license_path = Some(std::path::Path::new(&pair[1]))
            }
            Some("--branding-text") if branding.is_none() => {
                branding = Some(pair[1].to_str().ok_or("branding text must be Unicode")?)
            }
            Some("--cab-storage") if storage.is_none() => {
                storage = Some(match pair[1].to_str() {
                    Some("embedded") => package::CabStorage::Embedded,
                    Some("external") => package::CabStorage::External,
                    _ => return Err("--cab-storage must be embedded or external".into()),
                });
            }
            Some("--cab-size-mib") if cab_limit.is_none() => {
                let mib = pair[1]
                    .to_str()
                    .ok_or("CAB size must be an integer")?
                    .parse::<u64>()?;
                if !(1..=2047).contains(&mib) {
                    return Err("CAB size must be 1..2047 MiB".into());
                }
                cab_limit = Some(mib * 1024 * 1024);
            }
            Some("--launch-file") if launch_file.is_none() => {
                launch_file = Some(std::path::PathBuf::from(&pair[1]))
            }
            Some("--launch-arg") => launch_arguments.push(
                pair[1]
                    .to_str()
                    .ok_or("launch argument must be Unicode")?
                    .to_owned(),
            ),
            _ => return Err("unknown or duplicate build option".into()),
        }
    }
    if !remainder.is_empty() {
        return Err("build option requires a value".into());
    }
    if launch_file.is_none() && !launch_arguments.is_empty() {
        return Err("--launch-arg requires --launch-file".into());
    }
    let launch = launch_file.map(|file| finish::Launch {
        file,
        arguments: launch_arguments,
    });
    package::build_with_finish(
        std::path::Path::new(&args[1]),
        std::path::Path::new(&args[2]),
        license_path,
        branding.unwrap_or("InstallEmbly"),
        storage.unwrap_or(package::CabStorage::Embedded),
        cab_limit,
        launch.as_ref(),
    )?;
    println!(
        "Created {} (not installed)",
        std::path::Path::new(&args[2]).display()
    );
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Express MSI Builder requires Windows.");
    std::process::exit(1);
}
