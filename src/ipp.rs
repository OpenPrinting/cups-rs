//! Low-level IPP (Internet Printing Protocol) request and response handling
//!
//! This module provides type-safe wrappers around CUPS IPP functions for building
//! and sending custom IPP requests. It's useful for advanced use cases that aren't
//! covered by the higher-level destination and job APIs.
//!
//! # Examples
//!
//! ## Creating and Sending an IPP Request
//!
//! ```no_run
//! use cups_rs::{IppRequest, IppOperation, IppTag, IppValueTag, ConnectionFlags, get_default_destination};
//!
//! let printer = get_default_destination().expect("No default printer");
//! let connection = printer.connect(ConnectionFlags::Scheduler, Some(5000), None)
//!     .expect("Failed to connect");
//!
//! let mut request = IppRequest::new(IppOperation::GetPrinterAttributes)
//!     .expect("Failed to create request");
//!
//! request.add_string(IppTag::Operation, IppValueTag::Uri,
//!                   "printer-uri", "ipp://localhost/printers/default")
//!     .expect("Failed to add attribute");
//!
//! let response = request.send(&connection, connection.resource_path())
//!     .expect("Failed to send request");
//!
//! if response.is_successful() {
//!     println!("Request successful!");
//! }
//! ```

use crate::bindings;
use crate::compat::{count_to_usize, usize_to_count};
use crate::connection::HttpConnection;
use crate::error::{Error, Result};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::ptr;

/// IPP attribute group tags
///
/// These tags define which group an IPP attribute belongs to in an IPP message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IppTag {
    Zero,
    Operation,
    Job,
    Printer,
    Subscription,
    EventNotification,
    Document,
    System,
    UnsupportedGroup,
}

impl From<IppTag> for bindings::ipp_tag_t {
    fn from(tag: IppTag) -> bindings::ipp_tag_t {
        match tag {
            IppTag::Zero => bindings::ipp_tag_e_IPP_TAG_ZERO,
            IppTag::Operation => bindings::ipp_tag_e_IPP_TAG_OPERATION,
            IppTag::Job => bindings::ipp_tag_e_IPP_TAG_JOB,
            IppTag::Printer => bindings::ipp_tag_e_IPP_TAG_PRINTER,
            IppTag::Subscription => bindings::ipp_tag_e_IPP_TAG_SUBSCRIPTION,
            IppTag::EventNotification => bindings::ipp_tag_e_IPP_TAG_EVENT_NOTIFICATION,
            IppTag::Document => bindings::ipp_tag_e_IPP_TAG_DOCUMENT,
            IppTag::System => bindings::ipp_tag_e_IPP_TAG_SYSTEM,
            IppTag::UnsupportedGroup => bindings::ipp_tag_e_IPP_TAG_UNSUPPORTED_GROUP,
        }
    }
}

impl IppTag {
    /// Convert a raw group tag back into a known group
    pub(crate) fn from_code(code: bindings::ipp_tag_t) -> Option<Self> {
        Some(match code {
            bindings::ipp_tag_e_IPP_TAG_ZERO => Self::Zero,
            bindings::ipp_tag_e_IPP_TAG_OPERATION => Self::Operation,
            bindings::ipp_tag_e_IPP_TAG_JOB => Self::Job,
            bindings::ipp_tag_e_IPP_TAG_PRINTER => Self::Printer,
            bindings::ipp_tag_e_IPP_TAG_SUBSCRIPTION => Self::Subscription,
            bindings::ipp_tag_e_IPP_TAG_EVENT_NOTIFICATION => Self::EventNotification,
            bindings::ipp_tag_e_IPP_TAG_DOCUMENT => Self::Document,
            bindings::ipp_tag_e_IPP_TAG_SYSTEM => Self::System,
            bindings::ipp_tag_e_IPP_TAG_UNSUPPORTED_GROUP => Self::UnsupportedGroup,
            _ => return None,
        })
    }
}

/// IPP value tags
///
/// These tags define the type of value an IPP attribute contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IppValueTag {
    Integer,
    Boolean,
    Enum,
    String,
    Text,
    Name,
    Keyword,
    Uri,
    Charset,
    Language,
    MimeType,
    BeginCollection,
}

