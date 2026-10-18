//! Benign shortcut probe: displays its path, working directory and arguments.
#![windows_subsystem = "windows"]
#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(window: isize, text: *const u16, title: *const u16, flags: u32) -> i32;
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let text = format!(
        "바로가기 실행 시험\n\n실행 파일: {}\n\n작업 폴더: {}\n\n전달받은 인수:\n{args:#?}",
        std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
    let text: Vec<_> = text.encode_utf16().chain(Some(0)).collect();
    let title: Vec<_> = "InstallEmbly 바로가기 시험"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        MessageBoxW(0, text.as_ptr(), title.as_ptr(), 0x40);
    }
}
