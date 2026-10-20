use crate::{
    config::Product,
    msi::{
        Database, Result,
        Value::{Int as I, Null as N, Text as T},
    },
};

pub fn author(db: &Database, p: &Product) -> Result<()> {
    if !p.upgrade {
        return Ok(());
    }
    // AppSearch queries Signature even for raw registry searches with no file signature.
    db.execute("CREATE TABLE `Signature` (`Signature` CHAR(72) NOT NULL, `FileName` CHAR(255), `MinVersion` CHAR(20), `MaxVersion` CHAR(20), `MinSize` LONG, `MaxSize` LONG, `MinDate` LONG, `MaxDate` LONG, `Languages` CHAR(255) PRIMARY KEY `Signature`)")?;
    db.execute("CREATE TABLE `AppSearch` (`Property` CHAR(72) NOT NULL, `Signature_` CHAR(72) NOT NULL PRIMARY KEY `Property`, `Signature_`)")?;
    db.execute("CREATE TABLE `RegLocator` (`Signature_` CHAR(72) NOT NULL, `Root` SHORT NOT NULL, `Key` CHAR(255) NOT NULL, `Name` CHAR(255), `Type` SHORT PRIMARY KEY `Signature_`)")?;
    let state_key = format!(
        "Software\\InstallEmbly\\Packages\\{}\\{}",
        p.upgrade_code.trim_matches(['{', '}']),
        p.architecture
    );
    let state_guid = crate::payload::component_guid(std::path::Path::new(&format!(
        "upgrade-state|{}|{}",
        p.upgrade_code, p.architecture
    )))?;
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
            T("EMB_State"),
            T(&state_guid),
            T("INSTALLDIR"),
            I(if p.architecture != "x86" { 260 } else { 4 }),
            T("EMB_InstallLocation"),
        ],
    )?;
    db.insert(
        "FeatureComponents",
        &["Feature_", "Component_"],
        &[T("Main"), T("EMB_State")],
    )?;
    db.insert(
        "Registry",
        &["Registry", "Root", "Key", "Name", "Value", "Component_"],
        &[
            T("EMB_InstallLocation"),
            I(2),
            T(&state_key),
            T("InstallDir"),
            T("[INSTALLDIR]"),
            T("EMB_State"),
        ],
    )?;
    db.insert(
        "AppSearch",
        &["Property", "Signature_"],
        &[T("EMB_PREVIOUS_DIR"), T("EmbPreviousDir")],
    )?;
    db.insert(
        "RegLocator",
        &["Signature_", "Root", "Key", "Name", "Type"],
        &[
            T("EmbPreviousDir"),
            I(2),
            T(&state_key),
            T("InstallDir"),
            I(if p.architecture != "x86" { 18 } else { 2 }),
        ],
    )?;
    db.insert(
        "CustomAction",
        &["Action", "Type", "Source", "Target"],
        &[
            T("EmbUsePreviousDirectory"),
            I(51),
            T("INSTALLDIR"),
            T("[EMB_PREVIOUS_DIR]"),
        ],
    )?;
    db.execute("CREATE TABLE `Upgrade` (`UpgradeCode` CHAR(38) NOT NULL, `VersionMin` CHAR(20), `VersionMax` CHAR(20), `Language` CHAR(255), `Attributes` SHORT NOT NULL, `Remove` CHAR(255), `ActionProperty` CHAR(72) NOT NULL PRIMARY KEY `UpgradeCode`, `VersionMin`, `VersionMax`, `Language`, `Attributes`)")?;
    for (min, max, flags, property) in [
        (None, Some(p.version.as_str()), 0, "EMB_OLD_PRODUCTS"),
        (Some(p.version.as_str()), None, 2, "EMB_NEWER_PRODUCTS"),
        (
            Some(p.version.as_str()),
            Some(p.version.as_str()),
            770,
            "EMB_SAME_PRODUCTS",
        ),
    ] {
        db.insert(
            "Upgrade",
            &[
                "UpgradeCode",
                "VersionMin",
                "VersionMax",
                "Language",
                "Attributes",
                "Remove",
                "ActionProperty",
            ],
            &[
                T(&p.upgrade_code),
                min.map(T).unwrap_or(N),
                max.map(T).unwrap_or(N),
                N,
                I(flags),
                N,
                T(property),
            ],
        )?;
    }
    for (action, message) in [
        (
            "EmbBlockNewer",
            "더 높은 버전이 이미 설치되어 있습니다. 기존 버전을 유지합니다.",
        ),
        (
            "EmbBlockSame",
            "같은 버전의 다른 패키지가 이미 설치되어 있습니다. 기존 MSI로 복구하거나 제거한 뒤 설치하세요.",
        ),
    ] {
        db.insert(
            "CustomAction",
            &["Action", "Type", "Target"],
            &[T(action), I(19), T(message)],
        )?;
    }
    for table in ["InstallUISequence", "InstallExecuteSequence"] {
        db.insert(table,&["Action","Condition","Sequence"],&[T("EmbBlockRelocation"),T("NOT Installed AND EMB_OLD_PRODUCTS AND EMB_PREVIOUS_DIR AND NOT (INSTALLDIR ~= EMB_PREVIOUS_DIR)"),I(1100)])?;
        db.insert(table,&["Action","Condition","Sequence"],&[T("EmbBlockLayoutChange"),T("NOT Installed AND EMB_OLD_PRODUCTS AND (NOT EMB_PREVIOUS_LAYOUT OR EMB_PREVIOUS_LAYOUT <> EMB_EXPECTED_LAYOUT)"),I(80)])?;
        for (action, condition, sequence) in [
            ("FindRelatedProducts", "NOT Installed", 25),
            ("EmbBlockNewer", "NOT Installed AND EMB_NEWER_PRODUCTS", 50),
            ("EmbBlockSame", "NOT Installed AND EMB_SAME_PRODUCTS", 60),
            ("AppSearch", "NOT Installed", 70),
            (
                "EmbUsePreviousDirectory",
                "NOT Installed AND EMB_OLD_PRODUCTS AND EMB_PREVIOUS_DIR AND NOT INSTALLDIR",
                85,
            ),
        ] {
            db.insert(
                table,
                &["Action", "Condition", "Sequence"],
                &[T(action), T(condition), I(sequence)],
            )?;
        }
    }
    let layout = crate::msi::sha256(&format!(
        "{}|{}",
        p.architecture,
        p.install_parts()?.join("\\").to_uppercase()
    ))?
    .iter()
    .map(|v| format!("{v:02X}"))
    .collect::<String>();
    db.insert(
        "Property",
        &["Property", "Value"],
        &[T("EMB_EXPECTED_LAYOUT"), T(&layout)],
    )?;
    db.insert(
        "Registry",
        &["Registry", "Root", "Key", "Name", "Value", "Component_"],
        &[
            T("EMB_Layout"),
            I(2),
            T(&state_key),
            T("Layout"),
            T(&layout),
            T("EMB_State"),
        ],
    )?;
    db.insert(
        "AppSearch",
        &["Property", "Signature_"],
        &[T("EMB_PREVIOUS_LAYOUT"), T("EmbPreviousLayout")],
    )?;
    db.insert(
        "RegLocator",
        &["Signature_", "Root", "Key", "Name", "Type"],
        &[
            T("EmbPreviousLayout"),
            I(2),
            T(&state_key),
            T("Layout"),
            I(if p.architecture != "x86" { 18 } else { 2 }),
        ],
    )?;
    db.insert("CustomAction",&["Action","Type","Target"],&[T("EmbBlockLayoutChange"),I(19),T("기존 패키지의 업그레이드 정보가 없거나 아키텍처·설치 경로 구성이 변경되었습니다. 기존 버전을 제거한 뒤 설치하세요.")])?;
    db.insert("CustomAction",&["Action","Type","Target"],&[T("EmbBlockRelocation"),I(19),T("업그레이드 중에는 설치 위치를 변경할 수 없습니다. 기존 위치를 유지하거나 기존 버전을 제거한 뒤 설치하세요.")])?;
    // Install the new components first; remove only old, unshared resources within the transaction.
    db.insert(
        "InstallExecuteSequence",
        &["Action", "Condition", "Sequence"],
        &[
            T("InstallExecute"),
            T("EMB_OLD_PRODUCTS AND NOT Installed"),
            I(6500),
        ],
    )?;
    db.insert(
        "InstallExecuteSequence",
        &["Action", "Condition", "Sequence"],
        &[
            T("RemoveExistingProducts"),
            T("EMB_OLD_PRODUCTS AND NOT Installed"),
            I(6550),
        ],
    )?;
    if !p.actions.any() {
        crate::actions::error_dialog(db)?;
    }
    Ok(())
}
