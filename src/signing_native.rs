//! Windows-only Authenticode implementation. No SDK executables are invoked.
use super::Signing;
use crate::msi::Result;
use std::{
    ffi::{CStr, c_void},
    fs,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStrExt,
    path::Path,
    ptr,
};
type H = *mut c_void;
#[repr(C)]
struct Blob {
    size: u32,
    data: *mut u8,
}
#[repr(C)]
struct CertContext {
    encoding: u32,
    encoded: *const u8,
    encoded_size: u32,
    info: H,
    store: H,
}
#[repr(C)]
struct Usage {
    count: u32,
    identifiers: *const *const i8,
}
#[repr(C)]
struct SignerFile {
    size: u32,
    name: *const u16,
    file: H,
}
#[repr(C)]
struct SignerSubject {
    size: u32,
    index: *mut u32,
    choice: u32,
    file: *mut SignerFile,
}
#[repr(C)]
struct SignerStore {
    size: u32,
    cert: *const CertContext,
    policy: u32,
    store: H,
}
#[repr(C)]
struct SignerCert {
    size: u32,
    choice: u32,
    store: *mut SignerStore,
    window: H,
}
#[repr(C)]
struct Signature {
    size: u32,
    hash: u32,
    choice: u32,
    authcode: H,
    authenticated: H,
    unauthenticated: H,
}
#[repr(C)]
struct Guid {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}
#[repr(C)]
struct TrustFile {
    size: u32,
    path: *const u16,
    file: H,
    subject: *const Guid,
}
#[repr(C)]
struct SignatureSettings {
    size: u32,
    index: u32,
    flags: u32,
    secondary: u32,
    verified: u32,
    policy: H,
}
#[repr(C)]
struct TrustData {
    size: u32,
    policy: H,
    sip: H,
    ui: u32,
    revocation: u32,
    choice: u32,
    file: *mut TrustFile,
    action: u32,
    state: H,
    url: *mut u16,
    flags: u32,
    context: u32,
    signatures: *mut SignatureSettings,
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryExW(name: *const u16, file: H, flags: u32) -> H;
    fn GetProcAddress(module: H, name: *const u8) -> H;
    fn FreeLibrary(module: H) -> i32;
    fn GetLastError() -> u32;
    fn SetLastError(error: u32);
}
#[link(name = "crypt32")]
unsafe extern "system" {
    fn PFXImportCertStore(blob: *const Blob, password: *const u16, flags: u32) -> H;
    fn CertOpenStore(
        provider: *const u8,
        encoding: u32,
        crypto: usize,
        flags: u32,
        parameter: *const c_void,
    ) -> H;
    fn CertCloseStore(store: H, flags: u32) -> i32;
    fn CertEnumCertificatesInStore(store: H, previous: *const CertContext) -> *const CertContext;
    fn CertDuplicateCertificateContext(cert: *const CertContext) -> *const CertContext;
    fn CertFreeCertificateContext(cert: *const CertContext) -> i32;
    fn CertFindCertificateInStore(
        store: H,
        encoding: u32,
        flags: u32,
        kind: u32,
        parameter: *const c_void,
        previous: *const CertContext,
    ) -> *const CertContext;
    fn CertGetEnhancedKeyUsage(
        cert: *const CertContext,
        flags: u32,
        usage: *mut Usage,
        size: *mut u32,
    ) -> i32;
    fn CertVerifyTimeValidity(time: *const c_void, info: H) -> i32;
    fn CryptAcquireCertificatePrivateKey(
        cert: *const CertContext,
        flags: u32,
        reserved: H,
        key: *mut usize,
        spec: *mut u32,
        caller_free: *mut i32,
    ) -> i32;
}
#[link(name = "advapi32")]
unsafe extern "system" {
    fn CryptReleaseContext(key: usize, flags: u32) -> i32;
}
#[link(name = "ncrypt")]
unsafe extern "system" {
    fn NCryptFreeObject(key: usize) -> i32;
}
#[link(name = "wintrust")]
unsafe extern "system" {
    fn WinVerifyTrust(window: H, action: *const Guid, data: *mut TrustData) -> i32;
}

