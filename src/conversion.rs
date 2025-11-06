//! Conversion utilities between UnsafeCover and SerializedCover

use crate::ipc::{SerializedCover, SerializedCube};
use crate::UnsafeCover;

impl UnsafeCover {
    /// Serialize this cover for IPC
    pub(crate) fn serialize(&self) -> SerializedCover {
        unsafe {
            let count = (*self.ptr).count as usize;
            let wsize = (*self.ptr).wsize as usize;
            let sf_size = (*self.ptr).sf_size as usize;
            let data = (*self.ptr).data;

            let mut cubes = Vec::with_capacity(count);
            for i in 0..count {
                let cube_ptr = data.add(i * wsize);
                let mut cube_data = Vec::with_capacity(wsize);
                for j in 0..wsize {
                    cube_data.push(*cube_ptr.add(j));
                }
                cubes.push(SerializedCube { data: cube_data });
            }

            SerializedCover {
                count,
                wsize,
                sf_size,
                cubes,
            }
        }
    }
}