impl From<IppValueTag> for bindings::ipp_tag_t {
    fn from(tag: IppValueTag) -> bindings::ipp_tag_t {
        match tag {
            IppValueTag::Integer => bindings::ipp_tag_e_IPP_TAG_INTEGER,
            IppValueTag::Boolean => bindings::ipp_tag_e_IPP_TAG_BOOLEAN,
            IppValueTag::Enum => bindings::ipp_tag_e_IPP_TAG_ENUM,
            IppValueTag::String => bindings::ipp_tag_e_IPP_TAG_STRING,
            IppValueTag::Text => bindings::ipp_tag_e_IPP_TAG_TEXT,
            IppValueTag::Name => bindings::ipp_tag_e_IPP_TAG_NAME,
            IppValueTag::Keyword => bindings::ipp_tag_e_IPP_TAG_KEYWORD,
            IppValueTag::Uri => bindings::ipp_tag_e_IPP_TAG_URI,
            IppValueTag::Charset => bindings::ipp_tag_e_IPP_TAG_CHARSET,
            IppValueTag::Language => bindings::ipp_tag_e_IPP_TAG_LANGUAGE,
            IppValueTag::MimeType => bindings::ipp_tag_e_IPP_TAG_MIMETYPE,
            IppValueTag::BeginCollection => bindings::ipp_tag_e_IPP_TAG_BEGIN_COLLECTION,
        }
    }
}

impl IppValueTag {
    /// Convert a raw value tag back into a known type, defaulting to `String`
    pub(crate) fn from_code(code: bindings::ipp_tag_t) -> Self {
        match code {
            bindings::ipp_tag_e_IPP_TAG_INTEGER => Self::Integer,
            bindings::ipp_tag_e_IPP_TAG_BOOLEAN => Self::Boolean,
            bindings::ipp_tag_e_IPP_TAG_ENUM => Self::Enum,
            bindings::ipp_tag_e_IPP_TAG_STRING => Self::String,
            bindings::ipp_tag_e_IPP_TAG_TEXT => Self::Text,
            bindings::ipp_tag_e_IPP_TAG_NAME => Self::Name,
            bindings::ipp_tag_e_IPP_TAG_KEYWORD => Self::Keyword,
            bindings::ipp_tag_e_IPP_TAG_URI => Self::Uri,
            bindings::ipp_tag_e_IPP_TAG_CHARSET => Self::Charset,
            bindings::ipp_tag_e_IPP_TAG_LANGUAGE => Self::Language,
            bindings::ipp_tag_e_IPP_TAG_MIMETYPE => Self::MimeType,
            bindings::ipp_tag_e_IPP_TAG_BEGIN_COLLECTION => Self::BeginCollection,
            _ => Self::String,
        }
    }

    fn is_text_like(self) -> bool {
        matches!(self, Self::Text | Self::Name | Self::Keyword | Self::Uri)
    }
}

/// IPP operation codes
///
/// These codes identify the operation being performed in an IPP request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IppOperation {
    PrintJob,
    ValidateJob,
    CreateJob,
    SendDocument,
    CancelJob,
    GetJobAttributes,
    GetJobs,
    GetPrinterAttributes,
    HoldJob,
    ReleaseJob,
    PausePrinter,
    ResumePrinter,
    CupsAddModifyPrinter,
    CupsCreateLocalPrinter,
    CupsDeletePrinter,
    CupsSetDefault,
    CupsMoveJob,
    SetPrinterAttributes,
    EnablePrinter,
    DisablePrinter,
    CreatePrinter,
    DeletePrinter,
    GetPrinters,
    GetSystemAttributes,
    /// An operation code not covered above, e.g. a vendor extension
    Other(u16),
}

