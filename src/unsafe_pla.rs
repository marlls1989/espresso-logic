//! Unsafe PLA wrapper for use in worker processes only

use crate::sys;
use std::ffi::CString;
use std::io;
use std::os::raw::c_int;
use std::path::Path;
use std::ptr;

/// UNSAFE: PLA wrapper that uses global state
///
/// Only used internally by worker processes.
pub(crate) struct UnsafePLA {
    pub(crate) ptr: sys::pPLA,
}

impl UnsafePLA {
    /// Load PLA from file content
    pub(crate) fn from_bytes(content: &[u8]) -> io::Result<Self> {
        use std::io::Write;
        
        // Write to temp file
        let mut temp = tempfile::NamedTempFile::new()?;
        temp.write_all(content)?;
        temp.flush()?;

        let path_str = temp.path().to_str()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid path"))?;

        let c_path = CString::new(path_str)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Path contains null byte"))?;

        let file_mode = CString::new("r").unwrap();
        let file = unsafe { libc::fopen(c_path.as_ptr(), file_mode.as_ptr()) };

        if file.is_null() {
            return Err(io::Error::last_os_error());
        }

        // Tear down any existing cube state
        unsafe {
            if !sys::cube.fullset.is_null() {
                sys::setdown_cube();
                if !sys::cube.part_size.is_null() {
                    libc::free(sys::cube.part_size as *mut libc::c_void);
                    sys::cube.part_size = ptr::null_mut();
                }
            }
        }

        let mut pla_ptr: sys::pPLA = ptr::null_mut();

        let result = unsafe {
            sys::read_pla(file as *mut _, 1, 1, sys::FD_type as c_int, &mut pla_ptr)
        };

        unsafe { libc::fclose(file) };

        if result == libc::EOF {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Failed to read PLA",
            ));
        }

        Ok(UnsafePLA { ptr: pla_ptr })
    }

    /// Get statistics
    pub(crate) fn stats(&self) -> (usize, usize, usize) {
        unsafe {
            let f = (*self.ptr).F;
            let d = (*self.ptr).D;
            let r = (*self.ptr).R;

            (
                if !f.is_null() { (*f).count as usize } else { 0 },
                if !d.is_null() { (*d).count as usize } else { 0 },
                if !r.is_null() { (*r).count as usize } else { 0 },
            )
        }
    }

    /// Minimize this PLA
    pub(crate) fn minimize(&self) -> UnsafePLA {
        unsafe {
            let f = (*self.ptr).F;
            let d = (*self.ptr).D;
            let r = (*self.ptr).R;

            let f_copy = sys::sf_save(f);
            let d_copy = if !d.is_null() {
                sys::sf_save(d)
            } else {
                sys::sf_new(0, sys::cube.size as c_int)
            };
            let r_copy = if !r.is_null() {
                sys::sf_save(r)
            } else {
                sys::sf_new(0, sys::cube.size as c_int)
            };

            let minimized_f = sys::espresso(f_copy, d_copy, r_copy);

            let new_pla = sys::new_PLA();
            (*new_pla).F = minimized_f;
            (*new_pla).D = d_copy;
            (*new_pla).R = r_copy;

            UnsafePLA { ptr: new_pla }
        }
    }

    /// Write to bytes
    pub(crate) fn to_bytes(&self, pla_type: crate::PLAType) -> io::Result<Vec<u8>> {
        let mut temp = tempfile::NamedTempFile::new()?;
        
        let file = unsafe { libc::fdopen(temp.as_raw_fd(), c"w".as_ptr()) };
        if file.is_null() {
            return Err(io::Error::other("Failed to open temp file"));
        }

        unsafe {
            sys::fprint_pla(file as *mut _, self.ptr, pla_type as c_int);
            libc::fflush(file);
            // Don't close - temp file owns the fd
        }

        use std::io::{Read, Seek, SeekFrom};
        temp.seek(SeekFrom::Start(0))?;
        let mut content = Vec::new();
        temp.read_to_end(&mut content)?;

        Ok(content)
    }
}

impl Drop for UnsafePLA {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                sys::free_PLA(self.ptr);
            }
        }
    }
}

use std::os::unix::io::AsRawFd;

