/// z-ai/glm-4.7
/// wow~ ⊙o⊙
use anyhow::Result;
use chrono::Local;
use clap::Parser;
use digest::Digest;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::ThreadPoolBuilder;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs::{File, create_dir_all};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

/// Rust 高性能文件查重工具 (完全复刻 PowerShell 脚本功能)
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// 要扫描的根目录路径 (必填)
    #[arg(short, long)]
    directory_path: PathBuf,

    /// 查重模式: 'Name' (仅按文件名) 或 'Hash' (按内容哈希)
    #[arg(short, long, default_value = "hash")]
    match_mode: String,

    /// Hash模式下的算法: 'MD5', 'SHA256', 'SHA1'
    #[arg(short, long, default_value = "md5")]
    algorithm: String,

    /// 是否递归扫描子目录 (默认: true)
    #[arg(short, long, default_value_t = true)]
    recurse: bool,

    /// 仅扫描指定扩展名的文件 (例如: jpg;png)
    #[arg(short = 'i', long, value_delimiter = ';')]
    include_extensions: Vec<String>,

    /// 排除指定扩展名的文件 (例如: tmp;log)
    #[arg(short = 'e', long, value_delimiter = ';')]
    exclude_extensions: Vec<String>,

    /// 开启后将重复文件移动到归档文件夹，并在移动前提示确认
    #[arg(short, long)]
    move_duplicates: bool,

    /// 指定报告文件的存放目录 (默认为扫描目录)
    #[arg(long)]
    report_dir: Option<PathBuf>,

    /// 指定报告文件的名字 (默认为 <时间戳>_dedup_report.txt)
    #[arg(long)]
    report_name: Option<String>,

    /// 限制用于并行计算的线程数 (默认为系统所有核心，设为 4 或 8 可减少磁盘 I/O 压力)
    #[arg(long, default_value = "0")]
    threads: usize,

    /// 最小文件大小过滤 (例如: 100KB, 1MB, 1G)
    #[arg(long, default_value = "0B")]
    min_size: String,

    /// 最大文件大小过滤 (例如: 500MB, 1G)
    /// 如果不填，默认无限制 (0B 也是无限制，但为了明确推荐留空)
    #[arg(long, default_value = "")]
    max_size: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let scan_path = &args.directory_path;

    // --- 解析大小参数 ---
    let min_size_bytes = parse_size(&args.min_size).unwrap_or(0);

    // 修复默认值逻辑：如果用户没传 max_size (为空字符串)，则视为无上限
    // 否则，解析用户输入的值。如果解析失败，也视为无上限。
    let max_size_bytes = if args.max_size.is_empty() {
        u64::MAX
    } else {
        parse_size(&args.max_size).unwrap_or(u64::MAX)
    };

    // --- 初始化线程池 ---
    let num_threads = if args.threads == 0 {
        num_cpus::get()
    } else {
        args.threads
    };

    ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .expect("Failed to initialize thread pool");

    println!("========================================");
    println!("🚀 开始扫描: {}", scan_path.display());
    println!(
        "🔧 模式: {}, 算法: {}",
        args.match_mode.to_uppercase(),
        args.algorithm.to_uppercase()
    );
    println!("🧵 并行线程数: {}", num_threads);
    if min_size_bytes > 0 || max_size_bytes < u64::MAX {
        let min_str = format_bytes(min_size_bytes);
        let max_str = if max_size_bytes == u64::MAX {
            "无限".to_string()
        } else {
            format_bytes(max_size_bytes)
        };
        println!("📏 大小过滤: {} - {}", min_str, max_str);
    }
    println!("========================================");
    let start_time = std::time::Instant::now();

    if !scan_path.exists() {
        anyhow::bail!("错误: 路径不存在: {}", scan_path.display());
    }

    // 准备过滤器
    let includes: Vec<String> = args
        .include_extensions
        .iter()
        .map(|s| {
            s.trim_start_matches('.')
                .trim_start_matches('*')
                .to_lowercase()
        })
        .collect();
    let excludes: Vec<String> = args
        .exclude_extensions
        .iter()
        .map(|s| {
            s.trim_start_matches('.')
                .trim_start_matches('*')
                .to_lowercase()
        })
        .collect();

    // --- 步骤 1: 收集文件 + 大小预筛选 (P0: 使用 max_depth) ---
    let scan_spinner = ProgressBar::new_spinner();
    scan_spinner.set_style(
        ProgressStyle::with_template("{spinner:.dim.cyan} {msg}")
            .unwrap()
            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ "),
    );
    scan_spinner.set_message("正在扫描目录并过滤文件...");

    // 存储元组: (Path, FileSize)
    // P1 优化: 在这里缓存大小，避免后续重复读取 metadata
    let mut files: Vec<(PathBuf, u64)> = Vec::new();

    // P0 优化: 根据递归参数设置 max_depth
    let mut walker_builder = walkdir::WalkDir::new(scan_path);
    if !args.recurse {
        walker_builder = walker_builder.max_depth(1);
    }
    let base_walker = walker_builder
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file());

    for entry in base_walker {
        let path = entry.path();

        // P1 优化: 提前读取大小用于过滤
        let valid_size = if let Ok(meta) = entry.metadata() {
            let size = meta.len();
            if size < min_size_bytes || size > max_size_bytes {
                false
            } else {
                true
            }
        } else {
            false
        };

        if !valid_size {
            continue;
        }

        let valid = if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_lower = ext.to_lowercase();
            if !includes.is_empty() && !includes.contains(&ext_lower) {
                false
            } else if !excludes.is_empty() && excludes.contains(&ext_lower) {
                false
            } else {
                true
            }
        } else {
            includes.is_empty()
        };

        if valid {
            files.push((path.to_path_buf(), entry.metadata().unwrap().len()));
        }
    }

    scan_spinner.finish_with_message(format!("扫描完成，共找到 {} 个有效文件。", files.len()));

    if files.is_empty() {
        println!("❌ 没有找到符合条件的文件，退出。");
        return Ok(());
    }

    // --- 步骤 2 & 3: 分组 & Hash ---
    let duplicates_map: HashMap<String, Vec<PathBuf>> =
        if args.match_mode.eq_ignore_ascii_case("name") {
            group_by_name(&files)
        } else {
            group_by_hash(&files, &args.algorithm)?
        };

    // 3. 过滤出真正的重复项 (组内文件数 > 1)
    let duplicates: Vec<_> = duplicates_map
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .collect();

    // 4. 统计总重复文件数
    let total_dup_files: usize = duplicates.iter().map(|(_, paths)| paths.len()).sum();

    let duration = start_time.elapsed();

    if duplicates.is_empty() {
        println!("🎉 恭喜！未发现重复文件。");
    } else {
        println!("✅ 扫描完成！耗时: {:.2} 秒", duration.as_secs_f64());
        println!(
            "📊 发现 {} 组重复文件，共 {} 个重复文件。",
            duplicates.len(),
            total_dup_files
        );

        // --- 构造报告路径 ---
        let target_dir = args.report_dir.clone().unwrap_or_else(|| scan_path.clone());

        // 文件名前加下划线
        let target_name = args.report_name.clone().unwrap_or_else(|| {
            format!("_{}_dedup_report.txt", Local::now().format("%Y%m%d_%H%M%S"))
        });

        let report_path = target_dir.join(&target_name);
        create_dir_all(&target_dir)?;

        let mut report = File::create(&report_path)?;

        println!("📄 报告路径: {}", report_path.display());

        // 写入详细参数 (确保完整性：无论是否为默认值，都记录实际值)
        writeln!(report, "========== 查重报告 ==========")?;
        writeln!(report)?;
        writeln!(report, "--- 运行参数详情 ---")?;
        writeln!(report, "扫描路径: {}", args.directory_path.display())?;
        writeln!(report, "匹配模式: {}", args.match_mode)?;
        writeln!(report, "哈希算法: {}", args.algorithm)?;
        writeln!(report, "递归扫描: {}", args.recurse)?;

        // 大小过滤：总是打印，明确标注默认值
        let max_str_display = if max_size_bytes == u64::MAX {
            "无限".to_string()
        } else {
            format_bytes(max_size_bytes)
        };
        writeln!(
            report,
            "大小范围: {} - {}",
            format_bytes(min_size_bytes),
            max_str_display
        )?;

        // 包含/排除扩展：总是打印
        if !args.include_extensions.is_empty() {
            writeln!(report, "包含扩展: {}", args.include_extensions.join(";"))?;
        } else {
            writeln!(report, "包含扩展: (所有)")?;
        }
        if !args.exclude_extensions.is_empty() {
            writeln!(report, "排除扩展: {}", args.exclude_extensions.join(";"))?;
        } else {
            writeln!(report, "排除扩展: (无)")?;
        }

        writeln!(report, "移动重复: {}", args.move_duplicates)?;

        if let Some(ref dir) = args.report_dir {
            writeln!(report, "报告目录: {}", dir.display())?;
        } else {
            writeln!(report, "报告目录: (默认: 扫描目录)")?;
        }
        if let Some(ref name) = args.report_name {
            writeln!(report, "自定义名称: {}", name)?;
        }
        writeln!(report, "线程限制: {}", args.threads)?;
        writeln!(report, "-----------------------")?;
        writeln!(report)?;

        // 写入统计数据
        writeln!(report, "--- 统计结果 ---")?;
        writeln!(report, "耗时: {:.2}s", duration.as_secs_f64())?;
        writeln!(report, "重复组数: {}", duplicates.len())?;
        writeln!(report, "重复文件总数: {}", total_dup_files)?;
        writeln!(report, "=================================")?;
        writeln!(report)?;

        // 写入详细列表
        for (key, paths) in &duplicates {
            writeln!(report, "【{}】数量: {}", key, paths.len())?;
            for p in paths {
                writeln!(report, "  -> {}", p.display())?;
            }
            writeln!(report)?;
        }
        println!("📄 报告已成功生成: {}", report_path.display());

        // --- 步骤 4: 移动文件 (带双重确认) ---
        if args.move_duplicates {
            perform_move_duplicates(&duplicates, &target_name)?;
        }
    }

    Ok(())
}

