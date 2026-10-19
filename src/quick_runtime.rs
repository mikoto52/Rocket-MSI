//! Self-contained Windows Installer embedded UI. Built for x86 and x64 without a web runtime.
#![allow(non_snake_case)]
mod quick_policy;
use std::{
    ffi::c_void,
    ptr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicIsize, Ordering},
        mpsc,
    },
    thread,
};
type H = *mut c_void;
#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegGetValueW(
        key: H,
        subkey: *const u16,
        value: *const u16,
        flags: u32,
        kind: *mut u32,
        data: *mut c_void,
        size: *mut u32,
    ) -> i32;
}
fn system_dark() -> bool {
    let mut light = 1u32;
    let mut size = 4u32;
    unsafe {
        RegGetValueW(
            (-2147483647isize) as H,
            wide(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize").as_ptr(),
            wide("AppsUseLightTheme").as_ptr(),
            0x10,
            ptr::null_mut(),
            &mut light as *mut _ as *mut c_void,
            &mut size,
        ) == 0
            && light == 0
    }
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct MouseTrack {
    size: u32,
    flags: u32,
    window: H,
    time: u32,
}
#[repr(C)]
struct MonitorInfo {
    size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}
#[repr(C)]
struct MinMaxInfo {
    reserved: Point,
    max_size: Point,
    max_position: Point,
    min_track: Point,
    max_track: Point,
}
#[repr(C)]
struct Margins {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}
#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmExtendFrameIntoClientArea(w: H, margins: *const Margins) -> i32;
    fn DwmSetWindowAttribute(w: H, attribute: u32, value: *const c_void, size: u32) -> i32;
}
#[repr(C)]
struct Msg {
    hwnd: H,
    msg: u32,
    wp: usize,
    lp: isize,
    time: u32,
    point: Point,
    private: u32,
}
#[repr(C)]
struct Paint {
    dc: H,
    erase: i32,
    rect: Rect,
    restore: i32,
    update: i32,
    reserved: [u8; 32],
}
#[repr(C)]
struct Class {
    style: u32,
    proc: Option<unsafe extern "system" fn(H, u32, usize, isize) -> isize>,
    extra: i32,
    window_extra: i32,
    instance: H,
    icon: H,
    cursor: H,
    background: H,
    menu: *const u16,
    name: *const u16,
}
#[repr(C)]
struct DrawItem {
    kind: u32,
    id: u32,
    item: u32,
    action: u32,
    state: u32,
    window: H,
    dc: H,
    rect: Rect,
    data: usize,
}
#[link(name = "user32")]
unsafe extern "system" {
    fn RegisterClassW(class: *const Class) -> u16;
    fn UnregisterClassW(name: *const u16, instance: H) -> i32;
    fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: H,
        menu: H,
        instance: H,
        param: H,
    ) -> H;
    fn DefWindowProcW(w: H, m: u32, wp: usize, lp: isize) -> isize;
    fn DestroyWindow(w: H) -> i32;
    fn GetMessageW(msg: *mut Msg, w: H, min: u32, max: u32) -> i32;
    fn TranslateMessage(msg: *const Msg) -> i32;
    fn DispatchMessageW(msg: *const Msg) -> isize;
    fn PostQuitMessage(code: i32);
    fn ShowWindow(w: H, show: i32) -> i32;
    fn SetWindowTextW(w: H, text: *const u16) -> i32;
    fn EnableWindow(w: H, enabled: i32) -> i32;
    fn MoveWindow(w: H, x: i32, y: i32, width: i32, height: i32, repaint: i32) -> i32;
    fn SetTimer(w: H, id: usize, ms: u32, proc: H) -> usize;
    fn GetClientRect(w: H, rect: *mut Rect) -> i32;
    fn BeginPaint(w: H, paint: *mut Paint) -> H;
    fn EndPaint(w: H, paint: *const Paint) -> i32;
    fn FillRect(dc: H, rect: *const Rect, brush: H) -> i32;
    fn DrawTextW(dc: H, text: *const u16, count: i32, rect: *mut Rect, flags: u32) -> i32;
    fn DrawFocusRect(dc: H, rect: *const Rect) -> i32;
    fn InvalidateRect(w: H, rect: *const Rect, erase: i32) -> i32;
    fn LoadCursorW(instance: H, name: *const u16) -> H;
    fn MessageBoxW(w: H, text: *const u16, title: *const u16, flags: u32) -> i32;
    #[cfg(target_pointer_width = "64")]
    fn GetWindowLongPtrW(w: H, index: i32) -> isize;
    #[cfg(target_pointer_width = "64")]
    fn SetWindowLongPtrW(w: H, index: i32, value: isize) -> isize;
    fn GetDpiForWindow(w: H) -> u32;
    fn SendMessageW(w: H, msg: u32, wp: usize, lp: isize) -> isize;
    fn IsDialogMessageW(w: H, msg: *mut Msg) -> i32;
    fn SetFocus(w: H) -> H;
    fn GetFocus() -> H;
    fn GetWindowRect(w: H, rect: *mut Rect) -> i32;
    fn GetSystemMetrics(index: i32) -> i32;
    fn GetParent(w: H) -> H;
    fn TrackMouseEvent(track: *mut MouseTrack) -> i32;
    fn IsZoomed(w: H) -> i32;
    fn MonitorFromWindow(w: H, flags: u32) -> H;
    fn GetMonitorInfoW(monitor: H, info: *mut MonitorInfo) -> i32;
    fn SetWindowPos(w: H, after: H, x: i32, y: i32, cx: i32, cy: i32, flags: u32) -> i32;
    fn ScreenToClient(w: H, point: *mut Point) -> i32;
    fn CallWindowProcW(proc: isize, w: H, msg: u32, wp: usize, lp: isize) -> isize;
}
// On 32-bit Windows these APIs are exported under their non-Ptr names.
#[cfg(target_pointer_width = "32")]
#[link(name = "user32")]
unsafe extern "system" {
    #[link_name = "GetWindowLongW"]
    fn get_data(w: H, index: i32) -> isize;
    #[link_name = "SetWindowLongW"]
    fn set_data(w: H, index: i32, value: isize) -> isize;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateSolidBrush(color: u32) -> H;
    fn DeleteObject(object: H) -> i32;
    fn SelectObject(dc: H, object: H) -> H;
    fn GetStockObject(index: i32) -> H;
    fn Polygon(dc: H, points: *const Point, n: i32) -> i32;
    fn RoundRect(
        dc: H,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        width: i32,
        height: i32,
    ) -> i32;
    fn CreatePen(style: i32, width: i32, color: u32) -> H;
    fn MoveToEx(dc: H, x: i32, y: i32, old: *mut Point) -> i32;
    fn LineTo(dc: H, x: i32, y: i32) -> i32;
    fn CreateCompatibleDC(dc: H) -> H;
    fn CreateCompatibleBitmap(dc: H, width: i32, height: i32) -> H;
    fn DeleteDC(dc: H) -> i32;
    fn BitBlt(
        dest: H,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        source: H,
        sx: i32,
        sy: i32,
        operation: u32,
    ) -> i32;
    fn SetTextColor(dc: H, color: u32) -> u32;
    fn SetBkMode(dc: H, mode: i32) -> i32;
    fn CreateFontW(
        height: i32,
        width: i32,
        escape: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike: u32,
        charset: u32,
        out: u32,
        clip: u32,
        quality: u32,
        family: u32,
        face: *const u16,
    ) -> H;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> H;
}
#[link(name = "msi")]
unsafe extern "system" {
    fn MsiGetPropertyW(h: u32, name: *const u16, value: *mut u16, len: *mut u32) -> u32;
    fn MsiSetPropertyW(h: u32, name: *const u16, value: *const u16) -> u32;
    fn MsiQueryProductStateW(code: *const u16) -> i32;
    fn MsiRecordGetInteger(h: u32, field: u32) -> i32;
    fn MsiRecordGetStringW(h: u32, field: u32, value: *mut u16, len: *mut u32) -> u32;
    fn MsiFormatRecordW(h: u32, record: u32, value: *mut u16, len: *mut u32) -> u32;
    fn MsiGetComponentPathW(
        product: *const u16,
        component: *const u16,
        path: *mut u16,
        len: *mut u32,
    ) -> i32;
    fn MsiEnumRelatedProductsW(
        upgrade: *const u16,
        reserved: u32,
        index: u32,
        code: *mut u16,
    ) -> u32;
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn property(h: u32, name: &str) -> String {
    unsafe {
        let mut n = 0;
        MsiGetPropertyW(h, wide(name).as_ptr(), ptr::null_mut(), &mut n);
        let mut b = vec![0; n as usize + 1];
        n = b.len() as u32;
        if MsiGetPropertyW(h, wide(name).as_ptr(), b.as_mut_ptr(), &mut n) != 0 {
            return String::new();
        }
        String::from_utf16_lossy(&b[..n as usize])
    }
}
fn record(h: u32, field: u32) -> String {
    unsafe {
        let mut n = 0;
        MsiRecordGetStringW(h, field, ptr::null_mut(), &mut n);
        let mut b = vec![0; n as usize + 1];
        n = b.len() as u32;
        if MsiRecordGetStringW(h, field, b.as_mut_ptr(), &mut n) != 0 {
            return String::new();
        }
        String::from_utf16_lossy(&b[..n as usize])
    }
}
fn formatted(h: u32) -> String {
    unsafe {
        let mut n = 0;
        MsiFormatRecordW(0, h, ptr::null_mut(), &mut n);
        let mut b = vec![0; n as usize + 1];
        n = b.len() as u32;
        if MsiFormatRecordW(0, h, b.as_mut_ptr(), &mut n) != 0 {
            return record(h, 0);
        }
        String::from_utf16_lossy(&b[..n as usize])
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Choice {
    Install,
    Repair,
    Remove,
    Cancel,
}
impl Choice {
    fn label(self) -> &'static str {
        match self {
            Self::Install => "설치",
            Self::Repair => "복구",
            Self::Remove => "제거",
            Self::Cancel => "유지보수",
        }
    }
}
#[derive(Clone, PartialEq)]
struct Launch {
    product: String,
    component: String,
    arguments: Vec<String>,
}
impl Launch {
    fn run(&self) -> Result<(), String> {
        unsafe {
            let mut size = 32768;
            let mut path = vec![0u16; size as usize];
            if MsiGetComponentPathW(
                wide(&self.product).as_ptr(),
                wide(&self.component).as_ptr(),
                path.as_mut_ptr(),
                &mut size,
            ) != 3
            {
                return Err("설치된 실행 파일의 위치를 찾을 수 없습니다.".into());
            }
            let path = std::path::PathBuf::from(String::from_utf16_lossy(&path[..size as usize]));
            if !path.is_file() {
                return Err("설치된 실행 파일이 없습니다.".into());
            }
            std::process::Command::new(&path)
                .args(&self.arguments)
                .current_dir(path.parent().ok_or("작업 폴더가 없습니다.")?)
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("프로그램을 실행하지 못했습니다: {e}"))
        }
    }
}
#[derive(Clone, PartialEq)]
struct Model {
    name: String,
    publisher: String,
    version: String,
    branding: String,
    installed: bool,
    action: Choice,
    phase: u8,
    status: String,
    result: Option<i32>,
    total: i64,
    ticks: i64,
    reverse: bool,
    step: i64,
    action_data: bool,
    preview: bool,
    launch: Option<Launch>,
    launch_checked: bool,
    auto_attempted: bool,
}
struct Session {
    model: Arc<Mutex<Model>>,
    cancel: Arc<AtomicBool>,
    window: Arc<AtomicIsize>,
    thread: thread::JoinHandle<()>,
}
static SESSION: Mutex<Option<Session>> = Mutex::new(None);
struct Window {
    dark: bool,
    model: Arc<Mutex<Model>>,
    cancel: Arc<AtomicBool>,
    window: Arc<AtomicIsize>,
    sender: Option<mpsc::Sender<Choice>>,
    primary: H,
    secondary: H,
    third: H,
    checkbox: H,
    minimize: H,
    close: H,
    checkbox_proc: isize,
    minimize_proc: isize,
    close_proc: isize,
    hover_caption: H,
    font: H,
    heading: H,
    dpi: i32,
    last_phase: u8,
    last_model: Option<Model>,
}
impl Window {
    fn color(&self, light: u32, dark: u32) -> u32 {
        if self.dark { dark } else { light }
    }
    fn background(&self) -> u32 {
        self.color(rgb(255, 255, 255), rgb(0, 0, 0))
    }
    fn foreground(&self) -> u32 {
        self.color(rgb(65, 75, 95), rgb(255, 255, 255))
    }
}
unsafe fn data(w: H) -> *mut Window {
    #[cfg(target_pointer_width = "64")]
    {
        unsafe { GetWindowLongPtrW(w, -21) as *mut Window }
    }
    #[cfg(target_pointer_width = "32")]
    {
        unsafe { get_data(w, -21) as *mut Window }
    }
}
unsafe fn attach(w: H, p: *mut Window) {
    #[cfg(target_pointer_width = "64")]
    {
        unsafe {
            SetWindowLongPtrW(w, -21, p as isize);
        }
    }
    #[cfg(target_pointer_width = "32")]
    {
        unsafe {
            set_data(w, -21, p as isize);
        }
    }
}
fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}
unsafe fn fill(dc: H, r: Rect, color: u32) {
    unsafe {
        let brush = CreateSolidBrush(color);
        FillRect(dc, &r, brush);
        DeleteObject(brush);
    }
}
unsafe fn rounded(dc: H, r: Rect, color: u32, radius: i32, background: u32) {
    unsafe {
        fill(dc, r, background);
        let brush = CreateSolidBrush(color);
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, GetStockObject(8));
        RoundRect(dc, r.left, r.top, r.right, r.bottom, radius * 2, radius * 2);
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(brush);
    }
}
unsafe fn paint_checkbox(w: H, u: &Window, dc: H) {
    unsafe {
        let mut r = Rect::default();
        GetClientRect(w, &mut r);
        fill(dc, r, u.background());
        let s = |n: i32| n * u.dpi / 96;
        let checked = SendMessageW(w, 0x00f0, 0, 0) == 1;
        let y = (r.bottom - s(20)) / 2;
        let brush = CreateSolidBrush(if checked {
            rgb(80, 82, 200)
        } else {
            u.background()
        });
        let pen = CreatePen(
            0,
            s(1).max(1),
            if checked {
                rgb(80, 82, 200)
            } else {
                rgb(163, 173, 188)
            },
        );
        let old_brush = SelectObject(dc, brush);
        let old_pen = SelectObject(dc, pen);
        RoundRect(dc, 0, y, s(20), y + s(20), s(7), s(7));
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(pen);
        DeleteObject(brush);
        if checked {
            let pen = CreatePen(0, s(2).max(2), rgb(255, 255, 255));
            let old = SelectObject(dc, pen);
            MoveToEx(dc, s(5), y + s(10), ptr::null_mut());
            LineTo(dc, s(9), y + s(14));
            LineTo(dc, s(16), y + s(6));
            SelectObject(dc, old);
            DeleteObject(pen);
        }
        let label = Rect {
            left: s(30),
            top: 0,
            right: r.right,
            bottom: r.bottom,
        };
        text(dc, u.font, "설치 후 실행", label, u.foreground(), 0x24);
        if GetFocus() == w {
            DrawFocusRect(dc, &label);
        }
    }
}
unsafe extern "system" fn checkbox_proc(w: H, msg: u32, wp: usize, lp: isize) -> isize {
    unsafe {
        let p = data(GetParent(w));
        if p.is_null() {
            return DefWindowProcW(w, msg, wp, lp);
        }
        let u = &*p;
        if msg == 0x000f {
            let mut paint: Paint = std::mem::zeroed();
            let dc = BeginPaint(w, &mut paint);
            paint_checkbox(w, u, dc);
            EndPaint(w, &paint);
            return 0;
        }
        if msg == 0x0318 {
            paint_checkbox(w, u, wp as H);
            return 0;
        }
        // Retain the native checkbox's check state, keyboard and accessibility behavior.
        CallWindowProcW(u.checkbox_proc, w, msg, wp, lp)
    }
}
unsafe fn text(dc: H, font: H, s: &str, r: Rect, color: u32, flags: u32) {
    unsafe {
        let old = SelectObject(dc, font);
        SetTextColor(dc, color);
        SetBkMode(dc, 1);
        let mut r = r;
        DrawTextW(dc, wide(s).as_ptr(), -1, &mut r, flags | 0x800);
        SelectObject(dc, old);
    }
}
unsafe fn layout(w: H, u: &mut Window) {
    unsafe {
        let mut r = Rect::default();
        GetClientRect(w, &mut r);
        let s = |n: i32| n * u.dpi / 96;
        let (maintenance, phase) = {
            let m = u.model.lock().unwrap();
            (m.installed && m.phase == 0, m.phase)
        };
        MoveWindow(
            u.primary,
            r.right - s(if maintenance { 328 } else { 176 }),
            r.bottom - s(86),
            s(140),
            s(44),
            1,
        );
        MoveWindow(
            u.secondary,
            r.right
                - s(if maintenance {
                    480
                } else if phase == 1 {
                    176
                } else {
                    328
                }),
            r.bottom - s(86),
            s(140),
            s(44),
            1,
        );
        MoveWindow(
            u.third,
            r.right - s(176),
            r.bottom - s(86),
            s(140),
            s(44),
            1,
        );
        MoveWindow(u.checkbox, s(36), r.bottom - s(82), s(245), s(30), 1);
        MoveWindow(u.close, r.right - s(46), 0, s(46), s(36), 1);
        MoveWindow(u.minimize, r.right - s(92), 0, s(46), s(36), 1);
    }
}
unsafe fn refresh(w: H, u: &mut Window) {
    unsafe {
        let snapshot = u.model.lock().unwrap().clone();
        if u.last_model.as_ref() == Some(&snapshot) {
            return;
        }
        u.last_model = Some(snapshot);
        let (phase, installed, can_launch, has_launch, auto) = {
            let m = u.model.lock().unwrap();
            let available = m.phase == 2
                && quick_policy::launch_available(
                    m.result,
                    !m.installed && m.launch.is_some(),
                    m.action == Choice::Install,
                );
            (
                m.phase,
                m.installed,
                available,
                m.launch.is_some(),
                quick_policy::auto_launch(available, m.launch_checked, m.auto_attempted),
            )
        };
        if phase != u.last_phase {
            let primary = if phase == 2 {
                "완료"
            } else if phase == 1 {
                "진행 중"
            } else if installed {
                "복구"
            } else {
                "설치"
            };
            SetWindowTextW(u.primary, wide(primary).as_ptr());
            SetWindowTextW(
                u.secondary,
                wide(if can_launch {
                    "실행"
                } else if phase == 0 && installed {
                    "삭제"
                } else {
                    "취소"
                })
                .as_ptr(),
            );
            EnableWindow(u.primary, (phase != 1) as i32);
            ShowWindow(u.primary, if phase == 1 { 0 } else { 5 });
            ShowWindow(
                u.secondary,
                if (phase == 0 && !installed) || (phase == 2 && !can_launch) {
                    0
                } else {
                    5
                },
            );
            ShowWindow(u.third, if phase == 0 && installed { 5 } else { 0 });
            ShowWindow(u.checkbox, if phase == 0 && has_launch { 5 } else { 0 });
            u.last_phase = phase;
            layout(w, u);
        }
        if auto {
            let launch = {
                let mut m = u.model.lock().unwrap();
                m.auto_attempted = true;
                m.launch.clone()
            };
            if let Some(launch) = launch {
                match launch.run() {
                    Ok(()) => {
                        DestroyWindow(w);
                        return;
                    }
                    Err(error) => {
                        u.model.lock().unwrap().status = error;
                    }
                }
            }
        }
        InvalidateRect(w, ptr::null(), 0);
    }
}
unsafe fn paint(w: H, u: &Window) {
    unsafe {
        let mut p: Paint = std::mem::zeroed();
        let target = BeginPaint(w, &mut p);
        let mut r = Rect::default();
        GetClientRect(w, &mut r);
        let buffer = CreateCompatibleDC(target);
        let bitmap = CreateCompatibleBitmap(target, r.right.max(1), r.bottom.max(1));
        let buffered = !buffer.is_null() && !bitmap.is_null();
        let old_bitmap = if buffered {
            SelectObject(buffer, bitmap)
        } else {
            ptr::null_mut()
        };
        let dc = if buffered { buffer } else { target };
        fill(dc, r, u.background());
        let s = |n: i32| n * u.dpi / 96;
        let m = u.model.lock().unwrap();
        let title = if m.phase == 2 {
            match m.result {
                Some(0 | 3010 | 1641) => format!("{} {} 완료", m.name, m.action.label()),
                Some(1602) => "작업이 취소되었습니다.".into(),
                _ => "작업 결과 확인".into(),
            }
        } else if m.phase == 0 {
            format!("{} 설치", m.name)
        } else {
            format!("{} {} 중", m.name, m.action.label())
        };
        text(
            dc,
            u.heading,
            &title,
            Rect {
                left: s(36),
                top: s(58),
                right: r.right - s(170),
                bottom: s(102),
            },
            u.color(rgb(25, 32, 43), rgb(255, 255, 255)),
            0x10,
        );
        text(
            dc,
            u.font,
            &format!("Publisher: {}\nVersion: {}", m.publisher, m.version),
            Rect {
                left: s(36),
                top: s(108),
                right: r.right - s(170),
                bottom: s(158),
            },
            u.color(rgb(75, 85, 100), rgb(225, 225, 225)),
            0x10,
        );
        let x = r.right - s(130);
        let y = s(86);
        let z = s(44);
        for (points, color) in [
            (
                [
                    Point { x, y: y - z / 2 },
                    Point { x: x + z, y },
                    Point { x, y: y + z / 2 },
                    Point { x: x - z, y },
                ],
                rgb(90, 223, 202),
            ),
            (
                [
                    Point { x: x - z, y },
                    Point { x, y: y + z / 2 },
                    Point {
                        x,
                        y: y + z * 3 / 2,
                    },
                    Point { x: x - z, y: y + z },
                ],
                rgb(41, 115, 155),
            ),
            (
                [
                    Point { x, y: y + z / 2 },
                    Point { x: x + z, y },
                    Point { x: x + z, y: y + z },
                    Point {
                        x,
                        y: y + z * 3 / 2,
                    },
                ],
                rgb(25, 77, 119),
            ),
        ] {
            let brush = CreateSolidBrush(color);
            let old = SelectObject(dc, brush);
            let pen = SelectObject(dc, GetStockObject(8));
            Polygon(dc, points.as_ptr(), 4);
            SelectObject(dc, pen);
            SelectObject(dc, old);
            DeleteObject(brush);
        }
        text(
            dc,
            u.font,
            &m.status,
            Rect {
                left: s(36),
                top: r.bottom - s(185),
                right: r.right - s(36),
                bottom: r.bottom - s(130),
            },
            u.color(rgb(85, 96, 113), rgb(225, 225, 225)),
            0x10,
        );
        if m.phase == 1 {
            let bar = Rect {
                left: s(36),
                top: r.bottom - s(118),
                right: r.right - s(36),
                bottom: r.bottom - s(112),
            };
            fill(dc, bar, u.color(rgb(230, 234, 242), rgb(55, 55, 55)));
            let percent = if m.total > 0 {
                (m.ticks * 100 / m.total).clamp(0, 100)
            } else {
                0
            };
            fill(
                dc,
                Rect {
                    right: bar.left + ((bar.right - bar.left) as i64 * percent / 100) as i32,
                    ..bar
                },
                rgb(80, 82, 200),
            );
        }
        text(
            dc,
            u.font,
            &m.branding,
            Rect {
                left: s(36),
                top: r.bottom - s(29),
                right: r.right - s(36),
                bottom: r.bottom - s(6),
            },
            rgb(132, 140, 153),
            0,
        );
        if buffered {
            BitBlt(target, 0, 0, r.right, r.bottom, dc, 0, 0, 0x00cc0020);
            SelectObject(buffer, old_bitmap);
        }
        if !bitmap.is_null() {
            DeleteObject(bitmap);
        }
        if !buffer.is_null() {
            DeleteDC(buffer);
        }
        EndPaint(w, &p);
    }
}
unsafe fn apply_frame(w: H) {
    unsafe {
        let enabled = 2i32;
        let dark = system_dark() as i32;
        DwmSetWindowAttribute(w, 20, &dark as *const _ as *const c_void, 4);
        DwmSetWindowAttribute(w, 2, &enabled as *const _ as *const c_void, 4);
        DwmExtendFrameIntoClientArea(
            w,
            &Margins {
                left: 1,
                right: 1,
                top: 1,
                bottom: 1,
            },
        );
        // Rounded corners are optional on Windows 11; Windows 10 ignores this attribute.
        DwmSetWindowAttribute(w, 33, &enabled as *const _ as *const c_void, 4);
    }
}
unsafe extern "system" fn caption_proc(w: H, msg: u32, wp: usize, lp: isize) -> isize {
    unsafe {
        let p = data(GetParent(w));
        if p.is_null() {
            return DefWindowProcW(w, msg, wp, lp);
        }
        let u = &mut *p;
        if msg == 0x0200 {
            if u.hover_caption != w {
                let previous = u.hover_caption;
                u.hover_caption = w;
                if !previous.is_null() {
                    InvalidateRect(previous, ptr::null(), 0);
                }
                InvalidateRect(w, ptr::null(), 0);
            }
            TrackMouseEvent(&mut MouseTrack {
                size: std::mem::size_of::<MouseTrack>() as u32,
                flags: 2,
                window: w,
                time: 0,
            });
        } else if msg == 0x02a3 && u.hover_caption == w {
            u.hover_caption = ptr::null_mut();
            InvalidateRect(w, ptr::null(), 0);
        }
        let original = if w == u.close {
            u.close_proc
        } else {
            u.minimize_proc
        };
        CallWindowProcW(original, w, msg, wp, lp)
    }
}
unsafe extern "system" fn wndproc(w: H, msg: u32, wp: usize, lp: isize) -> isize {
    unsafe {
        // Retain the standard overlapped-window styles for DWM shadow and Aero Snap,
        // while replacing the non-client caption with our own controls.
        if msg == 0x0083 && wp != 0 {
            if IsZoomed(w) != 0 {
                let mut info: MonitorInfo = std::mem::zeroed();
                info.size = std::mem::size_of::<MonitorInfo>() as u32;
                if GetMonitorInfoW(MonitorFromWindow(w, 2), &mut info) != 0 {
                    *(lp as *mut Rect) = info.work;
                }
            }
            return 0;
        }
        if msg == 0x0006 || msg == 0x031e {
            apply_frame(w);
        }
        let p = data(w);
        if p.is_null() {
            return DefWindowProcW(w, msg, wp, lp);
        }
        let u = &mut *p;
        match msg {
            // Keep WS_THICKFRAME for DWM, but reject interactive size/maximize commands.
            0x0112 if matches!(wp & 0xfff0, 0xf000 | 0xf030) => 0,
            0x00a3 => 0,
            0x001a | 0x031a => {
                u.dark = system_dark();
                apply_frame(w);
                InvalidateRect(w, ptr::null(), 0);
                for child in [
                    u.primary,
                    u.secondary,
                    u.third,
                    u.checkbox,
                    u.minimize,
                    u.close,
                ] {
                    InvalidateRect(child, ptr::null(), 0);
                }
                0
            }
            0x0024 => {
                let info = &mut *(lp as *mut MinMaxInfo);
                info.min_track = Point {
                    x: 760 * u.dpi / 96,
                    y: 480 * u.dpi / 96,
                };
                info.max_track = Point {
                    x: info.min_track.x,
                    y: info.min_track.y,
                };
                0
            }
            0x0014 => 1, // Double-buffered WM_PAINT already covers the background.
            0x0084 => {
                let mut point = Point {
                    x: (lp as i16) as i32,
                    y: ((lp >> 16) as i16) as i32,
                };
                ScreenToClient(w, &mut point);
                let mut r = Rect::default();
                GetClientRect(w, &mut r);
                if point.y >= 0 && point.y < 36 * u.dpi / 96 && point.x < r.right - 92 * u.dpi / 96
                {
                    2
                } else {
                    1
                }
            }
            0x0005 => {
                layout(w, u);
                0
            }
            0x000f => {
                paint(w, u);
                0
            }
            0x0113 => {
                refresh(w, u);
                0
            }
            0x0135 | 0x0138 => {
                SetBkMode(wp as H, 1);
                SetTextColor(wp as H, u.foreground());
                GetStockObject(if u.dark { 4 } else { 0 }) as isize
            }
            0x002b => {
                let d = &*(lp as *const DrawItem);
                if d.id == 201 || d.id == 202 {
                    let pressed = d.state & 1 != 0 || u.hover_caption == d.window;
                    fill(
                        d.dc,
                        d.rect,
                        if pressed {
                            if d.id == 202 {
                                rgb(220, 65, 65)
                            } else {
                                u.color(rgb(234, 237, 242), rgb(55, 55, 55))
                            }
                        } else {
                            u.background()
                        },
                    );
                    text(
                        d.dc,
                        u.font,
                        if d.id == 202 { "×" } else { "−" },
                        d.rect,
                        if pressed && d.id == 202 {
                            rgb(255, 255, 255)
                        } else {
                            u.color(rgb(90, 100, 115), rgb(255, 255, 255))
                        },
                        0x25,
                    );
                    return 1;
                }
                let primary = d.id == 101;
                let disabled = d.state & 4 != 0;
                rounded(
                    d.dc,
                    d.rect,
                    if primary {
                        if disabled {
                            rgb(170, 174, 207)
                        } else {
                            rgb(80, 82, 200)
                        }
                    } else {
                        u.color(rgb(243, 245, 249), rgb(45, 45, 45))
                    },
                    5 * u.dpi / 96,
                    u.background(),
                );
                let m = u.model.lock().unwrap();
                let caption = if primary {
                    if m.phase == 2 {
                        "완료"
                    } else if m.phase == 1 {
                        "진행 중"
                    } else if m.installed {
                        "복구"
                    } else {
                        "설치"
                    }
                } else if d.id == 104 {
                    "취소"
                } else if m.phase == 2 && m.launch.is_some() && matches!(m.result, Some(0 | 3010)) {
                    "실행"
                } else if m.phase == 0 && m.installed {
                    "삭제"
                } else {
                    "취소"
                };
                text(
                    d.dc,
                    u.font,
                    caption,
                    d.rect,
                    if primary {
                        rgb(255, 255, 255)
                    } else {
                        u.foreground()
                    },
                    0x25,
                );
                if d.state & 16 != 0 {
                    let mut r = d.rect;
                    r.left += 4;
                    r.top += 4;
                    r.right -= 4;
                    r.bottom -= 4;
                    DrawFocusRect(d.dc, &r);
                }
                1
            }
            0x0111 if wp & 0xffff == 103 => {
                u.model.lock().unwrap().launch_checked =
                    SendMessageW(u.checkbox, 0x00f0, 0, 0) == 1;
                0
            }
            0x0111 if wp & 0xffff == 201 => {
                ShowWindow(w, 6);
                0
            }
            0x0111 if wp & 0xffff == 202 => {
                SendMessageW(w, 0x0010, 0, 0);
                0
            }
            0x0111 if wp & 0xffff == 104 => {
                SendMessageW(w, 0x0010, 0, 0);
                0
            }
            0x0111 if wp & 0xffff == 101 || wp & 0xffff == 102 => {
                let primary = wp & 0xffff == 101;
                let mut m = u.model.lock().unwrap();
                if m.preview {
                    drop(m);
                    DestroyWindow(w);
                    return 0;
                }
                if m.phase == 2 {
                    let launch = if !primary && matches!(m.result, Some(0 | 3010)) {
                        m.launch.clone()
                    } else {
                        None
                    };
                    drop(m);
                    if let Some(launch) = launch {
                        if let Err(error) = launch.run() {
                            u.model.lock().unwrap().status = error;
                            return 0;
                        }
                    }
                    DestroyWindow(w);
                    return 0;
                }
                if m.phase == 0 && (primary || m.installed) {
                    let choice = if !m.installed {
                        Choice::Install
                    } else if primary {
                        Choice::Repair
                    } else {
                        Choice::Remove
                    };
                    m.action = choice;
                    m.phase = 1;
                    m.status = format!("{} 준비 중입니다…", choice.label());
                    drop(m);
                    if let Some(tx) = u.sender.take() {
                        let _ = tx.send(choice);
                    }
                    refresh(w, u);
                    return 0;
                }
                drop(m);
                SendMessageW(w, 0x0010, 0, 0);
                0
            }
            0x0010 => {
                let m = u.model.lock().unwrap();
                let phase = m.phase;
                let prompt = format!(
                    "{} {}를 정말로 취소하시겠습니까?",
                    m.name,
                    if m.installed && phase == 0 {
                        "유지보수"
                    } else {
                        m.action.label()
                    }
                );
                let preview = m.preview;
                drop(m);
                if phase == 2 || preview {
                    DestroyWindow(w);
                    return 0;
                }
                if MessageBoxW(w, wide(&prompt).as_ptr(), wide("취소 확인").as_ptr(), 0x124) != 6
                {
                    return 0;
                }
                u.cancel.store(true, Ordering::SeqCst);
                if phase == 0 {
                    if let Some(tx) = u.sender.take() {
                        let _ = tx.send(Choice::Cancel);
                    }
                    DestroyWindow(w);
                } else {
                    u.model.lock().unwrap().status =
                        "취소를 요청했습니다. 작업이 정리될 때까지 기다려 주세요.".into();
                }
                0
            }
            0x0002 => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(w, msg, wp, lp),
        }
    }
}
fn window_thread(
    model: Arc<Mutex<Model>>,
    cancel: Arc<AtomicBool>,
    window: Arc<AtomicIsize>,
    sender: mpsc::Sender<Choice>,
) {
    unsafe {
        let name = wide("RocketMsiQuickEmbeddedWindow");
        let instance = GetModuleHandleW(ptr::null());
        let class = Class {
            style: 3,
            proc: Some(wndproc),
            extra: 0,
            window_extra: 0,
            instance,
            icon: ptr::null_mut(),
            cursor: LoadCursorW(ptr::null_mut(), 32512usize as *const u16),
            background: ptr::null_mut(),
            menu: ptr::null(),
            name: name.as_ptr(),
        };
        if RegisterClassW(&class) == 0 {
            return;
        }
        let title = format!("{} · Quick MSI", model.lock().unwrap().name);
        let w = CreateWindowExW(
            0x00040000, // WS_EX_APPWINDOW.
            name.as_ptr(),
            wide(&title).as_ptr(),
            0x02ce0000, // Standard caption, sizing frame, system menu and minimize box, plus WS_CLIPCHILDREN.
            100,
            100,
            760,
            480,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null_mut(),
        );
        if w.is_null() {
            UnregisterClassW(name.as_ptr(), instance);
            return;
        }
        let dpi = GetDpiForWindow(w).max(96) as i32;
        let mut outer = Rect::default();
        GetWindowRect(w, &mut outer);
        let width = 760 * dpi / 96;
        let height = 480 * dpi / 96;
        MoveWindow(
            w,
            ((GetSystemMetrics(0) - width) / 2).max(0),
            ((GetSystemMetrics(1) - height) / 2).max(0),
            width,
            height,
            0,
        );
        let font = CreateFontW(
            -16 * dpi / 96,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            wide("Segoe UI").as_ptr(),
        );
        let heading = CreateFontW(
            -28 * dpi / 96,
            0,
            0,
            0,
            600,
            0,
            0,
            0,
            1,
            0,
            0,
            5,
            0,
            wide("Segoe UI").as_ptr(),
        );
        let primary = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("설치").as_ptr(),
            0x5001000b,
            0,
            0,
            140,
            44,
            w,
            101usize as H,
            instance,
            ptr::null_mut(),
        );
        let secondary = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("취소").as_ptr(),
            0x5001000b,
            0,
            0,
            140,
            44,
            w,
            102usize as H,
            instance,
            ptr::null_mut(),
        );
        let third = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("취소").as_ptr(),
            0x5001000b,
            0,
            0,
            140,
            44,
            w,
            104usize as H,
            instance,
            ptr::null_mut(),
        );
        let checkbox = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("설치 후 실행").as_ptr(),
            0x50010003,
            0,
            0,
            245,
            30,
            w,
            103usize as H,
            instance,
            ptr::null_mut(),
        );
        SendMessageW(checkbox, 0x0030, font as usize, 0);
        let minimize = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("최소화").as_ptr(),
            0x5000000b,
            0,
            0,
            46,
            36,
            w,
            201usize as H,
            instance,
            ptr::null_mut(),
        );
        let close = CreateWindowExW(
            0,
            wide("BUTTON").as_ptr(),
            wide("닫기").as_ptr(),
            0x5000000b,
            0,
            0,
            46,
            36,
            w,
            202usize as H,
            instance,
            ptr::null_mut(),
        );
        let mut u = Box::new(Window {
            dark: system_dark(),
            model,
            cancel,
            window,
            sender: Some(sender),
            primary,
            secondary,
            third,
            checkbox,
            minimize,
            close,
            checkbox_proc: 0,
            minimize_proc: 0,
            close_proc: 0,
            hover_caption: ptr::null_mut(),
            font,
            heading,
            dpi,
            last_phase: 255,
            last_model: None,
        });
        attach(w, &mut *u);
        #[cfg(target_pointer_width = "64")]
        {
            u.checkbox_proc = SetWindowLongPtrW(checkbox, -4, checkbox_proc as *const () as isize);
            u.minimize_proc = SetWindowLongPtrW(minimize, -4, caption_proc as *const () as isize);
            u.close_proc = SetWindowLongPtrW(close, -4, caption_proc as *const () as isize);
        }
        #[cfg(target_pointer_width = "32")]
        {
            u.checkbox_proc = set_data(checkbox, -4, checkbox_proc as *const () as isize);
            u.minimize_proc = set_data(minimize, -4, caption_proc as *const () as isize);
            u.close_proc = set_data(close, -4, caption_proc as *const () as isize);
        }
        apply_frame(w);
        SetWindowPos(w, ptr::null_mut(), 0, 0, 0, 0, 0x0037);
        u.window.store(w as isize, Ordering::SeqCst);
        layout(w, &mut u);
        refresh(w, &mut u);
        SetTimer(w, 1, 200, ptr::null_mut());
        ShowWindow(w, 5);
        SetFocus(primary);
        let mut msg: Msg = std::mem::zeroed();
        while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            if msg.msg == 0x0100 && msg.wp == 27 {
                SendMessageW(w, 0x0010, 0, 0);
                continue;
            }
            if msg.msg == 0x0100 && msg.wp == 13 {
                let focus = GetFocus();
                SendMessageW(
                    if [secondary, third, close, minimize].contains(&focus) {
                        focus
                    } else {
                        primary
                    },
                    0x00f5,
                    0,
                    0,
                );
                continue;
            }
            if IsDialogMessageW(w, &mut msg) == 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        u.window.store(0, Ordering::SeqCst);
        DeleteObject(font);
        DeleteObject(heading);
        UnregisterClassW(name.as_ptr(), instance);
    }
}
fn start(model: Model) -> (Session, mpsc::Receiver<Choice>) {
    let model = Arc::new(Mutex::new(model));
    let cancel = Arc::new(AtomicBool::new(false));
    let window = Arc::new(AtomicIsize::new(0));
    let (tx, rx) = mpsc::channel();
    let (m, c, w) = (model.clone(), cancel.clone(), window.clone());
    let thread = thread::spawn(move || window_thread(m, c, w, tx));
    (
        Session {
            model,
            cancel,
            window,
            thread,
        },
        rx,
    )
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn InitializeEmbeddedUI(
    h: u32,
    _resources: *const u16,
    level: *mut u32,
) -> u32 {
    unsafe {
        if level.is_null() {
            return 1603;
        }
        // Attribute=1 deliberately leaves basic/passive and silent installations alone.
        if *level & 7 != 5 {
            return 0;
        }
        if SESSION.lock().unwrap().is_some() {
            return 1603;
        }
        let name = property(h, "ProductName");
        let installed = MsiQueryProductStateW(wide(&property(h, "ProductCode")).as_ptr()) == 5;
        let component = property(h, "EMB_QUICK_LAUNCH_COMPONENT");
        let mut related = [0u16; 39];
        let has_related = MsiEnumRelatedProductsW(
            wide(&property(h, "UpgradeCode")).as_ptr(),
            0,
            0,
            related.as_mut_ptr(),
        ) == 0;
        let launch = if !installed && !has_related && !component.is_empty() {
            let count = property(h, "EMB_QUICK_LAUNCH_COUNT")
                .parse::<usize>()
                .unwrap_or(0)
                .min(1024);
            Some(Launch {
                product: property(h, "ProductCode"),
                component,
                arguments: (0..count)
                    .map(|i| property(h, &format!("EMB_QUICK_LAUNCH_ARG_{i}")))
                    .collect(),
            })
        } else {
            None
        };
        let model = Model {
            name,
            publisher: property(h, "Manufacturer"),
            version: property(h, "ProductVersion"),
            branding: property(h, "BrandingText"),
            installed,
            action: Choice::Install,
            phase: 0,
            status: if installed {
                "수행할 작업을 선택해 주세요."
            } else {
                "설치를 시작하려면 설치를 눌러주세요."
            }
            .into(),
            result: None,
            total: 0,
            ticks: 0,
            reverse: false,
            step: 0,
            action_data: false,
            preview: false,
            launch,
            launch_checked: false,
            auto_attempted: false,
        };
        let (session, rx) = start(model);
        let choice = match rx.recv() {
            Ok(choice) => choice,
            Err(_) => {
                let _ = session.thread.join();
                return 1603;
            }
        };
        if choice == Choice::Cancel {
            let _ = session.thread.join();
            return 1602;
        }
        let props = match choice {
            Choice::Repair => vec![("REMOVE", ""), ("REINSTALL", "ALL")],
            Choice::Remove => vec![("REINSTALL", ""), ("REMOVE", "ALL")],
            _ => vec![],
        };
        for (k, v) in props {
            if MsiSetPropertyW(h, wide(k).as_ptr(), wide(v).as_ptr()) != 0 {
                let mut m = session.model.lock().unwrap();
                m.phase = 2;
                m.result = Some(1603);
                m.status = "작업 설정을 적용하지 못했습니다.".into();
                drop(m);
                let _ = session.thread.join();
                return 1603;
            }
        }
        *SESSION.lock().unwrap() = Some(session);
        *level = 2;
        0
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "system" fn EmbeddedUIHandler(kind: u32, record_handle: u32) -> i32 {
    unsafe {
        let slot = SESSION.lock().unwrap();
        let Some(session) = slot.as_ref() else {
            return 0;
        };
        let message = kind & 0xff000000;
        if message == 0x0a000000 {
            let mut m = session.model.lock().unwrap();
            let op = MsiRecordGetInteger(record_handle, 1);
            let value = i64::from(MsiRecordGetInteger(record_handle, 2)).max(0);
            match op {
                0 => {
                    m.total = value;
                    m.reverse = MsiRecordGetInteger(record_handle, 3) == 1;
                    m.ticks = if m.reverse { value } else { 0 };
                    m.action_data = false;
                }
                1 => {
                    m.step = value;
                    m.action_data = MsiRecordGetInteger(record_handle, 3) == 1;
                }
                2 => {
                    m.ticks = if m.reverse {
                        m.ticks - value
                    } else {
                        m.ticks + value
                    };
                }
                3 => m.total += value,
                _ => {}
            }
        } else if message == 0x08000000 {
            session.model.lock().unwrap().status = record(record_handle, 2);
        } else if message == 0x09000000 {
            let mut m = session.model.lock().unwrap();
            if m.action_data {
                let step = m.step;
                m.ticks += if m.reverse { -step } else { step };
            }
        } else if message == 0x01000000 || message == 0 {
            let description = formatted(record_handle);
            let window = session.window.load(Ordering::SeqCst) as H;
            session.model.lock().unwrap().status = description.clone();
            return MessageBoxW(
                window,
                wide(&description).as_ptr(),
                wide("Quick MSI").as_ptr(),
                (kind & 0x00ffffff) | 0x10,
            );
        } else if message == 0x1b000000 {
            session.model.lock().unwrap().result = Some(quick_policy::install_end_result(
                MsiRecordGetInteger(record_handle, 3),
            ));
        } else {
            return 0;
        }
        if session.cancel.load(Ordering::SeqCst) {
            2
        } else {
            1
        }
    }
}
#[unsafe(no_mangle)]
pub extern "system" fn ShutdownEmbeddedUI() -> u32 {
    let session = SESSION.lock().unwrap().take();
    if let Some(s) = session {
        {
            let mut m = s.model.lock().unwrap();
            m.phase = 2;
            let action = m.action.label();
            m.status = match m.result {
                Some(0 | 3010 | 1641) => format!("{}을(를) 성공적으로 {action}했습니다.", m.name),
                Some(1602) => "작업이 취소되었습니다.".into(),
                Some(code) => format!("작업이 완료되지 않았습니다. MSI 결과: {code}"),
                None => "작업이 종료되었습니다. 결과는 MSI 로그를 확인해주세요.".into(),
            };
        }
        let _ = s.thread.join();
    }
    0
}
// Read-only design preview: no MSI session, file operations, or installation.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn QuickMsiPreview(_w: H, _instance: H, args: *const u8, _show: i32) {
    let mode = if args.is_null() {
        String::new()
    } else {
        unsafe {
            std::ffi::CStr::from_ptr(args.cast())
                .to_string_lossy()
                .trim()
                .to_owned()
        }
    };
    let model = Model {
        name: "Rocket MSI Demo".into(),
        publisher: "Rocket MSI".into(),
        version: "1.0.0".into(),
        branding: "Rocket MSI".into(),
        installed: mode == "maintenance",
        action: Choice::Install,
        phase: if mode == "complete" {
            2
        } else if mode == "progress" {
            1
        } else {
            0
        },
        status: "디자인 미리보기입니다. 프로그램을 설치하지 않습니다.".into(),
        result: if mode == "complete" { Some(0) } else { None },
        total: 100,
        ticks: 45,
        reverse: false,
        step: 0,
        action_data: false,
        preview: true,
        launch: if mode == "maintenance" {
            None
        } else {
            Some(Launch {
                product: String::new(),
                component: String::new(),
                arguments: Vec::new(),
            })
        },
        launch_checked: false,
        auto_attempted: false,
    };
    let (s, _rx) = start(model);
    let _ = s.thread.join();
}
