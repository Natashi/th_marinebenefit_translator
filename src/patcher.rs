use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Read, Write, BufReader, BufRead};
use std::ptr::{read_unaligned, addr_of};

use nutil::*;
use crate::executable::*;
use crate::headers::*;

use iced_x86::{Decoder, DecoderOptions, Instruction};
use encoding_rs::SHIFT_JIS;
use encoding_rs_io::DecodeReaderBytesBuilder;
use regex::Regex;
use bytebuffer::ByteBuffer;

static STRING_SEARCH_REGIONS: &[(u32, u32)] = &[
	//Spell names
	(0x2c4f18, 0x2c6003),
	//(0x02c54e0, 0x2c6003),
	
	//Pause menu strings
	(0x2c6170, 0x2c6263),
	
	//Music names
	(0x2c6374, 0x2c6517),
	
	//Menu strings
	(0x2c6518, 0x2c681f),
	
	//Menu strings 2: String Harder
	(0x2c6820, 0x2c6ee3),
	
	//Stage strings
	(0x2c75c8, 0x2c7737),
	
	//Dialogues
	(0x2c7738, 0x2cdc1b),

	//Game name
	(0x2cdc6c, 0x2cdc8b),
	
	//Player spell names
	(0x2ceb38, 0x2cec1f),
	
	//Endings
	(0x2cec28, 0x2d0ac4),
];

pub struct StringRef {
	pub str: Vec<u8>,		//String text as bytes
	pub addr_virt: u32,		//Virtual addr of the string
	pub addr_phys: u32,		//Physical addr of the string
	pub xrefs: Vec<u32>		//Physical addrs of instrs referencing the string
}

#[derive(PartialEq, Eq)]
pub enum PatcherType {
	Loader,
	Patcher,
}
pub struct Patcher {
	ptype: PatcherType,
	
	file: Option<File>,
	exe: Executable,
	
	map_strings: HashMap<u32, StringRef>,
}
impl Patcher {
	pub fn new_loader() -> Self {
		Self {
			ptype: PatcherType::Loader,
			file: None,
			exe: Executable::new(),
			map_strings: HashMap::new(),
		}
	}
	pub fn new_patcher() -> Self {
		Self {
			ptype: PatcherType::Patcher,
			file: None,
			exe: Executable::new(),
			map_strings: HashMap::new(),
		}
	}
	
	pub fn initialize(&mut self, path: &str) -> Result<(), NError> {
		self.file = match File::open(path) {
			Err(e) => return Err(NError::ErrIO(e)),
			Ok(t) => Some(t),
		};
		
		self.exe.initialize(self.file.as_mut().unwrap())?;
		
		if self.exe.get_section(".text").is_none()
			| self.exe.get_section(".data").is_none()
			| self.exe.get_section(".rdata").is_none()
			| self.exe.get_section(".rsrc").is_none()
		{
			return Err(NError::ErrInvalidExe);
		}
		
		Ok(())
	}
	
	// ----------------------------------------------------------
	// Loader methods
	
