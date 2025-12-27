use regex::Regex;
use std::error::Error;
use std::fs;
use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
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
const ERASE_LINE: &str = "\u{001B}[2K";
const RESET_COLOR: &str = "\u{001B}[0m";
const RED_COLOR: &str = "\u{001B}[38;5;196m";
const YELLOW_COLOR: &str = "\u{001B}[38;5;185m";
const GREEN_COLOR: &str = "\u{001B}[38;5;82m";

//const MULTITHREADING_MIN_CONDITION: i32 = 400_000;

fn main() {
    if let Err(why) = enable_ansi_escape_codes() {
        println!(
            "ANSI escape codes can't be activated. Reason: {}\n Some messages will be displayed incorrectly",
            why
        )
    };

    println!("Enter directory or file path:");
    let mut path_str = String::new();
    io::stdin()
        .read_line(&mut path_str)
        .expect("Failed to read path");

    println!("Enter file extensions (e.g. txt,rs,class):");
    let mut extensions_str = String::new();
    io::stdin()
        .read_line(&mut extensions_str)
        .expect("Failed to read extensions");

    let path = Path::new(path_str.trim());
    if path.is_file() {
        let file_name = path.file_name().unwrap().to_str().unwrap().to_owned();
        println!("\nFile: {}", &file_name);

        let now = Instant::now();
        match count_file(path) {
            Err(why) => println!("{}", build_err(get_secs(&now), why)),
            Ok(res) => println!("{}", build_ok_file(get_secs(&now), res)),
        };
    } else if path.is_dir() {
        let cores_num = match thread::available_parallelism() {
            Ok(res) => res.get(),
            Err(why) => {
                println!("{}", why);
                0
            }
        };

        if cores_num < 2 {
            if cores_num == 0 {
                println!(
                "{}",
                build_warning(
                    "Couldn't determine the number of available cores of the system\nOnly single-threaded processing available".to_string()
                )
            );
            }

            print!("\nPath: {}Extensions: {}", path_str, extensions_str);
            start_single_thread(path, &extensions_str);
        } else {
            println!(
                "The system was found as multithreading. If there are many files in the directory, you can consider using multi-threaded processing [y] to reduce execution time or single-threaded processing [n] as a default execution configuration"
            );
            let mult_th_answer = read_answer();

            if mult_th_answer {
                let (tx, rx) = mpsc::channel();
                thread::spawn(move || show_awaiting_message(AwaitingType::FileCounting, &rx));
                let total_files_counter = match count_all_files(path) {
                    Err(why) => {
                        println!("{}", build_err_no_time(why));
                        0 - 1
                    }
                    Ok(res) => res,
                };
                let _ = tx.send(true);
                thread::sleep(Duration::from_secs(1));

                if total_files_counter < 0 {
                    exit();
                    return;
                } else if total_files_counter == 0 {
                    println!(
                        "{}",
                        build_warning("Found no files in specified directory".to_string())
                    );
                    exit();
                    return;
                } else {
                    print!("\nPath: {}Extensions: {}", &path_str, extensions_str);
                    start_multi_thread(
                        path,
                        cores_num,
                        total_files_counter.try_into().unwrap(),
                        &extensions_str,
                    );
                }
            } else {
                println!("\nPath: {}Extensions: {}", &path_str, extensions_str);
                start_single_thread(path, &extensions_str);
            }
        }
    } else {
        println!(
            "{}\nCouldn't find neither file nor directory using the path{}",
            RED_COLOR, RESET_COLOR
        );
    }

    exit();
}

fn read_answer() -> bool {
    loop {
        let mut mult_th_answer = String::new();
        println!("Input [y/n]:");
        io::stdin()
            .read_line(&mut mult_th_answer)
            .expect("Failed to read answer");

        let clean_str = mult_th_answer.trim();

        if clean_str == "y" || clean_str == "Y" {
            println!("Multi-threaded processing started");
            return true;
        } else if clean_str == "n" || clean_str == "N" {
            println!("Single-threaded processing started");
            return false;
        }
    }
}

