use crate::bindings;
use crate::error::Error;
use std::ffi::CStr;

pub fn get_cups_error_details() -> (i32, String) {
    unsafe {
        #[cfg(cups3)]
        let error_code = bindings::cupsGetError();

        #[cfg(cups2)]
        let error_code = bindings::cupsLastError();

        #[cfg(cups3)]
        let error_msg = bindings::cupsGetErrorString();

        #[cfg(cups2)]
        let error_msg = bindings::cupsLastErrorString();

        let message = if error_msg.is_null() {
            "Unknown CUPS error".to_string()
        } else {
            CStr::from_ptr(error_msg).to_string_lossy().into_owned()
        };

        (error_code as i32, message)
    }
}

pub fn cups_error_to_our_error(operation: &str, dest_name: Option<&str>) -> Error {
    let (code, message) = get_cups_error_details();

    match code {
        0 => Error::ServerUnavailable,

        401 => Error::AuthenticationRequired(dest_name.unwrap_or("unknown").to_string()),

        403 => Error::PermissionDenied(dest_name.unwrap_or("unknown").to_string()),

        404 => Error::DestinationNotFound(dest_name.unwrap_or("unknown").to_string()),

        503 => {
            Error::PrinterNotAccepting(dest_name.unwrap_or("unknown").to_string(), message.clone())
        }

        bindings::ipp_status_e_IPP_STATUS_ERROR_NOT_ACCEPTING_JOBS => {
            Error::PrinterNotAccepting(dest_name.unwrap_or("unknown").to_string(), message.clone())
        }

        _ => {
            if message.contains("offline") || message.contains("unreachable") {
                Error::PrinterOffline(dest_name.unwrap_or("unknown").to_string())
            } else if message.contains("timeout") {
                Error::Timeout
            } else if message.contains("network") || message.contains("connection") {
                Error::NetworkError(message)
            } else {
                Error::ServerError(format!("{} failed (code {}): {}", operation, code, message))
            }
        }
    }
}

pub fn validate_document_format(format: &str, dest_name: &str) -> Result<(), Error> {
    let supported_formats = [
        "application/pdf",
        "application/postscript",
        "text/plain",
        "image/jpeg",
        "image/png",
    ];

    if !supported_formats.contains(&format) {
        return Err(Error::InvalidFormat(
            format.to_string(),
            dest_name.to_string(),
        ));
    }

    Ok(())
}

pub fn check_document_size(size: usize, max_size: Option<usize>) -> Result<(), Error> {
    let limit = max_size.unwrap_or(100 * 1024 * 1024); // 100MB default

    if size > limit {
        return Err(Error::DocumentTooLarge(size, limit));
    }

    Ok(())
}
