use crate::msi::{
    self, Database, Result,
    Value::{Int as I, Null as N, Text as T},
};
#[cfg(test)]
use std::process::Command;
use std::{
    fs,
    path::{Path, PathBuf},
};

const BUTTON_BACK_X: i32 = 174;
const BUTTON_NEXT_X: i32 = 240;
const BUTTON_CANCEL_X: i32 = 314;
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CabStorage {
    Embedded,
    External,
}

struct Work(PathBuf);
impl Drop for Work {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A deliberately fixed product for the first vertical slice, not a general release builder.
#[cfg(test)]
pub fn build(
    source: &Path,
    output: &Path,
    license: Option<&Path>,
    branding: &str,
    storage: CabStorage,
) -> Result<()> {
    build_with_limit(source, output, license, branding, storage, None)
}

#[cfg(test)]
pub fn build_with_limit(
    source: &Path,
    output: &Path,
    license: Option<&Path>,
    branding: &str,
    storage: CabStorage,
    cab_limit: Option<u64>,
) -> Result<()> {
    build_with_finish(source, output, license, branding, storage, cab_limit, None)
}

pub fn build_with_finish(
    source: &Path,
    output: &Path,
    license: Option<&Path>,
    branding: &str,
    storage: CabStorage,
    cab_limit: Option<u64>,
    launch: Option<&crate::finish::Launch>,
) -> Result<()> {
    build_configured(&BuildSpec {
        source,
        output,
        license,
        branding,
        storage,
        cab_limit,
        launch,
        product: None,
        quick_msi: false,
        terms_url: None,
    })
}
pub struct BuildSpec<'a> {
    pub source: &'a Path,
    pub output: &'a Path,
    pub license: Option<&'a Path>,
    pub branding: &'a str,
    pub storage: CabStorage,
    pub cab_limit: Option<u64>,
    pub launch: Option<&'a crate::finish::Launch>,
    pub product: Option<&'a crate::config::Product>,
    pub quick_msi: bool,
    pub terms_url: Option<&'a str>,
}
pub fn build_configured(spec: &BuildSpec<'_>) -> Result<()> {
    build_selected(spec, None)
}
pub fn build_selected(
    spec: &BuildSpec<'_>,
    selected: Option<Vec<crate::payload::File>>,
) -> Result<()> {
    build_selected_signed(spec, selected, None)
}
pub fn build_selected_signed(
    spec: &BuildSpec<'_>,
    selected: Option<Vec<crate::payload::File>>,
    signing: Option<(&crate::signing::Signing, &Path, Option<&str>)>,
) -> Result<()> {
    let BuildSpec {
        source,
        output,
        license,
        branding,
        storage,
        cab_limit,
        launch,
        ..
    } = *spec;
    let fallback = crate::config::Product::default();
    if spec.quick_msi && spec.license.is_some() {
        return Err("Quick MSI first unit does not yet support License; use UI mode=wizard".into());
    }
    let product = spec.product.unwrap_or(&fallback);
    if spec
        .terms_url
        .is_some_and(|url| !crate::terms::valid_url(url))
    {
        return Err("Quick MSI terms must be an HTTP(S) URL".into());
    }
    product.validate()?;
    if cab_limit == Some(0) {
        return Err("CAB size must be positive".into());
    }
    if branding.contains('\0') || branding.contains(['\r', '\n']) {
        return Err("branding text must be a single line without NUL".into());
    }
    let license = license.map(crate::license::load).transpose()?;
    if selected.is_none() && !source.is_file() && !source.is_dir() {
        return Err("source must be an existing file or directory".into());
    }
    if output.exists() {
        return Err("output already exists; choose a new path".into());
    }
    if output
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("msi"))
    {
        return Err("output must have .msi extension".into());
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let parent = fs::canonicalize(parent)?;
    if storage == CabStorage::External && parent.join("Data1.cab").exists() {
        return Err(
            "Data1.cab already exists; choose a separate output folder for each package".into(),
        );
    }
    let mut files = match selected {
        Some(files) => files,
        None => crate::payload::collect(source, &parent)?,
    };
    if files.is_empty() {
        return Err("select at least one payload file".into());
    }
    if spec.product.is_some() {
        crate::payload::namespace(&mut files, product)?;
    }
    crate::shortcut::validate_payload(&product.shortcuts, &files, product)?;
    crate::actions::validate_payload(product, &files)?;
    if let Some(launch) = launch {
        launch.resolve(&files)?;
        launch.target()?;
    }
    let package_code = msi::guid()?;
    let work = Work(parent.join(format!(".emb-{}", package_code.trim_matches(['{', '}']))));
    fs::create_dir(&work.0)?;
    // ASCII MSI identifiers isolate DDF syntax from user file names.
    let mut sizes = Vec::new();
    for file in &files {
        i32::try_from(file.source.metadata()?.len()).map_err(|_| {
            format!(
                "file exceeds supported 2 GiB boundary: {}",
                file.source.display()
            )
        })?;
        let copied = fs::copy(&file.source, work.0.join(&file.id))?;
        crate::cabinet::normalize_staged_timestamp(&work.0.join(&file.id))?;
        sizes.push(i32::try_from(copied).map_err(|_| {
            format!(
                "file exceeds supported 2 GiB boundary: {}",
                file.source.display()
            )
        })?);
    }
    let cabinets = crate::cabinet::create(
        &work.0,
        &files,
        &sizes,
        cab_limit,
        storage == CabStorage::Embedded,
    )?;
    let staged = work.0.join("prototype.msi");
    let db = Database::create(&staged)?;
    schema(&db)?;
    if spec.quick_msi {
        if let Some(url) = spec.terms_url {
            db.insert(
                "Property",
                &["Property", "Value"],
                &[T("EMB_QUICK_TERMS_URL"), T(url)],
            )?;
        }
        let bytes = if product.architecture == "arm64" {
            include_bytes!(concat!(env!("OUT_DIR"), "/emb-quick-arm64.dll")).as_slice()
        } else if product.architecture == "x64" {
            include_bytes!(concat!(env!("OUT_DIR"), "/emb-quick-x64.dll")).as_slice()
        } else {
            include_bytes!(concat!(env!("OUT_DIR"), "/emb-quick-x86.dll")).as_slice()
        };
        let dll = work.0.join("embquick.dll");
        fs::write(&dll, bytes)?;
        db.embedded_ui(&dll)?;
    }
    for (key, value) in [
        ("ProductCode", product.product_code.as_str()),
        ("UpgradeCode", product.upgrade_code.as_str()),
        ("ProductName", product.name.as_str()),
        ("ProductVersion", product.version.as_str()),
        ("Manufacturer", product.manufacturer.as_str()),
        ("ProductLanguage", "1042"),
        ("ALLUSERS", "1"),
        ("INSTALLLEVEL", "1"),
        ("DefaultUIFont", "Normal"),
        ("ARPNOMODIFY", "1"),
        (
            "SecureCustomProperties",
            if product.upgrade {
                "INSTALLDIR;ACCEPTLICENSE;EMB_OLD_PRODUCTS;EMB_NEWER_PRODUCTS;EMB_SAME_PRODUCTS"
            } else {
                "INSTALLDIR;ACCEPTLICENSE"
            },
        ),
        ("BrandingText", branding),
    ] {
        db.insert("Property", &["Property", "Value"], &[T(key), T(value)])?;
    }
    if spec.product.is_some() {
        db.insert(
            "Property",
            &["Property", "Value"],
            &[T("REINSTALLMODE"), T(product.existing_files.mode())],
        )?;
    }
    let program_files = if product.architecture != "x86" {
        "ProgramFiles64Folder"
    } else {
        "ProgramFilesFolder"
    };
    for (id, parent, name) in [
        ("TARGETDIR", None, "SourceDir"),
        (program_files, Some("TARGETDIR"), "."),
        (
            if product.architecture != "x86" {
                "System64Folder"
            } else {
                "SystemFolder"
            },
            Some("TARGETDIR"),
            ".",
        ),
        ("TempFolder", Some("TARGETDIR"), "."),
        ("CommonAppDataFolder", Some("TARGETDIR"), "."),
        ("WindowsFolder", Some("TARGETDIR"), "."),
        ("EMB_SYSTEMDRIVE", Some("TARGETDIR"), "."),
    ] {
        if matches!(
            id,
            "System64Folder"
                | "SystemFolder"
                | "TempFolder"
                | "EMB_SYSTEMDRIVE"
                | "CommonAppDataFolder"
                | "WindowsFolder"
        ) && !files.iter().any(|f| {
            f.relative
                .components()
                .next()
                .and_then(|c| c.as_os_str().to_str())
                .and_then(|s| crate::payload::root_directory(s, product.architecture != "x86"))
                == Some(id)
        }) && !product.shortcuts.iter().any(|s| {
            s.working_directory
                .to_str()
                .and_then(|v| crate::payload::root_directory(v, product.architecture != "x86"))
                == Some(id)
        }) {
            continue;
        }
        db.insert(
            "Directory",
            &["Directory", "Directory_Parent", "DefaultDir"],
            &[T(id), parent.map(T).unwrap_or(N), T(name)],
        )?;
    }
    let parts = product.install_parts()?;
    if files
        .iter()
        .any(|f| f.relative.starts_with("{SystemDrive}"))
        || product
            .shortcuts
            .iter()
            .any(|s| s.working_directory == Path::new("{SystemDrive}"))
    {
        // WindowsVolume is a property, not a valid standard Directory identifier.
        db.insert(
            "CustomAction",
            &["Action", "Type", "Source", "Target"],
            &[
                T("EmbSetSystemDrive"),
                I(51),
                T("EMB_SYSTEMDRIVE"),
                T("[WindowsVolume]"),
            ],
        )?;
        for table in ["InstallUISequence", "InstallExecuteSequence"] {
            db.insert(
                table,
                &["Action", "Sequence"],
                &[T("EmbSetSystemDrive"), I(750)],
            )?;
        }
    }
    let mut ancestor = program_files.to_owned();
    for (index, part) in parts.iter().enumerate() {
        let id = if index + 1 == parts.len() {
            "INSTALLDIR".to_owned()
        } else {
            format!("INSTALL_PARENT_{index}")
        };
        let name = crate::payload::msi_name(Path::new(part))?;
        db.insert(
            "Directory",
            &["Directory", "Directory_Parent", "DefaultDir"],
            &[T(&id), T(&ancestor), T(&name)],
        )?;
        ancestor = id;
    }
    db.insert(
        "Feature",
        &[
            "Feature",
            "Title",
            "Description",
            "Display",
            "Level",
            "Directory_",
            "Attributes",
        ],
        &[
            T("Main"),
            T("기본 파일"),
            T("설치 테스트 파일"),
            I(1),
            I(1),
            T("INSTALLDIR"),
            I(0),
        ],
    )?;
    author_payload(&db, &files, &sizes, product.architecture != "x86")?;
    crate::registry::author(&db, product)?;
    crate::shortcut::author(&db, product, &files)?;
    for (index, cab) in cabinets.iter().enumerate() {
        let reference = if storage == CabStorage::Embedded {
            format!("#{}", cab.name)
        } else {
            cab.name.clone()
        };
        db.insert(
            "Media",
            &["DiskId", "LastSequence", "Cabinet"],
            &[I(index as i32 + 1), I(cab.files.end as i32), T(&reference)],
        )?;
        if storage == CabStorage::Embedded {
            db.stream(&cab.name, &work.0.join(&cab.name))?;
        }
    }
    for (name, bytes) in [
        (
            "WelcomeArt",
            include_bytes!("../assets/installer/welcome.bmp").as_slice(),
        ),
        (
            "BannerArt",
            include_bytes!("../assets/installer/banner.bmp").as_slice(),
        ),
        (
            "RepairArt",
            include_bytes!("../assets/installer/repair.bmp").as_slice(),
        ),
        (
            "RemoveArt",
            include_bytes!("../assets/installer/remove.bmp").as_slice(),
        ),
    ] {
        let path = work.0.join(format!("{name}.bmp"));
        fs::write(&path, bytes)?;
        db.binary(name, &path)?;
    }
    sequences(&db)?;
    ui(
        &db,
        license.as_deref(),
        if spec.product.is_some() {
            product.existing_files.mode()
        } else {
            "amus"
        },
    )?;
    let footer_width = msi::branding_width(branding)?;
    db.execute(&format!(
        "UPDATE `Control` SET `Width` = {footer_width} WHERE `Control` = 'Branding'"
    ))?;
    if !product.allow_directory_change {
        db.execute("UPDATE `Control` SET `Attributes` = 1 WHERE `Dialog_` = 'Path' AND `Control` = 'Folder'")?;
        db.execute("UPDATE `Control` SET `Attributes` = 1 WHERE `Dialog_` = 'Path' AND `Control` = 'Browse'")?;
        db.execute("UPDATE `Control` SET `Text` = '이 폴더에 [ProductName]을(를) 설치하려면, \"다음\"을 눌러주세요.' WHERE `Dialog_` = 'Path' AND `Control` = 'PathLabel'")?;
    }
    if let Some(launch) = launch {
        author_finish_launch(&db, &files, launch)?;
        if spec.quick_msi {
            let file = launch.resolve(&files)?;
            for (name, value) in [
                ("EMB_QUICK_LAUNCH_COMPONENT", file.guid.clone()),
                ("EMB_QUICK_LAUNCH_COUNT", launch.arguments.len().to_string()),
            ] {
                db.insert("Property", &["Property", "Value"], &[T(name), T(&value)])?;
            }
            for (i, arg) in launch.arguments.iter().enumerate() {
                if !arg.is_empty() {
                    db.insert(
                        "Property",
                        &["Property", "Value"],
                        &[T(&format!("EMB_QUICK_LAUNCH_ARG_{i}")), T(arg)],
                    )?;
                }
            }
        }
    }
    crate::actions::author(&db, product, &files, &work.0)?;
    crate::upgrade::author(&db, product)?;
    order_controls(&db)?;
    if license.is_some() {
        db.insert("LaunchCondition", &["Condition","Description"],
            &[T("Installed OR ACCEPTLICENSE = \"1\""), T("이용약관에 동의해야 설치할 수 있습니다. 무인 설치는 ACCEPTLICENSE=1을 명시하세요.")])?;
        db.insert(
            "InstallExecuteSequence",
            &["Action", "Sequence"],
            &[T("LaunchConditions"), I(100)],
        )?;
    }
    db.summary(
        &package_code,
        &product.architecture,
        if product.architecture == "arm64" {
            500
        } else if spec.quick_msi {
            450
        } else {
            200
        },
    )?;
    db.commit()?;
    drop(db);
    verify(&staged)?;
    if let Some((config, base, password)) = signing {
        crate::signing::sign(&staged, config, base, password)?;
    }
    let target = parent.join(output.file_name().ok_or("missing output filename")?);
    let cabinet_names: Vec<_> = cabinets.iter().map(|c| c.name.clone()).collect();
    publish_set(&staged, &target, storage, &cabinet_names)?;
    if storage == CabStorage::External {
        for cab in &cabinets {
            println!(
                "External CAB: {} ({} bytes); distribute with MSI",
                parent.join(&cab.name).display(),
                parent.join(&cab.name).metadata()?.len()
            );
        }
    }
    println!(
        "ProductCode: {}\nUpgradeCode: {}\nPackageCode: {package_code}",
        product.product_code, product.upgrade_code
    );
    Ok(())
}