fn exit() {
    println!("\nPress Enter to exit...");
    io::stdin().read_line(&mut String::new()).unwrap();
}

fn get_secs(instant: &Instant) -> f64 {
    instant.elapsed().as_millis() as f64 / 1000.0
}

fn show_awaiting_message(aw_type: AwaitingType, rx: &Receiver<bool>) {
    let message = match aw_type {
        AwaitingType::Progress => "In progress",
        AwaitingType::FileCounting => "Counting files",
    };

    print!("{}", SET_CURSOR_INVISIBLE);
    let mut counter = 0;
    loop {
        if let Ok(stop_printing) = rx.try_recv()
            && stop_printing
        {
            print!("{}", SET_CURSOR_VISIBLE);
            return;
        }

        println!(
            "{}{}",
            message,
            ".".repeat(counter) + BEGINNING_OF_PREV_LINE
        );
        counter += 1;

        if counter == 4 {
            counter = 0;
        }
        thread::sleep(Duration::from_secs(1));

        print!("{}", ERASE_LINE);
    }
}

fn start_single_thread(path: &Path, extensions_str: &str) {
    let clean_extensions_str = extensions_str.trim();
    let ext_vec: Vec<String> = clean_extensions_str.split(',').map(String::from).collect();

    let now = Instant::now();

    let (tx, rx) = mpsc::channel();
    thread::spawn(move || show_awaiting_message(AwaitingType::Progress, &rx));
    match count_dir(path, &ext_vec) {
        Err(why) => println!("{}", build_err(get_secs(&now), why)),
        Ok(res) => println!("{}", build_ok_dir(get_secs(&now), res.rows, res.files)),
    };

    let _ = tx.send(true);
    thread::sleep(Duration::from_secs(1));
}

fn start_multi_thread(path: &Path, cores: usize, files: usize, extensions_str: &str) {
    let clean_extensions_str = extensions_str.trim();
    let ext_vec: Vec<String> = clean_extensions_str.split(',').map(String::from).collect();

    let now = Instant::now();
    let (tx_aw, rx_aw) = mpsc::channel();
    thread::spawn(move || show_awaiting_message(AwaitingType::Progress, &rx_aw));

    let (tx_th, rx_th) = mpsc::channel();
    let batch_size = files / cores;
    println!("Batch: {} Threads: {}", batch_size, cores);
    match extract_path_vec(path, files) {
        Ok(paths) => {
            let paths_ptr = Arc::new(paths);
            for i in 0..cores {
                let tx_cln = tx_th.clone();
                let ext_vec_cln = ext_vec.clone();
                let paths_ptr_cln = paths_ptr.clone();
                if i < cores - 1 {
                    let from = batch_size * i;
                    let to = batch_size * (i + 1);
                    thread::spawn(move || {
                        count_dirs(paths_ptr_cln, from, to, &ext_vec_cln, &tx_cln)
                    });
                } else {
                    let remainder = files % cores;
                    let from = batch_size * i;
                    let to = batch_size * (i + 1) + remainder;
                    thread::spawn(move || {
                        count_dirs(paths_ptr_cln, from, to, &ext_vec_cln, &tx_cln)
                    });
                }
            }
        }
        Err(why) => println!("{}", why),
    }

    let mut row_counter = 0;
    let mut file_counter = 0;
    let mut threads = 0;
    loop {
        match rx_th.try_recv() {
            Ok(res) => match res {
                Ok(res) => {
                    row_counter += res.rows;
                    file_counter += res.files;
                    threads += 1
                }
                Err(why) => println!("{}", why),
            },

            Err(_why) => {}
        }
        thread::sleep(Duration::from_millis(700));
        if threads == cores {
            break;
        }
    }

    let _ = tx_aw.send(true);
    thread::sleep(Duration::from_secs(1));

    println!(
        "{}",
        build_ok_dir(get_secs(&now), row_counter, file_counter)
    )
}

