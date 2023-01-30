//! UEFI-compatible logging backend

#![deny(missing_docs)]
#![deny(clippy::pedantic)]
#![deny(clippy::missing_docs_in_private_items)]

use alloc::{boxed::Box, string::String};
use core::fmt::Write;
use core::ptr::NonNull;
use log::{Level, LevelFilter, Metadata, Record};
use uefi::{
    prelude::*,
    proto::{console::text::Color, console::text::Output, loaded_image::LoadedImage},
    Result,
};

/// Logger that logs to UEFI stdout
struct UefiLogger {
    /// Maximum level to log
    max_level: LevelFilter,

    /// Writer to write messages to
    writer: Option<NonNull<Output<'static>>>,
}

impl log::Log for UefiLogger {
    /// Returns whether or not this logger is active for a given message
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= self.max_level
    }

    /// Logs a record
    fn log(&self, record: &Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let writer = if let Some(mut ptr) = self.writer {
            unsafe { ptr.as_mut() }
        } else {
            return;
        };

        let foreground = match record.level() {
            Level::Error => Color::Red,
            Level::Warn => Color::Yellow,
            Level::Info => Color::White,
            Level::Debug => Color::Blue,
            Level::Trace => Color::Cyan,
        };
        // We assign all of these because they return a Result that has to be checked. We don't
        // care about that as we can not really do anything.
        let _ = writer.set_color(foreground, Color::Black);
        let _ = write!(writer, "{}", record.level());
        let _ = writer.set_color(Color::White, Color::Black);
        let _ = write!(writer, " - {}\r\n", record.args());
    }

    /// Does not do anything - needed to comply with the trait
    fn flush(&self) {}
}

// The logger is not thread-safe, but the UEFI boot environment only uses one processor.
unsafe impl Sync for UefiLogger {}
unsafe impl Send for UefiLogger {}

/// Creates the logger and registers it with the log crate, parsing the
/// command line to figure out the actual logger configuration.
pub(crate) fn init_from_cmdline(system_table: &mut SystemTable<Boot>) -> Result<()> {
    // Load command line
    let cmdline = {
        let boot_services = system_table.boot_services();
        let loaded_image =
            boot_services.open_protocol_exclusive::<LoadedImage>(boot_services.image_handle())?;
        let cmdline_cstr16 = loaded_image
            .load_options_as_cstr16()
            // If this fails, we have no load options and we return an empty string.
            .unwrap_or(cstr16!("empty"));
        // We need this to call .split()
        let mut cmdline = String::new();
        let _ = cmdline_cstr16.as_str_in_buf(&mut cmdline);
        cmdline
    };

    // Check all parameters
    let mut quiet = false;
    let mut max_level = LevelFilter::Info;
    for piece in cmdline.split(' ') {
        if piece == "quiet" {
            quiet = true;
        }
        if piece == "lanzaboote.clearscreen" {
            let _ = system_table.stdout().clear();
        }
        if piece.starts_with("lanzaboote.loglevel=") {
            max_level = loglevel_from_kernel_param(piece);
        }
    }

    if quiet {
        max_level = LevelFilter::Error;
    }

    // Set up and register the logger
    let logger = UefiLogger {
        max_level,
        writer: NonNull::new(system_table.stdout() as *const _ as *mut _),
    };
    let boxed_logger = Box::new(logger);
    // This is the same as set_boxed_logger() but that needs the std feature...
    log::set_logger(unsafe { &*Box::into_raw(boxed_logger) })
        .map(|()| log::set_max_level(max_level))
        .expect("Unable to register logger");

    Ok(())
}

/// Takes a piece of kernel command line, splits it at '=',
/// and parses the right-hand side as a log level.
fn loglevel_from_kernel_param(param: &str) -> LevelFilter {
    match param.split('=').last() {
        Some("trace") => LevelFilter::Trace,
        Some("debug") => LevelFilter::Debug,
        Some("warn") => LevelFilter::Warn,
        Some("error") => LevelFilter::Error,
        Some("info" | _) | None => LevelFilter::Info,
    }
}
