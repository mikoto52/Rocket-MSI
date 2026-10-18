//! Native, synchronous deferred MSI action. No shell or file association.
mod action_protocol;
use action_protocol::{Request, quote};
use std::{
    ffi::c_void,
    path::Path,
    ptr,
    time::{Duration, Instant},
};
#[link(name = "msi")]
unsafe extern "system" {
    fn MsiGetPropertyW(h: u32, name: *const u16, value: *mut u16, len: *mut u32) -> u32;
    fn MsiCreateRecord(n: u32) -> u32;
    fn MsiRecordSetStringW(h: u32, field: u32, text: *const u16) -> u32;
    fn MsiRecordSetInteger(h: u32, field: u32, value: i32) -> u32;
    fn MsiProcessMessage(h: u32, kind: u32, record: u32) -> i32;
    fn MsiCloseHandle(h: u32) -> u32;
}
#[repr(C)]
struct Startup {
    cb: u32,
    reserved: *mut u16,
    desktop: *mut u16,
    title: *mut u16,
    x: u32,
    y: u32,
    xsize: u32,
    ysize: u32,
    xchars: u32,
    ychars: u32,
    fill: u32,
    flags: u32,
    show: u16,
    reserved_size: u16,
    reserved2: *mut u8,
    input: *mut c_void,
    output: *mut c_void,
    error: *mut c_void,
}
#[repr(C)]
struct Process {
    process: *mut c_void,
    thread: *mut c_void,
    pid: u32,
    tid: u32,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateProcessW(
        app: *const u16,
        command: *mut u16,
        pa: *const c_void,
        ta: *const c_void,
        inherit: i32,
        flags: u32,
        environment: *const c_void,
        directory: *const u16,
        startup: *const Startup,
        process: *mut Process,
    ) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, timeout: u32) -> u32;
    fn GetExitCodeProcess(handle: *mut c_void, code: *mut u32) -> i32;
    fn TerminateProcess(handle: *mut c_void, code: u32) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
    fn GetLastError() -> u32;
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn message(h: u32, kind: u32, id: &str, event: &str) -> i32 {
    unsafe {
        let r = MsiCreateRecord(2);
        if r == 0 {
            return 0;
        }
        MsiRecordSetStringW(r, 0, wide("InstallEmbly action [1]: [2]").as_ptr());
        MsiRecordSetStringW(r, 1, wide(id).as_ptr());
        MsiRecordSetStringW(r, 2, wide(event).as_ptr());
        let result = MsiProcessMessage(h, kind, r);
        MsiCloseHandle(r);
        result
    }
}
fn cancelled(h: u32) -> bool {
    unsafe {
        let r = MsiCreateRecord(3);
        if r == 0 {
            return false;
        }
        MsiRecordSetInteger(r, 1, 2);
        MsiRecordSetInteger(r, 2, 0);
        MsiRecordSetInteger(r, 3, 0);
        let result = MsiProcessMessage(h, 0x0a000000, r);
        MsiCloseHandle(r);
        result == 2
    }
}
fn property(h: u32) -> Result<String, String> {
    let name = wide("CustomActionData");
    let mut n = 0;
    let mut empty = 0;
    let code = unsafe { MsiGetPropertyW(h, name.as_ptr(), &mut empty, &mut n) };
    if code != 0 && code != 234 {
        return Err(format!("read action data: {code}"));
    }
    if n > 32767 {
        return Err("action data is too large".into());
    }
    let mut buffer = vec![0u16; n as usize + 1];
    n += 1;
    let code = unsafe { MsiGetPropertyW(h, name.as_ptr(), buffer.as_mut_ptr(), &mut n) };
    if code != 0 {
        return Err(format!("read action data: {code}"));
    }
    String::from_utf16(&buffer[..n as usize]).map_err(|_| "invalid action data Unicode".into())
}
enum Failure {
    Cancelled,
    Other(String),
}
fn execute(h: u32, r: &Request) -> Result<u32, Failure> {
    if !Path::new(&r.executable).is_absolute()
        || !Path::new(&r.directory).is_absolute()
        || r.executable.contains('\0')
        || r.directory.contains('\0')
        || r.args.iter().any(|a| a.contains('\0'))
    {
        return Err(Failure::Other(
            "invalid executable or working directory".into(),
        ));
    }
    if !Path::new(&r.executable).is_file() {
        return Err(Failure::Other("executable not found".into()));
    }
    if !Path::new(&r.directory).is_dir() {
        return Err(Failure::Other("working directory not found".into()));
    }
    let app = wide(&r.executable);
    let cwd = wide(&r.directory);
    let line = std::iter::once(quote(&r.executable))
        .chain(r.args.iter().map(|a| quote(a)))
        .collect::<Vec<_>>()
        .join(" ");
    if line.encode_utf16().count() > 32766 {
        return Err(Failure::Other("command line is too long".into()));
    }
    let mut line = wide(&line);
    let mut si: Startup = unsafe { std::mem::zeroed() };
    si.cb = std::mem::size_of::<Startup>() as u32;
    si.flags = 1;
    si.show = if r.hide { 0 } else { 1 };
    let mut pi: Process = unsafe { std::mem::zeroed() };
    let created = unsafe {
        CreateProcessW(
            app.as_ptr(),
            line.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            0,
            0x400 | if r.hide { 0x08000000 } else { 0 },
            ptr::null(),
            cwd.as_ptr(),
            &si,
            &mut pi,
        )
    };
    if created == 0 {
        return Err(Failure::Other(format!(
            "CreateProcess failed: {}",
            unsafe { GetLastError() }
        )));
    }
    struct Owned(*mut c_void);
    impl Drop for Owned {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    let process = Owned(pi.process);
    let _thread = Owned(pi.thread);
    let start = Instant::now();
    loop {
        let wait = unsafe { WaitForSingleObject(process.0, 200) };
        if wait == 0 {
            let mut code = 0;
            if unsafe { GetExitCodeProcess(process.0, &mut code) } == 0 {
                return Err(Failure::Other("cannot read process exit code".into()));
            }
            return Ok(code);
        }
        if wait != 258 {
            return Err(Failure::Other(format!("wait failed: {}", unsafe {
                GetLastError()
            })));
        }
        let cancel = cancelled(h);
        let timeout = r.timeout > 0 && start.elapsed() >= Duration::from_secs(r.timeout);
        if cancel || timeout {
            unsafe {
                TerminateProcess(process.0, 1602);
                WaitForSingleObject(process.0, 5000);
            }
            return Err(if cancel {
                Failure::Cancelled
            } else {
                Failure::Other("timeout (direct process terminated; external side effects are not rolled back)".into())
            });
        }
    }
}
fn run(h: u32) -> u32 {
    let mut r = match property(h).and_then(|s| Request::parse(&s)) {
        Ok(r) => r,
        Err(e) => {
            message(h, 0x04000000, "invalid-data", &e);
            return 1603;
        }
    };
    if r.directory.is_empty() {
        r.directory = Path::new(&r.executable)
            .parent()
            .and_then(|p| p.to_str())
            .unwrap_or("")
            .into();
    }
    if message(h, 0x04000000, &r.id, "start") == 2 {
        return 1602;
    }
    let start = Instant::now();
    let result = execute(h, &r);
    let failure = match result {
        Ok(0) => {
            message(
                h,
                0x04000000,
                &r.id,
                &format!("success, elapsed_ms={}", start.elapsed().as_millis()),
            );
            return 0;
        }
        Ok(code) => format!("exit_code={code}"),
        Err(Failure::Cancelled) => return 1602,
        Err(Failure::Other(reason)) => reason,
    };
    message(
        h,
        0x04000000,
        &r.id,
        &format!(
            "failure: {failure}; policy={}; elapsed_ms={}",
            r.policy,
            start.elapsed().as_millis()
        ),
    );
    if r.policy == "notify" && r.ui >= 4 {
        if message(
            h,
            0x02000000,
            &r.id,
            &format!("실행 실패: {failure}\n알림 후 다음 항목을 계속 진행합니다."),
        ) == 2
        {
            return 1602;
        }
    }
    if r.policy == "stop" {
        if r.ui >= 4 {
            message(
                h,
                0x01000000,
                &r.id,
                &format!("실행 실패: {failure}\n설치 또는 제거를 중단합니다."),
            );
        }
        1603
    } else {
        0
    }
}
#[unsafe(no_mangle)]
pub extern "system" fn EmbRun(h: u32) -> u32 {
    run(h)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(child: &str) -> Request {
        let exe = std::env::current_exe().unwrap();
        Request {
            id: "runtime-test".into(),
            policy: "stop".into(),
            hide: true,
            ui: 2,
            timeout: 5,
            executable: exe.to_str().unwrap().into(),
            directory: exe.parent().unwrap().to_str().unwrap().into(),
            args: vec![
                "--exact".into(),
                format!("tests::{child}"),
                "--ignored".into(),
            ],
        }
    }
    #[test]
    fn executes_direct_process_and_reads_exit_codes() {
        assert!(matches!(execute(0, &request("child_success")), Ok(0)));
        assert!(matches!(execute(0, &request("child_failure")), Ok(17)));
    }
    #[test]
    fn missing_target_and_bad_working_directory_fail_before_launch() {
        let mut r = request("child_success");
        r.executable.push_str(".missing.exe");
        assert!(matches!(execute(0,&r),Err(Failure::Other(s)) if s=="executable not found"));
        let mut r = request("child_success");
        r.directory.push_str("\\missing-working-directory");
        assert!(matches!(execute(0,&r),Err(Failure::Other(s)) if s=="working directory not found"));
    }
    #[test]
    fn timeout_terminates_direct_process() {
        let mut r = request("child_sleep");
        r.timeout = 1;
        assert!(matches!(execute(0,&r),Err(Failure::Other(s)) if s.starts_with("timeout")));
    }
    #[test]
    #[ignore]
    fn child_success() {}
    #[test]
    #[ignore]
    fn child_failure() {
        std::process::exit(17);
    }
    #[test]
    #[ignore]
    fn child_sleep() {
        std::thread::sleep(Duration::from_secs(30));
    }
}
