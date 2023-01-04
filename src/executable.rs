use std::fs::File;
use std::io::{self, Seek, SeekFrom};
use std::mem::{size_of};

use crate::headers::*;

use nutil::*;

//Executable class
pub struct Executable {
	pub offset_pe_header: u32,
	pub offset_section_table: u32,
	
	pub pe_header: PEHeaderCOFF,
	pub pe_header2: PEHeaderOptional,
	pub pe_header_win: PEHeaderWindows,
	
	pub sections : Vec<PESectionHeader>,
}
impl Executable {
	pub fn new() -> Self {
		Self {
			offset_pe_header: 0,
			pe_header: PEHeaderCOFF::default(),
			pe_header2: PEHeaderOptional::default(),
			pe_header_win: PEHeaderWindows::default(),
			
			offset_section_table: 0,
			sections: Vec::new(),
		}
	}
	pub fn initialize(&mut self, file: &mut File) -> Result<(), NError> {
		macro_rules! fread_t {
			( $type:ty, $out:expr ) => {
				unsafe {
					let res: Result<$type, io::Error> = nutil::read_t(file);
					if res.is_err() {
						return Err(NError::ErrIO(res.err().unwrap()));
					}
					$out = res.unwrap();
				}
			};
		}
		macro_rules! check_err_from_io {
			( $wrp:expr ) => {
				if $wrp.is_err() {
					return Err(NError::ErrIO($wrp.unwrap_err()));
				}
			};
		}
		
		let file_size: u64;
		{
			let res = file.metadata();
			file_size = if res.is_ok() { res.unwrap().len() } else { 0 }
		}
		
		if file_size < 0x10000 {
			return Err(NError::ErrInvalidExe);
		}
		{
			let mz_header : u16 = nutil::read_t_or(file, 0u16);
			if mz_header != 0x5a4d {	//Check MZ header
				return Err(NError::ErrInvalidExe);
			}
		}
		
		check_err_from_io!(file.seek(SeekFrom::Start(0x3c)));
		self.offset_pe_header = nutil::read_t_or(file, 0u32);
		
		let min_size = (self.offset_pe_header as usize) 
			+ (size_of::<PEHeaderCOFF>() + size_of::<PEHeaderOptional>() 
				+ size_of::<PEHeaderWindows>());
		if file_size < min_size as u64 {
			return Err(NError::ErrInvalidExe);
		}
		
		{
			check_err_from_io!(file.seek(SeekFrom::Start(self.offset_pe_header as u64)));
			
			fread_t!(PEHeaderCOFF, self.pe_header);
			//println!("{}", obj_to_string(&self.pe_header));
			
			if self.pe_header.magic != 0x00004550 || self.pe_header.n_sections == 0 {
				return Err(NError::ErrInvalidExe);
			}
			
			fread_t!(PEHeaderOptional, self.pe_header2);
			fread_t!(PEHeaderWindows, self.pe_header_win);
			
			if self.pe_header2.magic != 0x010b {
				return Err(NError::ErrInvalidExe);
			}
		}
		{
			self.offset_section_table = self.offset_pe_header 
				+ size_of::<PEHeaderCOFF>() as u32 + self.pe_header.sz_opt_headers as u32;
			check_err_from_io!(file.seek(SeekFrom::Start(self.offset_section_table as u64)));
			
			for _ in 0..self.pe_header.n_sections {
				let section: PESectionHeader;
				fread_t!(PESectionHeader, section);
				self.sections.push(section);
			}
		}
		
		Ok(())
	}
	
	pub fn get_section(&self, name: &str) -> Option<&PESectionHeader> {
		let mut buf = [0u8; 8];
		buf[..name.len()].clone_from_slice(name.as_bytes());
		
		for i in &self.sections {
			if buf == i.name.to_le_bytes() {
				return Some(i);
			}
		}
		None
	}
}