fn group_by_name(files: &[(PathBuf, u64)]) -> HashMap<String, Vec<PathBuf>> {
    files.iter().fold(HashMap::new(), |mut acc, (path, _size)| {
        if let Some(file_name) = path.file_stem().and_then(|s| s.to_str()) {
            acc.entry(file_name.to_string())
                .or_default()
                .push(path.clone());
        }
        acc
    })
}

fn group_by_hash(
    files: &[(PathBuf, u64)],
    algorithm: &str,
) -> Result<HashMap<String, Vec<PathBuf>>> {
    // --- 步骤 2: 按大小分组 (使用缓存的大小，性能优化) ---
    println!("⚙️  准备: 按文件大小预筛选...");
    let size_pb = ProgressBar::new(files.len() as u64);
    size_pb.set_style(
        ProgressStyle::with_template("{elapsed_precise} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("##-"),
    );
    size_pb.set_message("正在按大小分组...");

    let mut files_by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for (path, size) in files {
        files_by_size.entry(*size).or_default().push(path.clone());
        size_pb.inc(1);
    }
    size_pb.finish_with_message("按大小分组完成");

    // P1 内存优化: 此时 files (包含 size) 可以释放一部分内存，或者等 candidates 构建后释放
    // 这里我们继续使用 files_by_size

    // 收集冲突候选者
    let candidates: Vec<PathBuf> = files_by_size
        .values()
        .filter(|v| v.len() > 1)
        .flat_map(|v| v.iter().cloned())
        .collect();

    if candidates.is_empty() {
        return Ok(HashMap::new());
    }

    // --- 步骤 3: 计算 Hash (使用 par_iter + fold/reduce 解决类型收集问题) ---
    println!("⚙️  执行: 计算文件 Hash...");
    let hash_pb = ProgressBar::new(candidates.len() as u64);
    hash_pb.set_style(
        ProgressStyle::with_template("{elapsed_precise} [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("##-"),
    );

    let algo_lower = algorithm.to_lowercase();
    let hash_pb_ref = &hash_pb; // 引用进度条用于在闭包中更新

    // 修复：使用 fold + reduce 模式来正确收集 HashMap<String, Vec<PathBuf>>
    // 并在 filter_map 中更新进度条
    let hash_map: HashMap<String, Vec<PathBuf>> = candidates
        .into_par_iter()
        .filter_map(|path| {
            let hash_result = if algo_lower.contains("sha256") {
                compute_hash::<sha2::Sha256>(&path)
            } else if algo_lower.contains("sha1") {
                compute_hash::<sha1::Sha1>(&path)
            } else {
                compute_hash::<md5::Md5>(&path)
            };

            // 更新进度条 (ProgressBar 是 Sync 的，可以安全地在并行闭包中调用 inc)
            hash_pb_ref.inc(1);

            hash_result.map(|h| (h, path))
        })
        .fold(
            || HashMap::<String, Vec<PathBuf>>::new(),
            |mut acc: HashMap<String, Vec<PathBuf>>, (hash, path)| {
                acc.entry(hash).or_default().push(path);
                acc
            },
        )
        .reduce(
            || HashMap::<String, Vec<PathBuf>>::new(),
            |mut acc: HashMap<String, Vec<PathBuf>>, mut b| {
                for (k, v) in b.drain() {
                    acc.entry(k).or_default().extend(v);
                }
                acc
            },
        );

    hash_pb.finish_with_message("Hash 计算完成");

    Ok(hash_map)
}

fn perform_move_duplicates(
    duplicates: &[(String, Vec<PathBuf>)],
    report_name_ref: &str,
) -> Result<()> {
    // 计算需要移动的文件总数
    let total_files_to_move: usize = duplicates
        .iter()
        .map(|(_, paths)| if paths.len() > 1 { paths.len() - 1 } else { 0 })
        .sum();

    println!(
        "⚠️  警告: 共 {} 组重复文件，预计移动 {} 个重复文件到归档目录。",
        duplicates.len(),
        total_files_to_move
    );
    println!("📄 参考报告名: {}", report_name_ref);

    // --- P0: 双重确认机制 ---
    print!("第 1 步确认: 是否开始移动? [y/N]: ");
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    if !buf.trim().eq_ignore_ascii_case("y") {
        anyhow::bail!("用户取消操作。");
    }

    print!("第 2 步确认: 最后一次确认，确定要移动吗? [y/N]: ");
    io::stdout().flush()?;
    buf.clear();
    io::stdin().read_line(&mut buf)?;
    if !buf.trim().eq_ignore_ascii_case("y") {
        anyhow::bail!("用户取消操作。");
    }

    // 归档目录分层逻辑
    let archive_base = PathBuf::from("_archive");
    let now = Local::now();
    let archive_dir = archive_base
        .join(now.format("%Y").to_string())
        .join(now.format("%m").to_string())
        .join(now.format("%d").to_string());

    create_dir_all(&archive_dir)?;
    println!("📦 归档目录结构: {}", archive_dir.display());

    // 创建进度条
    let move_pb = ProgressBar::new(total_files_to_move as u64);
    move_pb.set_style(
        ProgressStyle::with_template(
            "{elapsed_precise} [{bar:40.yellow/blue}] {pos}/{len} ({eta})",
        )
        .unwrap()
        .progress_chars("##-"),
    );

    let mut moved_count = 0;
    let mut error_count = 0;

    for (_key, paths) in duplicates {
        let mut sorted_paths = paths.clone();
        sorted_paths.sort();

        if sorted_paths.len() > 1 {
            for file_to_move in sorted_paths.iter().skip(1) {
                let file_name = file_to_move
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                let dest = archive_dir.join(&file_name);

                // 确定最终路径 (处理重名)
                let final_dest = if dest.exists() {
                    let mut counter = 1;
                    loop {
                        let stem = file_to_move
                            .file_stem()
                            .unwrap()
                            .to_string_lossy()
                            .to_string();
                        let ext = file_to_move
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("");
                        let new_name = format!("{} ({})", stem, counter);
                        let mut candidate = archive_dir.join(&new_name);
                        candidate.set_extension(ext);

                        if !candidate.exists() {
                            break candidate;
                        }
                        counter += 1;
                    }
                } else {
                    dest
                };

                // P1 优化: 跨设备移动兼容性 (rename 失败则尝试 copy + remove)
                let move_result = std::fs::rename(file_to_move, &final_dest);
                if let Err(e) = move_result {
                    // 修复：检查错误信息字符串 "cross-device"，因为 ErrorKind 中没有 CrossDeviceLinkError
                    if e.kind() == io::ErrorKind::Unsupported
                        || e.to_string().to_lowercase().contains("cross-device")
                    {
                        match std::fs::copy(file_to_move, &final_dest) {
                            Ok(_) => {
                                if let Err(remove_err) = std::fs::remove_file(file_to_move) {
                                    eprintln!(
                                        "移动失败 (复制成功但删除原文件失败): {} -> {:?} (Error: {})",
                                        file_to_move.display(),
                                        final_dest,
                                        remove_err
                                    );
                                    error_count += 1;
                                } else {
                                    moved_count += 1;
                                }
                            }
                            Err(copy_err) => {
                                eprintln!(
                                    "跨设备复制失败: {} -> {:?} (Error: {})",
                                    file_to_move.display(),
                                    final_dest,
                                    copy_err
                                );
                                error_count += 1;
                            }
                        }
                    } else {
                        eprintln!(
                            "移动失败: {} -> {:?} (Error: {})",
                            file_to_move.display(),
                            final_dest,
                            e
                        );
                        error_count += 1;
                    }
                } else {
                    moved_count += 1;
                }

                move_pb.inc(1);
            }
        }
    }

    move_pb.finish_with_message("文件移动完成");

    println!("\n✅ 移动完成！");
    println!("📦 归档目录: {}", archive_dir.display());
    println!("✅ 成功移动: {} 个", moved_count);
    if error_count > 0 {
        println!("❌ 失败: {} 个 (请查看上方报错)", error_count);
    }

    Ok(())
}

fn compute_hash<D: Digest>(path: &Path) -> Option<String> {
    if let Ok(mut file) = File::open(path) {
        let mut hasher = D::new();
        let mut buffer = [0u8; 8192];
        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => hasher.update(&buffer[..n]),
                Err(_) => return None,
            }
        }
        Some(hex::encode(hasher.finalize()))
    } else {
        None
    }
}

