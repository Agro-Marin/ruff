use colored::Colorize;
use std::io;
use ty::{ExitStatus, run};

#[cfg(all(
    not(target_os = "macos"),
    not(target_os = "windows"),
    not(target_os = "openbsd"),
    not(target_os = "aix"),
    not(target_os = "android"),
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "riscv64"
    )
))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

fn main() -> ExitStatus {
    run().unwrap_or_else(|error| {
        use io::Write;

        if error.chain().any(|cause| {
            cause
                .downcast_ref::<io::Error>()
                .is_some_and(|ioerr| ioerr.kind() == io::ErrorKind::BrokenPipe)
        }) {
            return ExitStatus::Success;
        }

        let mut stderr = io::stderr().lock();

        writeln!(stderr, "{}", "ty failed".red().bold()).ok();
        for cause in error.chain() {
            writeln!(stderr, "  {} {cause}", "Cause:".bold()).ok();
        }

        ExitStatus::Error
    })
}
