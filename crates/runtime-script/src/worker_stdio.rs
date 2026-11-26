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
        ffi::{c_char, c_int},
        fs::File,
        io,
        os::windows::io::{AsHandle, AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
    };

    unsafe extern "C" {
        fn _open(path: *const c_char, flags: c_int, ...) -> c_int;
        fn _get_osfhandle(fd: c_int) -> isize;
        fn _dup2(source: c_int, destination: c_int) -> c_int;
        fn _close(fd: c_int) -> c_int;
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn SetStdHandle(stream: u32, handle: RawHandle) -> i32;
    }
    const CRT_STDOUT: c_int = 1;
    const O_WRONLY: c_int = 0x0001; // UCRT fcntl.h
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
    // SAFETY: the launcher supplies a valid stdout pipe at process startup.
    // Reading CRT fd 1 does not transfer ownership of its handle.
    let crt_initial = unsafe { _get_osfhandle(CRT_STDOUT) };
    if matches!(crt_initial, 0 | -1 | -2) {
        return Err(io::Error::other("worker requires an open CRT stdout"));
    }
    // SAFETY: the device path is NUL-terminated and no file is created, so
    // _open takes no creation mode. The replacement can be inherited by children.
    let sink_fd = unsafe { _open(c"\\\\.\\NUL".as_ptr(), O_WRONLY) };
    if sink_fd < 0 {
        return Err(io::Error::other("could not open the CRT stdout sink"));
    }
    // SAFETY: sink_fd is newly opened and distinct from the existing CRT fd 1.
    // _dup2 replaces the handle owned by fd 1, including closing its old handle.
    let redirected = unsafe { _dup2(sink_fd, CRT_STDOUT) };
    // SAFETY: _dup2 leaves the temporary descriptor owned by this function.
    let closed = unsafe { _close(sink_fd) };
    if redirected != 0 || closed != 0 {
        return Err(io::Error::other("could not redirect CRT stdout to the sink"));
    }
    // SAFETY: the successful _dup2 left fd 1 open and owning a writable NUL handle.
    let sink = unsafe { _get_osfhandle(CRT_STDOUT) } as RawHandle;
    // SAFETY: CRT keeps sink live. Update Win32 stdio and child inheritance
    // before running any Custom code.
    if unsafe { SetStdHandle(STD_OUTPUT_HANDLE, sink) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if initial as isize != crt_initial {
        // SAFETY: if CRT had a different handle, _dup2 did not close this
        // process-owned startup handle. Win32 now uses the CRT-owned NUL handle.
        drop(unsafe { OwnedHandle::from_raw_handle(initial) });
    }
    Ok(File::from(response))
}