	pub fn loader_load_strings_and_refs(&mut self) -> Result<(), NError> {
		macro_rules! wrap_io_operation {
			( $wrp:expr ) => {
				match $wrp {
					Err(e) => return Err(NError::ErrIO(e)),
					Ok(t) => t,
				}
			};
		}
		
		if self.ptype != PatcherType::Loader {
			return Err(NError::ErrInvalidOperation);
		}
		
		println!("Reading the executable...");
		
		let file = self.file.as_mut().unwrap();
		let img_base = unsafe { 
			read_unaligned(addr_of!(self.exe.pe_header_win.addr_base_image)) 
		};
		
		// Load strings
		{
			let rdata = self.exe.get_section(".rdata").unwrap();
			
			let mut str_bytes: Vec<u8> = Vec::new();
			let mut buffer = [0; 4096];
			
			let mut cls_add_string = |s_bytes: &mut Vec<u8>, addr_phys: u32| {
				//let dbg_str = SHIFT_JIS.decode(str_bytes.as_slice()).0.into_owned();
				
				// Calculate the virt addr from the given phys addr
				let addr_virt = img_base + rdata.addr_virtual 
					+ (addr_phys - rdata.addr_physical);
				
				let sref = StringRef {
					str: s_bytes.clone(),
					addr_virt,
					addr_phys,
					xrefs: Vec::new(),
				};
				self.map_strings.insert(addr_virt, sref);
				
				s_bytes.clear();
			};
			
			for (bound_begin, bound_end) in STRING_SEARCH_REGIONS {
				let bound_size = bound_end - bound_begin;
				if bound_size > 0 {
					let mut cur_pos = *bound_begin;
					let end_pos = *bound_end;
					
					while cur_pos < end_pos {
						let remain = end_pos - cur_pos;
						
						wrap_io_operation!(file.seek(SeekFrom::Start(cur_pos as u64)));
						let ioread: usize = wrap_io_operation!(file.read(&mut buffer[..]));
						let gcount = std::cmp::min(ioread as u32, remain);
						
						for i in 0..gcount {
							let cur_pos_b = cur_pos + i;
							
							let ch: char = buffer[i as usize] as char;
							match ch {
								'\0' => {
									if !str_bytes.is_empty() {
										let addr_phys = cur_pos_b - str_bytes.len() as u32;
										cls_add_string(&mut str_bytes, addr_phys);
									}
								}
								_ => str_bytes.push(ch as u8),
							}
						}
						cur_pos += gcount;
					}
					if !str_bytes.is_empty() {
						// Bound ended, flush remaining str
						
						let addr_phys = end_pos - str_bytes.len() as u32;
						cls_add_string(&mut str_bytes, addr_phys);
					}
				}
			}
		}
		
		// Load refs
		{
			let text = self.exe.get_section(".text").unwrap();
			
			/*
			use iced_x86::FastFormatter;
			
			let mut formatter = FastFormatter::new();
			formatter.options_mut().set_always_show_memory_size(false);
			formatter.options_mut().set_always_show_segment_register(false);
			formatter.options_mut().set_show_symbol_address(false);
			formatter.options_mut().set_space_after_operand_separator(false);
			formatter.options_mut().set_use_hex_prefix(true);
			formatter.options_mut().set_uppercase_hex(false);
			*/
			
			//let mut str_out = String::new();
			let mut instr = Instruction::default();
			
			let mut text_buf = Vec::new();
			text_buf.resize(text.sz_physical as usize, 0);
			
			wrap_io_operation!(file.seek(SeekFrom::Start(text.addr_physical as u64)));
			let avail_size: usize = wrap_io_operation!(file.read(&mut text_buf));
			
			// Use an x86 disassembler to iterate through all instructions
			// How I wish every instrs had identical fucking lengths :hatred:
			
			let mut decoder = Decoder::with_ip(32, &text_buf, 
				text.addr_physical as u64, DecoderOptions::NONE);
			
			let mut i_decode: usize = 0;
			while i_decode < avail_size && decoder.can_decode() {
				decoder.decode_out(&mut instr);
				
				let instr_len = instr.len();
				
				//str_out.clear();
				//formatter.format(&instr_out, &mut str_out);
				
				if instr_len == 5 {
					// 0xb8 [imm32] -> mov eax, [imm32]
					// 0x68 [imm32] -> push [imm32]
					let buf_slice = &text_buf[i_decode..(i_decode + instr_len)];
					
					let str_virt_addr = 
						if buf_slice.len() == 5 && buf_slice[0] == 0xb8 || buf_slice[0] == 0x68 {
							fn _slc_to_array(x : &[u8]) -> &[u8; 4] {	// Convert slice to array
								x.try_into().unwrap()
							}
							u32::from_le_bytes(*_slc_to_array(&buf_slice[1..]))
						}
						else { 
							0 
						};
					
					if str_virt_addr != 0 {
						// Check if the value is one of the strings we have
						if let Some(find) = self.map_strings.get_mut(&str_virt_addr) {
							find.xrefs.push(instr.ip32());
						}
					}
				}
				
				i_decode += instr_len;
			}
		}
		
		Ok(())
	}
	