struct Store(H);
impl Drop for Store {
    fn drop(&mut self) {
        unsafe {
            CertCloseStore(self.0, 0);
        }
    }
}
struct Cert(*const CertContext);
impl Drop for Cert {
    fn drop(&mut self) {
        unsafe {
            CertFreeCertificateContext(self.0);
        }
    }
}
struct Library(H);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            FreeLibrary(self.0);
        }
    }
}
struct Credentials {
    cert: Cert,
    store: Store,
}
// Explicit volatile clearing protects the temporary PFX/password buffers from
// dead-store elimination. Callers own and clear their own password strings.
struct Secret<T: Copy + Default>(Vec<T>);
impl<T: Copy + Default> Drop for Secret<T> {
    fn drop(&mut self) {
        for value in &mut self.0 {
            unsafe {
                ptr::write_volatile(value, T::default());
            }
        }
        std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    }
}
fn wide(s: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    s.as_ref().encode_wide().chain(Some(0)).collect()
}
fn failure(stage: &str, code: u32) -> Box<dyn std::error::Error> {
    format!("MSI signing failed ({stage}, Windows error 0x{code:08X}). Check the certificate, private key, password and timestamp server.").into()
}
unsafe fn eligible(cert: *const CertContext) -> bool {
    unsafe {
        if CertVerifyTimeValidity(ptr::null(), (*cert).info) != 0 {
            return false;
        }
        let mut bytes = 0;
        if CertGetEnhancedKeyUsage(cert, 0, ptr::null_mut(), &mut bytes) == 0 {
            return false;
        }
        let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        SetLastError(0);
        if CertGetEnhancedKeyUsage(cert, 0, buffer.as_mut_ptr().cast(), &mut bytes) == 0 {
            return false;
        }
        let last = GetLastError();
        let usage = &*buffer.as_ptr().cast::<Usage>();
        if usage.count == 0 {
            return last == 0x80092004;
        } // CRYPT_E_NOT_FOUND: unrestricted EKU.
        (0..usage.count as usize).any(|i| {
            let oid = CStr::from_ptr(*usage.identifiers.add(i));
            oid.to_bytes() == b"1.3.6.1.5.5.7.3.3" || oid.to_bytes() == b"2.5.29.37.0"
        })
    }
}
unsafe fn has_private_key(cert: *const CertContext) -> bool {
    unsafe {
        let (mut key, mut spec, mut free) = (0usize, 0u32, 0i32);
        if CryptAcquireCertificatePrivateKey(
            cert,
            0x10041,
            ptr::null_mut(),
            &mut key,
            &mut spec,
            &mut free,
        ) == 0
        {
            return false;
        }
        if free != 0 {
            if spec == u32::MAX {
                NCryptFreeObject(key);
            } else {
                CryptReleaseContext(key, 0);
            }
        }
        true
    }
}
fn credentials(config: &Signing, base: &Path, password: Option<&str>) -> Result<Credentials> {
    unsafe {
        if config.source == "pfx" {
            let path = base.join(&config.pfx_file);
            let mut data = Secret(fs::read(path)?);
            let length = u32::try_from(data.0.len()).map_err(|_| "PFX file is too large")?;
            let blob = Blob {
                size: length,
                data: data.0.as_mut_ptr(),
            };
            let pass = Secret(wide(password.unwrap_or("")));
            // NO_PERSIST_KEY | ALWAYS_CNG_KSP | USER_KEYSET: import into memory only.
            let raw = PFXImportCertStore(&blob, pass.0.as_ptr(), 0x9200);
            if raw.is_null() {
                return Err(failure("PFX import", GetLastError()));
            }
            let store = Store(raw);
            let mut current = ptr::null();
            let mut selected = None;
            loop {
                current = CertEnumCertificatesInStore(store.0, current);
                if current.is_null() {
                    break;
                }
                if eligible(current) && has_private_key(current) {
                    if selected.is_some() {
                        CertFreeCertificateContext(current);
                        return Err("PFX has multiple eligible signing certificates; use the certificate store and select a thumbprint".into());
                    }
                    selected = Some(Cert(CertDuplicateCertificateContext(current)));
                }
            }
            let cert = selected
                .ok_or("PFX has no valid code-signing certificate with a usable private key")?;
            Ok(Credentials { cert, store })
        } else {
            // CERT_STORE_PROV_SYSTEM_W, read-only existing My store.
            let flags = 0xc000
                | if config.store_location == "localMachine" {
                    0x20000
                } else {
                    0x10000
                };
            let raw = CertOpenStore(
                10usize as *const u8,
                0,
                0,
                flags,
                wide("My").as_ptr().cast(),
            );
            if raw.is_null() {
                return Err(failure("certificate store", GetLastError()));
            }
            let store = Store(raw);
            let mut hash: Vec<u8> = config
                .thumbprint
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            let blob = Blob {
                size: hash.len() as u32,
                data: hash.as_mut_ptr(),
            };
            let raw = CertFindCertificateInStore(
                store.0,
                0x10001,
                0,
                0x10000,
                (&blob as *const Blob).cast(),
                ptr::null(),
            );
            if raw.is_null() {
                return Err(failure("certificate not found", GetLastError()));
            }
            let cert = Cert(raw);
            if !eligible(cert.0) {
                return Err(
                    "Certificate is expired, not yet valid, or not usable for code signing".into(),
                );
            }
            Ok(Credentials { cert, store })
        }
    }
}
type SignFn = unsafe extern "system" fn(
    u32,
    *mut SignerSubject,
    *mut SignerCert,
    *mut Signature,
    H,
    u32,
    *const u8,
    *const u16,
    H,
    H,
    *mut H,
    H,
    H,
) -> i32;
type FreeFn = unsafe extern "system" fn(H) -> i32;
pub(super) fn sign(
    file: &Path,
    config: &Signing,
    base: &Path,
    password: Option<&str>,
) -> Result<()> {
    let credentials = credentials(config, base, password)?;
    let name = wide(file);
    let timestamp = wide(&config.timestamp_url);
    unsafe {
        // Only load the operating system copy; never use a bundled DLL/search PATH.
        let raw = LoadLibraryExW(wide("mssign32.dll").as_ptr(), ptr::null_mut(), 0x800);
        if raw.is_null() {
            return Err(failure("Windows signer API", GetLastError()));
        }
        let module = Library(raw);
        let sign = GetProcAddress(module.0, c"SignerSignEx2".as_ptr().cast());
        let free = GetProcAddress(module.0, c"SignerFreeSignerContext".as_ptr().cast());
        if sign.is_null() || free.is_null() {
            return Err("Windows Authenticode signing API is unavailable".into());
        }
        let sign: SignFn = std::mem::transmute(sign);
        let free: FreeFn = std::mem::transmute(free);
        let mut index = 0;
        let mut file = SignerFile {
            size: size_of::<SignerFile>() as u32,
            name: name.as_ptr(),
            file: ptr::null_mut(),
        };
        let mut subject = SignerSubject {
            size: size_of::<SignerSubject>() as u32,
            index: &mut index,
            choice: 1,
            file: &mut file,
        };
        let mut store = SignerStore {
            size: size_of::<SignerStore>() as u32,
            cert: credentials.cert.0,
            policy: 8,
            store: credentials.store.0,
        };
        let mut cert = SignerCert {
            size: size_of::<SignerCert>() as u32,
            choice: 2,
            store: &mut store,
            window: ptr::null_mut(),
        };
        let mut signature: Signature = zeroed();
        signature.size = size_of::<Signature>() as u32;
        signature.hash = 0x800c; // CALG_SHA_256
        let mut context = ptr::null_mut();
        let result = sign(
            0,
            &mut subject,
            &mut cert,
            &mut signature,
            ptr::null_mut(),
            2,
            c"2.16.840.1.101.3.4.2.1".as_ptr().cast(),
            if config.timestamp_url.is_empty() {
                ptr::null()
            } else {
                timestamp.as_ptr()
            },
            ptr::null_mut(),
            ptr::null_mut(),
            &mut context,
            ptr::null_mut(),
            ptr::null_mut(),
        );
        if !context.is_null() {
            free(context);
        }
        if result != 0 {
            return Err(failure("Authenticode/RFC3161", result as u32));
        }
    }
    Ok(())
}
pub(super) fn verify(file: &Path) -> Result<()> {
    let name = wide(file);
    unsafe {
        let mut file = TrustFile {
            size: size_of::<TrustFile>() as u32,
            path: name.as_ptr(),
            file: ptr::null_mut(),
            subject: ptr::null(),
        };
        let action = Guid {
            a: 0x00aac56b,
            b: 0xcd44,
            c: 0x11d0,
            d: [0x8c, 0xc2, 0, 0xc0, 0x4f, 0xc2, 0x95, 0xee],
        };
        let mut settings: SignatureSettings = zeroed();
        settings.size = size_of::<SignatureSettings>() as u32;
        settings.flags = 2;
        let mut secondary = 0;
        for index in 0..=32 {
            if index > secondary {
                break;
            }
            settings.index = index;
            let mut trust: TrustData = zeroed();
            trust.size = size_of::<TrustData>() as u32;
            trust.ui = 2;
            trust.choice = 1;
            trust.file = &mut file;
            trust.action = 1;
            trust.flags = 0x80;
            trust.signatures = &mut settings;
            let status = WinVerifyTrust(usize::MAX as H, &action, &mut trust);
            trust.action = 2;
            WinVerifyTrust(usize::MAX as H, &action, &mut trust);
            if status != 0 {
                return Err(format!("MSI signature verification failed (Windows error 0x{:08X}). Check signature integrity and the certificate trust chain.",status as u32).into());
            }
            if index == 0 {
                secondary = settings.secondary;
                if secondary > 32 {
                    return Err("Too many nested MSI signatures".into());
                }
                settings.flags = 1;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ffi_layout_matches_windows_abi() {
        let is64 = cfg!(target_pointer_width = "64");
        for (actual, x64, x86) in [
            (size_of::<CertContext>(), 40, 20),
            (size_of::<SignerFile>(), 24, 12),
            (size_of::<SignerSubject>(), 32, 16),
            (size_of::<SignerStore>(), 32, 16),
            (size_of::<SignerCert>(), 24, 16),
            (size_of::<Signature>(), 40, 24),
            (size_of::<TrustFile>(), 32, 16),
            (size_of::<TrustData>(), 88, 52),
            (size_of::<SignatureSettings>(), 32, 24),
        ] {
            assert_eq!(actual, if is64 { x64 } else { x86 });
        }
    }
}
