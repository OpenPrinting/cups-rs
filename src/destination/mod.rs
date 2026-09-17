mod dest_info;
mod media_size;
mod printer_state;

pub use dest_info::DestinationInfo;
pub use media_size::MediaSize;
pub use printer_state::PrinterState;

use crate::bindings;
use crate::compat::{CupsCount, count_to_usize, usize_to_count};
use crate::constants;
use crate::error::{Error, Result};
use crate::error_helpers::cups_error_to_our_error;
use crate::{HttpConnection, IppOperation, IppRequest, IppTag, IppValueTag};
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::os::raw::{c_int, c_uint, c_void};
use std::ptr;

pub type DestCallback<T> = dyn FnMut(u32, &Destination, &mut T) -> bool;

/// Represents a printer or class of printers available for printing
#[derive(Debug, Clone)]
pub struct Destination {
    /// Name of the destination
    pub name: String,
    /// Instance name or None for the default instance
    pub instance: Option<String>,
    /// True if this is the default destination
    pub is_default: bool,
    /// Options and attributes for this destination
    pub options: HashMap<String, String>,
}

#[derive(Clone, Copy)]
pub enum AttrKind {
    StringLike,
    IntegerLike,
    Boolean,
}

#[derive(Clone, Copy)]
pub struct AttrSpec<'a> {
    pub name: &'a str,
    pub kind: AttrKind,
}

impl Destination {
    /// Create a new Destination instance from raw cups_dest_t pointer
    pub(crate) unsafe fn from_raw(dest_ptr: *const bindings::cups_dest_s) -> Result<Self> {
        if dest_ptr.is_null() {
            return Err(Error::NullPointer);
        }

        let dest = unsafe { &*dest_ptr };
        // Extract name
        let name = if dest.name.is_null() {
            return Err(Error::NullPointer);
        } else {
            unsafe { CStr::from_ptr(dest.name) }
                .to_string_lossy()
                .into_owned()
        };

        // Extract instance (if any)
        let instance = if dest.instance.is_null() {
            None
        } else {
            Some(
                unsafe { CStr::from_ptr(dest.instance) }
                    .to_string_lossy()
                    .into_owned(),
            )
        };

        // Extract options
        let mut options = HashMap::new();
        if !dest.options.is_null() && dest.num_options > 0 {
            for i in 0..dest.num_options {
                unsafe {
                    let option = &*(dest.options.offset(i as isize));
                    if !option.name.is_null() && !option.value.is_null() {
                        let name = CStr::from_ptr(option.name).to_string_lossy().into_owned();
                        let value = CStr::from_ptr(option.value).to_string_lossy().into_owned();
                        options.insert(name, value);
                    }
                }
            }
        }

        Ok(Destination {
            name,
            instance,
            is_default: dest_is_default(dest.is_default),
            options,
        })
    }

    /// Get the state of this destination
    pub fn state(&self) -> PrinterState {
        match self.options.get("printer-state") {
            Some(state) => PrinterState::from_cups_state(state),
            None => PrinterState::Unknown,
        }
    }

    /// Get the reasons for the current state
    pub fn state_reasons(&self) -> Vec<String> {
        match self.options.get("printer-state-reasons") {
            Some(reasons) => reasons.split(',').map(|s| s.trim().to_string()).collect(),
            None => Vec::new(),
        }
    }

    /// Get a human-readable description of this destination
    pub fn info(&self) -> Option<&String> {
        self.options.get("printer-info")
    }

    /// Get the location of this destination
    pub fn location(&self) -> Option<&String> {
        self.options.get("printer-location")
    }

    /// Get the make and model of this destination
    pub fn make_and_model(&self) -> Option<&String> {
        self.options.get("printer-make-and-model")
    }

    /// Check if the destination is accepting jobs
    pub fn is_accepting_jobs(&self) -> bool {
        match self.options.get("printer-is-accepting-jobs") {
            Some(value) => value == "true",
            None => false,
        }
    }

    /// Get the URI associated with this destination
    pub fn uri(&self) -> Option<&String> {
        self.options.get("printer-uri-supported")
    }

    /// Get the device URI for this destination
    pub fn device_uri(&self) -> Option<&String> {
        self.options.get("device-uri")
    }

    /// Get the full name of this destination (including instance if any)
    pub fn full_name(&self) -> String {
        match &self.instance {
            Some(inst) => format!("{}/{}", self.name, inst),
            None => self.name.clone(),
        }
    }

