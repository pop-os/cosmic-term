use nix::fcntl::Flock;
use nix::unistd::Pid;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};

pub struct PidFile {
    _flock: Flock<File>,
}

#[derive(Debug)]
pub enum PidFileError {
    Locked(Option<Pid>),
    Io(io::Error),
}

impl PidFile {
    /// Create a new PidFile by acquiring the lock file
    /// Returns Ok(PidFile) if this is the first instance
    /// Returns Err(PidFileError::Locked) if another instance is running
    /// Returns Err(PidFileError::Io) if there's an IO error
    pub fn new() -> Result<Self, PidFileError> {
        let path = dirs::runtime_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("cosmic-term-dropdown.pid");
        
        // Open or create the file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(PidFileError::Io)?;
        
        // Try to acquire an exclusive lock (non-blocking)
        match Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock) {
            Ok(mut flock) => {
                // We got the lock! Write our PID so other instances can signal us
                let pid = std::process::id();
                flock.set_len(0).map_err(PidFileError::Io)?;
                write!(flock, "{}", pid).map_err(PidFileError::Io)?;
                flock.flush().map_err(PidFileError::Io)?;
                log::info!("Acquired lock file at {:?} with PID {}", path, pid);
                
                Ok(Self { _flock: flock })
            }
            Err((mut file, err)) => {
                // Lock is held by another process - read its PID so we can signal it
                log::info!("Lock file is held by another process: {}", err);
                
                let mut content = String::new();
                file.read_to_string(&mut content).map_err(PidFileError::Io)?;
                
                let existing_pid = content
                    .trim()
                    .parse::<i32>()
                    .ok()
                    .map(Pid::from_raw);
                
                Err(PidFileError::Locked(existing_pid))
            }
        }
    }
}