impl From<IppOperation> for bindings::ipp_op_t {
    fn from(op: IppOperation) -> bindings::ipp_op_t {
        match op {
            IppOperation::PrintJob => bindings::ipp_op_e_IPP_OP_PRINT_JOB,
            IppOperation::ValidateJob => bindings::ipp_op_e_IPP_OP_VALIDATE_JOB,
            IppOperation::CreateJob => bindings::ipp_op_e_IPP_OP_CREATE_JOB,
            IppOperation::SendDocument => bindings::ipp_op_e_IPP_OP_SEND_DOCUMENT,
            IppOperation::CancelJob => bindings::ipp_op_e_IPP_OP_CANCEL_JOB,
            IppOperation::GetJobAttributes => bindings::ipp_op_e_IPP_OP_GET_JOB_ATTRIBUTES,
            IppOperation::GetJobs => bindings::ipp_op_e_IPP_OP_GET_JOBS,
            IppOperation::GetPrinterAttributes => bindings::ipp_op_e_IPP_OP_GET_PRINTER_ATTRIBUTES,
            IppOperation::HoldJob => bindings::ipp_op_e_IPP_OP_HOLD_JOB,
            IppOperation::ReleaseJob => bindings::ipp_op_e_IPP_OP_RELEASE_JOB,
            IppOperation::PausePrinter => bindings::ipp_op_e_IPP_OP_PAUSE_PRINTER,
            IppOperation::ResumePrinter => bindings::ipp_op_e_IPP_OP_RESUME_PRINTER,
            IppOperation::CupsAddModifyPrinter => bindings::ipp_op_e_IPP_OP_CUPS_ADD_MODIFY_PRINTER,
            IppOperation::CupsCreateLocalPrinter => {
                bindings::ipp_op_e_IPP_OP_CUPS_CREATE_LOCAL_PRINTER
            }
            IppOperation::CupsDeletePrinter => bindings::ipp_op_e_IPP_OP_CUPS_DELETE_PRINTER,
            IppOperation::CupsSetDefault => bindings::ipp_op_e_IPP_OP_CUPS_SET_DEFAULT,
            IppOperation::CupsMoveJob => bindings::ipp_op_e_IPP_OP_CUPS_MOVE_JOB,
            IppOperation::SetPrinterAttributes => bindings::ipp_op_e_IPP_OP_SET_PRINTER_ATTRIBUTES,
            IppOperation::EnablePrinter => bindings::ipp_op_e_IPP_OP_ENABLE_PRINTER,
            IppOperation::DisablePrinter => bindings::ipp_op_e_IPP_OP_DISABLE_PRINTER,
            IppOperation::CreatePrinter => bindings::ipp_op_e_IPP_OP_CREATE_PRINTER,
            IppOperation::DeletePrinter => bindings::ipp_op_e_IPP_OP_DELETE_PRINTER,
            IppOperation::GetPrinters => bindings::ipp_op_e_IPP_OP_GET_PRINTERS,
            IppOperation::GetSystemAttributes => bindings::ipp_op_e_IPP_OP_GET_SYSTEM_ATTRIBUTES,
            IppOperation::Other(code) => bindings::ipp_op_t::from(code),
        }
    }
}

/// IPP status codes
///
/// These codes indicate the result of an IPP operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IppStatus {
    Ok,
    OkIgnoredOrSubstituted,
    OkConflicting,
    ErrorBadRequest,
    ErrorForbidden,
    ErrorNotAuthenticated,
    ErrorNotAuthorized,
    ErrorNotPossible,
    ErrorTimeout,
    ErrorNotFound,
    ErrorGone,
    ErrorRequestEntity,
    ErrorRequestValue,
    ErrorDocumentFormatNotSupported,
    ErrorOperationNotSupported,
    ErrorConflicting,
    ErrorAttributesNotSettable,
    ErrorPrinterIsDeactivated,
    ErrorTooManyJobs,
    ErrorInternalError,
}