    /// Get an option value by name
    pub fn get_option(&self, name: &str) -> Option<&String> {
        self.options.get(name)
    }

    /// Check if an option is present
    pub fn has_option(&self, name: &str) -> bool {
        self.options.contains_key(name)
    }

    /// Get all options
    pub fn get_options(&self) -> &HashMap<String, String> {
        &self.options
    }

    /// Get detailed information about this destination
    pub fn get_detailed_info(&self, http: *mut bindings::_http_s) -> Result<DestinationInfo> {
        let name_c = CString::new(self.name.as_str())?;
        let instance_c = match &self.instance {
            Some(instance) => Some(CString::new(instance.as_str())?),
            None => None,
        };

        let _instance_ptr = match &instance_c {
            Some(s) => s.as_ptr(),
            None => ptr::null(),
        };

        let mut num_options = 0;
        let mut options_ptr: *mut bindings::cups_option_s = ptr::null_mut();

        for (name, value) in &self.options {
            let name_c = CString::new(name.as_str())?;
            let value_c = CString::new(value.as_str())?;

            unsafe {
                num_options = bindings::cupsAddOption(
                    name_c.as_ptr(),
                    value_c.as_ptr(),
                    num_options,
                    &mut options_ptr,
                );
            }
        }

        let dest = bindings::cups_dest_s {
            name: name_c.into_raw(),
            instance: match instance_c {
                Some(s) => s.into_raw(),
                None => ptr::null_mut(),
            },
            is_default: raw_is_default(self.is_default),
            num_options,
            options: options_ptr,
        };

        let dest_ptr = &dest as *const bindings::cups_dest_s as *mut bindings::cups_dest_s;

        #[cfg(cups3)]
        let dinfo = unsafe { bindings::cupsCopyDestInfo(http, dest_ptr, 0) };

        #[cfg(cups2)]
        let dinfo = unsafe { bindings::cupsCopyDestInfo(http, dest_ptr) };

        unsafe {
            // cupsCopyDestInfo can reallocate the options array, so free the
            // current one rather than the one passed in
            if !dest.options.is_null() {
                bindings::cupsFreeOptions(dest.num_options, dest.options);
            }

            if !dest.name.is_null() {
                let _ = CString::from_raw(dest.name);
            }

            if !dest.instance.is_null() {
                let _ = CString::from_raw(dest.instance);
            }
        }

        if dinfo.is_null() {
            return Err(cups_error_to_our_error(
                "get destination info",
                Some(&self.name),
            ));
        }

        unsafe { DestinationInfo::from_raw(dinfo) }
    }

    /// Check if a specific option and value is supported by this destination
    pub fn is_option_supported(&self, http: *mut bindings::_http_s, option: &str) -> bool {
        match self.get_detailed_info(http) {
            Ok(info) => {
                // Create a raw cups_dest_t for this destination
                let name_c = match CString::new(self.name.as_str()) {
                    Ok(s) => s,
                    Err(_) => return false,
                };

                let instance_c = match &self.instance {
                    Some(instance) => match CString::new(instance.as_str()) {
                        Ok(s) => Some(s),
                        Err(_) => return false,
                    },
                    None => None,
                };

                let _instance_ptr = match &instance_c {
                    Some(s) => s.as_ptr(),
                    None => ptr::null(),
                };

                let mut num_options = 0;
                let mut options_ptr: *mut bindings::cups_option_s = ptr::null_mut();

                // Add all options
                for (name, value) in &self.options {
                    let name_c = match CString::new(name.as_str()) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                    let value_c = match CString::new(value.as_str()) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                    unsafe {
                        num_options = bindings::cupsAddOption(
                            name_c.as_ptr(),
                            value_c.as_ptr(),
                            num_options,
                            &mut options_ptr,
                        );
                    }
                }

                let mut dest = bindings::cups_dest_s {
                    name: name_c.into_raw(),
                    instance: match instance_c {
                        Some(s) => s.into_raw(),
                        None => ptr::null_mut(),
                    },
                    is_default: raw_is_default(self.is_default),
                    num_options,
                    options: options_ptr,
                };

                // Check if the option is supported
                let result = info.is_option_supported(http, &mut dest, option);

                // Free the resources
                unsafe {
                    if !dest.options.is_null() {
                        bindings::cupsFreeOptions(dest.num_options, dest.options);
                    }

                    // Need to free the raw strings we created
                    if !dest.name.is_null() {
                        let _ = CString::from_raw(dest.name);
                    }

                    if !dest.instance.is_null() {
                        let _ = CString::from_raw(dest.instance);
                    }
                }

                result
            }
            Err(_) => false,
        }
    }