fn author_payload(
    db: &Database,
    files: &[crate::payload::File],
    sizes: &[i32],
    x64: bool,
) -> Result<()> {
    use std::collections::{BTreeMap, BTreeSet};
    let mut dirs: BTreeMap<PathBuf, String> = BTreeMap::new();
    let mut aliases = BTreeSet::new();
    for file in files {
        let mut parent = PathBuf::new();
        let parts: Vec<_> = file.relative.components().collect();
        for part in &parts[..parts.len() - 1] {
            let ancestor = parent.clone();
            parent.push(part.as_os_str());
            if ancestor.as_os_str().is_empty()
                && let Some(root) = crate::payload::root_directory(
                    part.as_os_str().to_str().ok_or("invalid target root")?,
                    x64,
                )
            {
                dirs.insert(parent.clone(), root.to_owned());
                continue;
            }
            if !dirs.contains_key(&parent) {
                let id = crate::payload::identifier("D_", &parent)?;
                let parent_id = if ancestor.as_os_str().is_empty() {
                    "INSTALLDIR"
                } else {
                    dirs.get(&ancestor).ok_or("missing ancestor")?.as_str()
                };
                let name = crate::payload::msi_name(&parent)?;
                let alias = name.split('|').next().unwrap().to_owned();
                if !aliases.insert((crate::payload::key(&ancestor)?, alias)) {
                    return Err("short directory alias collision".into());
                }
                db.insert(
                    "Directory",
                    &["Directory", "Directory_Parent", "DefaultDir"],
                    &[T(&id), T(parent_id), T(&name)],
                )?;
                dirs.insert(parent.clone(), id);
            }
        }
    }
    let mut guids = BTreeSet::new();
    for (index, file) in files.iter().enumerate() {
        if !guids.insert(&file.guid) {
            return Err("component GUID collision".into());
        }
        let parent = file.relative.parent().ok_or("missing relative parent")?;
        let dir = if parent.as_os_str().is_empty() {
            "INSTALLDIR"
        } else {
            dirs.get(parent)
                .ok_or("missing install directory")?
                .as_str()
        };
        let name = if file.id == "Payload" {
            "hello.txt".to_owned()
        } else {
            crate::payload::msi_name(&file.relative)?
        };
        let alias = name.split('|').next().unwrap().to_uppercase();
        if !aliases.insert((crate::payload::key(parent)?, alias)) {
            return Err("short file alias collision".into());
        }
        db.insert(
            "Component",
            &[
                "Component",
                "ComponentId",
                "Directory_",
                "Attributes",
                "KeyPath",
            ],
            &[
                T(&file.component),
                T(&file.guid),
                T(dir),
                I(if x64 { 256 } else { 0 }),
                T(&file.id),
            ],
        )?;
        let version = msi::file_version(&file.source)?;
        db.insert(
            "File",
            &[
                "File",
                "Component_",
                "FileName",
                "FileSize",
                "Version",
                "Language",
                "Attributes",
                "Sequence",
            ],
            &[
                T(&file.id),
                T(&file.component),
                T(&name),
                I(sizes[index]),
                version.as_ref().map(|(v, _)| T(v)).unwrap_or(N),
                version
                    .as_ref()
                    .filter(|(_, l)| !l.is_empty())
                    .map(|(_, l)| T(l))
                    .unwrap_or(N),
                I(512),
                I(index as i32 + 1),
            ],
        )?;
        db.insert(
            "FeatureComponents",
            &["Feature_", "Component_"],
            &[T("Main"), T(&file.component)],
        )?;
    }
    Ok(())
}

#[cfg(test)]
fn publish(staged: &Path, target: &Path, storage: CabStorage) -> Result<()> {
    publish_set(staged, target, storage, &["Data1.cab".into()])
}
fn publish_set(staged: &Path, target: &Path, storage: CabStorage, names: &[String]) -> Result<()> {
    let parent = target.parent().ok_or("missing output parent")?;
    let mut created = Vec::new();
    let operation = (|| -> std::io::Result<()> {
        if storage == CabStorage::External {
            for name in names {
                let path = parent.join(name);
                fs::hard_link(staged.parent().unwrap().join(name), &path)?;
                created.push(path);
            }
        }
        fs::hard_link(staged, target)
    })();
    if let Err(error) = operation {
        let mut failures = Vec::new();
        for path in created.iter().rev() {
            if let Err(e) = fs::remove_file(path) {
                failures.push(format!("{}: {e}", path.display()));
            }
        }
        return Err(format!("publish failed: {error}; cleanup failures: {failures:?}").into());
    }
    Ok(())
}

