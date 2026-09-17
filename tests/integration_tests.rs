use cups_rs::*;
use serial_test::serial;
use std::io::Write;
use std::time::Duration;
use tempfile::NamedTempFile;

fn cups_available() -> bool {
    match get_all_destinations() {
        Ok(_) => true,
        Err(_) => {
            println!("CUPS server not available - skipping integration tests");
            false
        }
    }
}

fn get_test_printer() -> Result<Destination> {
    let destinations = get_all_destinations()?;

    if let Ok(pdf_dest) = get_destination("PDF") {
        return Ok(pdf_dest);
    }

    destinations
        .into_iter()
        .next()
        .ok_or_else(|| Error::DestinationNotFound("No test printer available".to_string()))
}

#[test]
#[serial]
fn test_integration_destination_discovery() {
    if !cups_available() {
        return;
    }

    let destinations = get_all_destinations().expect("Should get destinations");
    assert!(
        !destinations.is_empty(),
        "Should have at least one destination"
    );

    for dest in &destinations {
        assert!(!dest.name.is_empty(), "Destination should have a name");
        println!("Found destination: {} ({})", dest.full_name(), dest.state());

        // Test basic properties
        let _state = dest.state();
        let _accepting = dest.is_accepting_jobs();
        let _reasons = dest.state_reasons();

        // Test optional properties
        if let Some(info) = dest.info() {
            assert!(!info.is_empty());
        }
        if let Some(location) = dest.location() {
            println!("  Location: {}", location);
        }
        if let Some(model) = dest.make_and_model() {
            println!("  Model: {}", model);
        }
    }
}

#[test]
#[serial]
fn test_integration_get_specific_destination() {
    if !cups_available() {
        return;
    }

    let destinations = get_all_destinations().expect("Should get destinations");
    if destinations.is_empty() {
        return;
    }

    let first_dest = &destinations[0];
    let specific_dest = get_destination(&first_dest.name).expect("Should get specific destination");

    assert_eq!(specific_dest.name, first_dest.name);
    assert_eq!(specific_dest.is_default, first_dest.is_default);
}

#[test]
#[serial]
fn test_integration_destination_not_found() {
    if !cups_available() {
        return;
    }

    let result = get_destination("NonExistentPrinter123");
    assert!(result.is_err());

    match result {
        Err(Error::DestinationNotFound(name)) => {
            assert_eq!(name, "NonExistentPrinter123");
        }
        _ => panic!("Expected DestinationNotFound error"),
    }
}

#[test]
#[serial]
fn test_integration_printer_capabilities() {
    if !cups_available() {
        return;
    }

    let printer = match get_test_printer() {
        Ok(p) => p,
        Err(_) => return, // Skip if no test printer
    };

    // Test getting detailed info
    let info_result = printer.get_detailed_info(std::ptr::null_mut());
    match info_result {
        Ok(info) => {
            println!("Successfully got detailed info for {}", printer.name);

            // Test media capabilities
            let media_count =
                info.get_media_count(std::ptr::null_mut(), printer.as_ptr(), MEDIA_FLAGS_DEFAULT);
            println!("Media count: {}", media_count);

            if media_count > 0 {
                // Test getting all media
                let all_media =
                    info.get_all_media(std::ptr::null_mut(), printer.as_ptr(), MEDIA_FLAGS_DEFAULT);
                match all_media {
                    Ok(media_list) => {
                        println!("Found {} media sizes", media_list.len());
                        for (i, media) in media_list.iter().take(3).enumerate() {
                            println!(
                                "  {}: {} ({:.1}\" x {:.1}\")",
                                i,
                                media.name,
                                media.width_inches(),
                                media.length_inches()
                            );
                        }
                    }
                    Err(e) => println!("Could not get media list: {}", e),
                }

                // Test getting default media
                let default_media = info.get_default_media(
                    std::ptr::null_mut(),
                    printer.as_ptr(),
                    MEDIA_FLAGS_DEFAULT,
                );
                match default_media {
                    Ok(media) => {
                        println!(
                            "Default media: {} ({:.1}\" x {:.1}\")",
                            media.name,
                            media.width_inches(),
                            media.length_inches()
                        );
                    }
                    Err(e) => println!("Could not get default media: {}", e),
                }
            }

            // Test option support
            let supports_copies = printer.is_option_supported(std::ptr::null_mut(), COPIES);
            let supports_media = printer.is_option_supported(std::ptr::null_mut(), MEDIA);
            let supports_duplex = printer.is_option_supported(std::ptr::null_mut(), SIDES);

            println!("Supports copies: {}", supports_copies);
            println!("Supports media: {}", supports_media);
            println!("Supports duplex: {}", supports_duplex);

            // Cleanup
            unsafe {
                let dest_ptr = printer.as_ptr();
                if !dest_ptr.is_null() {
                    let dest_box = Box::from_raw(dest_ptr);
                    if !dest_box.name.is_null() {
                        let _ = std::ffi::CString::from_raw(dest_box.name);
                    }
                    if !dest_box.instance.is_null() {
                        let _ = std::ffi::CString::from_raw(dest_box.instance);
                    }
                    if !dest_box.options.is_null() {
                        cups_rs::bindings::cupsFreeOptions(dest_box.num_options, dest_box.options);
                    }
                }
            }
        }
        Err(e) => {
            println!("Could not get detailed info for {}: {}", printer.name, e);
            // This might be expected if the printer doesn't support detailed queries
        }
    }
}

