use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;

// ============================================================
// 数据结构定义
// ============================================================
#[derive(Serialize)]
pub struct GameLinks {
    pub appid: u32,
    pub steamdb_url: String,
    pub hltb_url: Option<String>,
    pub query_method: String,
}

// ============================================================
// 辅助函数：路径与帮助信息
// ============================================================

/// 获取默认的缓存文件路径：<exe目录>/caches/search_maps.json
fn get_default_cache_path() -> PathBuf {
    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            return exe_dir.join("caches").join("search_maps.json");
        }
    }
    // 如果获取 exe 路径失败，退回到当前工作目录
    PathBuf::from("./caches/search_maps.json")
}

/// 打印 CLI 帮助信息
fn print_help() {
    let default_path = get_default_cache_path();
    println!("zqinfoss - Steam 游戏链接生成器");
    println!();
    println!("用法:");
    println!("  1. 交互模式 (双击运行):");
    println!("     zqinfoss");
    println!();
    println!("  2. 直接查询:");
    println!("     zqinfoss <steam_url> [cache_path]");
    println!(
        "     (如果不指定 cache_path，默认使用: {})",
        default_path.display()
    );
    println!();
    println!("  3. 缓存管理:");
    println!("     zqinfoss cache list              - 查看所有缓存记录");
    println!("     zqinfoss cache clear             - 清空并删除缓存文件");
    println!("     zqinfoss cache remove <appid>    - 删除指定的 appid 记录");
    println!();
    println!("参数:");
    println!("  <steam_url>   Steam 商店游戏页面的 URL。");
    println!("  [cache_path]  可选。自定义缓存文件路径。");
    println!();
    println!("示例:");
    println!("  zqinfoss \"https://store.steampowered.com/app/282140/SOMA/\"");
    println!("  zqinfoss cache list");
}

// ============================================================
// 缓存读写函数
// ============================================================

async fn load_cache(cache_path: &Path) -> HashMap<u32, String> {
    // 确保目录存在
    if let Some(parent) = cache_path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }

    if cache_path.exists() {
        match fs::read_to_string(cache_path).await {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => HashMap::new(),
        }
    } else {
        HashMap::new()
    }
}

async fn save_cache(cache_path: &Path, cache: &HashMap<u32, String>) {
    if let Some(parent) = cache_path.parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    if let Ok(data) = serde_json::to_string_pretty(cache) {
        if let Err(e) = fs::write(cache_path, data).await {
            eprintln!(
                "Warning: Failed to write cache to {}: {}",
                cache_path.display(),
                e
            );
        }
    }
}

// ============================================================
// 缓存管理命令处理
// ============================================================

/// 处理 `zqinfoss cache ...` 命令
async fn handle_cache_command(args: &[String]) {
    let cache_path = get_default_cache_path();

    if args.len() < 3 {
        eprintln!("缺少缓存操作指令。用法: zqinfoss cache <list|clear|remove>");
        return;
    }

    let action = args[2].as_str();

    match action {
        "list" => {
            let cache = load_cache(&cache_path).await;
            if cache.is_empty() {
                println!("当前没有缓存记录。");
            } else {
                println!("缓存记录 ({}):", cache_path.display());
                for (appid, name) in cache {
                    println!("  [{}] {}", appid, name);
                }
            }
        }
        "clear" => {
            if cache_path.exists() {
                match fs::remove_file(&cache_path).await {
                    Ok(_) => println!("✓ 缓存文件已删除。"),
                    Err(e) => eprintln!("✗ 删除失败: {}", e),
                }
            } else {
                println!("缓存文件本就不存在。");
            }
        }
        "remove" => {
            if args.len() < 4 {
                eprintln!("缺少 appid。用法: zqinfoss cache remove <appid>");
                return;
            }
            let appid_str = &args[3];
            match appid_str.parse::<u32>() {
                Ok(appid) => {
                    let mut cache = load_cache(&cache_path).await;
                    if cache.remove(&appid).is_some() {
                        save_cache(&cache_path, &cache).await;
                        println!("✓ 已移除 appid {} 的记录", appid);
                    } else {
                        println!("缓存中没有找到 appid {}", appid);
                    }
                }
                Err(_) => eprintln!("appid 必须是数字"),
            }
        }
        _ => eprintln!("未知的缓存操作: {}", action),
    }
}