fn schema(db: &Database) -> Result<()> {
    for sql in [
        "CREATE TABLE `Shortcut` (`Shortcut` CHAR(72) NOT NULL, `Directory_` CHAR(72) NOT NULL, `Name` CHAR(255) NOT NULL, `Component_` CHAR(72) NOT NULL, `Target` CHAR(255), `Arguments` CHAR(255), `Description` CHAR(255), `Hotkey` SHORT, `Icon_` CHAR(72), `IconIndex` SHORT, `ShowCmd` SHORT, `WkDir` CHAR(72) PRIMARY KEY `Shortcut`)",
        "CREATE TABLE `RemoveFile` (`FileKey` CHAR(72) NOT NULL, `Component_` CHAR(72) NOT NULL, `FileName` CHAR(255), `DirProperty` CHAR(72) NOT NULL, `InstallMode` SHORT NOT NULL PRIMARY KEY `FileKey`)",
        "CREATE TABLE `Registry` (`Registry` CHAR(72) NOT NULL, `Root` SHORT NOT NULL, `Key` CHAR(255) NOT NULL, `Name` CHAR(255), `Value` CHAR(0), `Component_` CHAR(72) NOT NULL PRIMARY KEY `Registry`)",
        "CREATE TABLE `CustomAction` (`Action` CHAR(72) NOT NULL, `Type` SHORT NOT NULL, `Source` CHAR(72), `Target` CHAR(0) PRIMARY KEY `Action`)",
        "CREATE TABLE `RadioButton` (`Property` CHAR(72) NOT NULL, `Order` SHORT NOT NULL, `Value` CHAR(64) NOT NULL, `X` SHORT NOT NULL, `Y` SHORT NOT NULL, `Width` SHORT NOT NULL, `Height` SHORT NOT NULL, `Text` CHAR(64), `Help` CHAR(50) PRIMARY KEY `Property`, `Order`)",
        "CREATE TABLE `CheckBox` (`Property` CHAR(72) NOT NULL, `Value` CHAR(64) PRIMARY KEY `Property`)",
        "CREATE TABLE `ControlCondition` (`Dialog_` CHAR(72) NOT NULL, `Control_` CHAR(50) NOT NULL, `Action` CHAR(50) NOT NULL, `Condition` CHAR(255) NOT NULL PRIMARY KEY `Dialog_`, `Control_`, `Action`, `Condition`)",
        "CREATE TABLE `LaunchCondition` (`Condition` CHAR(255) NOT NULL, `Description` CHAR(255) NOT NULL PRIMARY KEY `Condition`)",
        "CREATE TABLE `Binary` (`Name` CHAR(72) NOT NULL, `Data` OBJECT NOT NULL PRIMARY KEY `Name`)",
        "CREATE TABLE `Property` (`Property` CHAR(72) NOT NULL, `Value` CHAR(0) NOT NULL PRIMARY KEY `Property`)",
        "CREATE TABLE `Directory` (`Directory` CHAR(72) NOT NULL, `Directory_Parent` CHAR(72), `DefaultDir` CHAR(255) NOT NULL PRIMARY KEY `Directory`)",
        "CREATE TABLE `Component` (`Component` CHAR(72) NOT NULL, `ComponentId` CHAR(38), `Directory_` CHAR(72) NOT NULL, `Attributes` SHORT NOT NULL, `Condition` CHAR(255), `KeyPath` CHAR(72) PRIMARY KEY `Component`)",
        "CREATE TABLE `File` (`File` CHAR(72) NOT NULL, `Component_` CHAR(72) NOT NULL, `FileName` CHAR(255) NOT NULL, `FileSize` LONG NOT NULL, `Version` CHAR(72), `Language` CHAR(20), `Attributes` SHORT, `Sequence` SHORT NOT NULL PRIMARY KEY `File`)",
        "CREATE TABLE `Feature` (`Feature` CHAR(38) NOT NULL, `Feature_Parent` CHAR(38), `Title` CHAR(64), `Description` CHAR(255), `Display` SHORT, `Level` SHORT NOT NULL, `Directory_` CHAR(72), `Attributes` SHORT NOT NULL PRIMARY KEY `Feature`)",
        "CREATE TABLE `FeatureComponents` (`Feature_` CHAR(38) NOT NULL, `Component_` CHAR(72) NOT NULL PRIMARY KEY `Feature_`, `Component_`)",
        "CREATE TABLE `Media` (`DiskId` SHORT NOT NULL, `LastSequence` LONG NOT NULL, `DiskPrompt` CHAR(64), `Cabinet` CHAR(255), `VolumeLabel` CHAR(32), `Source` CHAR(72) PRIMARY KEY `DiskId`)",
        "CREATE TABLE `InstallExecuteSequence` (`Action` CHAR(72) NOT NULL, `Condition` CHAR(255), `Sequence` SHORT PRIMARY KEY `Action`)",
        "CREATE TABLE `InstallUISequence` (`Action` CHAR(72) NOT NULL, `Condition` CHAR(255), `Sequence` SHORT PRIMARY KEY `Action`)",
        "CREATE TABLE `TextStyle` (`TextStyle` CHAR(72) NOT NULL, `FaceName` CHAR(32) NOT NULL, `Size` SHORT NOT NULL, `Color` LONG, `StyleBits` SHORT PRIMARY KEY `TextStyle`)",
        "CREATE TABLE `Dialog` (`Dialog` CHAR(72) NOT NULL, `HCentering` SHORT NOT NULL, `VCentering` SHORT NOT NULL, `Width` SHORT NOT NULL, `Height` SHORT NOT NULL, `Attributes` LONG, `Title` CHAR(128), `Control_First` CHAR(50) NOT NULL, `Control_Default` CHAR(50), `Control_Cancel` CHAR(50) PRIMARY KEY `Dialog`)",
        "CREATE TABLE `Control` (`Dialog_` CHAR(72) NOT NULL, `Control` CHAR(50) NOT NULL, `Type` CHAR(20) NOT NULL, `X` SHORT NOT NULL, `Y` SHORT NOT NULL, `Width` SHORT NOT NULL, `Height` SHORT NOT NULL, `Attributes` LONG, `Property` CHAR(72), `Text` CHAR(0), `Control_Next` CHAR(50), `Help` CHAR(50) PRIMARY KEY `Dialog_`, `Control`)",
        "CREATE TABLE `ControlEvent` (`Dialog_` CHAR(72) NOT NULL, `Control_` CHAR(50) NOT NULL, `Event` CHAR(50) NOT NULL, `Argument` CHAR(255) NOT NULL, `Condition` CHAR(255), `Ordering` SHORT PRIMARY KEY `Dialog_`, `Control_`, `Event`, `Argument`, `Condition`)",
        "CREATE TABLE `EventMapping` (`Dialog_` CHAR(72) NOT NULL, `Control_` CHAR(50) NOT NULL, `Event` CHAR(50) NOT NULL, `Attribute` CHAR(50) NOT NULL PRIMARY KEY `Dialog_`, `Control_`, `Event`)",
    ] {
        db.execute(sql)?;
    }
    Ok(())
}
fn sequences(db: &Database) -> Result<()> {
    for (action, seq) in [
        ("ValidateProductID", 700),
        ("CostInitialize", 800),
        ("FileCost", 900),
        ("CostFinalize", 1000),
        ("InstallValidate", 1400),
        ("InstallInitialize", 1500),
        ("ProcessComponents", 1600),
        ("UnpublishFeatures", 1800),
        ("RemoveRegistryValues", 2600),
        ("RemoveShortcuts", 3200),
        ("RemoveFiles", 3500),
        ("InstallFiles", 4000),
        ("CreateShortcuts", 4500),
        ("WriteRegistryValues", 5000),
        ("RegisterUser", 6000),
        ("RegisterProduct", 6100),
        ("PublishFeatures", 6300),
        ("PublishProduct", 6400),
        ("InstallFinalize", 6600),
    ] {
        db.insert(
            "InstallExecuteSequence",
            &["Action", "Sequence"],
            &[T(action), I(seq)],
        )?;
    }
    for (action, condition, seq) in [
        ("CostInitialize", "1", 800),
        ("FileCost", "1", 900),
        ("CostFinalize", "1", 1000),
        ("Welcome", "NOT Installed", 1200),
        ("Maintenance", "Installed", 1210),
        ("Progress", "1", 1250),
        ("ExecuteAction", "1", 1300),
        ("Complete", "1", -1),
        ("Cancelled", "1", -2),
        ("Failed", "1", -3),
    ] {
        db.insert(
            "InstallUISequence",
            &["Action", "Condition", "Sequence"],
            &[T(action), T(condition), I(seq)],
        )?;
    }
    Ok(())
}

fn dialog(
    db: &Database,
    name: &str,
    first: &str,
    default: &str,
    cancel: &str,
    modal: bool,
) -> Result<()> {
    db.insert(
        "Dialog",
        &[
            "Dialog",
            "HCentering",
            "VCentering",
            "Width",
            "Height",
            "Attributes",
            "Title",
            "Control_First",
            "Control_Default",
            "Control_Cancel",
        ],
        &[
            T(name),
            I(50),
            I(50),
            I(390),
            I(280),
            I(if modal { 3 } else { 1 }),
            T("[ProductName] 설치"),
            T(first),
            T(default),
            T(cancel),
        ],
    )?;
    let welcome = matches!(name, "Welcome" | "Cancelled" | "Complete");
    control(
        db,
        name,
        "Artwork",
        "Bitmap",
        0,
        0,
        if welcome { 130 } else { 390 },
        if welcome { 240 } else { 52 },
        None,
        if welcome { "WelcomeArt" } else { "BannerArt" },
        None,
    )?;
    let title = match name {
        "Welcome" => "{\\Heading}[ProductName] [BrandingText] 마법사에 오신것을 환영합니다.",
        "Path" => "{\\PageHeading}설치 폴더 선택",
        "License" => "{\\PageHeading}사용권 계약",
        "Ready" => "{\\PageHeading}설치 준비 완료",
        "Maintenance" => "{\\PageHeading}설치 복구 및 제거",
        "RepairReady" => "{\\PageHeading}[ProductName] 복구",
        "RemoveReady" => "{\\PageHeading}[ProductName] 제거",
        "Progress" => "{\\PageHeading}[ProductName]을(를) 설치합니다.",
        "Complete" => "{\\Heading}[BrandingText] 마법사가 완료되었습니다.",
        "Cancelled" => "{\\Heading}[BrandingText] 마법사가 중단되었습니다.",
        _ => "{\\PageHeading}작업 실패",
    };
    control(
        db,
        name,
        "Heading",
        "Text",
        if welcome { 150 } else { 20 },
        if welcome { 28 } else { 8 },
        if welcome { 220 } else { 285 },
        if welcome { 65 } else { 24 },
        None,
        title,
        None,
    )?;
    if !welcome {
        control(
            db,
            name,
            "Branding",
            "Text",
            2,
            234,
            90,
            12,
            None,
            "{\\Branding}[BrandingText]",
            None,
        )?;
        control(
            db,
            name,
            "Subtitle",
            "Text",
            20,
            32,
            285,
            14,
            None,
            if name == "Path" {
                "[ProductName]을(를) 설치할 폴더를 선택해 주세요."
            } else if name == "Progress" {
                "선택한 프로그램을 설치합니다."
            } else if name == "Ready" {
                "[ProductName]을 설치할 준비가 되었습니다."
            } else if name == "Maintenance" {
                "수행할 작업을 선택해주세요."
            } else if name == "License" {
                "다음의 사용권 계약을 잘 읽어보시기 바랍니다."
            } else if name == "RepairReady" {
                "사용자의 시스템에서 이 프로그램을 복구합니다."
            } else if name == "RemoveReady" {
                "사용자의 시스템에서 이 프로그램을 제거합니다."
            } else {
                "[ProductName] - [ProductVersion]"
            },
            None,
        )?;
        control(
            db,
            name,
            "HeaderLine",
            "Line",
            0,
            52,
            390,
            1,
            None,
            "",
            None,
        )?;
    }
    // Branding uses an opaque Text control (no Transparent attribute), with
    // the native dialog background. Keep the underlying separator continuous.
    control(db, name, "Line", "Line", 0, 240, 390, 1, None, "", None)
}
#[allow(clippy::too_many_arguments)]
fn control(
    db: &Database,
    dlg: &str,
    id: &str,
    kind: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    property: Option<&str>,
    text: &str,
    next: Option<&str>,
) -> Result<()> {
    db.insert(
        "Control",
        &[
            "Dialog_",
            "Control",
            "Type",
            "X",
            "Y",
            "Width",
            "Height",
            "Attributes",
            "Property",
            "Text",
            "Control_Next",
        ],
        &[
            T(dlg),
            T(id),
            T(kind),
            I(x),
            I(y),
            I(w),
            I(h),
            I(if kind == "Bitmap" {
                1
            } else if id == "Heading" || id == "Subtitle" {
                65539
            } else {
                3
            }),
            property.map(T).unwrap_or(N),
            T(text),
            next.map(T).unwrap_or(N),
        ],
    )
}

/// MSI uses the Control_First/Control_Next chain for painting as well as
/// navigation. Include passive controls so the rule is painted before its label.
fn author_finish_launch(
    db: &Database,
    files: &[crate::payload::File],
    launch: &crate::finish::Launch,
) -> Result<()> {
    let file = launch.resolve(files)?;
    let target = launch.target()?;
    // Type 18 (installed EXE) + 192 (asynchronous, do not wait).
    db.insert(
        "CustomAction",
        &["Action", "Type", "Source", "Target"],
        &[T("LaunchAfterInstall"), I(210), T(&file.id), T(&target)],
    )?;
    let allowed = format!(
        "NOT Installed AND NOT REMOVE AND NOT REINSTALL AND NOT UPGRADINGPRODUCTCODE AND NOT EMB_OLD_PRODUCTS AND UILevel = 5 AND ${} = 3",
        file.component
    );
    control(
        db,
        "Complete",
        "Launch",
        "CheckBox",
        150,
        202,
        220,
        20,
        Some("LAUNCHAFTERINSTALL"),
        "설치한 프로그램 실행(&R)",
        Some("Finish"),
    )?;
    db.insert(
        "CheckBox",
        &["Property", "Value"],
        &[T("LAUNCHAFTERINSTALL"), T("1")],
    )?;
    for (action, condition) in [
        ("Show", allowed.clone()),
        ("Hide", format!("NOT ({allowed})")),
    ] {
        db.insert(
            "ControlCondition",
            &["Dialog_", "Control_", "Action", "Condition"],
            &[T("Complete"), T("Launch"), T(action), T(&condition)],
        )?;
    }
    let run = format!("({allowed}) AND LAUNCHAFTERINSTALL = \"1\"");
    db.insert(
        "ControlEvent",
        &[
            "Dialog_",
            "Control_",
            "Event",
            "Argument",
            "Condition",
            "Ordering",
        ],
        &[
            T("Complete"),
            T("Finish"),
            T("DoAction"),
            T("LaunchAfterInstall"),
            T(&run),
            I(1),
        ],
    )?;
    db.insert(
        "ControlEvent",
        &[
            "Dialog_",
            "Control_",
            "Event",
            "Argument",
            "Condition",
            "Ordering",
        ],
        &[
            T("Complete"),
            T("Finish"),
            T("[LAUNCHAFTERINSTALL]"),
            T("{}"),
            T(&run),
            I(2),
        ],
    )?;
    db.execute("UPDATE `ControlEvent` SET `Ordering` = 3 WHERE `Dialog_` = 'Complete' AND `Control_` = 'Finish' AND `Event` = 'EndDialog'")?;
    db.execute("UPDATE `Control` SET `Text` = '마침(&F)' WHERE `Dialog_` = 'Complete' AND `Control` = 'Finish'")?;
    Ok(())
}