	pub fn loader_create_translation_file(&self, out_path: &str) -> Result<(), NError> {
		if self.ptype != PatcherType::Loader {
			return Err(NError::ErrInvalidOperation);
		}
		
		println!("Creating translation file...");
		
		let mut out_file = match File::create(out_path) {
			Err(e) => return Err(NError::ErrIO(e)),
			Ok(t) => t,
		};
		
		fn _write(this: &Patcher, file: &mut File) -> io::Result<()> {
			writeln!(file, "// Do not edit the hexadecimal values")?;
			writeln!(file)?;
			writeln!(file, "//    Format: [...] {{{{Replacing String}}}} {{{{Original String}}}} ...")?;
			writeln!(file, "// The \"Replacing String\" field may be left empty, in which case the string will not be patched.\n")?;
			writeln!(file, "// IMPORTANT: Strings of certain types have maximum sizes (in bytes, using Shift-JIS encoding).")?;
			writeln!(file, "//    Spell card name:    62 bytes")?;
			writeln!(file, "//    Dialogue line:      43 bytes")?;
			writeln!(file, "//    Ending line:        94 bytes")?;
			writeln!(file, "//    * Exceeding the max size can and will crash the game.")?;
			writeln!(file)?; writeln!(file)?;
			
			let mut vec_refs = this.map_strings
				.iter()
				.map(|x| x.1)
				.collect::<Vec<&StringRef>>();
			vec_refs.sort_by(|a, b| a.addr_phys.cmp(&b.addr_phys));
			
			for i in vec_refs {
				//if i.xrefs.len() == 0 { continue; }
				
				write!(file, "[{:08x},{:08x}] ", i.addr_virt, i.addr_phys)?;
				write!(file, "{{{{}}}}                {{{{")?;
				
				// Write string as raw bytes
				file.write_all(i.str.as_slice())?;
				
				write!(file, "}}}} ")?;
				
				let xrefs_vec = i.xrefs
					.iter()
					.map(|x| format!("{:08x}", x))
					.collect::<Vec<String>>();
				writeln!(file, "[{}]", xrefs_vec.join(","))?;
			}
			
			Ok(())
		}
		
		match _write(self, &mut out_file) {
			Err(e) => Err(NError::ErrIO(e)),
			_ => Ok(()),
		}
	}
	
	// ----------------------------------------------------------
	// Patcher methods
	
	pub fn patcher_load_string_ref_file(&mut self, path: &str) -> Result<(), NError> {
		if self.ptype != PatcherType::Patcher {
			return Err(NError::ErrInvalidOperation);
		}
		
		println!("Reading the translation file...");
		
		let out_file = match File::open(path) {
			Err(e) => return Err(NError::ErrIO(e)),
			Ok(t) => t,
		};
		
		// WARNING: This reads the file as Shift-JIS, and converts the lines into UTF-8 in Rust
		let file_reader = BufReader::new(
			DecodeReaderBytesBuilder::new()
				.encoding(Some(SHIFT_JIS))
				.build(out_file));
		
		let regex_pattern = concat!(
			r"(?:\[([0-9a-f]{8}),[0-9a-f]{8}\]\s+)",
			r"(?:\{\{(.+)\}\}\s+)",
			r"(?:\{\{.*\}\}\s+)",
			r"(?:\[((?:[0-9a-f]{8},?)+)\])",
		);
		let regex = match Regex::new(regex_pattern) {
			Err(e) => return Err(NError::ErrOther(e.to_string())),
			Ok(t) => t,
		};
		
		for line in file_reader.lines().flatten() {
			if line.len() < 20 || !line.starts_with('[') { continue; }
			
			let res_match = regex.captures(line.trim());
			if let Some(smatch) = &res_match {
				//Group 1: virtual addr
				//Group 2: replacing string
				//Group 3: xref list
				
				let s_addr_virt = smatch.get(1).unwrap().as_str();
				let s_patch_str = smatch.get(2).unwrap().as_str();
				let s_xref_list = smatch.get(3).unwrap().as_str();
				
				// If the replacing str is empty, don't patch that string
				if !s_patch_str.is_empty() {
					if let Ok(addr_virt) = u32::from_str_radix(s_addr_virt, 16) {
						// Convert UTF-8 string into Shift-JIS bytes
						let bytes_shjis = SHIFT_JIS.encode(s_patch_str).0.into_owned();
						
						let mut sref = StringRef {
							str: bytes_shjis,
							addr_virt,
							addr_phys: 0,
							xrefs: Vec::new(),
						};
						
						let vec_xref = s_xref_list
							.split(',')
							.map(|x| u32::from_str_radix(x, 16).unwrap_or_default())
							.filter(|x| *x > 0)
							.collect::<Vec<u32>>();
						for i in vec_xref {
							sref.xrefs.push(i);
						}
						
						self.map_strings.insert(sref.addr_virt, sref);
					}
				}
			}
		}
		
		println!("Found {} string(s) to be patched", self.map_strings.len());
		
		Ok(())
	}
	
