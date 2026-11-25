// Included in the generated worker before it invokes Custom code. The parent
// passes a private pipe as the initial stdout. Keep its writer separately and
// give stdout a sink before user code can obtain a standard output handle.

#[cfg(unix)]
fn response_pipe() -> std::io::Result<std::fs::File> {
    use std::{
        fs::File,
        io,
        os::fd::{AsFd, AsRawFd},
    };

    unsafe extern "C" {
        fn dup2(source: std::ffi::c_int, destination: std::ffi::c_int) -> std::ffi::c_int;
    }

    let stdout = io::stdout();
    let response = stdout.as_fd().try_clone_to_owned()?;
    let sink = File::options().write(true).open("/dev/null")?;
    // SAFETY: sink is an open writable descriptor and stdout is the inherited
    // pipe descriptor. Startup changes it before any user threads or writes.
    if unsafe { dup2(sink.as_raw_fd(), stdout.as_raw_fd()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(File::from(response))
}

#[cfg(windows)]
fn response_pipe() -> std::io::Result<std::fs::File> {
    use std::{
        fs::File,
        io,
        os::windows::io::{
            AsHandle, AsRawHandle, FromRawHandle, IntoRawHandle, OwnedHandle, RawHandle,
        },
    };

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetStdHandle(stream: u32, handle: RawHandle) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11_i32 as u32;

    let stdout = io::stdout();
    let initial = stdout.as_raw_handle();
    if initial.is_null()
        || initial as isize == -1
        || initial == io::stdin().as_raw_handle()
        || initial == io::stderr().as_raw_handle()
    {
        return Err(io::Error::other(
            "worker requires a dedicated response pipe",
        ));
    }
    let response = stdout.as_handle().try_clone_to_owned()?;
    let sink = File::options().write(true).open("NUL")?;
    // SAFETY: sink is writable and live. No Custom code is running. Rust's
    // stdio retrieves the current standard handle on each output operation.
    if unsafe { SetStdHandle(STD_OUTPUT_HANDLE, sink.as_raw_handle()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // The process owns the replacement handle, including during stdio teardown.
    let _ = sink.into_raw_handle();
    // SAFETY: startup inherited a separate process-owned pipe handle. It is no
    // longer in the standard handle table and the response owns its duplicate.
    drop(unsafe { OwnedHandle::from_raw_handle(initial) });
    Ok(File::from(response))
}