fn order_controls(db: &Database) -> Result<()> {
    for dialog in db.query("SELECT `Dialog`,`Control_First` FROM `Dialog`", 2)? {
        let name = &dialog[0];
        if name == "ActionError" {
            continue;
        } // MSI error buttons must not have tab chains.
        let rows = db.query(
            &format!("SELECT `Control`,`Control_Next` FROM `Control` WHERE `Dialog_` = '{name}'"),
            2,
        )?;
        let mut interactive = Vec::new();
        let mut current = dialog[1].clone();
        while !interactive.contains(&current) {
            let row = rows
                .iter()
                .find(|r| r[0] == current)
                .ok_or("broken control chain")?;
            interactive.push(current);
            if row[1].is_empty() {
                break;
            }
            current = row[1].clone();
        }
        let mut order: Vec<String> = Vec::new();
        for id in [
            "Artwork",
            "HeaderLine",
            "Line",
            "Branding",
            "Heading",
            "Subtitle",
        ] {
            if rows.iter().any(|r| r[0] == id) && !interactive.iter().any(|i| i == id) {
                order.push(id.to_owned());
            }
        }
        for row in &rows {
            if !order.contains(&row[0]) && !interactive.contains(&row[0]) {
                order.push(row[0].clone());
            }
        }
        order.extend(interactive);
        for (i, id) in order.iter().enumerate() {
            let next = &order[(i + 1) % order.len()];
            db.execute(&format!("UPDATE `Control` SET `Control_Next` = '{next}' WHERE `Dialog_` = '{name}' AND `Control` = '{id}'"))?;
        }
        db.execute(&format!(
            "UPDATE `Dialog` SET `Control_First` = '{}' WHERE `Dialog` = '{name}'",
            order[0]
        ))?;
    }
    Ok(())
}
fn event(db: &Database, dlg: &str, button: &str, kind: &str, arg: &str, order: i32) -> Result<()> {
    db.insert(
        "ControlEvent",
        &[
            "Dialog_",
            "Control_",
            "Event",
            "Argument",
            "Condition",
            "Ordering",
        ],
        &[T(dlg), T(button), T(kind), T(arg), T("1"), I(order)],
    )
}
fn cancel_confirmation(db: &Database, license: bool) -> Result<()> {
    db.insert(
        "Dialog",
        &[
            "Dialog",
            "HCentering",
            "VCentering",
            "Width",
            "Height",
            "Attributes",
            "Title",
            "Control_First",
            "Control_Default",
            "Control_Cancel",
        ],
        &[
            T("ConfirmCancel"),
            I(50),
            I(50),
            I(300),
            I(100),
            I(3),
            T("[ProductName] - 취소 확인"),
            T("No"),
            T("No"),
            T("No"),
        ],
    )?;
    control(
        db,
        "ConfirmCancel",
        "Body",
        "Text",
        15,
        15,
        270,
        42,
        None,
        "[ProductName] [EMB_CANCEL_ACTION]를 정말로 취소하시겠습니까?",
        None,
    )?;
    control(
        db,
        "ConfirmCancel",
        "Yes",
        "PushButton",
        144,
        72,
        66,
        18,
        None,
        "예(&Y)",
        Some("No"),
    )?;
    control(
        db,
        "ConfirmCancel",
        "No",
        "PushButton",
        218,
        72,
        66,
        18,
        None,
        "아니요(&N)",
        Some("Yes"),
    )?;
    event(db, "ConfirmCancel", "Yes", "EndDialog", "Exit", 1)?;
    event(db, "ConfirmCancel", "No", "EndDialog", "Return", 1)?;
    for dlg in [
        "Welcome",
        "License",
        "Path",
        "Ready",
        "Maintenance",
        "RepairReady",
        "RemoveReady",
        "Progress",
    ] {
        if dlg == "License" && !license {
            continue;
        }
        db.execute(&format!(
            "DELETE FROM `ControlEvent` WHERE `Dialog_` = '{dlg}' AND `Control_` = 'Cancel'"
        ))?;
        let choices: Vec<(&str, &str)> = match dlg {
            "RepairReady" => vec![("복구", "1")],
            "RemoveReady" => vec![("제거", "1")],
            "Maintenance" => vec![
                ("유지보수", "1"),
                ("복구", "MaintenanceChoice = \"Repair\""),
                ("제거", "MaintenanceChoice = \"Remove\""),
            ],
            "Progress" => vec![
                ("설치", "1"),
                ("유지보수", "Installed"),
                ("복구", "REINSTALL OR MaintenanceChoice = \"Repair\""),
                (
                    "제거",
                    "REMOVE ~= \"ALL\" OR MaintenanceChoice = \"Remove\"",
                ),
            ],
            _ => vec![("설치", "1")],
        };
        for (i, (action, condition)) in choices.iter().enumerate() {
            db.insert(
                "ControlEvent",
                &[
                    "Dialog_",
                    "Control_",
                    "Event",
                    "Argument",
                    "Condition",
                    "Ordering",
                ],
                &[
                    T(dlg),
                    T("Cancel"),
                    T("[EMB_CANCEL_ACTION]"),
                    T(action),
                    T(condition),
                    I(i as i32 + 1),
                ],
            )?;
        }
        event(db, dlg, "Cancel", "SpawnDialog", "ConfirmCancel", 10)?;
    }
    for (action, condition) in [
        (
            "Enable",
            "MaintenanceChoice = \"Repair\" OR MaintenanceChoice = \"Remove\"",
        ),
        ("Disable", "NOT MaintenanceChoice"),
    ] {
        db.insert(
            "ControlCondition",
            &["Dialog_", "Control_", "Action", "Condition"],
            &[T("Maintenance"), T("Proceed"), T(action), T(condition)],
        )?;
    }
    Ok(())
}
fn button(db: &Database, dlg: &str, id: &str, x: i32, text: &str, next: &str) -> Result<()> {
    control(
        db,
        dlg,
        id,
        "PushButton",
        x,
        254,
        66,
        18,
        None,
        text,
        Some(next),
    )
}
fn browse_folder(db: &Database) -> Result<()> {
    db.insert(
        "Dialog",
        &[
            "Dialog",
            "HCentering",
            "VCentering",
            "Width",
            "Height",
            "Attributes",
            "Title",
            "Control_First",
            "Control_Default",
            "Control_Cancel",
        ],
        &[
            T("BrowseFolder"),
            I(50),
            I(50),
            I(300),
            I(240),
            I(3),
            T("[ProductName] - 설치 폴더 찾아보기"),
            T("Combo"),
            T("Select"),
            T("Cancel"),
        ],
    )?;
    control(
        db,
        "BrowseFolder",
        "Combo",
        "DirectoryCombo",
        12,
        12,
        216,
        80,
        Some("EMB_BROWSE_PATH"),
        "폴더 위치",
        Some("Up"),
    )?;
    db.execute("UPDATE `Control` SET `Attributes` = 2031619 WHERE `Dialog_` = 'BrowseFolder' AND `Control` = 'Combo'")?;
    control(
        db,
        "BrowseFolder",
        "Up",
        "PushButton",
        236,
        12,
        52,
        18,
        None,
        "상위 폴더",
        Some("List"),
    )?;
    control(
        db,
        "BrowseFolder",
        "List",
        "DirectoryList",
        12,
        40,
        276,
        132,
        Some("EMB_BROWSE_PATH"),
        "하위 폴더",
        Some("Folder"),
    )?;
    control(
        db,
        "BrowseFolder",
        "Folder",
        "PathEdit",
        12,
        182,
        276,
        20,
        Some("EMB_BROWSE_PATH"),
        "",
        Some("Select"),
    )?;
    control(
        db,
        "BrowseFolder",
        "Select",
        "PushButton",
        120,
        214,
        80,
        18,
        None,
        "확인",
        Some("Cancel"),
    )?;
    control(
        db,
        "BrowseFolder",
        "Cancel",
        "PushButton",
        208,
        214,
        80,
        18,
        None,
        "취소(&C)",
        Some("Combo"),
    )?;
    event(db, "BrowseFolder", "Up", "DirectoryListUp", "0", 1)?;
    event(
        db,
        "BrowseFolder",
        "Select",
        "[INSTALLDIR]",
        "[EMB_BROWSE_PATH]",
        1,
    )?;
    event(
        db,
        "BrowseFolder",
        "Select",
        "SetTargetPath",
        "INSTALLDIR",
        2,
    )?;
    event(db, "BrowseFolder", "Select", "EndDialog", "Return", 3)?;
    event(db, "BrowseFolder", "Cancel", "EndDialog", "Return", 1)?;
    Ok(())
}
fn ui(db: &Database, license: Option<&str>, repair_mode: &str) -> Result<()> {
    for (name, size, bits) in [
        ("Normal", 9, 0),
        ("Heading", 12, 1),
        ("PageHeading", 10, 1),
        ("Branding", 8, 0),
    ] {
        db.insert(
            "TextStyle",
            &["TextStyle", "FaceName", "Size", "StyleBits"],
            &[T(name), T("맑은 고딕"), I(size), I(bits)],
        )?;
    }
    dialog(db, "Welcome", "Next", "Next", "Cancel", true)?;
    control(
        db,
        "Welcome",
        "Body",
        "Text",
        150,
        100,
        220,
        110,
        None,
        "[BrandingText] 마법사를 사용하여 사용자의 시스템에 [ProductName]을 설치합니다. 계속 하시려면 다음을 누르시고, 설치 마법사를 종료하시려면 취소를 눌러주세요.",
        None,
    )?;
    button(db, "Welcome", "Next", BUTTON_NEXT_X, "다음(&N) >", "Cancel")?;
    button(db, "Welcome", "Cancel", BUTTON_CANCEL_X, "취소(&C)", "Next")?;
    event(
        db,
        "Welcome",
        "Next",
        "NewDialog",
        if license.is_some() { "License" } else { "Path" },
        1,
    )?;
    event(db, "Welcome", "Cancel", "EndDialog", "Exit", 1)?;

    if let Some(rtf) = license {
        dialog(db, "License", "Terms", "Next", "Cancel", true)?;
        control(
            db,
            "License",
            "Terms",
            "ScrollableText",
            20,
            72,
            350,
            122,
            None,
            rtf,
            Some("Accept"),
        )?;
        db.execute("UPDATE `Control` SET `Attributes` = 7 WHERE `Dialog_` = 'License' AND `Control` = 'Terms'")?;
        control(
            db,
            "License",
            "Accept",
            "CheckBox",
            20,
            204,
            350,
            20,
            Some("ACCEPTLICENSE"),
            "이용약관을 읽었으며 동의합니다(&A).",
            Some("Back"),
        )?;
        db.insert(
            "CheckBox",
            &["Property", "Value"],
            &[T("ACCEPTLICENSE"), T("1")],
        )?;
        button(db, "License", "Back", BUTTON_BACK_X, "< 뒤로(&B)", "Next")?;
        button(db, "License", "Next", BUTTON_NEXT_X, "다음(&N) >", "Cancel")?;
        button(
            db,
            "License",
            "Cancel",
            BUTTON_CANCEL_X,
            "취소(&C)",
            "Terms",
        )?;
        db.execute("UPDATE `Control` SET `Attributes` = 1 WHERE `Dialog_` = 'License' AND `Control` = 'Next'")?;
        for (action, condition) in [
            ("Enable", "ACCEPTLICENSE = \"1\""),
            ("Disable", "NOT (ACCEPTLICENSE = \"1\")"),
        ] {
            db.insert(
                "ControlCondition",
                &["Dialog_", "Control_", "Action", "Condition"],
                &[T("License"), T("Next"), T(action), T(condition)],
            )?;
        }
        db.insert(
            "ControlEvent",
            &[
                "Dialog_",
                "Control_",
                "Event",
                "Argument",
                "Condition",
                "Ordering",
            ],
            &[
                T("License"),
                T("Next"),
                T("NewDialog"),
                T("Path"),
                T("ACCEPTLICENSE = \"1\""),
                I(1),
            ],
        )?;
        event(db, "License", "Back", "NewDialog", "Welcome", 1)?;
        event(db, "License", "Cancel", "EndDialog", "Exit", 1)?;
    }
    dialog(db, "Path", "PathLabel", "Next", "Cancel", true)?;
    control(
        db,
        "Path",
        "PathLabel",
        "Text",
        20,
        82,
        350,
        44,
        None,
        "이 폴더에 [ProductName]을(를) 설치하려면, \"다음\"을 눌러주세요. 다른 폴더에 설치하시려면 아래 필드에 입력하거나, \"찾아보기\"를 눌러주세요.",
        Some("Folder"),
    )?;
    control(
        db,
        "Path",
        "Folder",
        "PathEdit",
        20,
        136,
        266,
        22,
        Some("INSTALLDIR"),
        "",
        Some("Browse"),
    )?;
    control(
        db,
        "Path",
        "Browse",
        "PushButton",
        294,
        136,
        76,
        22,
        None,
        "찾아보기...",
        Some("Back"),
    )?;
    event(db, "Path", "Browse", "[EMB_BROWSE_PATH]", "[INSTALLDIR]", 1)?;
    event(db, "Path", "Browse", "SpawnDialog", "BrowseFolder", 2)?;
    browse_folder(db)?;
    button(db, "Path", "Back", BUTTON_BACK_X, "< 뒤로(&B)", "Next")?;
    button(db, "Path", "Next", BUTTON_NEXT_X, "다음(&N) >", "Cancel")?;
    button(
        db,
        "Path",
        "Cancel",
        BUTTON_CANCEL_X,
        "취소(&C)",
        "PathLabel",
    )?;
    event(
        db,
        "Path",
        "Back",
        "NewDialog",
        if license.is_some() {
            "License"
        } else {
            "Welcome"
        },
        1,
    )?;
    event(db, "Path", "Next", "SetTargetPath", "INSTALLDIR", 1)?;
    event(db, "Path", "Next", "NewDialog", "Ready", 2)?;
    event(db, "Path", "Cancel", "EndDialog", "Exit", 1)?;

    dialog(db, "Ready", "Install", "Install", "Cancel", true)?;
    control(
        db,
        "Ready",
        "Body",
        "Text",
        20,
        82,
        350,
        120,
        None,
        "설치를 시작하려면 \"설치\"를 눌러주세요. 설치 설정을 검토 또는 수정하려면 \"뒤로\"를 누르시고, 설치 마법사를 종료하시려면 \"취소\"를 눌러주세요.",
        None,
    )?;
    button(db, "Ready", "Back", BUTTON_BACK_X, "< 뒤로(&B)", "Install")?;
    button(db, "Ready", "Install", BUTTON_NEXT_X, "설치(&I)", "Cancel")?;
    button(db, "Ready", "Cancel", BUTTON_CANCEL_X, "취소(&C)", "Back")?;
    event(db, "Ready", "Back", "NewDialog", "Path", 1)?;
    event(db, "Ready", "Install", "EndDialog", "Return", 1)?;
    event(db, "Ready", "Cancel", "EndDialog", "Exit", 1)?;

    dialog(db, "Maintenance", "Choice", "Proceed", "Cancel", true)?;
    control(
        db,
        "Maintenance",
        "RepairDescription",
        "Text",
        140,
        84,
        230,
        70,
        None,
        "프로그램을 가장 최근 설치 상태로 복구합니다. - 누락 혹은 손상된 파일, 바로가기 및 레지스트리 항목을 수정할 수 있습니다.",
        None,
    )?;
    control(
        db,
        "Maintenance",
        "RemoveDescription",
        "Text",
        140,
        174,
        230,
        42,
        None,
        "시스템에서 [ProductName]을(를) 제거합니다.",
        None,
    )?;
    control(
        db,
        "Maintenance",
        "RepairIcon",
        "Bitmap",
        20,
        78,
        32,
        32,
        None,
        "RepairArt",
        None,
    )?;
    control(
        db,
        "Maintenance",
        "RemoveIcon",
        "Bitmap",
        20,
        168,
        32,
        32,
        None,
        "RemoveArt",
        None,
    )?;
    control(
        db,
        "Maintenance",
        "Choice",
        "RadioButtonGroup",
        60,
        78,
        72,
        142,
        Some("MaintenanceChoice"),
        "",
        Some("Proceed"),
    )?;
    for (order, value, y, text) in [(1, "Repair", 0, "복구(&P)"), (2, "Remove", 90, "제거(&R)")]
    {
        db.insert(
            "RadioButton",
            &[
                "Property", "Order", "Value", "X", "Y", "Width", "Height", "Text",
            ],
            &[
                T("MaintenanceChoice"),
                I(order),
                T(value),
                I(0),
                I(y),
                I(72),
                I(24),
                T(text),
            ],
        )?;
    }
    // Keep descriptions outside the radio group's entire painted rectangle.
    // Native radio labels retain their click targets and keyboard shortcuts.
    button(
        db,
        "Maintenance",
        "Proceed",
        BUTTON_NEXT_X,
        "실행(&E)",
        "Cancel",
    )?;
    button(
        db,
        "Maintenance",
        "Cancel",
        BUTTON_CANCEL_X,
        "취소(&C)",
        "Choice",
    )?;
    for (name, action, label, condition, events) in [
        (
            "RepairReady",
            "복구",
            "복구(&P)",
            "MaintenanceChoice = \"Repair\"",
            vec![
                ("[REMOVE]", "{}"),
                ("ReinstallMode", repair_mode),
                ("Reinstall", "ALL"),
            ],
        ),
        (
            "RemoveReady",
            "제거",
            "제거(&R)",
            "MaintenanceChoice = \"Remove\"",
            vec![("[REINSTALL]", "{}"), ("[REMOVE]", "ALL")],
        ),
    ] {
        db.insert(
            "ControlEvent",
            &[
                "Dialog_",
                "Control_",
                "Event",
                "Argument",
                "Condition",
                "Ordering",
            ],
            &[
                T("Maintenance"),
                T("Proceed"),
                T("NewDialog"),
                T(name),
                T(condition),
                I(1),
            ],
        )?;
        dialog(db, name, "Proceed", "Proceed", "Cancel", true)?;
        control(
            db,
            name,
            "Body",
            "Text",
            20,
            82,
            350,
            120,
            None,
            &format!(
                "사용자의 시스템에서 [ProductName]을(를) {action}하려면, \"{action}\"를 눌러주세요. 설치 설정을 검토 혹은 변경하려면 \"뒤로\"를 누르시고, 마법사를 종료하려면 \"취소\"를 눌러주세요."
            ),
            None,
        )?;
        button(db, name, "Back", BUTTON_BACK_X, "< 뒤로(&B)", "Proceed")?;
        button(db, name, "Proceed", BUTTON_NEXT_X, label, "Cancel")?;
        button(db, name, "Cancel", BUTTON_CANCEL_X, "취소(&C)", "Back")?;
        event(db, name, "Back", "NewDialog", "Maintenance", 1)?;
        for (i, (kind, arg)) in events.iter().enumerate() {
            db.insert(
                "ControlEvent",
                &[
                    "Dialog_",
                    "Control_",
                    "Event",
                    "Argument",
                    "Condition",
                    "Ordering",
                ],
                &[
                    T(name),
                    T("Proceed"),
                    T(kind),
                    T(arg),
                    T(condition),
                    I(i as i32 + 1),
                ],
            )?;
        }
        event(db, name, "Proceed", "EndDialog", "Return", 4)?;
    }
    event(db, "Maintenance", "Cancel", "EndDialog", "Exit", 1)?;

    dialog(db, "Progress", "Cancel", "Cancel", "Cancel", false)?;
    progress_text(db)?;
    control(
        db,
        "Progress",
        "StatusLabel",
        "Text",
        20,
        138,
        30,
        16,
        None,
        "상태:",
        None,
    )?;
    control(
        db, "Progress", "Action", "Text", 52, 138, 318, 22, None, "", None,
    )?;
    control(
        db,
        "Progress",
        "Bar",
        "ProgressBar",
        20,
        165,
        350,
        15,
        None,
        "",
        None,
    )?;
    button(
        db,
        "Progress",
        "Cancel",
        BUTTON_CANCEL_X,
        "취소(&C)",
        "Cancel",
    )?;
    event(db, "Progress", "Cancel", "EndDialog", "Exit", 1)?;
    cancel_confirmation(db, license.is_some())?;
    for (ctrl, ev, attr) in [
        ("Action", "ActionText", "Text"),
        ("Bar", "SetProgress", "Progress"),
    ] {
        db.insert(
            "EventMapping",
            &["Dialog_", "Control_", "Event", "Attribute"],
            &[T("Progress"), T(ctrl), T(ev), T(attr)],
        )?;
    }
    for (name, text) in [
        (
            "Complete",
            "[BrandingText] 마법사가 [ProductName]을(를) 성공적으로 설치했습니다. 마법사를 종료하려면 \"마침\"을 눌러주세요.",
        ),
        (
            "Cancelled",
            "[ProductName] - [BrandingText] 마법사가 완료되기 전에 중단되었습니다.\n\n시스템이 변경되지 않았습니다. 나중에 이 프로그램을 설치하려면, 설치 마법사를 다시 실행해주세요.\n\n마법사를 종료하려면 \"마침\"을 눌러주세요.",
        ),
        (
            "Failed",
            "작업에 실패했습니다. 자세한 내용은 MSI 로그를 확인하세요.",
        ),
    ] {
        dialog(db, name, "Finish", "Finish", "Finish", true)?;
        let cancelled = matches!(name, "Cancelled" | "Complete");
        control(
            db,
            name,
            "Body",
            "Text",
            if cancelled { 150 } else { 20 },
            if cancelled { 100 } else { 82 },
            if cancelled { 220 } else { 350 },
            if cancelled { 128 } else { 110 },
            None,
            text,
            None,
        )?;
        button(
            db,
            name,
            "Finish",
            BUTTON_CANCEL_X,
            if cancelled { "마침(&F)" } else { "닫기" },
            "Finish",
        )?;
        event(db, name, "Finish", "EndDialog", "Return", 1)?;
    }
    completion_text(db)?;
    Ok(())
}

