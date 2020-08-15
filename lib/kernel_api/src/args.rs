// use alloc::vec::Vec;
// use alloc::vec;
// use alloc::boxed::Box;
// use alloc::string::String;
// use core::str;
// use core::ops::Index;
// use core::mem;
// use core::ptr;
// use core::slice;
//
//
// use crate::cstr::{CString, cstr_len};
// use crate::OsResult;
//
// #[derive(PartialEq, PartialOrd, Eq, Ord, Hash, Clone)]
// pub struct CArgs {
//     inner: Box<[u8]>,
// }
//
// pub struct ArgsIter {
//     inner: Box<[u8]>,
//     offset: usize,
// }
//
// pub unsafe fn args_len(args_ptr: *const u8) -> usize {
//     let mut index = 0;
//     while !(*args_ptr.offset(index) == 0 && *args_ptr.offset(index + 1) == 0) {
//         index += 1;
//     }
//     (index + 2) as usize
// }
//
// impl CArgs {
//     pub fn new(args: Vec<String>) -> OsResult<Self>  {
//         let mut inner_data = Vec::new();
//         for arg in args {
//             inner_data.reserve_exact(arg.len() + 1);
//             // for byte in arg.as_bytes() {
//             //     inner_data.push(*byte);
//             // }
//             // inner_data.push(0);
//
//             let c_arg = CString::new(arg)?;
//             inner_data.extend(c_arg.as_bytes_with_nul());
//
//         }
//         inner_data.reserve_exact(1);
//         inner_data.push(0);
//         let inner = inner_data.into_boxed_slice();
//         Ok(Self { inner })
//     }
//
//     pub unsafe fn from_vec_with_nul_unchecked(v: Vec<u8>) -> Self {
//         Self { inner: v.into_boxed_slice() }
//     }
//
//     pub unsafe fn from_raw(ptr: *mut u8) -> Self {
//         let len = args_len(ptr);
//         let slice = slice::from_raw_parts_mut(ptr, len);
//         Self { inner: Box::from_raw(slice as *mut [u8]) }
//     }
//
//     pub fn into_raw(self) -> *mut u8 {
//         Box::into_raw(self.into_inner()) as *mut u8
//     }
//
//     pub fn as_ptr(&self) -> *const u8 {
//         self.inner.as_ptr()
//     }
//
//     pub fn as_mut_ptr(&mut self) -> *mut u8 {
//         self.inner.as_mut_ptr()
//     }
//
//     fn into_inner(self) -> Box<[u8]> {
//         let this = mem::ManuallyDrop::new(self);
//         unsafe { ptr::read(&this.inner) }
//     }
//
//     pub fn as_bytes(&self) -> &[u8] {
//         &self.inner
//     }
//
//     pub fn len(&self) -> usize {
//         self.inner.len()
//     }
// }
//
// impl Default for CArgs {
//     fn default() -> Self {
//         Self { inner: vec![0, 0].into_boxed_slice() }
//     }
// }
//
// // impl IntoIterator for CArgs {
// //     type Item = String;
// //     type IntoIter = ArgsIter;
// //
// //     fn into_iter(self) -> Self::IntoIter {
// //         Self::IntoIter {
// //             inner: self.into_inner(),
// //             offset: 0,
// //         }
// //     }
// // }
//
// impl<'a> IntoIterator for &'a CArgs {
//     type Item = String;
//     type IntoIter = ArgsIter;
//
//     fn into_iter(self) -> Self::IntoIter {
//         use shim::io::Write;
//
//         let mut bytes_v = Vec::new();
//         let _bytes = bytes_v.write(self.as_bytes()).unwrap();
//         Self::IntoIter {
//             inner: bytes_v.into_boxed_slice(),
//             offset: 0,
//         }
//     }
// }
//
//
//
// impl<'a> Iterator for ArgsIter {
//     type Item = String;
//
//     fn next(&mut self) -> Option<Self::Item> {
//         let (ptr, len) = unsafe {
//             let ptr = self.inner.as_mut_ptr().offset(self.offset as isize);
//             let len = cstr_len(ptr as *const u8);
//             (ptr, len)
//         };
//         if unsafe { *ptr } == 0 {
//             None
//         } else {
//             self.offset += len;
//             let c_string = unsafe { CString::from_raw(ptr) };
//             Some(c_string.into_string().expect("CString failed to convert to String"))
//         }
//     }
// }
// //
// // impl<'a> Index<usize> for Args {
// //     type Output = &'a String;
// //
// //     fn index(&'a self, idx: usize) -> Self::Output {
// //         let arg_len = self.arg_lens[idx];
// //         let starting: u8 = self.arg_lens[0..idx].iter().sum();
// //         &String::from(str::from_utf8(&self.bytes[starting as usize..arg_len as usize]).unwrap())
// //     }
// // }
