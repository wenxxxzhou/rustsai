# rustsai

> Most of the README's text was translated by [gemini-3-flash-preview](https://blog.google/products/gemini/gemini-3-flash/ "blog.google").

## fast_dedup

```shell
/*
    ============================================================
    Parameter Explanations and Usage Examples
    ============================================================

    1. -DirectoryPath (Required): The root directory path to scan.
       Example: --directory-path "D:\Data"

    2. -MatchMode (Optional): The duplication detection mode.
       - 'Name': Groups files by base name (excluding extension). Faster performance.
       - 'Hash' (Default): Groups files by calculating hash values. High accuracy but slower.
       Example: --match-mode hash OR --match-mode name

    3. -Algorithm (Optional): The hashing algorithm used in 'Hash' mode.
       - 'MD5' (Default): Fastest performance.
       - 'SHA256': Highest collision resistance/security.
       - 'SHA1': Balanced/Moderate.
       Example: --algorithm SHA256

    4. -Recurse (Optional): Switch parameter. If enabled, scans all subdirectories recursively.
       Example: --no-recurse (Disable) OR --recurse (Enable, default)

    5. -IncludeExtensions (Optional): String array. Only scans files with specified extensions.
       Example: --include-extensions "jpg;png;gif"

    6. -ExcludeExtensions (Optional): String array. Excludes files with specified extensions.
       Example: --exclude-extensions "tmp;log;bak"

    7. -MoveDuplicates (Optional): Switch parameter. If enabled, moves duplicate files to an 
       archive folder. Requires user confirmation before moving.
       Example: --move-duplicates

    8. -ReportDir (Optional): Specifies the directory for the report file. 
       Defaults to the root of the scanned directory if not specified.
       Example: --report-dir "D:\Data\Reports"

    9. -ReportName (Optional): Specifies the filename of the report.
       Example: --report-name "ScanResults.txt"

    10. -Threads (Optional): Limits the number of threads for parallel computation. 
        Defaults to 0 (auto-detects all available logical cores).
        Note: For HDDs (Mechanical Drives), it is recommended to limit this to 4 or 8 
        to avoid Disk I/O bottlenecks.
        Example: --threads 4

    11. -MinSize (Optional): Minimum file size filter. Default is 0 (no filter).
        Example: --min-size "100KB" OR --min-size "1MB"

    12. -MaxSize (Optional): Maximum file size filter. Default is an empty string (no upper limit).
        If left blank, large files will not be filtered.
        Example: --max-size "500MB" OR --max-size "1GB"
*/
```