fn completion_text(db: &Database) -> Result<()> {
    for (suffix, action, condition) in [
        (
            "",
            "설치",
            "NOT (REMOVE ~= \"ALL\" OR MaintenanceChoice = \"Remove\" OR REINSTALL OR MaintenanceChoice = \"Repair\")",
        ),
        (
            "Repair",
            "복구",
            "NOT (REMOVE ~= \"ALL\" OR MaintenanceChoice = \"Remove\") AND (REINSTALL OR MaintenanceChoice = \"Repair\")",
        ),
        (
            "Remove",
            "제거",
            "REMOVE ~= \"ALL\" OR MaintenanceChoice = \"Remove\"",
        ),
    ] {
        let id = format!("Body{suffix}");
        if !suffix.is_empty() {
            control(
                db,
                "Complete",
                &id,
                "Text",
                150,
                100,
                220,
                90,
                None,
                &format!(
                    "[BrandingText] 마법사가 [ProductName]을(를) 성공적으로 {action}했습니다. 마법사를 종료하려면 \"마침\"을 눌러주세요."
                ),
                None,
            )?;
        }
        for (visibility, test) in [
            ("Show", condition.to_string()),
            ("Hide", format!("NOT ({condition})")),
        ] {
            db.insert(
                "ControlCondition",
                &["Dialog_", "Control_", "Action", "Condition"],
                &[T("Complete"), T(&id), T(visibility), T(&test)],
            )?;
        }
    }
    // Keep the optional launch checkbox below the completion message.
    db.execute(
        "UPDATE `Control` SET `Height` = 90 WHERE `Dialog_` = 'Complete' AND `Control` = 'Body'",
    )?;
    Ok(())
}

