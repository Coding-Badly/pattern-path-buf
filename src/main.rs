use std::ffi::OsStr;
//use std::fs::remove_file;
//use std::fs::File;
//use std::io::prelude::*;
use std::path::{Path, PathBuf};

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::os::windows::ffi::OsStrExt;

enum Segment {
    Literal(PathBuf),
    Pattern(String),
}

struct SegmentsBuilder {
    segments: Vec<Segment>,
    pending: Option<PathBuf>,
}

impl SegmentsBuilder {
    fn new() -> Self {
        Self {
            segments: Vec::new(),
            pending: None,
        }
    }
    fn add_pattern(&mut self, segment: &str) {
        if let Some(pending) = self.pending.take() {
            self.segments.push(Segment::Literal(pending));
        }
        self.segments.push(Segment::Pattern(segment.to_string()));
    }
    fn add_literal(&mut self, segment: &OsStr) {
        if let Some(pending) = &mut self.pending {
            pending.push(segment);
        } else {
            self.pending = Some(PathBuf::from(segment));
        }
    }
}

impl Into<Vec<Segment>> for SegmentsBuilder {
    fn into(mut self) -> Vec<Segment> {
        if let Some(pending) = self.pending.take() {
            self.segments.push(Segment::Literal(pending));
        }
        self.segments
    }
}

struct PatternPathBuf {
    segments: Vec<Segment>,
}

impl PatternPathBuf {
    fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let mut segments_builder = SegmentsBuilder::new();

        for segment in path.as_ref().iter() {
            if let Some(valid_utf_8) = segment.to_str() {
                if valid_utf_8.contains("{}") {
                    segments_builder.add_pattern(valid_utf_8);
                } else {
                    segments_builder.add_literal(segment);
                }
            } else {
                segments_builder.add_literal(segment);
            }
        }
        Self {
            segments: segments_builder.into(),
        }
    }
    fn resolve(&self, replacement: &str) -> PathBuf {
        let mut rv = PathBuf::new();
        for segment in self.segments.iter() {
            match segment {
                Segment::Literal(p) => rv.push(p),
                Segment::Pattern(s) => {
                    rv.push(s.replace("{}", replacement));
                }
            }
        }
        rv
    }
}

enum ScanningState {
    HaveNothing(usize),
    HaveLeft(usize, usize),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!();

    // Here the values 0x0066 and 0x006f correspond to 'f' and 'o'
    // respectively. The value 0xD800 is a lone surrogate half, invalid
    // in a UTF-16 sequence.
//    let source = [0x0066, 0x006f, 0xD800, 0x006f, 0x007B, 0x007D, 0x0062, 0x0061, 0x0072];
//    let os_string = OsString::from_wide(&source[..]);

//    let os_string = OsString::from("daemon.log.{}.gz");
//    let os_string = OsString::from("daemon.log.gz");
//    let os_string = OsString::from("daemon.log{.gz");
//    let os_string = OsString::from("daemon.log.gz{");
//    let os_string = OsString::from("{daemon.log.gz");
//    let os_string = OsString::from("{}daemon.log.gz");
//    let os_string = OsString::from("daemon.log.gz{}");
//    let os_string = OsString::from("daemon.log.g{}z");
    let os_string = OsString::from("{}daemon.{}.log.{}.gz");

    const LEFT_CURLY: u16 = '{' as u16;
    const RIGHT_CURLY: u16 = '}' as u16;

    let mut scanning_state = ScanningState::HaveNothing(0);

    for (index, code_unit) in os_string.as_os_str().encode_wide().enumerate() {
        match scanning_state {
            ScanningState::HaveNothing(anchor) => {
                if code_unit == LEFT_CURLY {
                    scanning_state = ScanningState::HaveLeft(anchor, index);
                }
            },
            ScanningState::HaveLeft(left, right) => {
                if code_unit == RIGHT_CURLY {
                    if left != right {
                        println!("  ({}..{})", left, right);
                    }
                    println!("Found eye-catcher.");
                    scanning_state = ScanningState::HaveNothing(index+1);
                }
                else {
                    scanning_state = ScanningState::HaveNothing(left);
                }
            },
        }
    }

