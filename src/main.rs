#![feature(bufreader_seek_relative)]
#![allow(non_snake_case)]

extern crate anyhow;
extern crate clap;
extern crate libc;
extern crate regex;
extern crate serde;
extern crate sha2;
extern crate walkdir;

use clap::{App, Arg};
use regex::Regex;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader};
use std::path::Path;
use walkdir::{Result, WalkDir};

fn collect<P: AsRef<Path> + std::cmp::Eq + std::hash::Hash>(
    directories: Vec<P>,
    exclude_path_regex: &Option<Regex>,
) -> BTreeMap<u64, BTreeSet<std::path::PathBuf>> {
    let mut size_map: BTreeMap<u64, BTreeSet<std::path::PathBuf>> = BTreeMap::new();

    let isMatchOK = |s: &str| match &exclude_path_regex {
        Some(ex) => !ex.is_match(&s),
        None => true,
    };

    for path in directories {
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| {
                (!e.file_type().is_dir())
                // Do not descend into paths excluded by -e patterns
                || (e.path().to_str().map(&isMatchOK).unwrap_or(false))
            })
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            // Exclude files whose pathname is matched by -e pattern
            .filter(|e| e.path().to_str().map(&isMatchOK).unwrap_or(false))
        {
            match entry.metadata() {
                Ok(m) => {
                    // no size filter here, just want to exclude files like these
                    size_map
                        .entry(m.len())
                        .or_insert_with(BTreeSet::new)
                        .insert(entry.into_path());
                }
                Err(_e) => {}
            }
        }
    }

    size_map
}

fn hashvec_from_reader<'a>(reader: &'a mut dyn BufRead) -> std::result::Result<Vec<u8>, std::io::Error> {
    let mut hasher = Sha256::new();

    'reading: loop {
        let consumed = match reader.fill_buf() {
            Ok(bytes) => {
                hasher.update(bytes);
                bytes.len()
            }
            Err(error) => {
                return Err(error);
            }
        };
        reader.consume(consumed);
        if consumed == 0 {
            break 'reading;
        }
    }

    Ok(hasher.finalize().to_vec())
}

fn walk<P: AsRef<Path> + std::cmp::Eq + std::hash::Hash>(
    directories: Vec<P>,
    emit_json: bool,
    avoid_compare_if_larger_than: Option<u64>,
    ignore_sizes_below: Option<u64>,
    show_non_duplicates: bool,
    always_hash: bool,
    exclude_path_regex: &Option<Regex>,
    exclude_sizemap: BTreeMap<u64, BTreeSet<std::path::PathBuf>>,
) -> std::result::Result<(), anyhow::Error> {
    let mut size_map: BTreeMap<u64, BTreeSet<_>> = BTreeMap::new();

    let isMatchOK = |s: &str| match &exclude_path_regex {
        Some(ex) => !ex.is_match(&s),
        None => true,
    };

    let mut output_table: Vec<(
        std::string::String,
        u64,
        std::collections::BTreeSet<&std::path::PathBuf>,
    )> = vec![];
    let quiet = emit_json;

    for path in directories {
        'walking: for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| {
                (!e.file_type().is_dir())
                // Do not descend into paths excluded by -e patterns
                || (e.path().to_str().map(&isMatchOK).unwrap_or(false))
            })
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
            // Exclude files whose pathname is matched by -e pattern
            .filter(|e| e.path().to_str().map(&isMatchOK).unwrap_or(false))
        {
            match entry.metadata() {
                Ok(m) => {
                    let size = m.len();
                    if let Some(s) = ignore_sizes_below {
                        if size < s {
                            continue 'walking;
                        }
                    }
                    size_map
                        .entry(m.len())
                        .or_insert_with(BTreeSet::new)
                        .insert(entry.into_path());
                }
                Err(_e) => {}
            }
        }
    }

    'iterating: for (size, set) in size_map.iter() {
        if let Some(max_size) = &avoid_compare_if_larger_than {
            if size > max_size {
                if !quiet {
                    println!("{:} (avoiding disambiguation)", size);
                }
                for entry in set {
                    if !quiet {
                        println!("  {:}", entry.display());
                    }
                }
                continue;
            }
        }

        let mut size_header_shown = false;

        if set.len() == 1 {
            if show_non_duplicates && !always_hash {
                if !size_header_shown {
                    if !quiet {
                        println!("{:}", size);
                    }
                }
                for entry in set {
                    if !quiet {
                        println!("  {:}", entry.display());
                    }
                }

                if emit_json {
                    let mut bin = BTreeSet::new();
                    bin.insert(set.iter().next().unwrap());
                    output_table.push(("".to_string(), *size, bin));
                }
            }
            if !always_hash {
                continue 'iterating;
            }
        }

        let mut hashbins = BTreeMap::new();
        for entry in set {
            let maybe_f = std::fs::File::open(entry);

            let potentially_excluded_file_set = exclude_sizemap.get(size);

            // hash excluded files (from -D) only when and if the size comes up
            let excluded_hash_set : BTreeSet<Vec<u8>> =
                potentially_excluded_file_set.map(|fs| fs.into_iter().map(|f| {
                    let maybe_f = std::fs::File::open(f);
                    match maybe_f {
                        Ok(f) => {
                            let mut excluded_file_reader = BufReader::with_capacity(8192, f);
                            hashvec_from_reader(&mut excluded_file_reader).map_err(
                                |error| {
                                    if !quiet {
                                        println!("{:?} error reading file {:}", error.kind(), entry.display());
                                    }
                                    error
                                }
                            )
                        }
                        Err(error) => Err(error),
                    }
                }))
                    .map( |x| x.filter_map(std::result::Result::ok).collect() )
                    .unwrap_or( BTreeSet::new() );

            if let Ok(f) = maybe_f {
                let mut reader = BufReader::with_capacity(8192, f);

                let hashvec = hashvec_from_reader(&mut reader).map_err(
                    |error| {
                        if !size_header_shown {
                            if !quiet {
                                println!("{:}", size);
                            }
                            size_header_shown = true;
                        }
                        if !quiet {
                            println!("{:?} error reading file {:}", error.kind(), entry.display());
                        }
                        error
                    }
                )?;

                // Exclude files present in excluded-files map at this stage:
                let present_in_excluded = excluded_hash_set.contains(&hashvec.as_slice().to_vec());

                if !present_in_excluded {
                    hashbins
                        .entry(hashvec)
                        .or_insert_with(BTreeSet::new)
                        .insert(entry);
                } else {
                    // don't include it then.
                }
            }
        }
        for (key, bin) in &hashbins {
            if !show_non_duplicates && bin.len() == 1 {
                continue;
            }
            if !size_header_shown {
                if !quiet {
                    println!("{:}", size);
                }
                size_header_shown = true;
            }
            if !quiet {
                println!("  {}", toUpperHex(&key));
                for entry in bin {
                    println!("    {:}", entry.display());
                }
            }

            if emit_json {
                output_table.push((toUpperHex(&key), *size, bin.clone()));
            }
        }
    }

    if emit_json {
        let json = serde_json::to_string(&output_table)?;
        println!("{}", json);
    }

    Ok(())
}

