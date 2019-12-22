# Dupes
Find duplicate files by SHA256.

## Dependencies
This program depends on crates: clap, regex, sha2, walkdir, libc, serde, serde\_json

## Usage
```
Dupes 0.1.1
fnordomat <GPG:46D46D1246803312401472B5A7427E237B7908CA>
Finds duplicate files (according to SHA256)

USAGE:
    dupes [FLAGS] [OPTIONS]

FLAGS:
    -A, --always-hash            Always include the hash, even if there is only one file of that size (implies -a 0)
    -j, --emit-json              Output in JSON format
    -h, --help                   Prints help information
    -S, --show-non-duplicates    List also files that are unique
    -V, --version                Prints version information

OPTIONS:
    -D, --anti_dir <anti_dir>...
            NEGATIVE directory (multiple instances possible) - don't list files that are present in one of the -D
            entries. Use this to find the difference between two sets of files (implies -A and -S)
    -a, --avoid-compare-if-larger <avoid_compare_if_larger_than>
            Compare files of size >= X by size only. Default 32 MiB. use -a 0 for unlimited

    -d, --dir <dir>...                                              Base directory (multiple instances possible)
    -e, --exclude-path <exclude_path>...
            Exclude part of path (glob); valid for both -d and -D

    -i, --ignore-smaller-than <ignore_smaller_than>
            Ignore all files smaller than given size (bytes). Default 0
```