// ============================================================
// Steam API 请求函数
// ============================================================

async fn fetch_name_from_steam(
    appid: u32,
    client: &reqwest::Client,
    progress_bar: &ProgressBar,
) -> Option<String> {
    let url = format!(
        "https://store.steampowered.com/api/appdetails?appids={}",
        appid
    );

    loop {
        let result = client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match result {
            Ok(resp) => {
                let json: Value = resp.json().await.ok()?;
                let data_obj = json.get(appid.to_string())?;
                if data_obj.get("success")?.as_bool()? {
                    let name = data_obj["data"]["name"].as_str()?.to_string();
                    return Some(name);
                } else {
                    return None;
                }
            }
            Err(e) => {
                let is_timeout = e.is_timeout();
                let should_retry = progress_bar.suspend(|| -> bool {
                    if is_timeout {
                        println!("⚠ 请求超时（5秒内未收到 Steam 响应）。");
                    } else {
                        println!("⚠ 网络请求失败：{}", e);
                    }
                    print!("是否重新尝试查询？(y/n): ");
                    let _ = io::stdout().flush();

                    let stdin = io::stdin();
                    let mut line = String::new();
                    if stdin.lock().read_line(&mut line).is_ok() {
                        let answer = line.trim().to_lowercase();
                        return answer == "y" || answer == "yes";
                    }
                    false
                });

                if should_retry {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                } else {
                    return None;
                }
            }
        }
    }
}

// ============================================================
// 核心处理函数
// ============================================================

pub async fn generate_links(
    steam_url: &str,
    cache_path: &Path,
    cache: &mut HashMap<u32, String>,
    client: &reqwest::Client,
    progress_bar: &ProgressBar,
) -> Result<GameLinks, String> {
    progress_bar.set_message("正在解析 Steam URL...");
    let clean_url = steam_url.split('?').next().unwrap_or(steam_url);
    let parts: Vec<&str> = clean_url.split('/').filter(|p| !p.is_empty()).collect();

    let app_index = parts
        .iter()
        .position(|&p| p == "app")
        .ok_or("Invalid Steam URL: URL 中未找到 'app' 路径段")?;

    if app_index + 2 >= parts.len() {
        return Err("URL 结构不完整：缺少 appid 或 slug 段".to_string());
    }

    let appid: u32 = parts[app_index + 1].parse().map_err(|_| {
        format!(
            "Invalid appid: 无法将 '{}' 解析为数字",
            parts[app_index + 1]
        )
    })?;

    let raw_slug = parts[app_index + 2];
    let cleaned_name = raw_slug
        .trim_matches('_')
        .replace('_', " ")
        .replace("  ", " ");
    let steamdb_url = format!("https://steamdb.info/app/{}/", appid);

    if !cleaned_name.is_empty() {
        progress_bar.set_message("✓ 从 URL 中提取到游戏名，直接生成链接");
        let encoded_name = urlencoding::encode(&cleaned_name).to_string();
        Ok(GameLinks {
            appid,
            steamdb_url,
            hltb_url: Some(format!("https://howlongtobeat.com/?q={}", encoded_name)),
            query_method: "DirectSlug".to_string(),
        })
    } else {
        progress_bar.set_message("正在查询本地缓存...");
        if let Some(cached_name) = cache.get(&appid) {
            progress_bar.set_message("✓ 命中本地缓存");
            let encoded_name = urlencoding::encode(cached_name).to_string();
            return Ok(GameLinks {
                appid,
                steamdb_url,
                hltb_url: Some(format!("https://howlongtobeat.com/?q={}", encoded_name)),
                query_method: "LocalCache".to_string(),
            });
        }

        progress_bar.set_message("正在请求 Steam AppID API...");
        let fetched_name = fetch_name_from_steam(appid, client, progress_bar).await;

        match fetched_name {
            Some(name) => {
                progress_bar.set_message("查询成功，正在写入新缓存...");
                cache.insert(appid, name.clone());
                save_cache(cache_path, cache).await;
                let encoded_name = urlencoding::encode(&name).to_string();
                Ok(GameLinks {
                    appid,
                    steamdb_url,
                    hltb_url: Some(format!("https://howlongtobeat.com/?q={}", encoded_name)),
                    query_method: "NetworkNewCache".to_string(),
                })
            }
            None => Ok(GameLinks {
                appid,
                steamdb_url,
                hltb_url: None,
                query_method: "Failed".to_string(),
            }),
        }
    }
}

