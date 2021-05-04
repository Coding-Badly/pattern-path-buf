use std::ffi::{OsStr, OsString};
// nfx:win use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

//type CodeUnit = u16;
type CodeUnit = u8;

enum Fragment {
    Literal(Vec<CodeUnit>),
    ReplacementMarker
}

struct Fragments {
    fragments: Vec<Fragment>,
    overhead_for_literals: usize,
    replacement_markers: usize,
}

impl Fragments {
    fn resolve(&self, replacement: &str) -> OsString {
//        let replacement: Vec<CodeUnit> = OsStr::new(replacement).encode_wide().collect();
        let replacement: &[CodeUnit] = replacement.as_bytes();
        let raw_size = self.overhead_for_literals + (self.replacement_markers * replacement.len());
        let mut raw: Vec<CodeUnit> = Vec::with_capacity(raw_size);
        for fragment in self.fragments.iter() {
            match fragment {
                Fragment::Literal(literal) => raw.extend_from_slice(&literal[..]),
                Fragment::ReplacementMarker => raw.extend_from_slice(&replacement[..]),
            }
        }
        assert!(raw.len() == raw_size);
//        OsString::from_wide(&raw[..])
        OsString::from_vec(raw)
    }
}

struct FragmentsBuilder {
    fragments: Fragments,
    pending: Option<Vec<CodeUnit>>,
}

impl FragmentsBuilder {
    fn new() -> Self {
        let fragments = Fragments {
            fragments: Vec::new(),
            overhead_for_literals: 0,
            replacement_markers: 0,
        };
        Self {
            fragments,
            pending: None,
        }
    }
    fn add_code_unit(&mut self, code_unit: CodeUnit) {
        if let Some(pending) = &mut self.pending {
            pending.push(code_unit);
        } else {
            self.pending = Some(vec![code_unit]);
        }
    }
    fn add_replacement_marker(&mut self) {
        self.push_pending();
        self.fragments.fragments.push(Fragment::ReplacementMarker);
        self.fragments.replacement_markers += 1;
    }
    fn has_replacement_marker(&self) -> bool {
        self.fragments.replacement_markers != 0
    }
    fn push_pending(&mut self) {
        if let Some(mut pending) = self.pending.take() {
            pending.shrink_to_fit();
            self.fragments.overhead_for_literals += pending.len();
            self.fragments.fragments.push(Fragment::Literal(pending));
        }
    }
}

impl Into<Fragments> for FragmentsBuilder {
    fn into(mut self) -> Fragments {
        self.push_pending();
        self.fragments
    }
}

enum Segment {
    Literal(PathBuf),
    CleanPattern(String),
    DirtyPattern(Fragments),
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
    fn add_clean_pattern(&mut self, segment: &str) {
        self.push_pending();
        self.segments.push(Segment::CleanPattern(segment.to_string()));
    }
    fn add_dirty_pattern(&mut self, segment: Fragments) {
        self.push_pending();
        self.segments.push(Segment::DirtyPattern(segment));
    }
    fn add_literal(&mut self, segment: &OsStr) {
        if let Some(pending) = &mut self.pending {
            pending.push(segment);
        } else {
            self.pending = Some(PathBuf::from(segment));
        }
    }
    fn push_pending(&mut self) {
        if let Some(pending) = self.pending.take() {
            self.segments.push(Segment::Literal(pending));
        }
    }
}

impl Into<Vec<Segment>> for SegmentsBuilder {
    fn into(mut self) -> Vec<Segment> {
        self.push_pending();
        self.segments
    }
}

pub struct PatternPathBuf {
    segments: Vec<Segment>,
}

enum ScanningState {
    HaveNothing,
    HaveLeftCurly,
}

pub const LEFT_CURLY: CodeUnit = '{' as CodeUnit;
pub const RIGHT_CURLY: CodeUnit = '}' as CodeUnit;

impl PatternPathBuf {
    pub fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let mut segments_builder = SegmentsBuilder::new();