fn toUpperHex(vec : &[u8]) -> String {
    vec.iter().map(|x| format!("{:X}", x)).collect::<String>()
}

fn main() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let matches = App::new("Dupes")
        .version("0.2.0")
        .author("fnordomat <GPG:46D46D1246803312401472B5A7427E237B7908CA>")
        .about("Finds duplicate files (according to SHA256)")
        .arg(Arg::with_name("dir")
             .short("d")
             .long("dir")
             .takes_value(true)
             .multiple(true)
             .help("Base directory (multiple instances possible)"))
        .arg(Arg::with_name("anti_dir")
             .short("D")
             .long("anti_dir")
             .takes_value(true)
             .multiple(true)
             .help("NEGATIVE directory (multiple instances possible) - don't list files that are present in one of the -D entries. Use this to find the difference between two sets of files (implies -A and -S)"))
        .arg(Arg::with_name("show_non_duplicates")
             .short("S")
             .long("show-non-duplicates")
             .help("List also files that are unique (automatically true if -D is used)"))
        .arg(Arg::with_name("always_hash")
             .short("A")
             .long("always-hash")
             .help("Always include the hash, even if there is only one file of that size (implies -a 0)"))
        .arg(Arg::with_name("ignore_smaller_than")
             .short("i")
             .long("ignore-smaller-than")
             .takes_value(true)
             .help("Ignore all files smaller than given size (bytes). Default 0"))
        .arg(Arg::with_name("avoid_compare_if_larger_than")
             .short("a")
             .long("avoid-compare-if-larger")
             .takes_value(true)
             .help("Compare files of size >= X by size only. Default 32 MiB. use -a 0 for unlimited"))
        .arg(Arg::with_name("exclude_path")
             .short("e")
             .long("exclude-path")
             .takes_value(true)
             .multiple(true)
             .help("Exclude part of path (glob); applies to both -d and -D"))
        .arg(Arg::with_name("emit_json")
             .short("j")
             .long("emit-json")
             .help("Output in JSON format"))
        .get_matches();

    fn parseBytesNum(string: &str) -> Option<u64> {
        let parseBytesRegex = Regex::new("([0-9]*)([kMGT]?)").ok().unwrap();
        let caps = parseBytesRegex.captures(string)?;
        let factor = match &caps[2] {
            "k" => 1024,
            "M" => 1024 * 1024,
            "G" => 1024 * 1024 * 1024,
            "T" => 1024 * 1024 * 1024 * 1024,
            _ => 1,
        };
        Some((&caps[1]).parse::<u64>().unwrap() * factor)
    };

    let mydirs: Vec<&str> = matches
        .values_of("dir")
        .map_or(["."].to_vec(), |x| x.collect());
    let myantidirs: Vec<&str> = matches
        .values_of("anti_dir")
        .map_or([].to_vec(), |x| x.collect());
    let ignore_sizes_below = matches
        .value_of("ignore_smaller_than")
        .and_then(|x| parseBytesNum(x));
    let exclude_exprs: Vec<&str> = matches
        .values_of("exclude_path")
        .map_or([].to_vec(), |x| x.collect());

    let show_non_duplicates =
        matches.occurrences_of("show_non_duplicates") > 0 || !myantidirs.is_empty();
    let always_hash = matches.occurrences_of("always_hash") > 0 || !myantidirs.is_empty();
    let emit_json = matches.occurrences_of("emit_json") > 0;

    let exclude_path_regex = if exclude_exprs.is_empty() {
        None
    } else {
        Some(Regex::new(&exclude_exprs.join("|")).unwrap())
    };

    let mut avoid_compare_if_larger_than: Option<u64> = matches
        .value_of("avoid_compare_if_larger_than")
        .map_or(Some(1024 * 1024 * 32), |x| parseBytesNum(x));

    if avoid_compare_if_larger_than == Some(0) || always_hash {
        avoid_compare_if_larger_than = None
    }

    // let quiet = emit_json; // may become an independent option in the future

    let excluded_sizemap: BTreeMap<u64, BTreeSet<std::path::PathBuf>> =
        collect(myantidirs, &exclude_path_regex);

    let _ = walk(
        mydirs,
        emit_json,
        avoid_compare_if_larger_than,
        ignore_sizes_below,
        show_non_duplicates,
        always_hash,
        &exclude_path_regex,
        excluded_sizemap,
    );
}
