/// @zai-org/GLM-5.2
/// wow~ ⊙o⊙
use chardetng::EncodingDetector;
use chrono::Local;
use encoding_rs::Encoding;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// 配置结构体：保存命令行参数解析后的结果。
struct Config {
    /// 帧率，例如 24、25、29.97
    fps: f64,

    /// 输入 EDL 文件路径
    input_path: PathBuf,

    /// 用户期望的输出路径
    ///
    /// 注意：
    /// 这只是“用户输入的目标路径”，
    /// 如果该文件已存在，程序可能会自动改名后再输出。
    output_path: PathBuf,

    /// 可选的输入编码。
    /// - Some("shift_jis") 表示用户明确指定编码
    /// - None 表示程序自动检测
    input_encoding: Option<String>,
}

/// 解码后的结果。
struct DecodeResult {
    /// 解码后的文本内容
    content: String,

    /// 本次使用的编码名称，方便打印提示信息
    encoding_name: String,

    /// 解码时是否遇到非法字节
    had_errors: bool,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // 只传 -h / --help 时，直接显示帮助
    if args.len() == 2 && (args[1] == "-h" || args[1] == "--help") {
        print_help(&args[0]);
        return;
    }

    let config = match parse_args(&args) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("错误: {}", err);
            eprintln!();
            print_help(&args[0]);
            return;
        }
    };

    // 先读取原始字节，而不是直接按 UTF-8 文本去读
    // 这是为了兼容多种可能的 EDL 编码。
    let bytes = match fs::read(&config.input_path) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("无法读取输入文件 {}: {}", config.input_path.display(), e);
            return;
        }
    };

    // 解码输入文件：
    // - 已知编码时优先按指定编码解码
    // - 否则先看 BOM，再自动检测
    let decode_result = match decode_edl_bytes(&bytes, config.input_encoding.as_deref()) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("解码输入文件失败: {}", err);
            return;
        }
    };

    // 把本次解码策略告诉用户，便于排查问题
    if let Some(user_encoding) = &config.input_encoding {
        if decode_result.had_errors {
            eprintln!(
                "已使用指定编码: {}，但检测到部分非法字节，已使用替代字符继续处理。",
                user_encoding
            );
        } else {
            eprintln!("已使用指定编码: {}", user_encoding);
        }
    } else if decode_result.had_errors {
        eprintln!(
            "未指定编码，自动检测结果: {}，但检测到部分非法字节，已使用替代字符继续处理。",
            decode_result.encoding_name
        );
    } else {
        eprintln!("未指定编码，自动检测结果: {}", decode_result.encoding_name);
    }

    // 解析 EDL 文本，生成字幕条目
    let srt_entries = parse_edl_to_entries(&decode_result.content);

    // 如果用户指定的输出文件已存在，
    // 则自动在文件名后追加时间戳，避免覆盖旧文件。
    let final_output_path = resolve_output_path(&config.output_path);

    // 如果最终输出路径和用户原始输入路径不同，说明发生了自动重命名
    if final_output_path != config.output_path {
        eprintln!(
            "输出文件已存在，已自动改名为: {}",
            final_output_path.display()
        );
    }

    match write_srt(&final_output_path, &srt_entries, config.fps) {
        Ok(_) => println!("转换成功! 输出文件: {}", final_output_path.display()),
        Err(e) => eprintln!("写入输出文件时出错: {}", e),
    }
}

/// 解析命令行参数。
///
/// 支持：
/// 1. edl2srt <帧率> <输入.edl> <输出.srt>
/// 2. edl2srt <帧率> <输入.edl> <输出.srt> --input-encoding <编码名>
fn parse_args(args: &[String]) -> Result<Config, String> {
    if args.len() < 4 {
        return Err("参数不足。".to_string());
    }

    let fps: f64 = args[1].parse().map_err(|_| {
        format!(
            "无效的帧率格式 '{}'。请输入数字（例如 24 或 23.976）。",
            args[1]
        )
    })?;

    if fps <= 0.0 {
        return Err("帧率必须大于 0。".to_string());
    }

    let input_path = PathBuf::from(&args[2]);
    let output_path = PathBuf::from(&args[3]);

    let mut input_encoding: Option<String> = None;

    let mut i = 4;
    while i < args.len() {
        match args[i].as_str() {
            "--input-encoding" => {
                if i + 1 >= args.len() {
                    return Err("参数 --input-encoding 缺少编码名。".to_string());
                }
                input_encoding = Some(args[i + 1].clone());
                i += 2;
            }
            "-h" | "--help" => {
                return Err("帮助参数请单独使用。".to_string());
            }
            other => {
                return Err(format!("无法识别的参数: {}", other));
            }
        }
    }

    Ok(Config {
        fps,
        input_path,
        output_path,
        input_encoding,
    })
}

/// 打印命令行帮助。
fn print_help(program: &str) {
    println!(
        r#"EDL 转 SRT 工具

用途:
  将 EDL 文件转换为 SRT 字幕文件。
  输入 EDL 支持已知编码优先，未知编码自动识别。
  输出 SRT 固定为 UTF-8 无 BOM。
  如果输出文件已存在，程序会自动追加时间戳生成新文件名。

用法:
  {0} <帧率> <输入文件.edl> <输出文件.srt>
  {0} <帧率> <输入文件.edl> <输出文件.srt> --input-encoding <编码名>
  {0} -h
  {0} --help

参数:
  <帧率>                数字，支持整数或小数，例如 24、25、29.97、23.976
  <输入文件.edl>        源 EDL 文件路径
  <输出文件.srt>        生成的 SRT 文件路径（UTF-8 无 BOM）
  --input-encoding      已知输入编码时优先使用；未提供时自动检测

示例:
  {0} 30 input.edl output.srt
  {0} 30 input.edl output.srt --input-encoding shift_jis
  {0} 25 input.edl output.srt --input-encoding utf-16le
  {0} 29.97 project/main.edl subtitles/final_sub.srt

说明:
  - 自动识别依赖 chardetng，适合大多数常见文本编码
  - 已知输入编码时，建议显式传入 --input-encoding 以获得更稳定结果
  - 支持相对路径和绝对路径
  - 如果输出文件已存在，程序会自动改名，而不是覆盖旧文件
  - 请确保输出目录具有写入权限
"#,
        program
    );
}