        for segment in path.as_ref().iter() {
            if let Some(valid_utf_8) = segment.to_str() {
                if valid_utf_8.contains("{}") {
                    segments_builder.add_clean_pattern(valid_utf_8);
                } else {
                    segments_builder.add_literal(segment);
                }
            } else {
                let mut fragments_builder = FragmentsBuilder::new();
                let mut scanning_state = ScanningState::HaveNothing;

//                for code_unit in segment.encode_wide() {
                for code_unit in segment.as_bytes() {
                    match scanning_state {
                        ScanningState::HaveNothing => {
                            if *code_unit == LEFT_CURLY {
                                scanning_state = ScanningState::HaveLeftCurly;
                            }
                            else {
                                fragments_builder.add_code_unit(*code_unit);
                            }
                        },
                        ScanningState::HaveLeftCurly => {
                            if *code_unit == RIGHT_CURLY {
                                fragments_builder.add_replacement_marker();
                            }
                            else {
                                fragments_builder.add_code_unit(LEFT_CURLY);
                                fragments_builder.add_code_unit(*code_unit);
                            }
                            scanning_state = ScanningState::HaveNothing;
                        },
                    }
                }
                match scanning_state {
                    ScanningState::HaveLeftCurly =>
                        fragments_builder.add_code_unit(LEFT_CURLY),
                    _ => {},
                }
                if fragments_builder.has_replacement_marker() {
                    segments_builder.add_dirty_pattern(fragments_builder.into());
                }
                else {
                    segments_builder.add_literal(segment);
                }
            }
        }
        Self {
            segments: segments_builder.into(),
        }
    }
    pub fn resolve(&self, replacement: &str) -> PathBuf {
        let mut rv = PathBuf::new();
        for segment in self.segments.iter() {
            match segment {
                Segment::Literal(p) => rv.push(p),
                Segment::CleanPattern(s) => {
                    rv.push(s.replace("{}", replacement));
                }
                Segment::DirtyPattern(f) => {
                    rv.push(f.resolve(replacement));
                }
            }
        }
        rv
    }
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
        use std::os::unix::ffi::OsStringExt;

        fn simple_bad_string() -> OsString {
            let source = vec![0x66, 0x6f, 0x80, 0x6f];
            OsString::from_vec(source)
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
            let source = [0x0066, 0x006f, 0xD800, 0x006f, LEFT_CURLY, RIGHT_CURLY, 0x0062, 0x0061, 0x0072];
            OsString::from_wide(&source[..])
        }

        fn crazy_bad_string_with_marker() -> OsString {
            let source = [LEFT_CURLY, 0x0066, 0x006f, 0xD800, 0x006f, LEFT_CURLY, RIGHT_CURLY, 0x0062, 0x0061, 0x0072, LEFT_CURLY];
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
        fn bad_with_marker_is_ok() {
            let mut path = PathBuf::from("C:\\ProgramData\\Gremlin\\Agent");
            path.push(simple_bad_string_with_marker());
            path.push("daemon.log.{}.gz");
            let tm = PatternPathBuf::new(path);
            assert!(tm.segments.len() == 3);
            let r = tm.resolve("0");
            assert!(r.to_str().is_none());
            let s = format!("{:?}", r);
            assert!(s == "\"C:\\\\ProgramData\\\\Gremlin\\\\Agent\\\\fo\\u{d800}o0bar\\\\daemon.log.0.gz\"");
            let r = tm.resolve("99");
            assert!(r.to_str().is_none());
            let s = format!("{:?}", r);
            assert!(s == "\"C:\\\\ProgramData\\\\Gremlin\\\\Agent\\\\fo\\u{d800}o99bar\\\\daemon.log.99.gz\"");
        }

        #[test]
        fn crazy_with_marker_is_ok() {
            let mut path = PathBuf::from("C:\\ProgramData\\Gremlin\\Agent");
            path.push(crazy_bad_string_with_marker());
            path.push("daemon.log.{}.gz");
            let tm = PatternPathBuf::new(path);
            assert!(tm.segments.len() == 3);
            let r = tm.resolve("0");
            assert!(r.to_str().is_none());
            let s = format!("{:?}", r);
            assert!(s == "\"C:\\\\ProgramData\\\\Gremlin\\\\Agent\\\\{fo\\u{d800}o0bar{\\\\daemon.log.0.gz\"");
            let r = tm.resolve("whatever");
            assert!(r.to_str().is_none());
            let s = format!("{:?}", r);
            assert!(s == "\"C:\\\\ProgramData\\\\Gremlin\\\\Agent\\\\{fo\\u{d800}owhateverbar{\\\\daemon.log.whatever.gz\"");
        }
    }
}