    match scanning_state {
        ScanningState::HaveNothing(anchor) => {
            // Skip if nothing is left
            println!("  ({}..)", anchor);
        },
        ScanningState::HaveLeft(left, _right) => {
            // Skip if left == _right
            println!("  ({}..)", left);
        }
    }

/*
    let path_buf = PathBuf::from(os_string);
    println!("path_buf = {:?}", path_buf);

    let path_buf = PathBuf::from("daemon.{}.log.gz");
    println!("path_buf = {:?}", path_buf);

    let mut file = File::create(&path_buf)?;
    file.write_all(b"Hello, world!")?;

    let path_buf = path_buf.canonicalize()?;
    println!("path_buf = {:?}", path_buf);
    // "\\\\?\\C:\\Users\\brian\\Documents\\junk\\fo\u{d800}o"

    remove_file(&path_buf)?;

    let mut pb2 = PathBuf::new();
    for rover in path_buf.iter() {
        // rover is &OsStr
        println!("{:?}", rover);
        if let Some(as_str) = rover.to_str() {
            println!("  can substitute");
            if as_str.contains("{}") {
                println!("  must substitute");
            }
        } else {
            println!("  tough luck");
        }
        pb2.push(rover);
    }
    println!("pb2      = {:?}", pb2);

    /*
    "\\\\?\\C:"
    "\\"
    "Users"
    "brian"
    "Documents"
    "junk"
    "fo\u{d800}o"
    */
*/
    Ok(())
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    mod for_both {
        use super::*;
        use std::path::MAIN_SEPARATOR;

        fn simple<S: ToString>(segment: S) {
            let path = segment.to_string();
            let tm = PatternPathBuf::new(&path);
            assert!(tm.segments.len() == 1);
            let r = tm.resolve("1");
            assert!(r.to_str() == Some(&path));
        }

        fn something_dir<S: ToString>(segment: S) {
            let mut path = segment.to_string();
            path.push_str("tmp");
            let tm = PatternPathBuf::new(&path);
            assert!(tm.segments.len() == 1);
            let r = tm.resolve("1");
            assert!(r.to_str() == Some(&path));
        }

        #[test]
        fn empty_is_ok() {
            let tm = PatternPathBuf::new("");
            assert!(tm.segments.len() == 0);
            let r = tm.resolve("1");
            assert!(r.to_str() == Some(""));
        }

        #[test]
        fn root_is_ok() {
            simple(MAIN_SEPARATOR);
        }

        #[test]
        fn current_is_ok() {
            simple(".");
        }

        #[test]
        fn simple_is_ok() {
            let tm = PatternPathBuf::new("tmp");
            assert!(tm.segments.len() == 1);
            let r = tm.resolve("1");
            assert!(r.to_str() == Some("tmp"));
        }

        #[test]
        fn root_dir_is_ok() {
            something_dir(MAIN_SEPARATOR);
        }

        #[test]
        fn current_dir_is_ok() {
            something_dir(".");
        }

        #[test]
        fn just_replacement_is_ok() {
            let tm = PatternPathBuf::new("{}");
            assert!(tm.segments.len() == 1);
            let r = tm.resolve("1");
            assert!(r.to_str() == Some("1"));
            let r = tm.resolve("9");
            assert!(r.to_str() == Some("9"));
            let r = tm.resolve("what ever");
            assert!(r.to_str() == Some("what ever"));
        }

        #[test]
        fn double_replacement_is_ok() {
            let tm = PatternPathBuf::new("{}{}");
            assert!(tm.segments.len() == 1);
            let r = tm.resolve("1");
            assert!(r.to_str() == Some("11"));
            let r = tm.resolve("first second third");
            assert!(r.to_str() == Some("first second thirdfirst second third"));
        }
    }

    #[cfg(unix)]
    mod for_unix {
        use super::*;
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStrExt;

        fn simple_bad_string() -> OsString {
            let source = [0x66, 0x6f, 0x80, 0x6f];
            OsString::from_bytes(&source[..])
        }

        #[test]
        fn full_example_1_is_ok() {
            let tm = PatternPathBuf::new("/var/log/gremlin/daemon.log.{}.gz");
            assert!(tm.segments.len() == 2);
            let r = tm.resolve("0");
            assert!(r.to_str() == Some("/var/log/gremlin/daemon.log.0.gz"));
        }
    }

    #[cfg(windows)]
    mod for_windows {
        use super::*;
        use std::ffi::OsString;
        use std::os::windows::ffi::OsStringExt;

        fn simple_bad_string() -> OsString {
            let source = [0x0066, 0x006f, 0xD800, 0x006f];
            OsString::from_wide(&source[..])
        }

        fn simple_bad_string_with_marker() -> OsString {
            let source = [0x0066, 0x006f, 0xD800, 0x006f, 0x007B, 0x007D, 0x0062, 0x0061, 0x0072];
            OsString::from_wide(&source[..])
        }

        #[test]
        fn full_example_1_is_ok() {
            let tm = PatternPathBuf::new("C:\\ProgramData\\Gremlin\\Agent\\daemon.log.{}.gz");
            assert!(tm.segments.len() == 2);
            let r = tm.resolve("0");
            assert!(r.to_str() == Some("C:\\ProgramData\\Gremlin\\Agent\\daemon.log.0.gz"));
        }

        #[test]
        fn mix_is_ok() {
            let tm = PatternPathBuf::new(
                "C:\\ProgramData\\Gremlin\\Agent{}\\Middle{}Insert\\daemon.log.{}.gz\\pointless\\tail");
            assert!(tm.segments.len() == 5);
            let r = tm.resolve("0");
            assert!(r.to_str() == Some(
                "C:\\ProgramData\\Gremlin\\Agent0\\Middle0Insert\\daemon.log.0.gz\\pointless\\tail"));
        }

        #[test]
        fn simple_bad_is_ok() {
            let tm = PatternPathBuf::new(simple_bad_string());
            assert!(tm.segments.len() == 1);
            let r = tm.resolve("0");
            assert!(r.to_str().is_none());
            assert!(r.to_string_lossy() == simple_bad_string().to_string_lossy());
        }

        #[test]
        fn bad_with_marker_is_literal() {
            let mut path = PathBuf::from("C:\\ProgramData\\Gremlin\\Agent");
            path.push(simple_bad_string_with_marker());
            path.push("daemon.log.{}.gz");
            let tm = PatternPathBuf::new(path);
            assert!(tm.segments.len() == 2);
            let r = tm.resolve("0");
            assert!(r.to_str().is_none());
            let s = format!("{:?}", r);
            assert!(s == "\"C:\\\\ProgramData\\\\Gremlin\\\\Agent\\\\fo\\u{d800}o{}bar\\\\daemon.log.0.gz\"");
            let r = tm.resolve("99");
            assert!(r.to_str().is_none());
            let s = format!("{:?}", r);
            assert!(s == "\"C:\\\\ProgramData\\\\Gremlin\\\\Agent\\\\fo\\u{d800}o{}bar\\\\daemon.log.99.gz\"");
        }
    }
}