impl IppStatus {
    pub fn from_code(code: bindings::ipp_status_t) -> Self {
        match code {
            bindings::ipp_status_e_IPP_STATUS_OK => IppStatus::Ok,
            bindings::ipp_status_e_IPP_STATUS_OK_IGNORED_OR_SUBSTITUTED => {
                IppStatus::OkIgnoredOrSubstituted
            }
            bindings::ipp_status_e_IPP_STATUS_OK_CONFLICTING => IppStatus::OkConflicting,
            bindings::ipp_status_e_IPP_STATUS_ERROR_BAD_REQUEST => IppStatus::ErrorBadRequest,
            bindings::ipp_status_e_IPP_STATUS_ERROR_FORBIDDEN => IppStatus::ErrorForbidden,
            bindings::ipp_status_e_IPP_STATUS_ERROR_NOT_AUTHENTICATED => {
                IppStatus::ErrorNotAuthenticated
            }
            bindings::ipp_status_e_IPP_STATUS_ERROR_NOT_AUTHORIZED => IppStatus::ErrorNotAuthorized,
            bindings::ipp_status_e_IPP_STATUS_ERROR_NOT_POSSIBLE => IppStatus::ErrorNotPossible,
            bindings::ipp_status_e_IPP_STATUS_ERROR_TIMEOUT => IppStatus::ErrorTimeout,
            bindings::ipp_status_e_IPP_STATUS_ERROR_NOT_FOUND => IppStatus::ErrorNotFound,
            bindings::ipp_status_e_IPP_STATUS_ERROR_GONE => IppStatus::ErrorGone,
            bindings::ipp_status_e_IPP_STATUS_ERROR_REQUEST_ENTITY => IppStatus::ErrorRequestEntity,
            bindings::ipp_status_e_IPP_STATUS_ERROR_REQUEST_VALUE => IppStatus::ErrorRequestValue,
            bindings::ipp_status_e_IPP_STATUS_ERROR_DOCUMENT_FORMAT_NOT_SUPPORTED => {
                IppStatus::ErrorDocumentFormatNotSupported
            }
            bindings::ipp_status_e_IPP_STATUS_ERROR_OPERATION_NOT_SUPPORTED => {
                IppStatus::ErrorOperationNotSupported
            }
            bindings::ipp_status_e_IPP_STATUS_ERROR_CONFLICTING => IppStatus::ErrorConflicting,
            bindings::ipp_status_e_IPP_STATUS_ERROR_ATTRIBUTES_NOT_SETTABLE => {
                IppStatus::ErrorAttributesNotSettable
            }
            bindings::ipp_status_e_IPP_STATUS_ERROR_PRINTER_IS_DEACTIVATED => {
                IppStatus::ErrorPrinterIsDeactivated
            }
            bindings::ipp_status_e_IPP_STATUS_ERROR_TOO_MANY_JOBS => IppStatus::ErrorTooManyJobs,
            bindings::ipp_status_e_IPP_STATUS_ERROR_INTERNAL => IppStatus::ErrorInternalError,
            _ => IppStatus::ErrorInternalError,
        }
    }

    pub fn is_successful(&self) -> bool {
        matches!(
            self,
            IppStatus::Ok | IppStatus::OkIgnoredOrSubstituted | IppStatus::OkConflicting
        )
    }
}

/// An IPP request message
///
/// Represents an IPP request that can be customized with attributes and sent to a CUPS server.
/// The request is automatically freed when dropped.
///
/// # Examples
///
/// ```no_run
/// use cups_rs::{IppRequest, IppOperation, IppTag, IppValueTag};
///
/// let mut request = IppRequest::new(IppOperation::GetPrinterAttributes)
///     .expect("Failed to create request");
///
/// request.add_string(IppTag::Operation, IppValueTag::Keyword,
///                   "requested-attributes", "printer-state")
///     .expect("Failed to add attribute");
/// ```
pub struct IppRequest {
    ipp: *mut bindings::_ipp_s,
    _phantom: PhantomData<bindings::_ipp_s>,
}

impl IppRequest {
    /// Create a new IPP request
    pub fn new(operation: IppOperation) -> Result<Self> {
        let ipp = unsafe { bindings::ippNewRequest(operation.into()) };

        if ipp.is_null() {
            return Err(Error::UnsupportedFeature(
                "Failed to create IPP request".to_string(),
            ));
        }

        Ok(IppRequest {
            ipp,
            _phantom: PhantomData,
        })
    }

    /// Get the raw pointer to the ipp_t structure
    pub fn as_ptr(&self) -> *mut bindings::_ipp_s {
        self.ipp
    }