    /// Get a pointer to a raw cups_dest_s for this destination
    pub fn as_ptr(&self) -> *mut bindings::cups_dest_s {
        // Create a raw cups_dest_t for this destination
        let name_c = match CString::new(self.name.as_str()) {
            Ok(s) => s,
            Err(_) => return ptr::null_mut(),
        };

        let instance_c = match &self.instance {
            Some(instance) => match CString::new(instance.as_str()) {
                Ok(s) => Some(s),
                Err(_) => return ptr::null_mut(),
            },
            None => None,
        };

        let _instance_ptr = match &instance_c {
            Some(s) => s.as_ptr(),
            None => ptr::null(),
        };

        let mut num_options = 0;
        let mut options_ptr: *mut bindings::cups_option_s = ptr::null_mut();

        // Add all options
        for (name, value) in &self.options {
            let name_c = match CString::new(name.as_str()) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let value_c = match CString::new(value.as_str()) {
                Ok(s) => s,
                Err(_) => continue,
            };

            unsafe {
                num_options = bindings::cupsAddOption(
                    name_c.as_ptr(),
                    value_c.as_ptr(),
                    num_options,
                    &mut options_ptr,
                );
            }
        }

        // Create the raw cups_dest_s struct
        let dest = Box::new(bindings::cups_dest_s {
            name: name_c.into_raw(),
            instance: match instance_c {
                Some(s) => s.into_raw(),
                None => ptr::null_mut(),
            },
            is_default: raw_is_default(self.is_default),
            num_options,
            options: options_ptr,
        });

        // Leak the box to keep the memory alive
        Box::into_raw(dest)
    }

    /// Fetch and populate missing attributes from the printer via IPP
    ///
    /// Note: The caller is responsible for passing an `HttpConnection` connected
    /// to the correct CUPS server for this destination.
    pub fn get_attrs(&mut self, conn: &HttpConnection, attrs: &[AttrSpec<'_>]) -> Result<()> {
        let missing: Vec<AttrSpec<'_>> = attrs
            .iter()
            .copied()
            .filter(|a| !self.options.contains_key(a.name))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }

        let uri = match self.options.get("printer-uri-supported") {
            Some(u) => u.as_str(),
            None => {
                return Err(Error::UnsupportedFeature(
                    "printer-uri-supported missing".to_string(),
                ));
            }
        };

        // Build GetPrinterAttributes
        let mut req = IppRequest::new(IppOperation::GetPrinterAttributes)?;
        req.add_string(IppTag::Operation, IppValueTag::Uri, "printer-uri", uri)?;

        // requested-attributes by names
        let names: Vec<&str> = missing.iter().map(|a| a.name).collect();
        req.add_strings(
            IppTag::Operation,
            IppValueTag::Keyword,
            "requested-attributes",
            &names,
        )?;

        // Post to the specific printer resource path, not the scheduler root
        let resource = uri
            .strip_prefix("ipp://")
            .or_else(|| uri.strip_prefix("ipps://"))
            .and_then(|rest| rest.split_once('/').map(|(_, path)| format!("/{}", path)))
            .ok_or_else(|| {
                Error::UnsupportedFeature(format!("invalid printer-uri-supported: {uri}"))
            })?;

        let resp = req.send(conn, &resource)?;

        for spec in missing {
            let Some(attr) = resp.find_attribute(spec.name, None) else {
                continue;
            };

            let mut vals: Vec<String> = Vec::new();
            for i in 0..attr.count() {
                match spec.kind {
                    AttrKind::StringLike => {
                        if let Some(s) = attr.get_string(i) {
                            let s = s.trim().to_string();
                            if !s.is_empty() {
                                vals.push(s);
                            }
                        }
                    }
                    AttrKind::IntegerLike => {
                        vals.push(attr.get_integer(i).to_string());
                    }
                    AttrKind::Boolean => {
                        vals.push(if attr.get_boolean(i) { "true" } else { "false" }.to_string());
                    }
                }
            }

            if !vals.is_empty() {
                self.options.insert(spec.name.to_string(), vals.join(","));
            }
        }

        Ok(())
    }
}

