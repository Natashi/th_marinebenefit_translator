
//COFF File Header
#[derive(Default)]
#[repr(packed(2))]
pub struct PEHeaderCOFF {
	pub magic: u32,
	pub machine: u16,
	pub n_sections: u16,
	pub timedate_stamp: u32,
	pub addr_symbol_table: u32,
	pub n_symbol_tables: u32,
	pub sz_opt_headers: u16,
	pub flags: u16,
}

//Optional Header
#[derive(Default)]
#[repr(packed(2))]
pub struct PEHeaderOptional {
	pub magic: u16,
	pub linker_version: u16,
	pub sz_code: u32,
	pub sz_init_data: u32,
	pub sz_uninit_data: u32,
	pub addr_entrypoint: u32,
	pub addr_base_code: u32,
	pub addr_base_data: u32,
}

//Optional Header Windows-Specific
#[derive(Default)]
#[repr(packed(2))]
pub struct PEHeaderWindows {
	pub addr_base_image: u32,
	pub align_sector: u32,
	pub align_file: u32,
	pub version_os: u32,
	pub version_image: u32,
	pub version_subsystem: u32,
	pub version_win32: u32,
	pub sz_image: u32,
	pub sz_headers: u32,
	pub checksum: u32,
	pub subsystem: u16,
	pub flags_dll: u16,
	pub sz_stack_reserve: u32,
	pub sz_stack_commit: u32,
	pub sz_heap_reserve: u32,
	pub sz_heap_commit: u32,
	pub flags_loader: u32,
	pub n_rva_sizes: u32,
}

//Section Headers
#[derive(Default, Clone, Copy)]
#[repr(packed(2))]
pub struct PESectionHeader {
	pub name: u64,
	pub sz_virtual: u32,
	pub addr_virtual: u32,
	pub sz_physical: u32,
	pub addr_physical: u32,
	pub irrelevant: [u32; 4]
}