#[test]
#[serial]
fn test_integration_job_lifecycle() {
    if !cups_available() {
        return;
    }

    let printer = match get_test_printer() {
        Ok(p) => p,
        Err(_) => {
            println!("No test printer available - skipping job test");
            return;
        }
    };

    println!("Testing job lifecycle with printer: {}", printer.name);

    // temporary test file
    let mut temp_file = NamedTempFile::new().expect("Should create temp file");
    writeln!(
        temp_file,
        "This is a test document for CUPS integration testing."
    )
    .unwrap();
    writeln!(temp_file, "Created at: {}", chrono::Utc::now()).unwrap();
    writeln!(temp_file, "Printer: {}", printer.name).unwrap();
    temp_file.flush().unwrap();

    // Create a job
    let job_result = create_job(&printer, "Integration Test Job");
    let job = match job_result {
        Ok(j) => {
            println!("Created job: {} on printer {}", j.id, j.dest_name());
            j
        }
        Err(e) => {
            println!("Could not create job: {}", e);
            return; // Skip rest of test
        }
    };

    // Submit document to job
    let submit_result = job.submit_file(temp_file.path(), FORMAT_TEXT);
    match submit_result {
        Ok(()) => {
            println!("Successfully submitted document to job {}", job.id);
        }
        Err(e) => {
            println!("Failed to submit document: {}", e);
            // Try to cancel the job before returning
            let _ = job.cancel();
            return;
        }
    }

    // Close the job to start printing
    let close_result = job.close();
    match close_result {
        Ok(()) => {
            println!("Successfully closed job {} - printing started", job.id);
        }
        Err(e) => {
            println!("Failed to close job: {}", e);
            // Try to cancel if close failed
            let _ = job.cancel();
            return;
        }
    }

    // Check job status
    std::thread::sleep(Duration::from_millis(500)); // Give it a moment

    match get_job_info(job.id) {
        Ok(info) => {
            println!(
                "Job {} status: {} (size: {} bytes)",
                info.id, info.status, info.size
            );
            assert_eq!(info.id, job.id);
        }
        Err(e) => {
            println!("Could not get job info (job may have completed): {}", e);
        }
    }

    println!("Job lifecycle test completed successfully");
}