fn progress_text(db: &Database) -> Result<()> {
    let removing = "REMOVE ~= \"ALL\" OR MaintenanceChoice = \"Remove\"";
    let repairing = "NOT (REMOVE ~= \"ALL\" OR MaintenanceChoice = \"Remove\") AND (REINSTALL OR MaintenanceChoice = \"Repair\")";
    let installing = "NOT (REMOVE ~= \"ALL\" OR MaintenanceChoice = \"Remove\" OR REINSTALL OR MaintenanceChoice = \"Repair\")";
    for (suffix, action, condition) in [
        ("", "설치", installing),
        ("Repair", "복구", repairing),
        ("Remove", "제거", removing),
    ] {
        let heading = format!("Heading{suffix}");
        if !suffix.is_empty() {
            control(
                db,
                "Progress",
                &heading,
                "Text",
                20,
                8,
                285,
                24,
                None,
                &format!("{{\\PageHeading}}[ProductName]을(를) {action}합니다."),
                None,
            )?;
            db.execute(&format!("UPDATE `Control` SET `Attributes` = 65539 WHERE `Dialog_` = 'Progress' AND `Control` = '{heading}'"))?;
        }
        let body = format!("Body{suffix}");
        control(
            db,
            "Progress",
            &body,
            "Text",
            20,
            82,
            350,
            45,
            None,
            &format!(
                "[BrandingText] 마법사가 [ProductName]을(를) 사용자의 시스템에 {action}하는 동안 잠시 기다려 주세요. 이 작업은 몇 분 정도 걸릴 수 있습니다."
            ),
            None,
        )?;
        for id in [&heading, &body] {
            for (visibility, test) in [
                ("Show", condition.to_string()),
                ("Hide", format!("NOT ({condition})")),
            ] {
                db.insert(
                    "ControlCondition",
                    &["Dialog_", "Control_", "Action", "Condition"],
                    &[T("Progress"), T(id), T(visibility), T(&test)],
                )?;
            }
        }
    }
    Ok(())
}

