use std::io::{Read, Write, Result};
use std::mem::{size_of, MaybeUninit};
use std::slice::{from_raw_parts, from_raw_parts_mut};
use core::fmt;

pub unsafe fn read_into_byteptr<T, R: Read>(reader: &mut R, size: usize, out: *mut u8) -> Result<()> {
	let buffer_slice = from_raw_parts_mut(out, size);
	reader.read_exact(buffer_slice)
}

pub unsafe fn read_t<T, R: Read>(reader: &mut R) -> Result<T> {
	let t_size = size_of::<T>();
	let mut buffer = MaybeUninit::<T>::uninit();
	
	let res = read_into_byteptr::<T, R>(
		reader, t_size, buffer.as_mut_ptr() as *mut u8);
	if res.is_err() { Err(res.unwrap_err()) } else { Ok(buffer.assume_init()) }
}
pub unsafe fn read_into_t<T, R: Read>(reader: &mut R, out: &mut T) -> Result<()> {
	let t_size = size_of::<T>();
	let t_ptr = out as *mut T;
	
	let res = read_into_byteptr::<T, R>(
		reader, t_size, t_ptr as *mut u8);
	if res.is_err() { Err(res.unwrap_err()) } else { Ok(()) }
}

pub fn read_t_or<T, R: Read>(reader: &mut R, default: T) -> T {
	unsafe { read_t::<T, R>(reader) }.unwrap_or(default)
}

pub fn copy_to<R: Read, W: Write>(src: &mut R, dest: &mut W) -> usize {
	let mut buf = [0u8; 2048];
	let mut wrote = 0;
	loop {
		let read = src.read(&mut buf).unwrap_or(0);
		wrote += read;
		if read == 0 { break; }
		dest.write(&buf[..read]).unwrap_or_default();
	}
	return wrote;
}

pub unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
	from_raw_parts((p as *const T) as *const u8, size_of::<T>())
}

#[derive(Debug)]
pub enum NError {
	Ok,
	ErrInvalidOperation,
	ErrIO(std::io::Error),
	ErrInvalidExe,
	ErrNoSection,
	ErrOther(String),
}
impl fmt::Display for NError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match &self {
			NError::ErrInvalidOperation => write!(f, "Invalid operation"),
			NError::ErrIO(stde) => write!(f, "{:?}", stde),
			NError::ErrInvalidExe => write!(f, "Invalid executable input"),
			NError::ErrNoSection => write!(f, "Section not found"),
			NError::ErrOther(s) => write!(f, "{:?}", s),
			_ => write!(f, "No error"),
		}
	}
}

pub fn obj_to_string<T: Sized>(obj: &T) -> String {
	let bytes = unsafe { any_as_u8_slice(&obj) };
	let as_hexs = bytes.iter()
		.map(|x| format!("{:x}", x)).collect::<Vec<String>>();
	format!("{:?}", as_hexs.join(", "))
}