/// 解码 EDL 原始字节。
///
/// 优先级：
/// 1. 用户手动指定编码
/// 2. 文件自身 BOM
/// 3. 自动检测
fn decode_edl_bytes(
    bytes: &[u8],
    preferred_encoding: Option<&str>,
) -> Result<DecodeResult, String> {
    if let Some(label) = preferred_encoding {
        let encoding = Encoding::for_label(label.as_bytes())
            .ok_or_else(|| format!("不支持的编码: {}", label))?;

        let (content, had_errors) = decode_with_encoding(encoding, bytes);

        return Ok(DecodeResult {
            content,
            encoding_name: encoding.name().to_string(),
            had_errors,
        });
    }

    if let Some((encoding, bom_len)) = Encoding::for_bom(bytes) {
        let (content, had_errors) = decode_with_encoding(encoding, &bytes[bom_len..]);

        return Ok(DecodeResult {
            content,
            encoding_name: format!("{} (BOM)", encoding.name()),
            had_errors,
        });
    }

    let mut detector = EncodingDetector::new();
    detector.feed(bytes, true);
    let guessed = detector.guess(None, true);

    let (content, had_errors) = decode_with_encoding(guessed, bytes);

    Ok(DecodeResult {
        content,
        encoding_name: guessed.name().to_string(),
        had_errors,
    })
}

/// 用指定编码把字节解码为 Rust String。
fn decode_with_encoding(encoding: &'static Encoding, bytes: &[u8]) -> (String, bool) {
    let (cow, _, had_errors) = encoding.decode(bytes);
    (cow.into_owned(), had_errors)
}

/// 根据原始输出路径，决定最终真正写入的输出路径。
///
/// 规则：
/// - 如果文件不存在，直接使用原路径
/// - 如果文件已存在，则自动追加时间戳
///
/// 例如：
///   tomoni.srt
/// 会变成：
///   tomoni_20260711_194900.srt
fn resolve_output_path(path: &Path) -> PathBuf {
    if !path.exists() {
        return path.to_path_buf();
    }

    // parent() 取父目录
    // 如果拿不到，就退回当前目录 "."
    let parent = path.parent().unwrap_or_else(|| Path::new("."));

    // file_stem() 取“不带扩展名”的文件名
    // 例如 "tomoni.srt" -> "tomoni"
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    // extension() 取扩展名
    // 例如 "tomoni.srt" -> "srt"
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    // 用 chrono 生成当前本地时间
    // format("%Y%m%d_%H%M%S") 例如：
    // 20260711_194900
    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();

    // 如果原文件有扩展名，就保留扩展名
    // 没有扩展名也能工作
    let new_file_name = if ext.is_empty() {
        format!("{}_{}", stem, timestamp)
    } else {
        format!("{}_{}.{}", stem, timestamp, ext)
    };

    parent.join(new_file_name)
}

/// 把 EDL 文本解析成字幕条目列表。
///
/// 每条字幕由三部分组成：
/// (开始时间, 结束时间, 文本)
///
/// 这里保持你原本的解析思路：
/// - 数字开头的行：作为时间码行
/// - "* FROM CLIP NAME:" 行：作为字幕文本来源
fn parse_edl_to_entries(content: &str) -> Vec<(String, String, String)> {
    let mut srt_entries: Vec<(String, String, String)> = Vec::new();

    let mut current_start: Option<String> = None;
    let mut current_end: Option<String> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        if trimmed
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();

            if parts.len() >= 2 {
                let start_time = parts[parts.len() - 2].to_string();
                let end_time = parts[parts.len() - 1].to_string();

                current_start = Some(start_time);
                current_end = Some(end_time);
            }
        } else if trimmed.starts_with("* FROM CLIP NAME:") {
            if let (Some(start), Some(end)) = (&current_start, &current_end) {
                let raw_name = trimmed.trim_start_matches("* FROM CLIP NAME:");
                let clean_name = strip_audio_extension(raw_name.trim());

                srt_entries.push((start.clone(), end.clone(), clean_name));

                current_start = None;
                current_end = None;
            }
        }
    }

    srt_entries
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

    let ms_from_frames = ((frames as f64) / fps) * 1000.0;

    let total_ms = (hours * 3_600_000)
        + (minutes * 60_000)
        + (seconds * 1_000)
        + (ms_from_frames.round() as u32);

    let h = total_ms / 3_600_000;
    let remainder = total_ms % 3_600_000;

    let m = remainder / 60_000;
    let remainder = remainder % 60_000;

    let s = remainder / 1_000;
    let ms = remainder % 1_000;

    format!("{:02}:{:02}:{:02},{:03}", h, m, s, ms)
}

/// 去除常见音频扩展名。
///
/// 例如：
/// - hello.wav -> hello
/// - test.MP3  -> test
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

/// 写出 SRT 文件。
///
/// 输出固定为 UTF-8 无 BOM。
///
/// Rust 的 String 本身就是 UTF-8，
/// 只要我们不手动写入 BOM 字节，输出就是 UTF-8 无 BOM。
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