/// A collection of CUPS destinations with automatic cleanup
pub struct Destinations {
    dests: *mut bindings::cups_dest_s,
    num_dests: CupsCount,
    _marker: PhantomData<bindings::cups_dest_s>,
}

impl Destinations {
    /// Create a new empty destinations list
    pub fn new() -> Self {
        Destinations {
            dests: ptr::null_mut(),
            num_dests: 0,
            _marker: PhantomData,
        }
    }

    /// Get all available destinations from the default CUPS server
    pub fn get_all() -> Result<Self> {
        let mut dests: *mut bindings::cups_dest_s = ptr::null_mut();

        #[cfg(cups3)]
        let num_dests = unsafe { bindings::cupsGetDests(ptr::null_mut(), &mut dests) };

        #[cfg(cups2)]
        let num_dests = unsafe { bindings::cupsGetDests(&mut dests) };

        if num_dests <= 0 || dests.is_null() {
            return Err(Error::DestinationListFailed);
        }

        Ok(Destinations {
            dests,
            num_dests,
            _marker: PhantomData,
        })
    }

    /// Get a specific destination by name
    pub fn get_destination<S: AsRef<str>>(name: S) -> Result<Destination> {
        // Get all destinations first
        let all_dests = Self::get_all()?;

        // Find the specific destination
        let name_c = CString::new(name.as_ref())?;
        let dest_ptr = unsafe {
            bindings::cupsGetDest(
                name_c.as_ptr(),
                ptr::null(),
                all_dests.num_dests,
                all_dests.dests,
            )
        };

        if dest_ptr.is_null() {
            return Err(Error::DestinationNotFound(name.as_ref().to_string()));
        }

        // Convert to our Destination type
        unsafe { Destination::from_raw(dest_ptr) }
    }

    /// Get the default destination
    pub fn get_default() -> Result<Destination> {
        // Get all destinations first
        let all_dests = Self::get_all()?;

        for i in 0..count_to_usize(all_dests.num_dests) {
            unsafe {
                let dest = &*(all_dests.dests.offset(i as isize));
                if dest_is_default(dest.is_default) {
                    return Destination::from_raw(all_dests.dests.offset(i as isize));
                }
            }
        }

        Err(Error::DestinationNotFound("Default printer".to_string()))
    }

    /// Convert to a Vec of Destination objects
    pub fn to_vec(&self) -> Result<Vec<Destination>> {
        let mut destinations = Vec::with_capacity(count_to_usize(self.num_dests));

        for i in 0..count_to_usize(self.num_dests) {
            unsafe {
                match Destination::from_raw(self.dests.offset(i as isize)) {
                    Ok(dest) => destinations.push(dest),
                    Err(e) => {
                        eprintln!("Warning: Failed to parse destination at index {}: {}", i, e)
                    }
                }
            }
        }

        Ok(destinations)
    }

    /// Get the number of destinations
    pub fn len(&self) -> usize {
        count_to_usize(self.num_dests)
    }

    /// Check if there are no destinations
    pub fn is_empty(&self) -> bool {
        self.num_dests == 0
    }

    /// Get raw pointer to destinations array (for advanced usage)
    pub fn as_ptr(&self) -> *mut bindings::cups_dest_s {
        self.dests
    }

    /// Get number of destinations
    pub fn count(&self) -> usize {
        count_to_usize(self.num_dests)
    }

    /// Add a destination to the list of destinations
    ///
    /// If the named destination already exists, the destination list is returned unchanged.
    /// Adding a new instance of a destination creates a copy of that destination's options.
    ///
    /// # Arguments
    /// - `name`: Destination name
    /// - `instance`: Instance name or None for none/primary
    ///
    /// # Returns
    /// - `Ok(())`: Destination added successfully
    /// - `Err(Error)`: Failed to add destination
    pub fn add_destination(&mut self, name: &str, instance: Option<&str>) -> Result<()> {
        let name_c = CString::new(name)?;
        let instance_c = instance.map(|i| CString::new(i)).transpose()?;
        let instance_ptr = instance_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());

        let new_num_dests = unsafe {
            bindings::cupsAddDest(
                name_c.as_ptr(),
                instance_ptr,
                self.num_dests,
                &mut self.dests,
            )
        };

