//! Conversion utilities between UnsafeCover and SerializedCover

use crate::ipc::{SerializedCover, SerializedCube};
use crate::{sys, UnsafeCover};
use std::os::raw::c_int;

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

    /// Deserialize a cover from IPC data
    pub(crate) fn deserialize(sc: &SerializedCover) -> Self {
        unsafe {
            // Create a new cover with the appropriate size
            let ptr = sys::sf_new(sc.count as c_int, sc.sf_size as c_int);

            // Copy cube data
            let data = (*ptr).data;
            for (i, cube) in sc.cubes.iter().enumerate() {
                let cube_ptr = data.add(i * sc.wsize);
                for (j, &word) in cube.data.iter().enumerate() {
                    *cube_ptr.add(j) = word;
                }
            }

            // Update count
            (*ptr).count = sc.count as c_int;

            UnsafeCover::from_raw(ptr)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unsafe_espresso::UnsafeEspresso;

    #[test]
    fn test_cover_serialization_roundtrip() {
        // Initialize UnsafeEspresso to set up cube structure
        let _esp = UnsafeEspresso::new(2, 1);

        // Create a simple unsafe cover
        let cover = UnsafeCover::new(2, unsafe { sys::cube.size as usize });

        // Serialize and deserialize
        let serialized = cover.serialize();
        let deserialized = UnsafeCover::deserialize(&serialized);

        // Verify the serialization roundtrip worked
        assert_eq!(serialized.count, deserialized.serialize().count);
    }
}

