//! The unsafe Windows Installer boundary. Handles are owned and closed once.
use std::{ffi::c_void, path::Path, ptr};

type Handle = u32;
#[link(name = "msi")]
unsafe extern "system" {
    fn MsiVerifyPackageW(path: *const u16) -> u32;
    fn MsiOpenPackageExW(path: *const u16, options: u32, package: *mut Handle) -> u32;
    fn MsiOpenDatabaseW(path: *const u16, mode: *const u16, db: *mut Handle) -> u32;
    fn MsiCloseHandle(handle: Handle) -> u32;
    fn MsiDatabaseOpenViewW(db: Handle, sql: *const u16, view: *mut Handle) -> u32;
    fn MsiViewExecute(view: Handle, record: Handle) -> u32;
    fn MsiViewFetch(view: Handle, record: *mut Handle) -> u32;
    fn MsiCreateRecord(fields: u32) -> Handle;
    fn MsiRecordSetStringW(record: Handle, field: u32, value: *const u16) -> u32;
    fn MsiRecordSetInteger(record: Handle, field: u32, value: i32) -> u32;
    fn MsiRecordSetStreamW(record: Handle, field: u32, path: *const u16) -> u32;
    fn MsiRecordGetStringW(record: Handle, field: u32, value: *mut u16, len: *mut u32) -> u32;
    fn MsiDatabaseCommit(db: Handle) -> u32;
    fn MsiGetSummaryInformationW(
        db: Handle,
        path: *const u16,
        count: u32,
        info: *mut Handle,
    ) -> u32;
    fn MsiSummaryInfoSetPropertyW(
        info: Handle,
        property: u32,
        kind: u32,
        integer: i32,
        time: *const c_void,
        value: *const u16,
    ) -> u32;
    fn MsiSummaryInfoPersist(info: Handle) -> u32;
}
#[repr(C)]
struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}
#[link(name = "ole32")]
unsafe extern "system" {
    fn CoCreateGuid(guid: *mut Guid) -> i32;
    fn CoInitializeEx(reserved: *mut c_void, model: u32) -> i32;
    fn CoUninitialize();
}

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptOpenAlgorithmProvider(
        handle: *mut *mut c_void,
        algorithm: *const u16,
        provider: *const u16,
        flags: u32,
    ) -> i32;
    fn BCryptHash(
        handle: *mut c_void,
        secret: *const u8,
        secret_len: u32,
        data: *const u8,
        data_len: u32,
        output: *mut u8,
        output_len: u32,
    ) -> i32;
    fn BCryptCloseAlgorithmProvider(handle: *mut c_void, flags: u32) -> i32;
}
pub fn sha256(text: &str) -> Result<[u8; 32]> {
    let mut handle = ptr::null_mut();
    let code = unsafe {
        BCryptOpenAlgorithmProvider(&mut handle, wide("SHA256").as_ptr(), ptr::null(), 0)
    };
    if code < 0 {
        return Err(format!("BCryptOpenAlgorithmProvider: {code:#X}").into());
    }
    struct Algorithm(*mut c_void);
    impl Drop for Algorithm {
        fn drop(&mut self) {
            unsafe {
                BCryptCloseAlgorithmProvider(self.0, 0);
            }
        }
    }
    let algorithm = Algorithm(handle);
    let mut output = [0u8; 32];
    let code = unsafe {
        BCryptHash(
            algorithm.0,
            ptr::null(),
            0,
            text.as_ptr(),
            u32::try_from(text.len())?,
            output.as_mut_ptr(),
            32,
        )
    };
    if code < 0 {
        return Err(format!("BCryptHash: {code:#X}").into());
    }
    Ok(output)
}
/// Validates package recognition without starting an installation session.
pub fn verify_package(path: &Path) -> Result<()> {
    // Windows Installer sessions for the same product cannot be opened
    // concurrently, even when both handles ignore the installed state.
    static SESSION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _session_lock = SESSION_LOCK
        .lock()
        .map_err(|_| "Installer session lock poisoned")?;
    // Installer sessions do not accept the verbatim prefix returned by
    // Rust's Windows canonicalize, unlike the database query API.
    let absolute = std::fs::canonicalize(path)?;
    let plain = absolute
        .as_os_str()
        .to_str()
        .ok_or("MSI path is not valid Unicode")?;
    let plain = if let Some(unc) = plain.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else {
        plain.strip_prefix(r"\\?\").unwrap_or(plain).to_owned()
    };
    let path = Path::new(&plain);
    let hr = unsafe { CoInitializeEx(ptr::null_mut(), 2) };
    if hr < 0 {
        return Err(format!("CoInitializeEx failed: {hr:#X}").into());
    }
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _com = Com;
    check(
        unsafe { MsiVerifyPackageW(path_wide(path).as_ptr()) },
        "MsiVerifyPackageW",
    )?;
    let mut handle = 0;
    // IGNOREMACHINESTATE: open only; never run actions or install the package.
    check(
        unsafe { MsiOpenPackageExW(path_wide(path).as_ptr(), 1, &mut handle) },
        "MsiOpenPackageExW",
    )?;
    let _package = Owned(handle);
    Ok(())
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn path_wide(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
fn check(code: u32, operation: &str) -> Result<()> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!("{operation}: Windows Installer error {code}").into())
    }
}
struct Owned(Handle);
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe {
            MsiCloseHandle(self.0);
        }
    }
}
pub struct Database(Owned);
pub enum Value<'a> {
    Text(&'a str),
    Int(i32),
    Null,
}
impl Database {
    #[cfg(test)]
    pub fn restore_legacy_codepage_for_test(path: &Path) -> Result<()> {
        let db = Self::open(path, 1)?;
        let mut handle = 0;
        check(
            unsafe { MsiGetSummaryInformationW(db.0.0, ptr::null(), 1, &mut handle) },
            "test summary",
        )?;
        let info = Owned(handle);
        check(
            unsafe { MsiSummaryInfoSetPropertyW(info.0, 1, 3, 65001, ptr::null(), ptr::null()) },
            "legacy codepage",
        )?;
        check(
            unsafe { MsiSummaryInfoPersist(info.0) },
            "persist legacy summary",
        )?;
        drop(info);
        db.commit()
    }
    pub fn create(path: &Path) -> Result<Self> {
        Self::open(path, 3)
    }
    pub fn read(path: &Path) -> Result<Self> {
        Self::open(path, 0)
    }
    fn open(path: &Path, mode: usize) -> Result<Self> {
        let mut h = 0;
        check(
            unsafe { MsiOpenDatabaseW(path_wide(path).as_ptr(), mode as *const u16, &mut h) },
            "open database",
        )?;
        Ok(Self(Owned(h)))
    }
    fn view(&self, sql: &str) -> Result<Owned> {
        let mut h = 0;
        check(
            unsafe { MsiDatabaseOpenViewW(self.0.0, wide(sql).as_ptr(), &mut h) },
            sql,
        )?;
        Ok(Owned(h))
    }
    pub fn execute(&self, sql: &str) -> Result<()> {
        let v = self.view(sql)?;
        check(unsafe { MsiViewExecute(v.0, 0) }, sql)
    }
    pub fn insert(&self, table: &str, fields: &[&str], values: &[Value<'_>]) -> Result<()> {
        if fields.len() != values.len() {
            return Err("column/value mismatch".into());
        }
        let names = fields
            .iter()
            .map(|s| format!("`{s}`"))
            .collect::<Vec<_>>()
            .join(",");
        let markers = vec!["?"; values.len()].join(",");
        let view = self.view(&format!(
            "INSERT INTO `{table}` ({names}) VALUES ({markers})"
        ))?;
        let record = Owned(unsafe { MsiCreateRecord(values.len() as u32) });
        if record.0 == 0 {
            return Err("cannot allocate MSI record".into());
        }
        for (i, value) in values.iter().enumerate() {
            let f = i as u32 + 1;
            let code = unsafe {
                match value {
                    Value::Text(s) => MsiRecordSetStringW(record.0, f, wide(s).as_ptr()),
                    Value::Int(n) => MsiRecordSetInteger(record.0, f, *n),
                    Value::Null => 0,
                }
            };
            check(code, "set field")?;
        }
        check(
            unsafe { MsiViewExecute(view.0, record.0) },
            &format!("insert {table}"),
        )
    }
    pub fn stream(&self, name: &str, path: &Path) -> Result<()> {
        self.insert_stream("_Streams", name, path)
    }
    pub fn binary(&self, name: &str, path: &Path) -> Result<()> {
        self.insert_stream("Binary", name, path)
    }
    fn insert_stream(&self, table: &str, name: &str, path: &Path) -> Result<()> {
        let view = self.view(&format!(
            "INSERT INTO `{table}` (`Name`,`Data`) VALUES (?,?)"
        ))?;
        let record = Owned(unsafe { MsiCreateRecord(2) });
        check(
            unsafe { MsiRecordSetStringW(record.0, 1, wide(name).as_ptr()) },
            "stream name",
        )?;
        check(
            unsafe { MsiRecordSetStreamW(record.0, 2, path_wide(path).as_ptr()) },
            "stream data",
        )?;
        check(unsafe { MsiViewExecute(view.0, record.0) }, "insert stream")
    }
    pub fn summary(&self, package_code: &str, x64: bool) -> Result<()> {
        let mut h = 0;
        check(
            unsafe { MsiGetSummaryInformationW(self.0.0, ptr::null(), 10, &mut h) },
            "summary",
        )?;
        let info = Owned(h);
        // PID_CODEPAGE is VT_I2, and must precede all string properties.
        // Summary strings are ASCII; the database's Korean text is independent.
        check(
            unsafe { MsiSummaryInfoSetPropertyW(info.0, 1, 2, 1252, ptr::null(), ptr::null()) },
            "summary codepage",
        )?;
        for (id, value) in [
            (2, "Installation Database"),
            (3, "Express MSI Builder prototype"),
            (4, "Express MSI Builder"),
            (7, if x64 { "x64;1042" } else { "Intel;1042" }),
            (9, package_code),
            (18, "Express MSI Builder"),
        ] {
            check(
                unsafe {
                    MsiSummaryInfoSetPropertyW(info.0, id, 30, 0, ptr::null(), wide(value).as_ptr())
                },
                "summary string",
            )?;
        }
        for (id, value) in [(14, 200), (15, 2), (19, 2)] {
            check(
                unsafe {
                    MsiSummaryInfoSetPropertyW(info.0, id, 3, value, ptr::null(), ptr::null())
                },
                "summary integer",
            )?;
        }
        check(unsafe { MsiSummaryInfoPersist(info.0) }, "persist summary")
    }
    pub fn commit(&self) -> Result<()> {
        check(unsafe { MsiDatabaseCommit(self.0.0) }, "commit")
    }
    pub fn query(&self, sql: &str, columns: u32) -> Result<Vec<Vec<String>>> {
        let view = self.view(sql)?;
        check(unsafe { MsiViewExecute(view.0, 0) }, sql)?;
        let mut rows = vec![];
        loop {
            let mut h = 0;
            let code = unsafe { MsiViewFetch(view.0, &mut h) };
            if code == 259 {
                break;
            }
            check(code, "fetch")?;
            let record = Owned(h);
            let mut row = vec![];
            for col in 1..=columns {
                let mut len = 0;
                let code = unsafe { MsiRecordGetStringW(record.0, col, ptr::null_mut(), &mut len) };
                if code != 234 {
                    check(code, "measure string")?;
                }
                let mut buffer = vec![0u16; len as usize + 1];
                len += 1;
                check(
                    unsafe { MsiRecordGetStringW(record.0, col, buffer.as_mut_ptr(), &mut len) },
                    "read string",
                )?;
                row.push(String::from_utf16(&buffer[..len as usize])?);
            }
            rows.push(row);
        }
        Ok(rows)
    }
}
pub fn guid() -> Result<String> {
    let mut g = Guid {
        a: 0,
        b: 0,
        c: 0,
        d: [0; 8],
    };
    let code = unsafe { CoCreateGuid(&mut g) };
    if code < 0 {
        return Err(format!("CoCreateGuid: {code}").into());
    }
    Ok(format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        g.a, g.b, g.c, g.d[0], g.d[1], g.d[2], g.d[3], g.d[4], g.d[5], g.d[6], g.d[7]
    ))
}