        if new_num_dests > self.num_dests {
            self.num_dests = new_num_dests;
            Ok(())
        } else {
            // Destination already exists or error occurred
            Ok(()) // CUPS API treats existing destinations as success
        }
    }

    /// Remove a destination from the destination list
    ///
    /// Removing a destination/instance does not delete the class or printer queue,
    /// merely the lpoptions for that destination/instance.
    ///
    /// # Arguments
    /// - `name`: Destination name
    /// - `instance`: Instance name or None
    ///
    /// # Returns
    /// - `Ok(true)`: Destination was found and removed
    /// - `Ok(false)`: Destination was not found
    /// - `Err(Error)`: Failed to remove destination
    pub fn remove_destination(&mut self, name: &str, instance: Option<&str>) -> Result<bool> {
        let name_c = CString::new(name)?;
        let instance_c = instance.map(|i| CString::new(i)).transpose()?;
        let instance_ptr = instance_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());

        let old_count = self.num_dests;
        let new_num_dests = unsafe {
            bindings::cupsRemoveDest(
                name_c.as_ptr(),
                instance_ptr,
                self.num_dests,
                &mut self.dests,
            )
        };

        self.num_dests = new_num_dests;
        Ok(new_num_dests < old_count)
    }

    /// Set the default destination
    ///
    /// This marks one of the destinations in the list as the default destination.
    ///
    /// # Arguments
    /// - `name`: Destination name
    /// - `instance`: Instance name or None
    ///
    /// # Returns
    /// - `Ok(())`: Default destination set successfully
    /// - `Err(Error)`: Failed to set default destination
    pub fn set_default_destination(&mut self, name: &str, instance: Option<&str>) -> Result<()> {
        let name_c = CString::new(name)?;
        let instance_c = instance.map(|i| CString::new(i)).transpose()?;
        let instance_ptr = instance_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());

        unsafe {
            bindings::cupsSetDefaultDest(name_c.as_ptr(), instance_ptr, self.num_dests, self.dests);
        }

        Ok(())
    }

    /// Save the list of destinations to the user's lpoptions file
    ///
    /// This saves the current destination list and their options to the user's
    /// lpoptions file for persistence across sessions.
    ///
    /// # Returns
    /// - `Ok(())`: Destinations saved successfully
    /// - `Err(Error)`: Failed to save destinations
    pub fn save_to_lpoptions(&self) -> Result<()> {
        #[cfg(cups3)]
        let success =
            unsafe { bindings::cupsSetDests(ptr::null_mut(), self.num_dests, self.dests) };

        #[cfg(cups2)]
        let success =
            unsafe { bindings::cupsSetDests2(ptr::null_mut(), self.num_dests, self.dests) == 0 };

        if success {
            Ok(())
        } else {
            Err(Error::ConfigurationError(
                "Failed to save destinations to lpoptions".to_string(),
            ))
        }
    }

    /// Find a destination by name and instance
    ///
    /// # Arguments
    /// - `name`: Destination name to search for
    /// - `instance`: Instance name or None
    ///
    /// # Returns
    /// - `Some(Destination)`: Found destination
    /// - `None`: Destination not found
    pub fn find_destination(&self, name: &str, instance: Option<&str>) -> Option<Destination> {
        let name_c = match CString::new(name) {
            Ok(n) => n,
            Err(_) => return None,
        };
        let instance_c = instance.and_then(|i| CString::new(i).ok());
        let instance_ptr = instance_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());

        let dest_ptr = unsafe {
            bindings::cupsGetDest(name_c.as_ptr(), instance_ptr, self.num_dests, self.dests)
        };

        if dest_ptr.is_null() {
            None
        } else {
            unsafe { Destination::from_raw(dest_ptr).ok() }
        }
    }
}

/// Represents option conflicts and their resolutions
#[derive(Debug, Clone)]
pub struct OptionConflict {
    /// The conflicting option/value pairs
    pub conflicting_options: Vec<(String, String)>,
    /// The resolved option/value pairs to fix conflicts
    pub resolved_options: Vec<(String, String)>,
}

