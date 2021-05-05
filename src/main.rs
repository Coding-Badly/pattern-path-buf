use std::io::Write;
use std::path::PathBuf;

use pattern_path_buf::{PatternPathBuf};

#[cfg(unix)]
mod os {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    use pattern_path_buf::{LEFT_CURLY, RIGHT_CURLY};

    pub fn crazy_bad_string_with_marker() -> OsString {
        let source = vec![LEFT_CURLY, 0x66, 0x6f, 0x80, 0x6f, LEFT_CURLY, RIGHT_CURLY, 0x62, 0x61, 0x72, LEFT_CURLY];
        OsString::from_vec(source)
    }
}

#[cfg(windows)]
mod os {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use pattern_path_buf::{LEFT_CURLY, RIGHT_CURLY};

    pub fn crazy_bad_string_with_marker() -> OsString {
        let source = [LEFT_CURLY, 0x0066, 0x006f, 0xD800, 0x006f, LEFT_CURLY, RIGHT_CURLY, 0x0062, 0x0061, 0x0072, LEFT_CURLY];
        OsString::from_wide(&source[..])
    }
}

fn exercise_pattern_path_buf(ppb: &PatternPathBuf) -> Result<(), Box<dyn std::error::Error>> {
    for index in 0..10 {
        let replacement = index.to_string();
        let r = ppb.resolve(&replacement);
        if let Some(parent) = r.parent() {
            println!("Create the {} directory.", parent.display());
            std::fs::create_dir_all(parent)?;
        }
        println!("Create the {} file.", r.display());
        let mut file = std::fs::File::create(&r)?;
        println!("Write some stuff to the {} file.", r.display());
        file.write_all(b"Hello, world!")?;
        println!("Delete the {} file.", r.display());
        std::fs::remove_file(&r)?;
    }
    Ok(())
}

fn use_thread(ppb: PatternPathBuf) -> std::thread::JoinHandle<Result<(), std::io::Error>> {
    let range = 10..12;
    std::thread::spawn(move || {
        for index in range {
            let replacement = index.to_string();
            let r = ppb.resolve(&replacement);
            if let Some(parent) = r.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut file = std::fs::File::create(&r)?;
            file.write_all(b"Hello world from a thread!")?;
            std::fs::remove_file(&r)?;
        }
        Ok(())
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();

    let ppb = PatternPathBuf::new("./tmp/daemon.{}.log.gz");
    let handle = use_thread(ppb.clone());
    exercise_pattern_path_buf(&ppb)?;

    let mut pb = PathBuf::new();
    pb.push(".");
    pb.push("tmp");
    pb.push(os::crazy_bad_string_with_marker());
    pb.push("daemon.{}.log.gz");
    let ppb = PatternPathBuf::new(pb);
    exercise_pattern_path_buf(&ppb)?;

    handle.join().unwrap()?;

    Ok(())
}
