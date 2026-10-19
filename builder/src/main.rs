#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use emb_msi::{msi, project::Project};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{Emitter, State};
#[derive(Default)]
struct AppState {
    document: Mutex<Option<Document>>,
    building: AtomicBool,
    output: Mutex<Option<PathBuf>>,
}
struct Document {
    path: PathBuf,
    text: String,
}
#[derive(Serialize)]
struct Draft {
    xml: String,
    path: Option<String>,
}
fn xml(p: &Project) -> Result<String, String> {
    String::from_utf8(p.xml().map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn idle(s: &AppState) -> Result<(), String> {
    if s.building.load(Ordering::SeqCst) {
        Err("빌드가 진행 중입니다.".into())
    } else {
        Ok(())
    }
}
#[tauri::command]
fn new_project(state: State<AppState>) -> Result<Draft, String> {
    idle(&state)?;
    let mut p = Project::new().map_err(|e| e.to_string())?;
    p.files = Some(Vec::new());
    *state.document.lock().unwrap() = None;
    Ok(Draft {
        xml: xml(&p)?,
        path: None,
    })
}
#[tauri::command]
async fn open_project(state: State<'_, AppState>) -> Result<Option<Draft>, String> {
    idle(&state)?;
    let path = tauri::async_runtime::spawn_blocking(|| {
        rfd::FileDialog::new()
            .add_filter("Rocket MSI 프로젝트", &["embproj"])
            .pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = path else { return Ok(None) };
    let p = Project::load(&path).map_err(|e| e.to_string())?;
    let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let result = Draft {
        xml: xml(&p)?,
        path: Some(path.display().to_string()),
    };
    *state.document.lock().unwrap() = Some(Document { path, text });
    Ok(Some(result))
}
#[tauri::command]
async fn pick_path(kind: String) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dialog = rfd::FileDialog::new();
        let result = match kind.as_str() {
            "folder" => dialog.pick_folder(),
            "license" => dialog.add_filter("이용약관", &["txt", "rtf"]).pick_file(),
            "exe" => dialog.add_filter("실행 파일", &["exe"]).pick_file(),
            _ => return Err("지원하지 않는 선택 항목입니다.".into()),
        };
        Ok(result.map(|p| p.display().to_string()))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[derive(Serialize)]
struct PayloadItem {
    source: String,
    target: String,
}
fn payload_items(folder: &Path, prefix: &Path) -> Result<Vec<PayloadItem>, String> {
    let files = emb_msi::payload::collect(folder, Path::new("")).map_err(|e| e.to_string())?;
    Ok(files
        .into_iter()
        .map(|f| PayloadItem {
            source: f.source.display().to_string(),
            target: prefix.join(f.relative).display().to_string(),
        })
        .collect())
}
#[tauri::command]
async fn pick_payload(kind: String) -> Result<Vec<PayloadItem>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if kind == "directory" {
            let Some(folder) = rfd::FileDialog::new().pick_folder() else {
                return Ok(Vec::new());
            };
            payload_items(
                &folder,
                Path::new(
                    folder
                        .file_name()
                        .ok_or("드라이브 대신 폴더를 선택하세요.")?,
                ),
            )
        } else if kind == "files" {
            let files = rfd::FileDialog::new().pick_files().unwrap_or_default();
            Ok(files
                .into_iter()
                .map(|p| PayloadItem {
                    target: p.file_name().unwrap().to_string_lossy().into_owned(),
                    source: p.display().to_string(),
                })
                .collect())
        } else {
            Err("지원하지 않는 항목입니다.".into())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn inspect_payload(
    xml_text: String,
    state: State<'_, AppState>,
) -> Result<Vec<PayloadItem>, String> {
    let base = state
        .document
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|d| d.path.parent().map(Path::to_owned));
    tauri::async_runtime::spawn_blocking(move || {
        let p = Project::parse(&xml_text).map_err(|e| e.to_string())?;
        if let Some(files) = p.files {
            return Ok(files
                .into_iter()
                .map(|(source, target)| PayloadItem { source, target })
                .collect());
        }
        let source = PathBuf::from(&p.source);
        let source = if source.is_absolute() {
            source
        } else {
            base.ok_or("기존 원본 폴더를 읽으려면 프로젝트를 먼저 저장하세요.")?
                .join(source)
        };
        payload_items(&source, Path::new(""))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn generate_guid() -> Result<String, String> {
    msi::guid().map_err(|e| e.to_string())
}
fn save_atomic(p: &Project, path: &Path, previous: Option<&str>) -> Result<String, String> {
    if path.exists() {
        let expected = previous.ok_or("파일이 이미 있습니다. 다른 이름을 선택하세요.")?;
        if fs::read_to_string(path).map_err(|e| e.to_string())? != expected {
            return Err(
                "다른 프로그램에서 파일을 변경했습니다. 다시 열거나 다른 이름으로 저장하세요."
                    .into(),
            );
        }
        let temp = path.with_file_name(format!(
            ".emb-save-{}.tmp",
            msi::guid().map_err(|e| e.to_string())?
        ));
        p.save_new(&temp).map_err(|e| e.to_string())?;
        if let Err(e) = fs::rename(&temp, path) {
            let _ = fs::remove_file(&temp);
            return Err(e.to_string());
        }
    } else {
        p.save_new(path).map_err(|e| e.to_string())?;
    }
    xml(p)
}
#[tauri::command]
async fn save_project(
    xml_text: String,
    save_as: bool,
    state: State<'_, AppState>,
) -> Result<Option<Draft>, String> {
    idle(&state)?;
    let mut p = Project::parse(&xml_text).map_err(|e| e.to_string())?;
    let old = state
        .document
        .lock()
        .unwrap()
        .as_ref()
        .map(|d| (d.path.clone(), d.text.clone()));
    let path = if save_as || old.is_none() {
        tauri::async_runtime::spawn_blocking(|| {
            rfd::FileDialog::new()
                .add_filter("Rocket MSI 프로젝트", &["embproj"])
                .set_file_name("MyProject.embproj")
                .save_file()
        })
        .await
        .map_err(|e| e.to_string())?
    } else {
        old.as_ref().map(|d| d.0.clone())
    };
    let Some(mut path) = path else {
        return Ok(None);
    };
    if path.extension().is_none() {
        path.set_extension("embproj");
    }
    if !path
        .extension()
        .is_some_and(|s| s.eq_ignore_ascii_case("embproj"))
    {
        return Err(".embproj 확장자를 사용하세요.".into());
    }
    if let Some((old_path, _)) = &old
        && old_path.parent() != path.parent()
    {
        let base = old_path.parent().unwrap();
        p.source = base.join(&p.source).display().to_string();
        if let Some(files) = &mut p.files {
            for (source, _) in files {
                *source = base.join(&*source).display().to_string();
            }
        }
        p.output_directory = base.join(&p.output_directory).display().to_string();
        p.license = p
            .license
            .as_ref()
            .map(|v| base.join(v).display().to_string());
    }
    let previous = old.as_ref().filter(|d| d.0 == path).map(|d| d.1.as_str());
    let text = save_atomic(&p, &path, previous)?;
    *state.document.lock().unwrap() = Some(Document {
        path: path.clone(),
        text: text.clone(),
    });
    Ok(Some(Draft {
        xml: text,
        path: Some(path.display().to_string()),
    }))
}
#[tauri::command]
fn validate_project(xml_text: String) -> Result<String, String> {
    let p = Project::parse(&xml_text).map_err(|e| e.to_string())?;
    Ok(format!(
        "{} {} · {} · 레지스트리 {}개 · 바로가기 {}개 · 설치 실행 {}개 · 제거 실행 {}개",
        p.product.name,
        p.product.version,
        p.product.architecture,
        p.product.registry.len(),
        p.product.shortcuts.len(),
        p.product.actions.install.len(),
        p.product.actions.uninstall.len()
    ))
}
#[tauri::command]
async fn build_project(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    if state.building.swap(true, Ordering::SeqCst) {
        return Err("이미 빌드 중입니다.".into());
    }
    struct Reset<'a>(&'a AtomicBool);
    impl Drop for Reset<'_> {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }
    let _reset = Reset(&state.building);
    let (path, text) = state
        .document
        .lock()
        .unwrap()
        .as_ref()
        .map(|d| (d.path.clone(), d.text.clone()))
        .ok_or("프로젝트를 먼저 저장하세요.")?;
    let worker = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<PathBuf, String> {
        if fs::read_to_string(&path).map_err(|e| e.to_string())? != text {
            return Err("프로젝트 파일이 외부에서 변경되었습니다. 다시 열어주세요.".into());
        }
        worker
            .emit(
                "build-progress",
                "프로젝트 검증 완료 · CAB 및 MSI를 생성합니다.",
            )
            .ok();
        let p = Project::load(&path).map_err(|e| e.to_string())?;
        p.build(&path).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?;
    match result {
        Ok(output) => {
            let clean = output
                .display()
                .to_string()
                .trim_start_matches("\\\\?\\")
                .to_owned();
            *state.output.lock().unwrap() = PathBuf::from(&clean).parent().map(Path::to_owned);
            app.emit("build-progress", format!("빌드 완료 · {clean}"))
                .ok();
            Ok(clean)
        }
        Err(e) => {
            app.emit("build-progress", format!("빌드 실패 · {e}")).ok();
            Err(e)
        }
    }
}
#[tauri::command]
fn open_output(state: State<AppState>) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    let path = state
        .output
        .lock()
        .unwrap()
        .clone()
        .ok_or("먼저 MSI를 빌드하세요.")?;
    let exe =
        PathBuf::from(std::env::var_os("SystemRoot").ok_or("Windows 경로를 찾을 수 없습니다.")?)
            .join("explorer.exe");
    std::process::Command::new(exe)
        .arg(path)
        .creation_flags(0x08000000)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
fn main() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            new_project,
            open_project,
            pick_path,
            pick_payload,
            inspect_payload,
            generate_guid,
            save_project,
            validate_project,
            build_project,
            open_output
        ])
        .run(tauri::generate_context!())
        .expect("start Express MSI Builder");
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_import_keeps_nested_names_and_sources() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("import-test-{}", msi::guid().unwrap()));
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("one.txt"), "one").unwrap();
        fs::write(root.join("nested/two.txt"), "two").unwrap();
        let items = payload_items(&root, Path::new("Imported")).unwrap();
        assert_eq!(items.len(), 2);
        assert!(
            items
                .iter()
                .any(|f| f.target == "Imported\\one.txt" && Path::new(&f.source).is_file())
        );
        assert!(
            items
                .iter()
                .any(|f| f.target == "Imported\\nested\\two.txt" && Path::new(&f.source).is_file())
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn save_protects_external_changes_and_replaces_known_document() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("save-test-{}", msi::guid().unwrap()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("sample.embproj");
        let mut p = Project::new().unwrap();
        let first = save_atomic(&p, &path, None).unwrap();
        p.branding = "Changed".into();
        assert!(save_atomic(&p, &path, None).is_err());
        let second = save_atomic(&p, &path, Some(&first)).unwrap();
        assert!(second.contains("Changed"));
        fs::write(&path, "external edit").unwrap();
        assert!(save_atomic(&p, &path, Some(&second)).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "external edit");
        fs::remove_dir_all(root).unwrap();
    }
}