impl DestinationInfo {
    /// Check for option conflicts and get resolutions for a new option/value pair
    ///
    /// This function checks if adding a new option/value pair would conflict
    /// with existing options and provides resolutions if conflicts are found.
    ///
    /// # Arguments
    /// - `current_options`: Current option/value pairs
    /// - `new_option`: The new option name to check
    /// - `new_value`: The new option value to check
    ///
    /// # Returns
    /// - `Ok(None)`: No conflicts found
    /// - `Ok(Some(OptionConflict))`: Conflicts found with resolution
    /// - `Err(Error)`: Error checking conflicts or unresolvable conflict
    pub fn check_option_conflicts(
        &self,
        dest: &Destination,
        current_options: &[(String, String)],
        new_option: &str,
        new_value: &str,
    ) -> Result<Option<OptionConflict>> {
        // Convert current options to CUPS format
        let mut cups_options_ptr: *mut bindings::cups_option_s = ptr::null_mut();
        let mut num_options = 0;

        for (name, value) in current_options {
            let name_c = CString::new(name.as_str())?;
            let value_c = CString::new(value.as_str())?;

            unsafe {
                num_options = bindings::cupsAddOption(
                    name_c.as_ptr(),
                    value_c.as_ptr(),
                    num_options,
                    &mut cups_options_ptr,
                );
            }
        }

        let new_option_c = CString::new(new_option)?;
        let new_value_c = CString::new(new_value)?;

        // Get destination pointer (we need to create one temporarily)
        let dest_name_c = CString::new(dest.name.as_str())?;
        let dest_instance_c = dest
            .instance
            .as_ref()
            .map(|i| CString::new(i.as_str()))
            .transpose()?;
        let dest_instance_ptr = dest_instance_c
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());

        let dest_ptr = unsafe {
            bindings::cupsGetDest(
                dest_name_c.as_ptr(),
                dest_instance_ptr,
                1,               // We just need a temporary dest
                ptr::null_mut(), // Let CUPS find it
            )
        };

        if dest_ptr.is_null() {
            unsafe {
                if !cups_options_ptr.is_null() {
                    bindings::cupsFreeOptions(num_options, cups_options_ptr);
                }
            }
            return Err(Error::DestinationNotFound(dest.name.clone()));
        }

        let mut num_conflicts = 0;
        let mut conflicts: *mut bindings::cups_option_s = ptr::null_mut();
        let mut num_resolved = 0;
        let mut resolved: *mut bindings::cups_option_s = ptr::null_mut();

        let conflict_result = unsafe {
            bindings::cupsCopyDestConflicts(
                ptr::null_mut(), // Use CUPS_HTTP_DEFAULT
                dest_ptr,
                self.as_ptr(),
                num_options,
                cups_options_ptr,
                new_option_c.as_ptr(),
                new_value_c.as_ptr(),
                &mut num_conflicts,
                &mut conflicts,
                &mut num_resolved,
                &mut resolved,
            )
        };

        // Clean up temporary options
        unsafe {
            if !cups_options_ptr.is_null() {
                bindings::cupsFreeOptions(num_options, cups_options_ptr);
            }
        }

        let result = match conflict_result {
            1 => {
                // Conflicts found
                let mut conflicting_options = Vec::new();
                let mut resolved_options = Vec::new();

                // Extract conflicting options
                if !conflicts.is_null() && num_conflicts > 0 {
                    for i in 0..num_conflicts {
                        unsafe {
                            let option = &*conflicts.offset(i as isize);
                            if !option.name.is_null() && !option.value.is_null() {
                                let name =
                                    CStr::from_ptr(option.name).to_string_lossy().into_owned();
                                let value =
                                    CStr::from_ptr(option.value).to_string_lossy().into_owned();
                                conflicting_options.push((name, value));
                            }
                        }
                    }
                }

                // Extract resolved options
                if !resolved.is_null() && num_resolved > 0 {
                    for i in 0..num_resolved {
                        unsafe {
                            let option = &*resolved.offset(i as isize);
                            if !option.name.is_null() && !option.value.is_null() {
                                let name =
                                    CStr::from_ptr(option.name).to_string_lossy().into_owned();
                                let value =
                                    CStr::from_ptr(option.value).to_string_lossy().into_owned();
                                resolved_options.push((name, value));
                            }
                        }
                    }
                }

                // Clean up CUPS-allocated memory
                unsafe {
                    if !conflicts.is_null() {
                        bindings::cupsFreeOptions(num_conflicts, conflicts);
                    }
                    if !resolved.is_null() {
                        bindings::cupsFreeOptions(num_resolved, resolved);
                    }
                }

                if resolved_options.is_empty() && !conflicting_options.is_empty() {
                    // Unresolvable conflict
                    Err(Error::ConfigurationError(format!(
                        "Unresolvable option conflict: {} = {} conflicts with existing options",
                        new_option, new_value
                    )))
                } else {
                    Ok(Some(OptionConflict {
                        conflicting_options,
                        resolved_options,
                    }))
                }
            }
            0 => {
                // No conflicts
                Ok(None)
            }
            _ => {
                // Error occurred
                Err(Error::ConfigurationError(format!(
                    "Error checking option conflicts for {} = {}",
                    new_option, new_value
                )))
            }
        };

