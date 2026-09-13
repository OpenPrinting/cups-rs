//! # cups-rs: Safe Rust Bindings for CUPS (Common UNIX Printing System)
//!
//! `cups-rs` provides a safe, idiomatic Rust interface to the CUPS printing system API.
//! It wraps the C CUPS library with memory-safe abstractions while maintaining high performance.
//!
//! ## Features
//!
//! - **Printer Discovery**: Enumerate and discover available printers on the network
//! - **Job Management**: Create, submit, monitor, and cancel print jobs
//! - **Authentication**: Support for password callbacks and certificate-based authentication
//! - **Multi-document Jobs**: Submit multiple documents as a single print job
//! - **Advanced Queries**: Query printer capabilities, supported media, and options
//! - **IPP Protocol**: Low-level IPP request/response handling for custom workflows
//! - **Server Configuration**: Manage CUPS server settings, encryption, and user preferences
//! - **Localization**: Get localized printer option names and values
//!
//! ## Quick Start
//!
//! ### Discovering Printers
//!
//! ```no_run
//! use cups_rs::get_all_destinations;
//!
//! let printers = get_all_destinations().expect("Failed to get printers");
//! for printer in printers {
//!     println!("Printer: {} ({})", printer.name,
//!              if printer.is_default { "default" } else { "available" });
//! }
//! ```
//!
//! ### Printing a Document
//!
//! ```no_run
//! use cups_rs::{get_default_destination, create_job};
//!
//! let printer = get_default_destination().expect("No default printer");
//! let job = create_job(&printer, "My Document")
//!     .expect("Failed to create job");
//!
//! job.submit_file("document.pdf", "application/pdf")
//!     .expect("Failed to submit document");
//! ```
//!
//! ### Advanced Job Options
//!
//! ```no_run
//! use cups_rs::{get_destination, create_job_with_options, PrintOptions, ColorMode, DuplexMode, Orientation};
//!
//! let printer = get_destination("MyPrinter").expect("Printer not found");
//!
//! let options = PrintOptions::default()
//!     .copies(3)
//!     .color_mode(ColorMode::Color)
//!     .duplex(DuplexMode::TwoSidedPortrait)
//!     .orientation(Orientation::Landscape);
//!
//! let job = create_job_with_options(&printer, "Report", &options)
//!     .expect("Failed to create job");
//! ```
//!
//! ## Module Overview
//!
//! - [`auth`]: Authentication and security layer (password callbacks, certificates)
//! - [`config`]: CUPS server configuration (server, user, encryption settings)
//! - [`connection`]: Direct HTTP connections to printers and CUPS servers
//! - [`destination`]: Printer discovery and destination management
//! - [`ipp`]: Low-level IPP (Internet Printing Protocol) request/response handling
//! - [`job`]: Print job creation, submission, and management
//! - [`options`]: Print option parsing, encoding, and manipulation
//! - [`Error`] and [`Result`]: Error types and result handling
//!
//! ## API Coverage
//!
//! This library currently implements approximately 70% of the CUPS C API, focusing on:
//! - Core printing workflows
//! - Enterprise features (authentication, multi-server support)
//! - Advanced destination and job management
//! - Low-level IPP protocol access
//!
//! ## Safety
//!
//! All unsafe FFI calls to the CUPS C library are wrapped in safe Rust abstractions.
//! Memory management is handled automatically using RAII patterns with Drop implementations.
//!
//! ## Platform Support
//!
//! - Linux (tested)
//! - macOS (should work, uses system CUPS)
//! - Other UNIX-like systems with CUPS installed
//!
//! ## Requirements
//!
//! - CUPS development libraries (`libcups2-dev` on Debian/Ubuntu, `cups-devel` on Fedora)
//! - Rust 1.70 or later

/// Authentication and security layer for CUPS
///
/// This module provides functionality for:
/// - Password callback registration for GUI applications
/// - Client certificate callback setup
/// - Server certificate validation
/// - Authentication handling
pub mod auth;

