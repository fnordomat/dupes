# Dupes
Find duplicate files by SHA256.

## Dependencies
This program depends on crates: clap, regex, sha2, walkdir

## Usage
```
Dupes 0.1.0
fnordomat <GPG:46D46D1246803312401472B5A7427E237B7908CA>
Finds duplicate files (according to SHA256)

USAGE:
    dupes [FLAGS] [OPTIONS]

FLAGS:
    -h, --help                   Prints help information
    -S, --show-non-duplicates    List also files that are unique
    -V, --version                Prints version information

OPTIONS:
    -a, --avoid-compare-if-larger <avoid_compare_if_larger_than>
            Compare files of size >= X by size only. Default 32 MiB. use -a 0 for unlimited

    -d, --dir <dir>...                                              Base directory
    -e, --exclude-path <exclude_path>...                            Exclude part of path (glob)                                                                                              
    -i, --ignore-smaller-than <ignore_smaller_than>
            Ignore all files smaller than given size (bytes). Default 0
```