        result
    }
}

impl Drop for Destinations {
    fn drop(&mut self) {
        unsafe {
            if !self.dests.is_null() && self.num_dests > 0 {
                bindings::cupsFreeDests(self.num_dests, self.dests);
                self.dests = ptr::null_mut();
                self.num_dests = 0;
            }
        }
    }
}

/// Enumerate available destinations with a callback function
pub fn enum_destinations<T>(
    flags: u32,
    msec: i32,
    cancel: Option<&mut i32>,
    type_filter: u32,
    mask: u32,
    callback: &mut DestCallback<T>,
    user_data: &mut T,
) -> Result<bool> {
    // We need to create a context that will be passed to the C callback
    let mut context = EnumContext {
        callback,
        user_data,
    };

    let cancel_ptr = match cancel {
        Some(c) => c as *mut c_int,
        None => ptr::null_mut(),
    };

    #[cfg(cups3)]
    let success = unsafe {
        bindings::cupsEnumDests(
            flags,
            msec as c_int,
            cancel_ptr,
            type_filter as c_uint,
            mask as c_uint,
            Some(enum_dest_callback::<T>),
            &mut context as *mut _ as *mut c_void,
        )
    };

    #[cfg(cups2)]
    let success = unsafe {
        bindings::cupsEnumDests(
            flags,
            msec as c_int,
            cancel_ptr,
            type_filter as c_uint,
            mask as c_uint,
            Some(enum_dest_callback::<T>),
            &mut context as *mut _ as *mut c_void,
        ) != 0
    };

    if !success {
        Err(Error::EnumerationError(
            "Failed to enumerate destinations".to_string(),
        ))
    } else {
        Ok(true)
    }
}

// Context structure for the C callback
struct EnumContext<'a, T> {
    callback: &'a mut DestCallback<T>,
    user_data: &'a mut T,
}