/// Raw FFI bindings to the CUPS C library
///
/// This module contains auto-generated bindings created by bindgen.
/// Most users should use the safe wrappers in other modules instead.
#[allow(missing_docs)]
pub mod bindings;

mod compat;

/// CUPS server configuration management
///
/// Configure CUPS server settings including:
/// - Server hostname/address
/// - User credentials
/// - Encryption modes (never, if-requested, required, always)
/// - HTTP user agent strings
/// - Scoped configuration with automatic restoration
pub mod config;

/// Direct HTTP connection management for printers and CUPS servers
///
/// Provides low-level connection control with:
/// - Connection timeouts
/// - Cancellation support
/// - Direct device vs scheduler connections
/// - Connection monitoring via callbacks
pub mod connection;

/// CUPS constants and enums
pub mod constants;

/// Printer discovery and destination management
///
/// Core functionality for working with printers:
/// - Enumerate available destinations
/// - Query printer capabilities and status
/// - Manage destination lists (add/remove/default)
/// - Check supported options and media sizes
/// - Resolve option conflicts
pub mod destination;

/// DNS Service Discovery browsing and service resolution
///
/// Find printers and other services on the local network:
/// - Browse a service type as services appear and disappear
/// - Resolve a service to its hostname, port and TXT record
/// - Query a resolved service for its addresses
///
/// Requires CUPS 3 — `cups/dnssd.h` has no CUPS 2 equivalent.
#[cfg(cups3)]
pub mod dnssd;

mod error;
mod error_helpers;

/// Low-level IPP (Internet Printing Protocol) request/response handling
///
/// Build and send custom IPP requests for advanced use cases:
/// - Create IPP requests with custom operations
/// - Add attributes (strings, integers, booleans, arrays)
/// - Send requests and receive responses
/// - Parse response attributes
/// - Type-safe enums for IPP operations, tags, and status codes
pub mod ipp;

/// Print job creation, submission, and management
///
/// Comprehensive job handling:
/// - Create single and multi-document jobs
/// - Submit files and raw data
/// - Query job status and attributes
/// - Cancel and manage jobs
/// - Rich print options (copies, color, duplex, media, orientation)
pub mod job;

/// Print option parsing, encoding, and manipulation
///
/// Utilities for working with CUPS print options:
/// - Parse command-line style option strings
/// - Add/remove options from arrays
/// - Type-safe integer options
/// - Encode options to IPP attributes
/// - Get option values with type conversion
pub mod options;

pub use connection::{ConnectionFlags, HttpConnection, connect_to_destination};
pub use constants::*;
pub use destination::{
    Destination, DestinationInfo, Destinations, MediaSize, OptionConflict, PrinterState, copy_dest,
    enum_destinations, find_destinations, get_all_destinations, get_default_destination,
    get_destination, remove_dest,
};
#[cfg(cups3)]
pub use dnssd::{
    Dnssd, DnssdBrowseEvent, DnssdBrowser, DnssdResolveEvent, DnssdResolvedService, DnssdResolver,
    DnssdServiceResolver,
};
pub use error::{Error, ErrorCategory, Result};
pub use ipp::{
    IppAttribute, IppOperation, IppRequest, IppResponse, IppStatus, IppTag, IppValueTag,
};
pub use job::{
    ColorMode, DuplexMode, FORMAT_JPEG, FORMAT_PDF, FORMAT_POSTSCRIPT, FORMAT_TEXT, JobInfo,
    JobStatus, Orientation, PrintOptions, PrintQuality, cancel_job, create_job,
    create_job_with_options, get_active_jobs, get_completed_jobs, get_job_info, get_jobs,
};
pub use options::{
    add_integer_option, add_option, encode_option, encode_options, encode_options_with_group,
    get_integer_option, get_option, parse_options, remove_option,
};
