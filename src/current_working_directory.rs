// based on https://github.com/alacritty/alacritty/blob/4225cea231432fb23442b1da2463b4ec9dfd726c/alacritty/src/daemon.rs#L117
#[cfg(not(any(windows, target_os = "macos")))]
use std::os::fd::AsFd;
use std::path::PathBuf;

use alacritty_terminal::tty::Pty;

pub struct CWD {
    #[cfg(not(any(windows, target_os = "macos")))]
    master_fd: Option<std::os::fd::OwnedFd>,
    #[cfg(not(any(windows, target_os = "macos")))]
    shell_pid: u32,
}

impl CWD {
    pub fn new(pty: &Pty) -> Self {
        Self {
            #[cfg(not(any(windows, target_os = "macos")))]
            master_fd: pty.file().as_fd().try_clone_to_owned().ok(),
            #[cfg(not(any(windows, target_os = "macos")))]
            shell_pid: pty.child().id(),
        }
    }

    pub fn current_working_directory(&self) -> Option<PathBuf> {
        #[cfg(any(windows, target_os = "macos"))]
        {
            return None;
        }

        #[cfg(not(any(windows, target_os = "macos")))]
        {
            use std::os::fd::AsRawFd;

            let fd = self.master_fd.as_ref()?.as_raw_fd();
            foreground_process_path(fd, self.shell_pid).ok()
        }
    }
}

#[cfg(not(any(windows, target_os = "macos", target_os = "openbsd")))]
pub fn foreground_process_path(
    master_fd: std::os::fd::RawFd,
    shell_pid: u32,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut pid = unsafe { libc::tcgetpgrp(master_fd) };
    if pid < 0 {
        pid = shell_pid as libc::pid_t;
    }

    let link_path = if cfg!(target_os = "freebsd") {
        format!("/compat/linux/proc/{pid}/cwd")
    } else {
        format!("/proc/{pid}/cwd")
    };

    let cwd = std::fs::read_link(link_path)?;

    Ok(cwd)
}

#[cfg(target_os = "openbsd")]
pub fn foreground_process_path(
    master_fd: RawFd,
    shell_pid: u32,
) -> Result<PathBuf, Box<dyn Error>> {
    let mut pid = unsafe { libc::tcgetpgrp(master_fd) };
    if pid < 0 {
        pid = shell_pid as libc::pid_t;
    }
    let name = [libc::CTL_KERN, libc::KERN_PROC_CWD, pid];
    let mut buf = [0u8; libc::PATH_MAX as usize];
    let result = unsafe {
        libc::sysctl(
            name.as_ptr(),
            name.len().try_into().unwrap(),
            buf.as_mut_ptr() as *mut _,
            &mut buf.len() as *mut _,
            ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        Err(io::Error::last_os_error().into())
    } else {
        let foreground_path = unsafe { CStr::from_ptr(buf.as_ptr().cast()) }.to_str()?;
        Ok(PathBuf::from(foreground_path))
    }
}