fn extract_path_vec(path: &Path, files: usize) -> Result<Vec<PathBuf>, String> {
    let mut vec = Vec::with_capacity(files);
    let dir_iter = match fs::read_dir(path) {
        Err(why) => return Err(format!("Couldn't open directory: {}", why)),
        Ok(dir_iter) => dir_iter,
    };
    for entry in dir_iter {
        let path = entry.unwrap().path();
        if path.is_dir() {
            match count_all_files(&path) {
                Ok(file_num) => match extract_path_vec(&path, file_num.try_into().unwrap()) {
                    Err(why) => return Err(why),
                    Ok(mut res) => {
                        vec.append(&mut res);
                    }
                },
                Err(why) => return Err(why),
            }
        } else {
            vec.push(path.to_path_buf())
        }
    }

    Ok(vec)
}

fn count_dirs(
    paths_vec: Arc<Vec<PathBuf>>,
    from: usize,
    to: usize,
    ext_vec: &Vec<String>,
    tx: &Sender<Result<Total, String>>,
) {
    let mut row_counter = 0;
    let mut file_counter = 0;

    let paths = &paths_vec[from..to];
    for path in paths {
        match count_dir_file(path, ext_vec) {
            Err(why) => {
                let _ = tx.send(Err(why));
            }
            Ok(res) => {
                if res.ignore {
                    continue;
                }
                row_counter += res.rows;
                file_counter += 1;
            }
        }
    }

    let _ = tx.send(Ok(Total {
        rows: row_counter,
        files: file_counter,
    }));
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
            match count_dir(&path, ext_vec) {
                Err(why) => return Err(why),
                Ok(res) => {
                    row_counter += res.rows;
                    file_counter += res.files;
                }
            }
        } else {
            match count_dir_file(&path, ext_vec) {
                Err(why) => return Err(why),
                Ok(res) => {
                    if res.ignore {
                        continue;
                    }
                    row_counter += res.rows;
                    file_counter += 1;
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
    if let Some(val) = path.extension() {
        let path_ext = val.to_str().unwrap().to_owned();
        if ext_vec.contains(&path_ext) {
            match count_file(path) {
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

    Ok(FileTotal {
        rows: 0,
        ignore: true,
    })
}

fn count_file(path: &Path) -> Result<usize, String> {
    let display = path.display();
    let mut file = match File::open(path) {
        Err(why) => return Err(format!("Couldn't open {}: {}", display, why)),
        Ok(file) => file,
    };

    let mut file_content = String::new();
    if let Err(why) = file.read_to_string(&mut file_content) {
        return Err(format!("Couldn't read {}: {}", display, why));
    };

    let re = Regex::new("\n").unwrap();
    Ok(re.find_iter(&file_content).count())
}

fn count_all_files(path: &Path) -> Result<i32, String> {
    let dir_iter = match fs::read_dir(path) {
        Err(why) => return Err(format!("Couldn't open directory: {}", why)),
        Ok(dir_iter) => dir_iter,
    };

    let mut file_counter = 0;

    for entry in dir_iter {
        let path = entry.unwrap().path();
        if path.is_dir() {
            match count_all_files(&path) {
                Err(why) => return Err(why),
                Ok(res) => {
                    file_counter += res;
                }
            }
        } else {
            file_counter += 1;
        }
    }

    Ok(file_counter)
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
            let error: Box<dyn Error> = String::from("Failed to set console mode").into();
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

fn build_err_no_time(err_mes: String) -> String {
    format!("\n{}{}{}", RED_COLOR, err_mes, RESET_COLOR)
}

fn build_warning(warn_mes: String) -> String {
    format!("\n{}{}{}", YELLOW_COLOR, warn_mes, RESET_COLOR)
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

enum AwaitingType {
    Progress,
    FileCounting,
}
