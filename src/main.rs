use regex::Regex;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::Console::{
    CONSOLE_MODE, ENABLE_VIRTUAL_TERMINAL_PROCESSING, GetConsoleMode, GetStdHandle,
    STD_OUTPUT_HANDLE, SetConsoleMode,
};

const SET_CURSOR_VISIBLE: &str = "\u{001B}[?25h";
const SET_CURSOR_INVISIBLE: &str = "\u{001B}[?25l";
const BEGINNING_OF_PREV_LINE: &str = "\u{001B}[1F";
const ERASE_LINE: &str = "\u{001B}[K";
const RESET_COLOR: &str = "\u{001B}[0m";
const RED_COLOR: &str = "\u{001B}[38;5;196m";
const GREEN_COLOR: &str = "\u{001B}[38;5;82m";

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
    let (tx, rx) = mpsc::channel();

    if path.is_file() {
        let file_name = path.file_name().unwrap().to_str().unwrap().to_owned();
        println!("\nFile: {}", &file_name);

        let now = Instant::now();
        match count_file(&path) {
            Err(why) => println!("{}", build_err(get_secs(&now), why)),
            Ok(res) => println!("{}", build_ok_file(get_secs(&now), res)),
        };
    } else if path.is_dir() {
        println!("Enter file extensions (e.g. txt,rs,class):");
        let mut extensions_str = String::new();
        io::stdin()
            .read_line(&mut extensions_str)
            .expect("Failed to read extensions");

        print!("\nPath: {}Extensions: {}", &path_str, extensions_str);

        let clean_extensions_str = extensions_str.replace("\r", "").replace("\n", "");
        let ext_vec: Vec<String> = clean_extensions_str.split(',').map(String::from).collect();

        thread::spawn(move || show_progress(&rx));

        let now = Instant::now();
        match count_dir(&path, &ext_vec) {
            Err(why) => println!("{}", build_err(get_secs(&now), why)),
            Ok(res) => println!("{}", build_ok_dir(get_secs(&now), res.rows, res.files)),
        };
    } else {
        println!(
            "{}{}{}",
            RED_COLOR, "\nCouldn't find neither file nor directory using the path", RESET_COLOR
        );
    }

    let _ = tx.send(true);

    print!("{}", SET_CURSOR_VISIBLE);
    println!("\nPress Enter to exit...");
    io::stdin().read_line(&mut String::new()).unwrap();
}

fn get_secs(instant: &Instant) -> f64 {
    instant.elapsed().as_millis() as f64 / 1000.0
}

fn show_progress(rx: &Receiver<bool>) {
    let mut counter = 0;
    loop {
        match rx.try_recv() {
            Ok(stop_printing) => {
                if stop_printing {
                    return;
                }
            }
            _ => (),
        }

        print!("{}", SET_CURSOR_INVISIBLE);
        println!(
            "In progress{}",
            ".".repeat(counter) + BEGINNING_OF_PREV_LINE
        );
        counter = counter + 1;

        if counter == 4 {
            counter = 0;
        }
        thread::sleep(Duration::from_secs(1));

        print!("{}\r", ERASE_LINE);
    }
}

fn count_dir(path: &Path, ext_vec: &Vec<String>) -> Result<Total, String> {
    let dir_iter = match fs::read_dir(path) {
        Err(why) => return Err(format!("Couldn't open directory: {}", why)),
        Ok(dir_iter) => dir_iter,
    };

    let mut row_counter = 0;
    let mut file_counter = 0;
    for entry in dir_iter {
        let path = entry.unwrap().path();
        if path.is_dir() {
            match count_dir(&path, &ext_vec) {
                Err(why) => return Err(why),
                Ok(res) => {
                    row_counter = row_counter + res.rows;
                    file_counter = file_counter + res.files;
                }
            }
        } else {
            match count_dir_file(&path, &ext_vec) {
                Err(why) => return Err(why),
                Ok(res) => {
                    if res.ignore {
                        continue;
                    }
                    row_counter = row_counter + res.rows;
                    file_counter = file_counter + 1;
                }
            }
        }
    }

    Ok(Total {
        rows: row_counter,
        files: file_counter,
    })
}

fn count_dir_file(path: &Path, ext_vec: &Vec<String>) -> Result<FileTotal, String> {
    match path.extension() {
        Some(val) => {
            let path_ext = val.to_str().unwrap().to_owned();
            if ext_vec.contains(&path_ext) {
                match count_file(&path) {
                    Err(why) => return Err(why),
                    Ok(res) => {
                        return Ok(FileTotal {
                            rows: res,
                            ignore: false,
                        });
                    }
                };
            }
        }
        _ => (),
    }

    Ok(FileTotal {
        rows: 0,
        ignore: true,
    })
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

fn build_err(exec_time: f64, err_mes: String) -> String {
    format!(
        "Execution time: {} sec\n{}{}{}",
        exec_time, RED_COLOR, err_mes, RESET_COLOR
    )
}

fn build_ok_file(exec_time: f64, row_count: usize) -> String {
    format!(
        "Execution time: {} sec\n{}Total rows: {}{}",
        exec_time, GREEN_COLOR, RESET_COLOR, row_count
    )
}

fn build_ok_dir(exec_time: f64, row_count: usize, file_count: usize) -> String {
    format!(
        "Execution time: {} sec\n{}Total rows: {}{}\n{}Total files: {}{}",
        exec_time, GREEN_COLOR, RESET_COLOR, row_count, GREEN_COLOR, RESET_COLOR, file_count
    )
}

struct Total {
    rows: usize,
    files: usize,
}

struct FileTotal {
    rows: usize,
    ignore: bool,
}