// ============================================================
// 程序入口
// ============================================================

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // 1. 处理无参数情况 (双击 exe 时的交互模式)
    if args.len() == 1 {
        run_interactive_mode().await;
        return;
    }

    // 2. 处理 help 参数
    if args.len() == 2 && (args[1] == "--help" || args[1] == "-h") {
        print_help();
        return;
    }

    // 3. 处理 cache 子命令
    if args.len() >= 2 && args[1] == "cache" {
        handle_cache_command(&args).await;
        return;
    }

    // 4. 处理正常查询逻辑
    // 如果只传了 URL，使用默认缓存路径
    let cache_path = if args.len() == 2 {
        get_default_cache_path()
    } else if args.len() == 3 {
        PathBuf::from(&args[2])
    } else {
        eprintln!("参数过多。请输入 'zqinfoss --help' 查看用法。");
        return;
    };

    let steam_url = &args[1];
    run_query(steam_url, &cache_path).await;
}

/// 执行单次查询的核心封装
async fn run_query(steam_url: &str, cache_path: &Path) {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build HTTP client");

    let progress_bar = ProgressBar::new_spinner();
    progress_bar.set_style(
        ProgressStyle::with_template("{spinner} {msg}")
            .unwrap()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"),
    );
    progress_bar.enable_steady_tick(Duration::from_millis(80));

    let mut cache = load_cache(cache_path).await;

    match generate_links(steam_url, cache_path, &mut cache, &client, &progress_bar).await {
        Ok(links) => {
            progress_bar.finish_with_message("✓ 处理完成");
            if let Ok(json_str) = serde_json::to_string_pretty(&links) {
                println!("\n{}", json_str);
            }
        }
        Err(e) => {
            progress_bar.finish_with_message("✗ 处理失败");
            eprintln!("\nError: {}", e);
        }
    }
}

/// 双击 exe 时进入的交互循环
async fn run_interactive_mode() {
    println!("==================================");
    println!("    zqinfoss 交互模式");
    println!("    输入 Steam URL 开始查询");
    println!("    输入 help 查看帮助文档");
    println!("    输入 setpath 更改缓存文件位置");
    println!("    输入 exit 退出程序");
    println!("==================================\n");

    // 初始路径设为默认路径
    let mut current_cache_path = get_default_cache_path();
    println!("提示：当前缓存保存在 {}\n", current_cache_path.display());

    loop {
        print!("请输入指令或 URL: ");
        let _ = io::stdout().flush();

        let mut input = String::new();
        io::stdin().read_line(&mut input).expect("读取输入失败");
        let input = input.trim();

        if input.is_empty() {
            continue;
        }

        // 退出命令
        if input.eq_ignore_ascii_case("exit") {
            break;
        }

        // 查看帮助命令
        if input.eq_ignore_ascii_case("help") || input == "?" {
            print_help(); // 调用之前写好的打印帮助函数
            println!(); // 额外打印一个空行保持排版美观
            continue;
        }

        // 修改路径命令
        if input.eq_ignore_ascii_case("setpath") {
            print!("请输入新的缓存文件完整路径 (例如 D:\\my_cache.json): ");
            let _ = io::stdout().flush();
            let mut path_input = String::new();
            io::stdin()
                .read_line(&mut path_input)
                .expect("读取路径失败");
            let new_path = path_input.trim();

            if !new_path.is_empty() {
                current_cache_path = PathBuf::from(new_path);
                println!("✓ 缓存路径已更新为: {}\n", current_cache_path.display());
            } else {
                println!("路径不能为空，保持原路径不变。\n");
            }
            continue;
        }

        // 如果不是内部命令，当作 URL 执行查询
        run_query(input, &current_cache_path).await;
        println!("\n-----------------------------\n");
    }
}
