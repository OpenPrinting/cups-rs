use super::Job;
use crate::bindings;
use crate::error::{Error, Result};
use std::ffi::CString;
use std::ptr;

impl Job {
    pub fn close(&self) -> Result<()> {
        let dest = &self.dest;
        let dest_info = dest.get_detailed_info(ptr::null_mut())?;
        let dest_ptr = dest.as_ptr();

        if dest_ptr.is_null() {
            return Err(Error::NullPointer);
        }

        let status = unsafe {
            bindings::cupsCloseDestJob(ptr::null_mut(), dest_ptr, dest_info.as_ptr(), self.id)
        };

        unsafe {
            let dest_box = Box::from_raw(dest_ptr);
            if !dest_box.name.is_null() {
                let _ = CString::from_raw(dest_box.name);
            }
            if !dest_box.instance.is_null() {
                let _ = CString::from_raw(dest_box.instance);
            }
            if !dest_box.options.is_null() {
                bindings::cupsFreeOptions(dest_box.num_options, dest_box.options);
            }
        }

        if status == bindings::ipp_status_e_IPP_STATUS_OK as bindings::ipp_status_t {
            Ok(())
        } else {
            let (error_code, _) = crate::error_helpers::get_cups_error_details();
            let error_msg = format!("CUPS error {}", error_code);
            Err(Error::JobManagementFailed(format!(
                "Failed to close job {}: {}",
                self.id, error_msg
            )))
        }
    }

    pub fn cancel(&self) -> Result<()> {
        let dest = &self.dest;
        let dest_ptr = dest.as_ptr();

        if dest_ptr.is_null() {
            return Err(Error::NullPointer);
        }

        let status = unsafe { bindings::cupsCancelDestJob(ptr::null_mut(), dest_ptr, self.id) };

        unsafe {
            let dest_box = Box::from_raw(dest_ptr);
            if !dest_box.name.is_null() {
                let _ = CString::from_raw(dest_box.name);
            }
            if !dest_box.instance.is_null() {
                let _ = CString::from_raw(dest_box.instance);
            }
            if !dest_box.options.is_null() {
                bindings::cupsFreeOptions(dest_box.num_options, dest_box.options);
            }
        }

        if status == bindings::ipp_status_e_IPP_STATUS_OK as bindings::ipp_status_t {
            Ok(())
        } else {
            let (error_code, _) = crate::error_helpers::get_cups_error_details();
            let error_msg = format!("CUPS error {}", error_code);
            Err(Error::JobManagementFailed(format!(
                "Failed to cancel job {}: {}",
                self.id, error_msg
            )))
        }
    }
}
