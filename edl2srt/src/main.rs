/// z-ai/glm-4.7
/// wow~ ⊙o⊙
use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

fn main() {
    // 1. 获取并解析命令行参数
    let args: Vec<String> = env::args().collect();

    // 检查参数数量，现在需要: 程序名、帧率、输入文件、输出文件
    if args.len() < 4 {
        eprintln!("错误: 参数不足。");
        eprintln!("用法: {} <帧率> <输入文件.edl> <输出文件.srt>", args[0]);
        eprintln!("示例: {} 29.97 input.edl subtitles/output.srt", args[0]);
        return;
    }

    // 解析帧率 (支持任意数字，包括小数)
    let fps: f64 = match args[1].parse() {
        Ok(f) => {
            if f <= 0.0 {
                eprintln!("错误: 帧率必须大于 0。");
                return;
            }
            f
        }
        Err(_) => {
            eprintln!(
                "错误: 无效的帧率格式 '{}'。请输入数字（例如 24 或 23.976）。",
                args[1]
            );
            return;
        }
    };

    let input_path = &args[2];
    let output_path = Path::new(&args[3]);

    // 2. 打开并读取文件
    let file = match File::open(input_path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("无法打开输入文件 {}: {}", input_path, e);
            return;
        }
    };

    let reader = BufReader::new(file);
    let mut srt_entries: Vec<(String, String, String)> = Vec::new(); // (开始, 结束, 文本)
    let mut current_start: Option<String> = None;
    let mut current_end: Option<String> = None;

    // 3. 解析 EDL 内容
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("读取行时出错: {}", e);
                continue;
            }
        };

        let trimmed = line.trim();

        // 匹配时间码行 (以数字开头)
        if trimmed
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();

            // 确保字段数量足够
            if parts.len() >= 2 {
                // 倒数第二个是开始时间，最后一个是结束时间
                let end_time = parts[parts.len() - 1].to_string();
                let start_time = parts[parts.len() - 2].to_string();

                current_start = Some(start_time);
                current_end = Some(end_time);
            }
        }
        // 匹配文件名行
        else if trimmed.starts_with("* FROM CLIP NAME:") {
            if let (Some(start), Some(end)) = (&current_start, &current_end) {
                let raw_name = trimmed.trim_start_matches("* FROM CLIP NAME:");
                // 去除音频后缀
                let clean_name = strip_audio_extension(raw_name.trim());

                srt_entries.push((start.clone(), end.clone(), clean_name));

                current_start = None;
                current_end = None;
            }
        }
    }

    // 4. 写入 SRT 文件到指定位置
    match write_srt(&output_path, &srt_entries, fps) {
        Ok(_) => println!("转换成功! 输出文件: {}", output_path.display()),
        Err(e) => eprintln!("写入输出文件时出错: {}", e),
    }
}

/// 将 EDL 时间码 (HH:MM:SS:FF) 转换为 SRT 时间码 (HH:MM:SS,mmm)
fn convert_timecode(edl_time: &str, fps: f64) -> String {
    let parts: Vec<&str> = edl_time.split(':').collect();

    if parts.len() != 4 {
        return edl_time.to_string();
    }

    let hours: u32 = parts[0].parse().unwrap_or(0);
    let minutes: u32 = parts[1].parse().unwrap_or(0);
    let seconds: u32 = parts[2].parse().unwrap_or(0);
    let frames: u32 = parts[3].parse().unwrap_or(0);

    // 计算毫秒
    let ms_from_frames = ((frames as f64) / fps) * 1000.0;
    let total_ms =
        (hours * 3600000) + (minutes * 60000) + (seconds * 1000) + (ms_from_frames.round() as u32);

    let h = total_ms / 3600000;
    let remainder = total_ms % 3600000;
    let m = remainder / 60000;
    let remainder = remainder % 60000;
    let s = remainder / 1000;
    let ms = remainder % 1000;

    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

/// 去除文件名中的常见音频后缀
fn strip_audio_extension(filename: &str) -> String {
    let extensions = [
        ".flac", ".wav", ".mp3", ".aac", ".ogg", ".wma", ".m4a", ".aiff", ".aif",
    ];

    let lower_name = filename.to_lowercase();

    for ext in &extensions {
        if lower_name.ends_with(ext) {
            return filename[..filename.len() - ext.len()].to_string();
        }
    }

    filename.to_string()
}

/// 写入 SRT 文件
fn write_srt(path: &Path, entries: &[(String, String, String)], fps: f64) -> std::io::Result<()> {
    let mut output_file = File::create(path)?;

    for (index, (start, end, text)) in entries.iter().enumerate() {
        let srt_index = index + 1;
        let srt_start = convert_timecode(start, fps);
        let srt_end = convert_timecode(end, fps);

        writeln!(output_file, "{}", srt_index)?;
        writeln!(output_file, "{} --> {}", srt_start, srt_end)?;
        writeln!(output_file, "{}", text)?;
        writeln!(output_file)?;
    }

    Ok(())
}

/*
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
*/