    /// Add a string attribute
    pub fn add_string(
        &mut self,
        group: IppTag,
        value_tag: IppValueTag,
        name: &str,
        value: &str,
    ) -> Result<()> {
        let name_c = CString::new(name)?;
        let value_c = CString::new(value)?;

        let attr = unsafe {
            bindings::ippAddString(
                self.ipp,
                group.into(),
                value_tag.into(),
                name_c.as_ptr(),
                ptr::null(),
                value_c.as_ptr(),
            )
        };

        if attr.is_null() {
            Err(Error::UnsupportedFeature(format!(
                "Failed to add string attribute '{}'",
                name
            )))
        } else {
            Ok(())
        }
    }

    /// Add an integer attribute
    pub fn add_integer(
        &mut self,
        group: IppTag,
        value_tag: IppValueTag,
        name: &str,
        value: i32,
    ) -> Result<()> {
        let name_c = CString::new(name)?;

        let attr = unsafe {
            bindings::ippAddInteger(
                self.ipp,
                group.into(),
                value_tag.into(),
                name_c.as_ptr(),
                value,
            )
        };

        if attr.is_null() {
            Err(Error::UnsupportedFeature(format!(
                "Failed to add integer attribute '{}'",
                name
            )))
        } else {
            Ok(())
        }
    }

    /// Add a boolean attribute
    pub fn add_boolean(&mut self, group: IppTag, name: &str, value: bool) -> Result<()> {
        let name_c = CString::new(name)?;

        #[cfg(cups3)]
        let attr =
            unsafe { bindings::ippAddBoolean(self.ipp, group.into(), name_c.as_ptr(), value) };

        #[cfg(cups2)]
        let attr = unsafe {
            bindings::ippAddBoolean(
                self.ipp,
                group.into(),
                name_c.as_ptr(),
                if value { 1 } else { 0 },
            )
        };

        if attr.is_null() {
            Err(Error::UnsupportedFeature(format!(
                "Failed to add boolean attribute '{}'",
                name
            )))
        } else {
            Ok(())
        }
    }

    /// Add multiple string attributes
    pub fn add_strings(
        &mut self,
        group: IppTag,
        value_tag: IppValueTag,
        name: &str,
        values: &[&str],
    ) -> Result<()> {
        let name_c = CString::new(name)?;
        let values_c: Vec<CString> = values
            .iter()
            .map(|v| CString::new(*v).map_err(Error::from))
            .collect::<Result<Vec<_>>>()?;

        let values_ptrs: Vec<*const ::std::os::raw::c_char> =
            values_c.iter().map(|s| s.as_ptr()).collect();

        let attr = unsafe {
            bindings::ippAddStrings(
                self.ipp,
                group.into(),
                value_tag.into(),
                name_c.as_ptr(),
                usize_to_count(values.len()),
                ptr::null(),
                values_ptrs.as_ptr(),
            )
        };

        if attr.is_null() {
            Err(Error::UnsupportedFeature(format!(
                "Failed to add string array attribute '{}'",
                name
            )))
        } else {
            Ok(())
        }
    }

    /// Send this request and receive a response
    pub fn send(&self, connection: &HttpConnection, resource: &str) -> Result<IppResponse> {
        let resource_c = CString::new(resource)?;

        // Note: cupsDoRequest frees the request, so we need to create a copy
        // create an empty IPP message for the outgoing copy
        let request_copy = unsafe { bindings::ippNew() };
        if request_copy.is_null() {
            return Err(Error::UnsupportedFeature(
                "Failed to copy IPP request".to_string(),
            ));
        }

        unsafe {
            // Copy request header fields
            bindings::ippSetOperation(request_copy, bindings::ippGetOperation(self.ipp));
            bindings::ippSetRequestId(request_copy, bindings::ippGetRequestId(self.ipp));

            // Copy all attributes
            #[cfg(cups3)]
            bindings::ippCopyAttributes(request_copy, self.ipp, false, None, ptr::null_mut());

            #[cfg(cups2)]
            bindings::ippCopyAttributes(request_copy, self.ipp, 0, None, ptr::null_mut());
        }

        let response = unsafe {
            bindings::cupsDoRequest(connection.as_ptr(), request_copy, resource_c.as_ptr())
        };

        if response.is_null() {
            Err(Error::ServerError(
                "No response received from server".to_string(),
            ))
        } else {
            Ok(IppResponse {
                ipp: response,
                _phantom: PhantomData,
            })
        }
    }

