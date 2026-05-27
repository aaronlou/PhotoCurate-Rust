use crate::error::{PhotoCurateError, Result};

const LEGACY_GEMINI_SERVICE: &str = "com.photocurate.gemini";
const SERVICE_PREFIX: &str = "com.photocurate.ai";
const ACCOUNT: &str = "api-key";

#[cfg(target_os = "macos")]
pub fn stores_api_key() -> bool {
    true
}

#[cfg(target_os = "macos")]
pub fn get_api_key(provider: &str) -> Result<Option<String>> {
    use security_framework::passwords::get_generic_password;

    let service = service_name(provider);
    match get_generic_password(&service, ACCOUNT) {
        Ok(password) => String::from_utf8(password)
            .map(Some)
            .map_err(|e| PhotoCurateError::InvalidData(e.to_string())),
        Err(err) if err.code() == err_sec_item_not_found() => Ok(None),
        Err(err) => Err(PhotoCurateError::Other(format!(
            "failed to read API key from Keychain: {}",
            err
        ))),
    }
}

#[cfg(target_os = "macos")]
pub fn set_api_key(provider: &str, api_key: &str) -> Result<()> {
    use security_framework::passwords::{delete_generic_password, set_generic_password};

    let service = service_name(provider);
    match delete_generic_password(&service, ACCOUNT) {
        Ok(()) => {}
        Err(err) if err.code() == err_sec_item_not_found() => {}
        Err(err) => {
            return Err(PhotoCurateError::Other(format!(
                "failed to replace API key in Keychain: {}",
                err
            )));
        }
    }

    if api_key.is_empty() {
        return Ok(());
    }

    set_generic_password(&service, ACCOUNT, api_key.as_bytes()).map_err(|err| {
        PhotoCurateError::Other(format!("failed to store API key in Keychain: {}", err))
    })
}

#[cfg(target_os = "macos")]
pub fn get_legacy_gemini_api_key() -> Result<Option<String>> {
    use security_framework::passwords::get_generic_password;

    match get_generic_password(LEGACY_GEMINI_SERVICE, ACCOUNT) {
        Ok(password) => String::from_utf8(password)
            .map(Some)
            .map_err(|e| PhotoCurateError::InvalidData(e.to_string())),
        Err(err) if err.code() == err_sec_item_not_found() => Ok(None),
        Err(err) => Err(PhotoCurateError::Other(format!(
            "failed to read API key from Keychain: {}",
            err
        ))),
    }
}

#[cfg(target_os = "macos")]
fn service_name(provider: &str) -> String {
    let safe_provider = provider
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{SERVICE_PREFIX}.{safe_provider}")
}

#[cfg(target_os = "macos")]
fn err_sec_item_not_found() -> i32 {
    -25300
}

#[cfg(not(target_os = "macos"))]
pub fn stores_api_key() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn get_api_key(_provider: &str) -> Result<Option<String>> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
pub fn set_api_key(_provider: &str, _api_key: &str) -> Result<()> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn get_legacy_gemini_api_key() -> Result<Option<String>> {
    Ok(None)
}