#[test]
#[serial]
fn test_integration_job_with_options() {
    if !cups_available() {
        return;
    }

    let printer = match get_test_printer() {
        Ok(p) => p,
        Err(_) => return,
    };

    // Create a job with specific options
    let options = PrintOptions::new()
        .copies(2)
        .color_mode(ColorMode::Monochrome)
        .quality(PrintQuality::Draft);

    let job_result = create_job_with_options(&printer, "Options Test Job", &options);
    let job = match job_result {
        Ok(j) => {
            println!("Created job with options: {}", j.id);
            j
        }
        Err(e) => {
            println!("Could not create job with options: {}", e);
            return;
        }
    };

    // Create test content
    let test_content =
        "Test content for options verification\nCopies: 2\nColor: Monochrome\nQuality: Draft";

    let submit_result = job.submit_data(test_content.as_bytes(), FORMAT_TEXT, "options_test.txt");
    match submit_result {
        Ok(()) => {
            println!("Submitted document with options");
            let _ = job.close();
        }
        Err(e) => {
            println!("Failed to submit document with options: {}", e);
            let _ = job.cancel();
        }
    }
}

#[test]
#[serial]
fn test_integration_job_cancellation() {
    if !cups_available() {
        return;
    }

    let printer = match get_test_printer() {
        Ok(p) => p,
        Err(_) => return,
    };

    // Create a job
    let job = match create_job(&printer, "Cancellation Test Job") {
        Ok(j) => j,
        Err(e) => {
            println!("Could not create job for cancellation test: {}", e);
            return;
        }
    };

    println!("Created job {} for cancellation test", job.id);

    // Cancel it immediately
    let cancel_result = job.cancel();
    match cancel_result {
        Ok(()) => {
            println!("Successfully canceled job {}", job.id);
        }
        Err(e) => {
            println!("Failed to cancel job {}: {}", job.id, e);
        }
    }
}

#[test]
#[serial]
fn test_integration_get_jobs() {
    if !cups_available() {
        return;
    }

    // Test getting all jobs
    let all_jobs = get_jobs(None).unwrap_or_default();
    println!("Found {} total jobs", all_jobs.len());

    // Test getting active jobs
    let active_jobs = get_active_jobs(None).unwrap_or_default();
    println!("Found {} active jobs", active_jobs.len());

    // Test getting completed jobs
    let completed_jobs = get_completed_jobs(None).unwrap_or_default();
    println!("Found {} completed jobs", completed_jobs.len());

    // Print some job details
    for job in active_jobs.iter().take(3) {
        println!(
            "Active job {}: {} by {} on {} ({})",
            job.id, job.title, job.user, job.dest, job.status
        );
    }

    for job in completed_jobs.iter().take(3) {
        println!(
            "Completed job {}: {} by {} on {} ({})",
            job.id, job.title, job.user, job.dest, job.status
        );
    }
}

#[test]
#[serial]
fn test_integration_find_destinations_by_type() {
    if !cups_available() {
        return;
    }

    // Test finding local printers
    let local_printers = find_destinations(PRINTER_LOCAL, PRINTER_REMOTE).unwrap_or_default();
    println!("Found {} local printers", local_printers.len());

    // Test finding color printers
    let color_printers = find_destinations(PRINTER_COLOR, PRINTER_COLOR).unwrap_or_default();
    println!("Found {} color printers", color_printers.len());

    // Test finding duplex printers
    let duplex_printers = find_destinations(PRINTER_DUPLEX, PRINTER_DUPLEX).unwrap_or_default();
    println!("Found {} duplex printers", duplex_printers.len());
}

#[test]
#[serial]
fn test_integration_error_handling() {
    if !cups_available() {
        return;
    }

    // Non-existent printer
    let result = get_destination("NonExistentPrinter");
    assert!(result.is_err());
    if let Err(e) = result {
        println!("Expected error for non-existent printer: {}", e);
        assert!(matches!(e, Error::DestinationNotFound(_)));
    }

    // Invalid job ID
    let result = get_job_info(999999);
    assert!(result.is_err());
    if let Err(e) = result {
        println!("Expected error for invalid job ID: {}", e);
    }

    // Cancel non-existent job
    let result = cancel_job(999999);
    assert!(result.is_err());
    if let Err(e) = result {
        println!("Expected error for canceling non-existent job: {}", e);
    }
}
