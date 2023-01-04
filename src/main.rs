use std::{env, process::exit};

mod headers;
mod executable;
mod patcher;

use nutil::NError;
use patcher::Patcher;

fn main() {
	let argv: Vec<String> = env::args().collect();
	
	//println!("{:?}", &argv);
	//println!("{}", argv.len());
	
	if argv.len() < 4 {
		print_help_and_exit();
	}
	
	let mode = &argv[1].as_bytes()[0];
	
	match *mode as char {
		'g' => {
			fn _do_stuff(argv: &Vec<String>) -> Result<(), NError> {
				let path_exe = &argv[2];
				let path_out = &argv[3];
				
				let mut loader = Patcher::new_loader();
				loader.initialize(path_exe)?;
				loader.loader_load_strings_and_refs()?;
				loader.loader_create_translation_file(path_out)?;
				
				Ok(())
			}
			match _do_stuff(&argv) {
				Err(e) => print_and_exit(&e.to_string()),
				_ => println!("Done"),
			}
		},
		'b' => {
			if argv.len() < 5 {
				print_help_and_exit();
			}
			
			fn _do_stuff(argv: &Vec<String>) -> Result<(), NError> {
				let path_exe_in = &argv[2];
				let path_translation_file = &argv[3];
				let path_exe_out = &argv[4];
				
				let mut patcher = Patcher::new_patcher();
				patcher.initialize(path_exe_in)?;
				patcher.patcher_load_string_ref_file(path_translation_file)?;
				patcher.patcher_create_patch_exe(path_exe_out)?;
				
				Ok(())
			}
			match _do_stuff(&argv) {
				Err(e) => print_and_exit(&e.to_string()),
				_ => println!("Done"),
			}
		},
		_ => ()
	}
}

fn print_help_and_exit() {
	print_and_exit(r#"
Format: MODE ARGS...
    MODE can be:
        g [input exe] [output translation file]
            Generates a translation text file
        b [input exe] [input translation file] [output exe]
            Patches the .exe into a new .exe from the translation text file"#
	);
}
fn print_and_exit(s: &str) {
	println!("{}", s);
	exit(-1);
}