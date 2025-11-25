use regex::Regex;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle,
    STD_OUTPUT_HANDLE, SetConsoleMode,
};

fn main() {
    match enable_ansi_escape_codes() {
        Err(why) => println!(
            "ANSI escape codes can't be activated. Reason: {}\n Some messages will be displayed incorrectly",
            why
        ),
        Ok(_) => (),
    };

    println!("Enter directory or file path:");
    let mut path_str = String::new();
    io::stdin()
        .read_line(&mut path_str)
        .expect("Failed to read path");

    let path = Path::new(path_str.trim());

    if path.is_file() {
        let file_name = path.file_name().unwrap().to_str().unwrap().to_owned();
        println!("\nFile: {}", &file_name);

        match count_file(&path) {
            Err(why) => println!("\n\u{001B}[38;5;196m{}\u{001B}[0m", why),
            Ok(res) => println!("\u{001B}[38;5;82mTotal rows:\u{001B}[0m {}", res),
        };
    } else if path.is_dir() {
        println!("Enter file extensions (e.g. txt,rs,class):");
        let mut extensions_str = String::new();
        io::stdin()
            .read_line(&mut extensions_str)
            .expect("Failed to read extensions");

        print!("\nPath: {}Extensions: {}", &path_str, extensions_str);

        let clean_extention_str = extensions_str.replace("\r", "").replace("\n", "");
        let ext_vec: Vec<String> = clean_extention_str.split(',').map(String::from).collect();

        match count_dir(&path, &ext_vec) {
            Err(why) => println!("\n\u{001B}[38;5;196m{}\u{001B}[0m", why),
            Ok(res) => println!("\u{001B}[38;5;82mTotal rows:\u{001B}[0m {}", res),
        };
    } else {
        println!(
            "\n\u{001B}[38;5;196mCouldn't find neither file nor directory using the path\u{001B}[0m"
        );
    }

    println!("\nPress Enter to exit...");
    io::stdin().read_line(&mut String::new()).unwrap();
}

fn count_dir(path: &Path, ext_vec: &Vec<String>) -> Result<usize, String> {
    let dir_iter = match fs::read_dir(path) {
        Err(why) => return Err(format!("Couldn't open directory: {}", why)),
        Ok(dir_iter) => dir_iter,
    };

    let mut counter = 0;
    for entry in dir_iter {
        let path = entry.unwrap().path();
        if path.is_dir() {
            match count_dir(&path, &ext_vec) {
                Err(why) => return Err(why),
                Ok(res) => counter = counter + res,
            }
        } else {
            let path_ext = path.extension().unwrap().to_str().unwrap().to_owned();
            if ext_vec.contains(&path_ext) {
                match count_file(&path) {
                    Err(why) => return Err(why),
                    Ok(res) => counter = counter + res,
                };
            }
        }
    }

    Ok(counter)
}

fn count_file(path: &Path) -> Result<usize, String> {
    let display = path.display();
    let mut file = match File::open(&path) {
        Err(why) => return Err(format!("Couldn't open {}: {}", display, why)),
        Ok(file) => file,
    };

    let mut file_content = String::new();
    match file.read_to_string(&mut file_content) {
        Err(why) => return Err(format!("Couldn't read {}: {}", display, why)),
        Ok(_) => (),
    };

    let re = Regex::new("\n").unwrap();
    Ok(re.find_iter(&file_content).count())
}

fn enable_ansi_escape_codes() -> Result<(), Box<dyn Error>> {
    unsafe {
        let stdout_handle: HANDLE = GetStdHandle(STD_OUTPUT_HANDLE)?;

        let mut current_mode: CONSOLE_MODE = CONSOLE_MODE(0);
        if GetConsoleMode(stdout_handle, &mut current_mode).is_err() {
            let error: Box<dyn Error> = String::from("Failed to get console mode").into();
            return Err(error);
        }

        let new_mode = current_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING;

        if SetConsoleMode(stdout_handle, new_mode).is_err() {
            let error: Box<dyn Error> = String::from("Failed to get console mode").into();
            return Err(error);
        }
    }
    Ok(())
}