    /// Send this request to the default CUPS scheduler connection.
    pub fn send_default(&self, resource: &str) -> Result<IppResponse> {
        let resource_c = CString::new(resource)?;

        let request_copy = unsafe { bindings::ippNew() };
        if request_copy.is_null() {
            return Err(Error::UnsupportedFeature(
                "Failed to copy IPP request".to_string(),
            ));
        }

        unsafe {
            bindings::ippSetOperation(request_copy, bindings::ippGetOperation(self.ipp));
            bindings::ippSetRequestId(request_copy, bindings::ippGetRequestId(self.ipp));

            #[cfg(cups3)]
            bindings::ippCopyAttributes(request_copy, self.ipp, false, None, ptr::null_mut());

            #[cfg(cups2)]
            bindings::ippCopyAttributes(request_copy, self.ipp, 0, None, ptr::null_mut());
        }

        let response =
            unsafe { bindings::cupsDoRequest(ptr::null_mut(), request_copy, resource_c.as_ptr()) };

        if response.is_null() {
            Err(Error::ServerError(
                "No response received from server".to_string(),
            ))
        } else {
            Ok(IppResponse {
                ipp: response,
                _phantom: PhantomData,
            })
        }
    }
}

impl Drop for IppRequest {
    fn drop(&mut self) {
        if !self.ipp.is_null() {
            unsafe {
                bindings::ippDelete(self.ipp);
            }
            self.ipp = ptr::null_mut();
        }
    }
}

/// An IPP response message
///
/// Represents the response from an IPP request. Contains status code and attributes
/// that can be queried. The response is automatically freed when dropped.
///
/// # Examples
///
/// ```no_run
/// # use cups_rs::{IppRequest, IppOperation, IppTag, ConnectionFlags, get_default_destination};
/// # let printer = get_default_destination().unwrap();
/// # let connection = printer.connect(ConnectionFlags::Scheduler, Some(5000), None).unwrap();
/// # let request = IppRequest::new(IppOperation::GetPrinterAttributes).unwrap();
/// let response = request.send(&connection, connection.resource_path()).unwrap();
///
/// if response.is_successful() {
///     if let Some(attr) = response.find_attribute("printer-state", Some(IppTag::Printer)) {
///         println!("Printer state: {:?}", attr.get_integer(0));
///     }
/// }
/// ```
pub struct IppResponse {
    ipp: *mut bindings::_ipp_s,
    _phantom: PhantomData<bindings::_ipp_s>,
}

impl IppResponse {
    /// Get the raw pointer to the ipp_t structure
    pub fn as_ptr(&self) -> *mut bindings::_ipp_s {
        self.ipp
    }

    /// Get the status code from the response
    pub fn status(&self) -> IppStatus {
        let status_code = unsafe { bindings::ippGetStatusCode(self.ipp) };
        IppStatus::from_code(status_code)
    }

    /// Get the raw status code
    pub fn status_code(&self) -> u16 {
        unsafe { bindings::ippGetStatusCode(self.ipp) as u16 }
    }

    /// Check if the response indicates success
    pub fn is_successful(&self) -> bool {
        self.status().is_successful()
    }

    /// Get every attribute with this name
    pub fn attributes_named(&self, name: &str) -> Vec<IppAttribute> {
        let Ok(name_c) = CString::new(name) else {
            return Vec::new();
        };

        let mut found = Vec::new();
        let mut attr = unsafe {
            bindings::ippFindAttribute(self.ipp, name_c.as_ptr(), bindings::ipp_tag_e_IPP_TAG_ZERO)
        };
        while !attr.is_null() {
            found.push(IppAttribute { attr });
            attr = unsafe {
                bindings::ippFindNextAttribute(
                    self.ipp,
                    name_c.as_ptr(),
                    bindings::ipp_tag_e_IPP_TAG_ZERO,
                )
            };
        }

        found
    }