	pub fn patcher_create_patch_exe(&mut self, out_path: &str) -> Result<(), NError> {
		macro_rules! wrap_io_operation {
			( $wrp:expr ) => {
				match $wrp {
					Err(e) => return Err(NError::ErrIO(e)),
					Ok(t) => t,
				}
			};
		}
		
		if self.ptype != PatcherType::Patcher {
			return Err(NError::ErrInvalidOperation);
		}
		
		println!("Patching executable...");
		
		let out_file = &mut wrap_io_operation!(File::create(out_path));
		
		// Find the last section of the exe, new strings will be appended there
		let last_sect: &mut PESectionHeader = self.exe.sections
			.iter_mut()
			.max_by_key(|x| x.addr_physical).unwrap();
		
		let img_base = unsafe { 
			read_unaligned(addr_of!(self.exe.pe_header_win.addr_base_image)) 
		};
		
		// Addresses to place the new strings
		let str_reloc_base_addr_virt = img_base + last_sect.addr_virtual + last_sect.sz_physical;
		let str_reloc_base_addr_phys = last_sect.addr_physical + last_sect.sz_physical;
		
		let mut str_reloc_buffer = ByteBuffer::new();
		let mut reloc_size = 0u32;
		
		// Write strings into the temp buffer and update addresses
		{
			/*
			let mut vec_refs = self.map_strings
				.values_mut()
				.collect::<Vec<&mut StringRef>>();
			vec_refs.sort_by(|a, b| a.addr_phys.cmp(&b.addr_phys));
			*/
			let vec_refs = self.map_strings.values_mut();
			for str_ref in vec_refs {
				str_ref.addr_virt = str_reloc_base_addr_virt + reloc_size;
				str_ref.addr_phys = str_reloc_base_addr_phys + reloc_size;
				
				str_reloc_buffer.write_bytes(str_ref.str.as_slice());
				str_reloc_buffer.write_u8(0);
				
				// Add size, then align to 4 bytes
				reloc_size += str_ref.str.len() as u32 + 1;
				while reloc_size % 4 != 0 {
					str_reloc_buffer.write_u8(0);
					reloc_size += 1;
				}
			}
			
			let align_section = self.exe.pe_header_win.align_sector;
			let align_file = self.exe.pe_header_win.align_file;
			let align_max = std::cmp::max(align_section, align_file);
			{
				let old_sz_virt = last_sect.sz_virtual;
				let mut new_sz_phys = last_sect.sz_physical + reloc_size;
				while new_sz_phys % align_max != 0 {
					str_reloc_buffer.write_u8(0);
					new_sz_phys += 1;
				}
				last_sect.sz_physical = new_sz_phys;
				last_sect.sz_virtual = new_sz_phys;
				
				// Expand the image size to cover the new strings
				// The program will crash if it tries to read beyond the image size
				let new_size = {
					let mut res = self.exe.pe_header_win.sz_image + (new_sz_phys - old_sz_virt);
					if res % align_file > 0 {
						res = (res / align_file + 1) * align_file;
					}
					res
				};
				self.exe.pe_header_win.sz_image = new_size;
			}
		}
		
		// Copy exe
		{
			let org_file = self.file.as_mut().unwrap();
			wrap_io_operation!(org_file.seek(SeekFrom::Start(0)));
			wrap_io_operation!(out_file.seek(SeekFrom::Start(0)));
			
			copy_to(org_file, out_file);
			wrap_io_operation!(out_file.flush());
		}
		
		// Copy the string relocation buffer to the end of the exe
		{
			str_reloc_buffer.set_rpos(0);
			wrap_io_operation!(out_file.seek(SeekFrom::End(0)));
			
			copy_to(&mut str_reloc_buffer, out_file);
		}
		
		// Replace string refs
		{
			for str_ref in self.map_strings.values() {
				for i_xref in &str_ref.xrefs {
					// +1 for the initial opcode byte
					wrap_io_operation!(out_file.seek(SeekFrom::Start((i_xref + 1) as u64)));
					wrap_io_operation!(out_file.write_all(&str_ref.addr_virt.to_le_bytes()));
				}
			}
		}
		
		// Update section table
		{
			fn write_struct<T: Sized, W: Write>(dest: &mut W, val: &T) -> io::Result<()> {
				let bytes = unsafe { any_as_u8_slice(val) };
				dest.write_all(bytes)
			}
			
			wrap_io_operation!(out_file.seek(SeekFrom::Start(self.exe.offset_pe_header as u64)));
			wrap_io_operation!(write_struct(out_file, &self.exe.pe_header));
			wrap_io_operation!(write_struct(out_file, &self.exe.pe_header2));
			wrap_io_operation!(write_struct(out_file, &self.exe.pe_header_win));
			
			wrap_io_operation!(out_file.seek(SeekFrom::Start(self.exe.offset_section_table as u64)));
			for i_section in &self.exe.sections {
				wrap_io_operation!(write_struct(out_file, i_section));
			}
		}
		
		println!("Executable successfully patched");
		
		Ok(())
	}
}