// 简单的大小解析辅助函数
// 修复类型问题：在 else 分支使用 s.as_str() 确保返回 &str
fn parse_size(size_str: &str) -> Option<u64> {
    let s = size_str.trim().to_uppercase();
    // trim_end_matches 返回 &str
    let (num_part, unit_part) = if s.ends_with("KB") {
        (s.trim_end_matches("KB"), 1024u64)
    } else if s.ends_with("MB") {
        (s.trim_end_matches("MB"), 1024u64 * 1024)
    } else if s.ends_with("GB") {
        (s.trim_end_matches("GB"), 1024u64 * 1024 * 1024)
    } else if s.ends_with("B") {
        (s.trim_end_matches("B"), 1u64)
    } else {
        (s.as_str(), 1u64) // 默认视为 bytes，使用 .as_str() 返回 &str
    };

    num_part.parse::<u64>().ok().map(|n| n * unit_part)
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes < KB {
        format!("{} B", bytes)
    } else if bytes < MB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else if bytes < GB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    }
}

/*
    ============================================================
    参数解释与使用示例
    ============================================================
    0. .\target\release\fast_dedup.exe --directory-path "" --match-mode Hash --algorithm SHA256 --recurse

    1. -DirectoryPath (必填): 要扫描的根目录路径。
       示例: --directory-path "D:\Data"

    2. -MatchMode (可选): 查重模式。
       - 'Name' (默认为 Hash): 仅按文件基础名(不含扩展名)分组，速度快。
       - 'Hash': 计算文件哈希值分组，准确性高，但慢。
       示例: --match-mode hash 或 --match-mode name

    3. -Algorithm (可选): Hash模式下的算法。
       - 'MD5' (默认): 速度最快。
       - 'SHA256': 安全性最高。
       - 'SHA1': 中等。
       示例: --algorithm SHA256

    4. -Recurse (可选): 开关参数。如果存在，则递归扫描所有子目录。
       示例: --no-recurse (不递归) 或 --recurse (递归，默认)

    5. -IncludeExtensions (可选): 字符串数组。仅扫描指定扩展名的文件。
       示例: --include-extensions "jpg;png;gif"

    6. -ExcludeExtensions (可选): 字符串数组。排除指定扩展名的文件。
       示例: --exclude-extensions "tmp;log;bak"

    7. -MoveDuplicates (可选): 开关参数。如果存在，则将重复文件移动到归档文件夹，并在移动前提示确认。
       示例: --move-duplicates

    8. -ReportDir (可选): 指定报告文件的存放目录。如果不指定，默认为扫描目录根目录。
       示例: --report-dir "D:\Data\Reports"

    9. -ReportName (可选): 指定报告文件的名字。
       示例: --report-name "查重结果.txt"

    10. -Threads (可选): 限制并行计算使用的线程数。默认为 0 (自动检测所有核心)。
        建议: 如果在机械硬盘上运行，建议限制为 4 或 8，以避免磁盘 IOPS 瓶颈。
        示例: --threads 4

    11. -MinSize (可选): 最小文件大小过滤。默认值为 0 (不过滤)。
        示例: --min-size "100KB" 或 --min-size "1MB"

    12. -MaxSize (可选): 最大文件大小过滤。默认为空字符串 (代表无上限)。
        如果不填该参数，程序将不会过滤大文件。
        示例: --max-size "500MB" 或 --max-size "1GB"
*/

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
