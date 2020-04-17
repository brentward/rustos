use core::str;

use crate::*;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DirEnt {
    d_type: DirType,
    name: [u8; 512],
    name_len: u64,
}

#[derive(Copy, Clone, Debug)]
pub enum DirType {
    File,
    Dir,
    None,
}

impl DirEnt {
    pub fn new(d_type: DirType, name: &str) -> DirEnt {
        let name_len = name.len();
        let name_bytes = name.as_bytes();
        let mut name = [0u8; 512];
        name[..name_len as usize].clone_from_slice(name_bytes);

        DirEnt {
            d_type,
            name,
            name_len: name_len as u64,
        }
    }

    pub fn name(&self) -> Result<&str, str::Utf8Error> {
        str::from_utf8(&self.name[..self.name_len as usize])
    }

    pub fn set_name(&mut self, name: &str) -> Result<(), ()> {
        let name_len = name.len();
        if name_len > 512 {
            Err(())
        } else {
            let name_bytes = name.as_bytes();
            let mut name = [0u8; 512];
            name[..name_len].clone_from_slice(name_bytes);
            self.name.clone_from_slice(&name);
            self.name_len = name_len as u64;
            Ok(())
        }
    }

    pub fn d_type(&self) -> &DirType {
        &self.d_type
    }

    pub fn set_d_type(&mut self, d_type: DirType) {
        self.d_type = d_type;
    }
}

impl Default for DirEnt {
    fn default() -> Self {
        DirEnt {
            d_type: DirType::None,
            name: [0; 512],
            name_len: 0,
        }
    }
}