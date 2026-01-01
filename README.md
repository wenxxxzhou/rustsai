# rustsai

> I’m thrilled to have put together some scripts using AI. My sincere thanks go out to those pushing the boundaries of science, and of course, to the "machines" themselves.
> Most of the README's text was translated by [gemini-3-flash-preview](https://blog.google/products/gemini/gemini-3-flash/ "blog.google").

___

## *AIntelligence*

oneAPI

- <https://openrouter.ai/>
- <https://aihubmix.com/>
- <https://siliconflow.cn/>
- <https://chatanywhere.apifox.cn/>
  - <https://api.chatanywhere.tech/#/shop/>
  - <https://github.com/chatanywhere/GPT_API_free>
  - <https://gitee.com/chatanywhere/GPT_API_free>

LEADERBOARDs

- <https://artificialanalysis.ai/>
- <https://lmarena.ai/zh/leaderboard>
- <https://agentset.ai/>
- <https://openlm.ai/leaderboard/>

KNOWLEDGEs

- <https://huggingface.co/>
- <https://www2.statmt.org/wmt25/>
- <https://lmsys.org/blog/>

SKILLx

- <https://matrix.tencent.com/ai-detect/>
- <https://smithery.ai/>
- <https://infographic.antv.vision/>
- <https://browserfly.app/>

___

## prop

### fast_dedup

```shell
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
```

### edl2srt

```shell
 ============================================================================
 参数说明与使用示例
 ============================================================================

 程序用途：
   将 EDL 文件转换为 SRT 字幕文件，并允许自定义输出路径。

 命令行参数：
   参数 1 (必需): 帧率 (FPS)
     - 类型: 数字 (整数或小数)
     - 说明: EDL 时间码与实际时间的转换比率。
     - 示例: 24, 25, 30, 23.976, 29.97, 59.94, 120 等。

   参数 2 (必需): 输入文件路径
     - 类型: 文件路径
     - 说明: 源 EDL 文件的位置。

   参数 3 (必需): 输出文件路径
     - 类型: 文件路径
     - 说明: 生成的 SRT 文件的保存位置。
     - 支持相对路径 (如 ./out.srt) 和绝对路径 (如 C:\subs\out.srt)。
     - 如果文件已存在，程序将覆盖它。

 使用示例：

   1. 基本用法 (当前目录):
      $ ./edl_to_srt 24 input.edl output.srt

   2. 指定子目录 (Linux/macOS):
      $ ./edl_to_srt 29.97 project/main.edl subtitles/final_sub.srt

   3. 指定绝对路径 (Windows):
      > edl_to_srt.exe 25 C:\Videos\edit.edl C:\Subtitles\export.srt

   4. 使用高帧率及自定义文件名:
      $ ./edl_to_srt 59.94 raw_data.edl high_fps_output.srt

 注意事项：
   - 程序不自动推断输出文件名，必须显式指定第3个参数。
   - 请确保输出路径的目录具有写入权限。
 ============================================================================
```
