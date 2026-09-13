// DNS-SD is built on cups/dnssd.h, a CUPS 3-only addition; nothing to run under CUPS 2.
#[cfg(not(cups3))]
fn main() {
    eprintln!("discover_system_services requires CUPS 3; skipping under CUPS 2.");
}

#[cfg(cups3)]
use cups_rs::Dnssd;
#[cfg(cups3)]
use std::sync::mpsc;
#[cfg(cups3)]
use std::time::{Duration, Instant};

#[cfg(cups3)]
fn main() -> cups_rs::Result<()> {
    let (error_sender, error_receiver) = mpsc::channel();
    let (browse_sender, browse_receiver) = mpsc::channel();
    let dnssd = Dnssd::new(error_sender)?;
    let _ipp = dnssd.browse("_ipp-system._tcp", None, browse_sender.clone())?;
    let _ipps = dnssd.browse("_ipps-system._tcp", None, browse_sender)?;
    let mut resolvers = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(5);

    while Instant::now() < deadline {
        while let Ok(service) = browse_receiver.try_recv() {
            if service.added {
                resolvers.push(dnssd.resolve_service(&service)?);
            }
        }
        for resolver in &mut resolvers {
            if let Some(resolved) = resolver.try_recv()? {
                let service = resolved.service;
                println!(
                    "{} {}:{} ({}) addresses={:?}",
                    service.name,
                    service.hostname,
                    service.port,
                    service.service_type,
                    resolved.addresses
                );
                for (name, value) in service.txt {
                    println!("  {name}={value}");
                }
            }
        }
        while let Ok(error) = error_receiver.try_recv() {
            eprintln!("DNS-SD error: {error}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    Ok(())
}
