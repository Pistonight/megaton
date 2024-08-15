use std::io::{BufRead, Lines};
use std::path::Path;
use std::{fs::File, io::BufReader};

use filetime::FileTime;

use crate::system::{self, Error};

// (very strong) assumptions of the depfiles:
// - the first rule is what we care about (the target)
// - the first line is just the target

pub fn are_deps_up_to_date(d_path: &Path, o_mtime: FileTime) -> Result<bool, Error> {
    if !d_path.exists() {
        return Ok(false);
    }
    let lines = BufReader::new(system::open(d_path)?).lines();
    for line in lines.skip(1) {
        // skip the <target>: \ line
        let line = match line {
            Ok(x) => x,
            Err(_) => return Ok(false),
        };
        let part = line.trim().trim_end_matches('\\').trim_end();
        if part.ends_with(':') {
            break;
        }
        let d_mtime = match system::get_modified_time(part) {
            Ok(x) => x,
            Err(_) => return Ok(false),
        };
        if d_mtime > o_mtime {
            return Ok(false);
        }
    }
    Ok(true)
}