pub fn verify(path: &Path) -> Result<()> {
    msi::verify_package(path)?;
    let db = Database::read(path)?;
    let file_rows = db.query(
        "SELECT `File`,`Component_`,`Sequence` FROM `File` ORDER BY `Sequence`",
        3,
    )?;
    if file_rows.is_empty() {
        return Err("unexpected payload".into());
    }
    let components = db.query("SELECT `Component`,`KeyPath` FROM `Component`", 2)?;
    for (i, file) in file_rows.iter().enumerate() {
        if file[2].parse::<usize>()? != i + 1
            || !components
                .iter()
                .any(|c| c[0] == file[1] && c[1] == file[0])
        {
            return Err("invalid file sequence or component key path".into());
        }
    }
    let media = db.query(
        "SELECT `DiskId`,`LastSequence`,`Cabinet` FROM `Media` ORDER BY `DiskId`",
        3,
    )?;
    if media.is_empty() {
        return Err("no cabinet media".into());
    }
    let mut previous = 0usize;
    for (index, row) in media.iter().enumerate() {
        let last = row[1].parse::<usize>()?;
        if row[0].parse::<usize>()? != index + 1 || last <= previous || last > file_rows.len() {
            return Err("invalid CAB sequence range".into());
        }
        previous = last;
        let reference = &row[2];
        let name = reference.strip_prefix('#').unwrap_or(reference);
        if name != "payload.cab"
            && !(name
                .strip_prefix("Data")
                .and_then(|s| s.strip_suffix(".cab"))
                .is_some_and(|n| {
                    !n.is_empty()
                        && n.bytes().all(|b| b.is_ascii_digit())
                        && n.parse::<usize>().is_ok_and(|v| v == index + 1)
                }))
        {
            return Err("unsupported CAB name".into());
        }
        if reference.starts_with('#') {
            if db
                .query(
                    &format!("SELECT `Name` FROM `_Streams` WHERE `Name` = '{name}'"),
                    1,
                )?
                .len()
                != 1
            {
                return Err(format!("embedded CAB missing: {name}").into());
            }
        } else {
            let cab_path = path.parent().unwrap_or(Path::new(".")).join(name);
            let mut cab = fs::File::open(&cab_path).map_err(|e| {
                format!(
                    "external CAB missing or unreadable: {}: {e}",
                    cab_path.display()
                )
            })?;
            let mut header = [0; 36];
            std::io::Read::read_exact(&mut cab, &mut header)?;
            if &header[..4] != b"MSCF"
                || u32::from_le_bytes(header[8..12].try_into()?) as u64 != cab.metadata()?.len()
            {
                return Err("external CAB has an invalid header or size".into());
            }
        }
    }
    if previous != file_rows.len() {
        return Err("CAB media sequence does not cover files".into());
    }
    let dialog_count = db.query("SELECT `Dialog` FROM `Dialog`", 1)?.len();
    let error_dialog_count = db
        .query(
            "SELECT `Dialog` FROM `Dialog` WHERE `Dialog` = 'ActionError'",
            1,
        )?
        .len();
    let browse_count = db
        .query(
            "SELECT `Dialog` FROM `Dialog` WHERE `Dialog` = 'BrowseFolder'",
            1,
        )?
        .len();
    let confirm_count = db
        .query(
            "SELECT `Dialog` FROM `Dialog` WHERE `Dialog` = 'ConfirmCancel'",
            1,
        )?
        .len();
    if !matches!(
        dialog_count - error_dialog_count - browse_count - confirm_count,
        10 | 11
    ) {
        return Err("wizard dialogs missing".into());
    }
    let images = db.query("SELECT `Name` FROM `Binary`", 1)?;
    for row in db.query("SELECT `Text` FROM `Control` WHERE `Type` = 'Bitmap'", 1)? {
        if !images.contains(&row) {
            return Err(format!("missing bitmap resource: {}", row[0]).into());
        }
    }
    for row in db.query(
        "SELECT `Dialog_`,`Control`,`X`,`Y`,`Width`,`Height` FROM `Control`",
        6,
    )? {
        let x: i32 = row[2].parse()?;
        let y: i32 = row[3].parse()?;
        let w: i32 = row[4].parse()?;
        let h: i32 = row[5].parse()?;
        if x < 0 || y < 0 || w <= 0 || h <= 0 || x + w > 390 || y + h > 280 {
            return Err(format!("control outside dialog: {}/{}", row[0], row[1]).into());
        }
    }
    println!(
        "Verified: restricted Installer session, {} files, {} CAB(s), {dialog_count} wizard dialogs (not ICE validation)",
        file_rows.len(),
        media.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cab_build_handles_out_of_range_dates_without_modifying_sources() -> Result<()> {
        use std::time::{Duration, UNIX_EPOCH};
        let base = std::env::current_dir()?.join("target");
        fs::create_dir_all(&base)?;
        let work = Work(base.join(format!("emb-cab-dates-{}", msi::guid()?)));
        let source = work.0.join("source");
        fs::create_dir_all(&source)?;
        let dates = [
            UNIX_EPOCH + Duration::from_secs(1),
            UNIX_EPOCH + Duration::from_secs(1_600_000_000),
            UNIX_EPOCH + Duration::from_secs(4_500_000_000),
        ];
        for (i, date) in dates.iter().enumerate() {
            let file = fs::File::create(source.join(format!("file{i}.txt")))?;
            use std::io::Write;
            (&file).write_all(format!("payload {i}").as_bytes())?;
            file.set_times(fs::FileTimes::new().set_modified(*date))?;
        }
        let output = work.0.join("output/Setup.msi");
        build(&source, &output, None, "Rocket MSI", CabStorage::External)?;
        let extracted = work.0.join("extracted");
        fs::create_dir(&extracted)?;
        let system = std::env::var_os("SystemRoot").ok_or("SystemRoot missing")?;
        let expanded = Command::new(PathBuf::from(system).join("System32/expand.exe"))
            .arg("-R")
            .arg(output.parent().unwrap().join("Data1.cab"))
            .arg("-F:*")
            .arg(&extracted)
            .output()?;
        assert!(expanded.status.success());
        let files = crate::payload::collect(&source, &work.0)?;
        for (i, date) in dates.iter().enumerate() {
            let path = source.join(format!("file{i}.txt"));
            assert_eq!(path.metadata()?.modified()?, *date);
            let entry = files
                .iter()
                .find(|f| f.source.file_name() == path.file_name())
                .unwrap();
            assert_eq!(fs::read(extracted.join(&entry.id))?, fs::read(&path)?);
        }
        Ok(())
    }
    #[test]
    fn finish_launch_is_opt_in_and_only_scheduled_from_success_ui() -> Result<()> {
        let base = std::env::current_dir()?.join("target");
        fs::create_dir_all(&base)?;
        let work = Work(base.join(format!("emb-finish-{}", msi::guid()?)));
        fs::create_dir(&work.0)?;
        let source = work.0.join("source");
        fs::create_dir_all(source.join("bin"))?;
        fs::write(source.join("bin/app.exe"), b"test bytes, never executed")?;
        let launch = crate::finish::Launch {
            file: "bin/app.exe".into(),
            arguments: vec!["two words".into(), "[literal]".into()],
        };
        let output = work.0.join("output/Setup.msi");
        build_with_finish(
            &source,
            &output,
            None,
            "Express MSI Builder",
            CabStorage::Embedded,
            None,
            Some(&launch),
        )?;
        let db = Database::read(&output)?;
        let action=db.query("SELECT `Type`,`Source`,`Target` FROM `CustomAction` WHERE `Action` = 'LaunchAfterInstall'",3)?;
        assert_eq!(action[0][0], "210");
        let files = crate::payload::collect(&source, &work.0)?;
        assert_eq!(action[0][1], files[0].id);
        assert_eq!(action[0][2], launch.target()?);
        assert!(db.query("SELECT `Action` FROM `InstallExecuteSequence` WHERE `Action` = 'LaunchAfterInstall'",1)?.is_empty());
        assert!(
            db.query(
                "SELECT `Value` FROM `Property` WHERE `Property` = 'LAUNCHAFTERINSTALL'",
                1
            )?
            .is_empty()
        );
        let events=db.query("SELECT `Dialog_`,`Control_`,`Condition`,`Ordering` FROM `ControlEvent` WHERE `Event` = 'DoAction'",4)?;
        assert_eq!(events.len(), 1);
        assert_eq!(&events[0][..2], ["Complete", "Finish"]);
        for gate in [
            "NOT Installed",
            "NOT REMOVE",
            "NOT REINSTALL",
            "NOT UPGRADINGPRODUCTCODE",
            "UILevel = 5",
            "LAUNCHAFTERINSTALL = \"1\"",
        ] {
            assert!(events[0][2].contains(gate));
        }
        assert_eq!(events[0][3], "1");
        assert_eq!(db.query("SELECT `Ordering` FROM `ControlEvent` WHERE `Dialog_` = 'Complete' AND `Event` = 'EndDialog'",1)?[0][0],"3");
        assert_eq!(db.query("SELECT `Action` FROM `ControlCondition` WHERE `Dialog_` = 'Complete' AND `Control_` = 'Launch'",1)?.len(),2);
        for path in [
            "../app.exe",
            "C:\\app.exe",
            "bin/missing.exe",
            "bin/app.txt",
        ] {
            let invalid = crate::finish::Launch {
                file: path.into(),
                arguments: vec![],
            };
            assert!(invalid.resolve(&files).is_err());
        }
        Ok(())
    }
    #[test]
    fn split_cabinets_preserve_sequence_contents_and_publish_as_a_set() -> Result<()> {
        let base = std::env::current_dir()?.join("target");
        fs::create_dir_all(&base)?;
        let work = Work(base.join(format!("emb-split-{}", msi::guid()?)));
        fs::create_dir(&work.0)?;
        let source = work.0.join("source");
        fs::create_dir_all(source.join("nested"))?;
        let mut state = 123456789u32;
        for name in ["a.bin", "nested/b.bin", "nested/c.bin"] {
            let bytes: Vec<u8> = (0..6000)
                .map(|_| {
                    state ^= state << 13;
                    state ^= state >> 17;
                    state ^= state << 5;
                    (state >> 24) as u8
                })
                .collect();
            fs::write(source.join(name), bytes)?;
        }
        let files = crate::payload::collect(&source, &work.0)?;
        for (mode, dir) in [
            (CabStorage::External, "external"),
            (CabStorage::Embedded, "embedded"),
        ] {
            let output = work.0.join(dir).join("Setup.msi");
            build_with_limit(
                &source,
                &output,
                None,
                "Express MSI Builder",
                mode,
                Some(8192),
            )?;
            let db = Database::read(&output)?;
            let media = db.query(
                "SELECT `DiskId`,`LastSequence`,`Cabinet` FROM `Media` ORDER BY `DiskId`",
                3,
            )?;
            assert_eq!(media.len(), 3);
            for (i, row) in media.iter().enumerate() {
                assert_eq!(row[0], (i + 1).to_string());
                assert_eq!(row[1], (i + 1).to_string());
                assert_eq!(
                    row[2],
                    format!(
                        "{}Data{}.cab",
                        if mode == CabStorage::Embedded {
                            "#"
                        } else {
                            ""
                        },
                        i + 1
                    )
                );
            }
            if mode == CabStorage::External {
                let system = std::env::var_os("SystemRoot").ok_or("SystemRoot missing")?;
                for (i, file) in files.iter().enumerate() {
                    let cab = output.parent().unwrap().join(format!("Data{}.cab", i + 1));
                    assert!(cab.metadata()?.len() <= 8192);
                    let dest = work.0.join(format!("extract-{i}"));
                    fs::create_dir(&dest)?;
                    let result = Command::new(PathBuf::from(&system).join("System32/expand.exe"))
                        .arg("-R")
                        .arg(&cab)
                        .arg("-F:*")
                        .arg(&dest)
                        .output()?;
                    assert!(result.status.success());
                    assert_eq!(fs::read(dest.join(&file.id))?, fs::read(&file.source)?);
                }
            } else {
                for i in 1..=3 {
                    assert_eq!(
                        db.query(
                            &format!("SELECT `Name` FROM `_Streams` WHERE `Name` = 'Data{i}.cab'"),
                            1
                        )?
                        .len(),
                        1
                    );
                }
            }
        }
        let failed = work.0.join("oversize/Setup.msi");
        assert!(
            build_with_limit(
                &source,
                &failed,
                None,
                "Express MSI Builder",
                CabStorage::External,
                Some(5000)
            )
            .is_err()
        );
        assert!(!failed.exists());
        let collision = work.0.join("collision");
        fs::create_dir(&collision)?;
        fs::write(collision.join("Data2.cab"), b"existing CAB")?;
        assert!(
            build_with_limit(
                &source,
                &collision.join("Setup.msi"),
                None,
                "Express MSI Builder",
                CabStorage::External,
                Some(8192)
            )
            .is_err()
        );
        assert!(!collision.join("Setup.msi").exists());
        assert!(!collision.join("Data1.cab").exists());
        assert_eq!(fs::read(collision.join("Data2.cab"))?, b"existing CAB");
        // Force the actual-size fallback: raw bytes fit, CAB overhead does not.
        let tiny = work.0.join("tiny");
        fs::create_dir(&tiny)?;
        for name in ["a", "b"] {
            fs::write(tiny.join(name), (0u8..80).collect::<Vec<_>>())?;
        }
        let fallback = work.0.join("fallback/Setup.msi");
        build_with_limit(
            &tiny,
            &fallback,
            None,
            "Express MSI Builder",
            CabStorage::External,
            Some(250),
        )?;
        let fallback_db = Database::read(&fallback)?;
        assert_eq!(
            fallback_db.query("SELECT `Cabinet` FROM `Media`", 1)?.len(),
            2
        );
        drop(fallback_db);
        assert!(
            build_with_limit(
                &tiny,
                &work.0.join("tiny-output/Setup.msi"),
                None,
                "Express MSI Builder",
                CabStorage::External,
                Some(100)
            )
            .is_err()
        );
        Ok(())
    }
    #[test]
    fn folder_payload_preserves_paths_content_and_stable_components() -> Result<()> {
        let base = std::env::current_dir()?.join("target");
        fs::create_dir_all(&base)?;
        let work = Work(base.join(format!("emb-folder-{}", msi::guid()?)));
        fs::create_dir(&work.0)?;
        let source = work.0.join("소스 폴더");
        fs::create_dir_all(source.join("한글 폴더/nested"))?;
        fs::create_dir_all(source.join("other"))?;
        for (name, contents) in [
            ("readme.txt", "root"),
            ("한글 폴더/config.json", "설정"),
            ("한글 폴더/nested/readme.txt", "nested"),
            ("other/readme.txt", "other"),
        ] {
            fs::write(source.join(name), contents)?;
        }
        let out = work.0.join("external");
        let output = out.join("Setup.msi");
        let before = crate::payload::collect(&source, &work.0)?;
        build(
            &source,
            &output,
            None,
            "Express MSI Builder",
            CabStorage::External,
        )?;
        let db = Database::read(&output)?;
        assert_eq!(db.query("SELECT `File` FROM `File`", 1)?.len(), 4);
        assert_eq!(db.query("SELECT `Directory` FROM `Directory`", 1)?.len(), 6);
        for file in &before {
            assert_eq!(
                db.query(
                    &format!(
                        "SELECT `ComponentId` FROM `Component` WHERE `Component` = '{}'",
                        file.component
                    ),
                    1
                )?[0][0],
                file.guid
            );
        }
        drop(db);
        let extracted = work.0.join("extracted");
        fs::create_dir(&extracted)?;
        let system = std::env::var_os("SystemRoot").ok_or("SystemRoot missing")?;
        let result = Command::new(PathBuf::from(system).join("System32/expand.exe"))
            .arg("-R")
            .arg(out.join("Data1.cab"))
            .arg("-F:*")
            .arg(&extracted)
            .output()?;
        assert!(result.status.success());
        for file in &before {
            assert_eq!(fs::read(extracted.join(&file.id))?, fs::read(&file.source)?);
        }
        fs::write(source.join("000-added.txt"), "new")?;
        let after = crate::payload::collect(&source, &work.0)?;
        for file in &before {
            let next = after.iter().find(|p| p.relative == file.relative).unwrap();
            assert_eq!(file.id, next.id);
            assert_eq!(file.guid, next.guid);
        }
        build(
            &source,
            &work.0.join("embedded/Setup.msi"),
            None,
            "Express MSI Builder",
            CabStorage::Embedded,
        )?;
        assert!(
            build(
                &source,
                &source.join("out/Setup.msi"),
                None,
                "Express MSI Builder",
                CabStorage::Embedded
            )
            .is_err()
        );
        let empty = work.0.join("empty");
        fs::create_dir(&empty)?;
        assert!(crate::payload::collect(&empty, &work.0).is_err());
        Ok(())
    }
    #[test]
    fn external_cab_set_extracts_and_rejects_incomplete_output() -> Result<()> {
        let base = std::env::current_dir()?.join("target");
        fs::create_dir_all(&base)?;
        let work = Work(base.join(format!("emb-external-{}", msi::guid()?)));
        fs::create_dir(&work.0)?;
        let source = work.0.join("한글 원본.txt");
        fs::write(&source, "external cabinet content 한글")?;
        let output = work.0.join("외부 설치.msi");
        build(
            &source,
            &output,
            None,
            "Express MSI Builder",
            CabStorage::External,
        )?;
        let cab = work.0.join("Data1.cab");
        let db = Database::read(&output)?;
        assert_eq!(
            db.query("SELECT `Cabinet` FROM `Media`", 1)?,
            vec![vec!["Data1.cab"]]
        );
        assert!(
            db.query(
                "SELECT `Name` FROM `_Streams` WHERE `Name` = 'payload.cab'",
                1
            )?
            .is_empty()
        );
        drop(db);
        let extracted = work.0.join("extracted");
        fs::create_dir(&extracted)?;
        let system = std::env::var_os("SystemRoot").ok_or("SystemRoot missing")?;
        let result = Command::new(PathBuf::from(system).join("System32/expand.exe"))
            .arg("-R")
            .arg(&cab)
            .arg("-F:*")
            .arg(&extracted)
            .output()?;
        assert!(
            result.status.success(),
            "CAB extraction failed: {:?}",
            result
        );
        assert!(
            extracted.join("Payload").exists(),
            "extraction stdout: {}; files: {:?}",
            String::from_utf8_lossy(&result.stdout),
            fs::read_dir(&extracted)?.collect::<Vec<_>>()
        );
        assert_eq!(fs::read(extracted.join("Payload"))?, fs::read(&source)?);
        let original_cab = fs::read(&cab)?;
        let second_output = work.0.join("second.msi");
        assert!(
            build(
                &source,
                &second_output,
                None,
                "Express MSI Builder",
                CabStorage::External
            )
            .is_err()
        );
        assert!(!second_output.exists());
        assert_eq!(fs::read(&cab)?, original_cab);
        fs::rename(&cab, work.0.join("saved.cab"))?;
        assert!(
            verify(&output)
                .unwrap_err()
                .to_string()
                .contains("external CAB")
        );
        fs::write(&cab, b"invalid cabinet")?;
        assert!(verify(&output).is_err());
        // Simulate an MSI filename collision appearing during publication.
        fs::remove_file(&cab)?;
        let staging = work.0.join("staging");
        fs::create_dir(&staging)?;
        fs::write(staging.join("Data1.cab"), &original_cab)?;
        fs::write(staging.join("test.msi"), b"staged MSI")?;
        let occupied = work.0.join("occupied.msi");
        fs::write(&occupied, b"keep existing MSI")?;
        assert!(publish(&staging.join("test.msi"), &occupied, CabStorage::External).is_err());
        assert!(
            !cab.exists(),
            "new CAB must be rolled back on MSI publish failure"
        );
        assert_eq!(fs::read(&occupied)?, b"keep existing MSI");
        Ok(())
    }
    #[test]
    fn license_packages_gate_navigation_and_silent_install() -> Result<()> {
        let base = std::env::current_dir()?.join("target");
        fs::create_dir_all(&base)?;
        let work = Work(base.join(format!("emb-license-{}", msi::guid()?)));
        fs::create_dir(&work.0)?;
        let source = Path::new("examples/payload/hello.txt");
        for extension in ["txt", "rtf"] {
            let terms = PathBuf::from(format!("examples/demo-terms.{extension}"));
            let output = work.0.join(format!("license-{extension}.msi"));
            build(
                source,
                &output,
                Some(&terms),
                "Custom Setup",
                CabStorage::Embedded,
            )?;
            let db = Database::read(&output)?;
            assert_eq!(
                db.query(
                    "SELECT `Value` FROM `Property` WHERE `Property` = 'BrandingText'",
                    1
                )?[0][0],
                "Custom Setup"
            );
            let texts = db.query("SELECT `Text` FROM `Control` WHERE `Type` = 'Text'", 1)?;
            assert!(
                texts
                    .iter()
                    .all(|row| !row[0].contains("InstallEmbly 마법사"))
            );
            assert!(
                texts
                    .iter()
                    .filter(|row| row[0].contains("[BrandingText] 마법사"))
                    .count()
                    >= 9
            );
            let brands = db.query(
                "SELECT `Dialog_` FROM `Control` WHERE `Control` = 'Branding'",
                1,
            )?;
            assert_eq!(brands.len(), 8);
            assert!(!brands.contains(&vec!["Welcome".to_string()]));
            assert!(!brands.contains(&vec!["Cancelled".to_string()]));
            assert!(!brands.contains(&vec!["Complete".to_string()]));
            for brand in &brands {
                let name = &brand[0];
                let rows = db.query(
                    &format!(
                        "SELECT `Control`,`Control_Next` FROM `Control` WHERE `Dialog_` = '{name}'"
                    ),
                    2,
                )?;
                let mut chain = Vec::new();
                let mut current = db.query(
                    &format!("SELECT `Control_First` FROM `Dialog` WHERE `Dialog` = '{name}'"),
                    1,
                )?[0][0]
                    .clone();
                while !chain.contains(&current) {
                    let next = rows.iter().find(|r| r[0] == current).unwrap()[1].clone();
                    chain.push(current);
                    current = next;
                }
                assert_eq!(chain.len(), rows.len());
                assert_eq!(current, chain[0]);
                assert!(
                    chain.iter().position(|c| c == "Line").unwrap()
                        < chain.iter().position(|c| c == "Branding").unwrap()
                );
            }
            assert_eq!(db.query("SELECT `X`,`Y`,`Width`,`Height` FROM `Control` WHERE `Dialog_` = 'License' AND `Control` = 'Branding'",4)?[0],vec!["2".to_string(),"234".into(),msi::branding_width("Custom Setup")?.to_string(),"12".into()]);
            assert_eq!(db.query("SELECT `X`,`Y`,`Width` FROM `Control` WHERE `Dialog_` = 'License' AND `Control` = 'Line'",3)?[0],vec!["0","240","390"]);
            assert_eq!(db.query("SELECT `Attributes` FROM `Control` WHERE `Dialog_` = 'License' AND `Control` = 'Branding'",1)?[0][0],"3");
            assert_eq!(db.query("SELECT `Dialog` FROM `Dialog`", 1)?.len(), 13);
            assert_eq!(db.query("SELECT `Argument` FROM `ControlEvent` WHERE `Dialog_` = 'Welcome' AND `Control_` = 'Next'",1)?[0][0],"License");
            assert_eq!(db.query("SELECT `Argument` FROM `ControlEvent` WHERE `Dialog_` = 'Path' AND `Control_` = 'Back'",1)?[0][0],"License");
            assert_eq!(db.query("SELECT `Attributes` FROM `Control` WHERE `Dialog_` = 'License' AND `Control` = 'Next'",1)?[0][0],"1");
            assert_eq!(db.query("SELECT `Condition` FROM `ControlEvent` WHERE `Dialog_` = 'License' AND `Control_` = 'Next'",1)?[0][0],"ACCEPTLICENSE = \"1\"");
            assert_eq!(
                db.query(
                    "SELECT `Action` FROM `ControlCondition` WHERE `Dialog_` = 'License'",
                    1
                )?
                .len(),
                2
            );
            assert_eq!(
                db.query("SELECT `Condition` FROM `LaunchCondition`", 1)?[0][0],
                "Installed OR ACCEPTLICENSE = \"1\""
            );
            assert!(
                db.query(
                    "SELECT `Value` FROM `Property` WHERE `Property` = 'ACCEPTLICENSE'",
                    1
                )?
                .is_empty()
            );
            let rtf = &db.query(
                "SELECT `Text` FROM `Control` WHERE `Dialog_` = 'License' AND `Control` = 'Terms'",
                1,
            )?[0][0];
            assert!(rtf.starts_with("{\\rtf1"));
            if extension == "txt" {
                assert!(rtf.contains("\\u"));
            }
        }
        let invalid = work.0.join("invalid.rtf");
        fs::write(&invalid, "not RTF")?;
        let output = work.0.join("must-not-exist.msi");
        assert!(
            build(
                source,
                &output,
                Some(&invalid),
                "Express MSI Builder",
                CabStorage::Embedded
            )
            .is_err()
        );
        assert!(!output.exists());
        Ok(())
    }
    #[test]
    fn creates_and_reopens_real_msi_without_installing() -> Result<()> {
        let base = std::env::current_dir()?.join("target");
        fs::create_dir_all(&base)?;
        let work = Work(base.join(format!("emb-test-{}", msi::guid()?)));
        fs::create_dir(&work.0)?;
        let source = work.0.join("한글 공백 원본.txt");
        fs::write(&source, "test payload")?;
        let output = work.0.join("설치 테스트.msi");
        build(
            &source,
            &output,
            None,
            "Express MSI Builder",
            CabStorage::Embedded,
        )?;
        let db = Database::read(&output)?;
        let repair = db.query("SELECT `Event`,`Argument` FROM `ControlEvent` WHERE `Dialog_` = 'RepairReady' AND `Control_` = 'Proceed' AND `Condition` = 'MaintenanceChoice = \"Repair\"' ORDER BY `Ordering`",2)?;
        let selection = db.query("SELECT `Event`,`Argument` FROM `ControlEvent` WHERE `Dialog_` = 'Maintenance' AND `Control_` = 'Proceed'", 2)?;
        assert_eq!(selection.len(), 2);
        for row in selection {
            assert_eq!(row[0], "NewDialog");
            assert!(matches!(row[1].as_str(), "RepairReady" | "RemoveReady"));
        }
        for name in ["RepairReady", "RemoveReady"] {
            assert_eq!(db.query(&format!("SELECT `Argument` FROM `ControlEvent` WHERE `Dialog_` = '{name}' AND `Control_` = 'Back' AND `Event` = 'NewDialog'"), 1)?, vec![vec!["Maintenance"]]);
            assert_eq!(db.query(&format!("SELECT `Argument` FROM `ControlEvent` WHERE `Dialog_` = '{name}' AND `Control_` = 'Cancel' AND `Event` = 'SpawnDialog'"), 1)?, vec![vec!["ConfirmCancel"]]);
        }
        assert!(
            db.query(
                "SELECT `Value` FROM `Property` WHERE `Property` = 'MaintenanceChoice'",
                1
            )?
            .is_empty()
        );
        assert_eq!(db.query("SELECT `Control_Default`,`Control_Cancel` FROM `Dialog` WHERE `Dialog` = 'ConfirmCancel'", 2)?, vec![vec!["No", "No"]]);
        for (button, result) in [("Yes", "Exit"), ("No", "Return")] {
            assert_eq!(db.query(&format!("SELECT `Argument` FROM `ControlEvent` WHERE `Dialog_` = 'ConfirmCancel' AND `Control_` = '{button}' AND `Event` = 'EndDialog'"), 1)?, vec![vec![result]]);
        }
        for dlg in ["Welcome", "Path", "Ready", "Maintenance", "Progress"] {
            assert_eq!(db.query(&format!("SELECT `Argument` FROM `ControlEvent` WHERE `Dialog_` = '{dlg}' AND `Control_` = 'Cancel' AND `Event` = 'SpawnDialog'"), 1)?, vec![vec!["ConfirmCancel"]]);
            assert!(db.query(&format!("SELECT `Argument` FROM `ControlEvent` WHERE `Dialog_` = '{dlg}' AND `Control_` = 'Cancel' AND `Event` = 'EndDialog'"), 1)?.is_empty());
        }
        assert_eq!(db.query("SELECT `Argument` FROM `ControlEvent` WHERE `Dialog_` = 'Maintenance' AND `Control_` = 'Cancel' AND `Event` = '[EMB_CANCEL_ACTION]' ORDER BY `Ordering`", 1)?, vec![vec!["유지보수"], vec!["복구"], vec!["제거"]]);
        assert_eq!(db.query("SELECT `Action` FROM `ControlCondition` WHERE `Dialog_` = 'Maintenance' AND `Control_` = 'Proceed'", 1)?.len(), 2);
        assert_eq!(
            repair,
            vec![
                vec!["[REMOVE]", "{}"],
                vec!["ReinstallMode", "amus"],
                vec!["Reinstall", "ALL"]
            ]
        );
        let remove = db.query("SELECT `Event`,`Argument` FROM `ControlEvent` WHERE `Dialog_` = 'RemoveReady' AND `Control_` = 'Proceed' AND `Condition` = 'MaintenanceChoice = \"Remove\"' ORDER BY `Ordering`",2)?;
        assert_eq!(
            remove,
            vec![vec!["[REINSTALL]", "{}"], vec!["[REMOVE]", "ALL"]]
        );
        assert_eq!(db.query("SELECT `Control` FROM `Control` WHERE `Dialog_` = 'Maintenance' AND `Type` = 'PushButton'",1)?.len(),2);
        assert_eq!(db.query("SELECT `Value` FROM `RadioButton` WHERE `Property` = 'MaintenanceChoice' ORDER BY `Order`",1)?,vec![vec!["Repair"],vec!["Remove"]]);
        let choice = db.query("SELECT `X`,`Y`,`Width`,`Height` FROM `Control` WHERE `Dialog_` = 'Maintenance' AND `Control` = 'Choice'", 4)?;
        let bounds: Vec<i32> = choice[0].iter().map(|v| v.parse().unwrap()).collect();
        for row in db.query("SELECT `Control`,`X`,`Y`,`Width`,`Height` FROM `Control` WHERE `Dialog_` = 'Maintenance' AND `Type` = 'Text'", 5)? {
            let rect: Vec<i32> = row[1..].iter().map(|v| v.parse().unwrap()).collect();
            assert!(rect[0] >= bounds[0] + bounds[2] || rect[0] + rect[2] <= bounds[0]
                || rect[1] >= bounds[1] + bounds[3] || rect[1] + rect[3] <= bounds[1],
                "radio group obscures {}", row[0]);
        }
        assert_eq!(db.query("SELECT `Text` FROM `Control` WHERE `Dialog_` = 'Maintenance' AND `Control` = 'Proceed'",1)?[0][0],"실행(&E)");
        assert_eq!(
            db.query(
                "SELECT `Value` FROM `Property` WHERE `Property` = 'BrandingText'",
                1
            )?[0][0],
            "Express MSI Builder"
        );
        assert_eq!(db.query("SELECT `Name` FROM `Binary`", 1)?.len(), 4);
        let art = db.query(
            "SELECT `Dialog_`,`Text`,`Width`,`Height` FROM `Control` WHERE `Type` = 'Bitmap'",
            4,
        )?;
        assert_eq!(art.len(), 12);
        for row in art {
            if row[1] == "RepairArt" || row[1] == "RemoveArt" {
                assert_eq!(row[0], "Maintenance");
                assert_eq!(&row[2..], ["32", "32"]);
                continue;
            }
            let expected = if matches!(row[0].as_str(), "Welcome" | "Cancelled" | "Complete") {
                ["WelcomeArt", "130", "240"]
            } else {
                ["BannerArt", "390", "52"]
            };
            assert_eq!(row[1..], expected);
        }
        let events = db.query("SELECT `Event`,`Argument` FROM `ControlEvent` WHERE `Dialog_` = 'Path' AND `Control_` = 'Next' ORDER BY `Ordering`",2)?;
        assert_eq!(
            events,
            vec![
                vec!["SetTargetPath", "INSTALLDIR"],
                vec!["NewDialog", "Ready"]
            ]
        );
        assert!(
            build(
                &source,
                &output,
                None,
                "Express MSI Builder",
                CabStorage::Embedded
            )
            .is_err(),
            "must not overwrite output"
        );
        assert_eq!(
            db.query(
                "SELECT `Text` FROM `Control` WHERE `Dialog_` = 'Complete' AND `Control` = 'Body'",
                1
            )?[0][0],
            "[BrandingText] 마법사가 [ProductName]을(를) 성공적으로 설치했습니다. 마법사를 종료하려면 \"마침\"을 눌러주세요."
        );
        assert_eq!(
            db.query(
                "SELECT `Value` FROM `Property` WHERE `Property` = 'ProductLanguage'",
                1
            )?[0][0],
            "1042"
        );
        drop(db);
        // The old database-only check passed this malformed package. The
        // Installer-session check must reject the exact original codepage bug.
        Database::restore_legacy_codepage_for_test(&output)?;
        assert!(Database::read(&output).is_ok());
        let failure = msi::verify_package(&output).unwrap_err().to_string();
        assert!(failure.contains("1620"), "unexpected failure: {failure}");
        Ok(())
    }
}