    /// Find an attribute by name
    pub fn find_attribute(&self, name: &str, group: Option<IppTag>) -> Option<IppAttribute> {
        let name_c = match CString::new(name) {
            Ok(s) => s,
            Err(_) => return None,
        };

        let group_tag = group
            .map(|g| g.into())
            .unwrap_or(bindings::ipp_tag_e_IPP_TAG_ZERO);

        let attr = unsafe { bindings::ippFindAttribute(self.ipp, name_c.as_ptr(), group_tag) };

        if attr.is_null() {
            None
        } else {
            Some(IppAttribute { attr })
        }
    }

    /// Get all attributes in the response
    pub fn attributes(&self) -> Vec<IppAttribute> {
        let mut attributes = Vec::new();

        #[cfg(cups3)]
        let mut attr = unsafe { bindings::ippGetFirstAttribute(self.ipp) };

        #[cfg(cups2)]
        let mut attr = unsafe { bindings::ippFirstAttribute(self.ipp) };

        while !attr.is_null() {
            attributes.push(IppAttribute { attr });

            #[cfg(cups3)]
            {
                attr = unsafe { bindings::ippGetNextAttribute(self.ipp) };
            }

            #[cfg(cups2)]
            {
                attr = unsafe { bindings::ippNextAttribute(self.ipp) };
            }
        }

        attributes
    }
}

impl Drop for IppResponse {
    fn drop(&mut self) {
        if !self.ipp.is_null() {
            unsafe {
                bindings::ippDelete(self.ipp);
            }
            self.ipp = ptr::null_mut();
        }
    }
}

/// An IPP attribute
///
/// Represents a single attribute from an IPP response. Attributes can contain
/// one or more values of various types (string, integer, boolean, etc.).
#[derive(Clone, Copy)]
pub struct IppAttribute {
    attr: *mut bindings::_ipp_attribute_s,
}

