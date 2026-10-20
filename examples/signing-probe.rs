//! Integration-test helper: sign an existing fixture, including untrusted fixtures.
//! Production builds sign only their staging file before publishing verified output.
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn run() -> emb_msi::msi::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: signing-probe <project.embproj> <fixture.msi>".into());
    }
    let project = std::fs::canonicalize(&args[0])?;
    let config = emb_msi::project::Project::load(&project)?;
    let password = std::env::var("ROCKET_MSI_PFX_PASSWORD").ok();
    emb_msi::signing::sign(
        std::path::Path::new(&args[1]),
        config
            .signing
            .as_ref()
            .ok_or("Signing configuration missing")?,
        project.parent().unwrap(),
        password.as_deref(),
    )
}
