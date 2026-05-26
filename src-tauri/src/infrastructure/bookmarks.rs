#[cfg(target_os = "macos")]
use objc2::msg_send;
#[cfg(target_os = "macos")]
use objc2::rc::Retained;
#[cfg(target_os = "macos")]
use objc2::runtime::{AnyObject, Bool};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSData, NSString, NSURL};
#[cfg(target_os = "macos")]
use std::sync::Mutex;

/// Holds resolved security-scoped NSURLs alive for the app's entire lifetime,
/// ensuring the kernel-granted sandbox access persists.
#[cfg(target_os = "macos")]
static SCOPED_URLS: Mutex<Vec<Retained<NSURL>>> = Mutex::new(Vec::new());

/// Create a security-scoped bookmark for a user-selected directory.
/// Must be called while the app still has temporary sandbox access
/// (i.e., soon after the user selected the directory via NSOpenPanel).
#[cfg(target_os = "macos")]
pub fn create_bookmark(path: &str) -> Result<Vec<u8>, String> {
    unsafe {
        let path_ns = NSString::from_str(path);
        let url = NSURL::fileURLWithPath(&path_ns);

        // NSURLBookmarkCreationWithSecurityScope = 1 << 11 = 2048
        let options: usize = 1 << 11;

        let mut error: *mut AnyObject = std::ptr::null_mut();

        let bookmark_data: *mut NSData = msg_send![
            &url,
            bookmarkDataWithOptions: options,
            includingResourceValuesForKeys: std::ptr::null::<AnyObject>(),
            relativeToURL: std::ptr::null::<NSURL>(),
            error: &mut error,
        ];

        if !error.is_null() {
            let desc: *mut NSString = msg_send![&*error, localizedDescription];
            let desc_cstr = std::ffi::CStr::from_ptr(msg_send![&*desc, UTF8String]);
            return Err(format!(
                "Bookmark creation failed: {}",
                desc_cstr.to_string_lossy()
            ));
        }

        if bookmark_data.is_null() {
            return Err("Bookmark creation returned nil".to_string());
        }

        let data = Retained::from_raw(bookmark_data)
            .ok_or_else(|| "Failed to retain bookmark NSData".to_string())?;
        let bytes: *const std::ffi::c_void = msg_send![&data, bytes];
        let len: usize = msg_send![&data, length];
        let vec = std::slice::from_raw_parts(bytes as *const u8, len).to_vec();
        Ok(vec)
    }
}

/// Resolve a security-scoped bookmark, start accessing the resource,
/// and return the current filesystem path.
/// The resolved NSURL is kept alive globally so sandbox access persists.
#[cfg(target_os = "macos")]
pub fn resolve_bookmark(data: &[u8]) -> Result<String, String> {
    unsafe {
        let ns_data = NSData::from_vec(data.to_vec());

        // NSURLBookmarkResolutionWithSecurityScope = 1 << 10 = 1024
        let options: usize = 1 << 10;

        let mut is_stale = Bool::NO;
        let mut error: *mut AnyObject = std::ptr::null_mut();

        let url: *mut NSURL = msg_send![
            objc2::class!(NSURL),
            URLByResolvingBookmarkData: &*ns_data,
            options: options,
            relativeToURL: std::ptr::null::<NSURL>(),
            bookmarkDataIsStale: &mut is_stale,
            error: &mut error,
        ];

        if !error.is_null() {
            let desc: *mut NSString = msg_send![&*error, localizedDescription];
            let desc_cstr = std::ffi::CStr::from_ptr(msg_send![&*desc, UTF8String]);
            return Err(format!(
                "Bookmark resolution failed: {}",
                desc_cstr.to_string_lossy()
            ));
        }

        if url.is_null() {
            return Err("Bookmark resolution returned nil URL".to_string());
        }

        let retained_url = Retained::from_raw(url)
            .ok_or_else(|| "Failed to retain resolved NSURL".to_string())?;

        // Start security-scoped access — required before any file I/O on this resource
        let started: bool = msg_send![&retained_url, startAccessingSecurityScopedResource];
        if !started {
            return Err("startAccessingSecurityScopedResource returned NO".to_string());
        }

        // Get the current path (may differ from original if directory was moved)
        let path_ns: *mut NSString = msg_send![&retained_url, path];
        let path_cstr = std::ffi::CStr::from_ptr(msg_send![&*path_ns, UTF8String]);
        let path = path_cstr.to_string_lossy().to_string();

        // Keep the NSURL alive for the app's lifetime so sandbox access persists
        SCOPED_URLS.lock().unwrap().push(retained_url);

        Ok(path)
    }
}

/// Release security-scoped access for a specific path.
/// Called when a directory is removed from the library.
#[cfg(target_os = "macos")]
pub fn release_access(path: &str) {
    let path_ns = NSString::from_str(path);
    let url = NSURL::fileURLWithPath(&path_ns);
    unsafe {
        let _: () = msg_send![&url, stopAccessingSecurityScopedResource];
    }
}

// ---- Non-macOS stubs ----

#[cfg(not(target_os = "macos"))]
pub fn create_bookmark(_path: &str) -> Result<Vec<u8>, String> {
    Ok(vec![])
}

#[cfg(not(target_os = "macos"))]
pub fn resolve_bookmark(_data: &[u8]) -> Result<String, String> {
    Err("Bookmarks only supported on macOS".to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn release_access(_path: &str) {}