impl IppAttribute {
    /// Get the attribute name
    pub fn name(&self) -> Option<String> {
        unsafe {
            let name_ptr = bindings::ippGetName(self.attr);
            if name_ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(name_ptr).to_string_lossy().into_owned())
            }
        }
    }

    /// Get the number of values
    pub fn count(&self) -> usize {
        count_to_usize(unsafe { bindings::ippGetCount(self.attr) })
    }

    /// Get the value type
    pub fn value_tag(&self) -> IppValueTag {
        IppValueTag::from_code(unsafe { bindings::ippGetValueTag(self.attr) })
    }

    /// Get the attribute group
    pub fn group_tag(&self) -> Option<IppTag> {
        IppTag::from_code(unsafe { bindings::ippGetGroupTag(self.attr) })
    }

    /// Get a string value
    pub fn get_string(&self, index: usize) -> Option<String> {
        unsafe {
            let value_ptr =
                bindings::ippGetString(self.attr, usize_to_count(index), ptr::null_mut());
            if value_ptr.is_null() {
                None
            } else {
                Some(CStr::from_ptr(value_ptr).to_string_lossy().into_owned())
            }
        }
    }

    /// Get an octetString value
    pub fn get_octet_string(&self, index: usize) -> Option<Vec<u8>> {
        unsafe {
            #[cfg(cups3)]
            let mut length: usize = 0;

            #[cfg(cups2)]
            let mut length: i32 = 0;

            #[cfg(cups3)]
            let data = bindings::ippGetOctetString(self.attr, index, &mut length);

            #[cfg(cups2)]
            let data = bindings::ippGetOctetString(self.attr, usize_to_count(index), &mut length);

            if data.is_null() {
                None
            } else {
                Some(std::slice::from_raw_parts(data as *const u8, length as usize).to_vec())
            }
        }
    }

    /// Get an integer value
    pub fn get_integer(&self, index: usize) -> i32 {
        unsafe { bindings::ippGetInteger(self.attr, usize_to_count(index)) }
    }

    /// Get a boolean value
    pub fn get_boolean(&self, index: usize) -> bool {
        let value = unsafe { bindings::ippGetBoolean(self.attr, usize_to_count(index)) };

        #[cfg(cups3)]
        {
            value
        }

        #[cfg(cups2)]
        {
            value != 0
        }
    }

    /// Get collection values as name-value maps
    pub fn collections(&self) -> Vec<HashMap<String, String>> {
        if self.value_tag() != IppValueTag::BeginCollection {
            return Vec::new();
        }

        (0..self.count())
            .filter_map(|index| self.collection_at(index))
            .collect()
    }

    fn collection_at(&self, index: usize) -> Option<HashMap<String, String>> {
        let collection = unsafe { bindings::ippGetCollection(self.attr, usize_to_count(index)) };
        if collection.is_null() {
            return None;
        }

        let mut members = HashMap::new();

        #[cfg(cups3)]
        let mut attr = unsafe { bindings::ippGetFirstAttribute(collection) };

        #[cfg(cups2)]
        let mut attr = unsafe { bindings::ippFirstAttribute(collection) };

        while !attr.is_null() {
            let member = IppAttribute { attr };
            if let Some(name) = member.name()
                && member.value_tag().is_text_like()
                && let Some(value) = member.get_string(0)
            {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    members.insert(name, trimmed.to_string());
                }
            }

            #[cfg(cups3)]
            {
                attr = unsafe { bindings::ippGetNextAttribute(collection) };
            }

            #[cfg(cups2)]
            {
                attr = unsafe { bindings::ippNextAttribute(collection) };
            }
        }

        Some(members)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipp_request_creation() {
        let request = IppRequest::new(IppOperation::GetPrinterAttributes);
        assert!(request.is_ok());
    }

    #[test]
    fn test_ipp_add_string() {
        let mut request = IppRequest::new(IppOperation::GetPrinterAttributes).unwrap();
        let result = request.add_string(
            IppTag::Operation,
            IppValueTag::Uri,
            "printer-uri",
            "ipp://localhost/printers/test",
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_ipp_add_integer() {
        let mut request = IppRequest::new(IppOperation::GetJobs).unwrap();
        let result = request.add_integer(IppTag::Operation, IppValueTag::Integer, "limit", 10);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ipp_add_boolean() {
        let mut request = IppRequest::new(IppOperation::GetJobs).unwrap();
        let result = request.add_boolean(IppTag::Operation, "my-jobs", true);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ipp_status() {
        assert!(IppStatus::Ok.is_successful());
        assert!(IppStatus::OkIgnoredOrSubstituted.is_successful());
        assert!(!IppStatus::ErrorBadRequest.is_successful());
        assert!(!IppStatus::ErrorNotFound.is_successful());
    }

    #[test]
    fn test_ipp_request_send_preserves_operation() {
        use crate::{ConnectionFlags, get_default_destination};

        // Skip test if no CUPS server
        let printer = match get_default_destination() {
            Ok(p) => p,
            Err(_) => return,
        };

        // Skip test if connection fails
        let connection = match printer.connect(ConnectionFlags::Scheduler, Some(5000), None) {
            Ok(c) => c,
            Err(_) => return,
        };

        // Create a GetPrinterAttributes request
        let mut request = IppRequest::new(IppOperation::GetPrinterAttributes).unwrap();

        // Use the actual printer URI if available, otherwise fallback to a plausible one
        let uri = printer
            .uri()
            .cloned()
            .unwrap_or_else(|| "ipp://localhost/printers/default".to_string());

        // Add minimal required attributes
        request
            .add_string(IppTag::Operation, IppValueTag::Uri, "printer-uri", &uri)
            .unwrap();

        // Post to the specific printer resource path, not the scheduler root
        let resource = uri
            .strip_prefix("ipp://")
            .or_else(|| uri.strip_prefix("ipps://"))
            .and_then(|rest| rest.split_once('/').map(|(_, path)| format!("/{}", path)))
            .unwrap_or_else(|| "/".to_string());

        // Send the request
        let response = request.send(&connection, &resource);

        // If the operation code was LOST (became 0), CUPS returns ErrorBadRequest (0x0400).
        // Since we preserved it, this should return a successful response or another error.
        if let Ok(resp) = response {
            assert_ne!(
                resp.status(),
                IppStatus::ErrorBadRequest,
                "Operation code was lost in send() copy"
            );
        }
    }
}