// C-compatible callback function that bridges to our Rust callback
#[cfg(cups3)]
unsafe extern "C" fn enum_dest_callback<T>(
    user_data: *mut c_void,
    flags: c_uint,
    dest_ptr: *mut bindings::cups_dest_s,
) -> bool {
    // Reconstruct our context
    let context = unsafe { &mut *(user_data as *mut EnumContext<T>) };

    // Convert the raw destination to our Rust type
    unsafe {
        match Destination::from_raw(dest_ptr) {
            Ok(dest) => {
                // Call the user's callback
                if (context.callback)(flags, &dest, context.user_data) {
                    true // Continue enumeration
                } else {
                    false // Stop enumeration
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse destination: {}", e);
                true // Continue enumeration despite error
            }
        }
    }
}

// C-compatible callback function that bridges to our Rust callback
#[cfg(cups2)]
unsafe extern "C" fn enum_dest_callback<T>(
    user_data: *mut c_void,
    flags: c_uint,
    dest_ptr: *mut bindings::cups_dest_s,
) -> c_int {
    let context = unsafe { &mut *(user_data as *mut EnumContext<T>) };

    unsafe {
        match Destination::from_raw(dest_ptr) {
            Ok(dest) => {
                if (context.callback)(flags, &dest, context.user_data) {
                    1
                } else {
                    0
                }
            }
            Err(e) => {
                eprintln!("Warning: Failed to parse destination: {}", e);
                1
            }
        }
    }
}

#[cfg(cups3)]
fn dest_is_default(value: bool) -> bool {
    value
}

#[cfg(cups2)]
fn dest_is_default(value: c_int) -> bool {
    value != 0
}

#[cfg(cups3)]
fn raw_is_default(value: bool) -> bool {
    value
}

#[cfg(cups2)]
fn raw_is_default(value: bool) -> c_int {
    if value { 1 } else { 0 }
}

/// Get all available printer destinations
pub fn get_all_destinations() -> Result<Vec<Destination>> {
    Destinations::get_all()?.to_vec()
}

/// Get a specific destination by name
pub fn get_destination<S: AsRef<str>>(name: S) -> Result<Destination> {
    Destinations::get_destination(name)
}

/// Get the default destination
pub fn get_default_destination() -> Result<Destination> {
    Destinations::get_default()
}

/// Copy a destination from one destination array to another
pub fn copy_dest(
    dest: *const bindings::cups_dest_s,
    num_dests: usize,
    dests: *mut *mut bindings::cups_dest_s,
) -> usize {
    count_to_usize(unsafe {
        bindings::cupsCopyDest(
            dest as *mut bindings::cups_dest_s,
            usize_to_count(num_dests),
            dests,
        )
    })
}

/// Remove a destination from an array
pub fn remove_dest(
    name: &str,
    instance: Option<&str>,
    num_dests: usize,
    dests: *mut *mut bindings::cups_dest_s,
) -> Result<usize> {
    let name_c = CString::new(name)?;
    let instance_c = match instance {
        Some(i) => Some(CString::new(i)?),
        None => None,
    };

    let instance_ptr = match &instance_c {
        Some(s) => s.as_ptr(),
        None => ptr::null(),
    };

    let result = unsafe {
        bindings::cupsRemoveDest(
            name_c.as_ptr(),
            instance_ptr,
            usize_to_count(num_dests),
            dests,
        )
    };

    Ok(count_to_usize(result))
}

/// Find available destinations with specific filter criteria
pub fn find_destinations(type_filter: u32, mask: u32) -> Result<Vec<Destination>> {
    let mut destinations = Vec::new();

    enum_destinations(
        constants::DEST_FLAGS_NONE,
        5000, // 5 second timeout
        None,
        type_filter,
        mask,
        &mut |flags, dest, dests: &mut Vec<Destination>| {
            if (flags & constants::DEST_FLAGS_REMOVED) == 0 {
                dests.push(dest.clone());
            }
            true // Continue enumeration
        },
        &mut destinations,
    )?;

    Ok(destinations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_destination_creation() {
        let mut options = std::collections::HashMap::new();
        options.insert("printer-state".to_string(), "3".to_string());
        options.insert("printer-info".to_string(), "Test Printer".to_string());
        options.insert("printer-is-accepting-jobs".to_string(), "true".to_string());

        let dest = Destination {
            name: "TestPrinter".to_string(),
            instance: None,
            is_default: false,
            options,
        };

        assert_eq!(dest.name, "TestPrinter");
        assert_eq!(dest.full_name(), "TestPrinter");
        assert_eq!(dest.state(), PrinterState::Idle);
        assert!(dest.is_accepting_jobs());
        assert_eq!(dest.info(), Some(&"Test Printer".to_string()));
    }

    #[test]
    fn test_destination_with_instance() {
        let dest = Destination {
            name: "TestPrinter".to_string(),
            instance: Some("instance1".to_string()),
            is_default: true,
            options: std::collections::HashMap::new(),
        };

        assert_eq!(dest.full_name(), "TestPrinter/instance1");
        assert!(dest.is_default);
    }

    #[test]
    fn test_destination_state_parsing() {
        let mut options = std::collections::HashMap::new();

        // Test different printer states
        options.insert("printer-state".to_string(), "4".to_string());
        let dest = Destination {
            name: "Test".to_string(),
            instance: None,
            is_default: false,
            options: options.clone(),
        };
        assert_eq!(dest.state(), PrinterState::Processing);

        options.insert("printer-state".to_string(), "5".to_string());
        let dest = Destination {
            name: "Test".to_string(),
            instance: None,
            is_default: false,
            options: options.clone(),
        };
        assert_eq!(dest.state(), PrinterState::Stopped);
    }

    #[test]
    fn test_destination_state_reasons() {
        let mut options = std::collections::HashMap::new();
        options.insert(
            "printer-state-reasons".to_string(),
            "media-tray-empty-error,toner-low-warning".to_string(),
        );

        let dest = Destination {
            name: "Test".to_string(),
            instance: None,
            is_default: false,
            options,
        };

        let reasons = dest.state_reasons();
        assert_eq!(reasons.len(), 2);
        assert!(reasons.contains(&"media-tray-empty-error".to_string()));
        assert!(reasons.contains(&"toner-low-warning".to_string()));
    }
}
