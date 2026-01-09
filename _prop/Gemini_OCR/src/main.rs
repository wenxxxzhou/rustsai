// GLM-4.7
// === 导入部分 ===
use anyhow::{Context, Result};
use base64::Engine;
use clap::Parser;
use dotenvy::dotenv;
use lopdf::Document;
use reqwest::Client;
use serde::Deserialize;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
// 不再需要 urlencoding

// === 1. 定义命令行参数结构体 ===
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// 输入文件路径（图片或 PDF）
    #[arg(short, long)]
    input: PathBuf,

    /// 结果存放目录
    #[arg(short, long)]
    output_dir: PathBuf,
}

// === 2. 定义 API 响应结构体 ===
#[derive(Debug, Deserialize)]
struct BaiduOcrResponse {
    #[serde(rename = "words_result")]
    words_result: Option<Vec<WordResult>>,
    // 修复：error_code 通常是整数
    #[serde(rename = "error_code")]
    error_code: Option<i64>,
    #[serde(rename = "error_msg")]
    error_msg: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WordResult {
    words: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    #[serde(rename = "access_token")]
    access_token: String,
}

// === 3. 核心客户端逻辑 ===
struct BaiduOcrClient {
    api_key: String,
    secret_key: String,
    access_token: Option<String>,
    client: Client,
}

impl BaiduOcrClient {
    fn new(api_key: String, secret_key: String) -> Self {
        Self {
            api_key,
            secret_key,
            access_token: None,
            client: Client::new(),
        }
    }

    // 获取 Access Token
    async fn get_access_token(&mut self) -> Result<String> {
        if let Some(token) = &self.access_token {
            return Ok(token.clone());
        }

        let url = format!(
            "https://aip.baidubce.com/oauth/2.0/token?grant_type=client_credentials&client_id={}&client_secret={}",
            self.api_key, self.secret_key
        );

        let resp: TokenResponse = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to send token request")?
            .json::<TokenResponse>()
            .await
            .context("Failed to parse token response")?;

        self.access_token = Some(resp.access_token.clone());
        Ok(resp.access_token)
    }

    // 识别单页内容
    async fn recognize_page(
        &mut self,
        file_data: Vec<u8>,
        is_pdf: bool,
        page_num: usize,
    ) -> Result<String> {
        let token = self.get_access_token().await?;

        // 1. Base64 编码
        let base64_str = base64::engine::general_purpose::STANDARD.encode(&file_data);

        // 2. 不需要手动 URL 编码！reqwest .form() 会自动处理

        let url = format!(
            "https://aip.baidubce.com/rest/2.0/ocr/v1/accurate_basic?access_token={}",
            token
        );

        // 声明 page_num_str 的生命周期
        #[allow(unused_assignments)]
        let mut page_num_str = String::new();

        let mut params = Vec::new();
        if is_pdf {
            // 直接传入 base64 字符串切片
            params.push(("pdf_file", base64_str.as_str()));

            page_num_str = page_num.to_string();
            params.push(("pdf_file_num", page_num_str.as_str()));
        } else {
            // 直接传入 base64 字符串切片
            params.push(("image", base64_str.as_str()));
        }

        let resp: BaiduOcrResponse = self
            .client
            .post(&url)
            .form(&params) // reqwest 会自动对 value 进行 URL 编码
            .send()
            .await
            .context("Failed to send OCR request")?
            .json::<BaiduOcrResponse>()
            .await
            .context("Failed to parse OCR response")?;

        // 检查 API 业务错误
        if let Some(code) = resp.error_code {
            anyhow::bail!(
                "Baidu API Error {}: {}",
                code,
                resp.error_msg.unwrap_or("Unknown error".to_string())
            );
        }

        let words = resp
            .words_result
            .context("No words found in response")?
            .iter()
            .map(|w| w.words.clone())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(words)
    }
}

// 获取 PDF 总页数
fn get_pdf_page_count(path: &Path) -> Result<usize> {
    let doc = Document::load(path).context("Failed to load PDF to check page count")?;
    Ok(doc.get_pages().len())
}

// === 4. 程序入口 ===
#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();

    let cli = Cli::parse();

    if !cli.input.exists() {
        anyhow::bail!("Input file does not exist: {}", cli.input.display());
    }

    fs::create_dir_all(&cli.output_dir).context("Failed to create output directory")?;

    let api_key = env::var("BAIDU_OCR_API_KEY").context("Missing BAIDU_OCR_API_KEY")?;
    let secret_key = env::var("BAIDU_OCR_SECRET_KEY").context("Missing BAIDU_OCR_SECRET_KEY")?;
    let mut client = BaiduOcrClient::new(api_key, secret_key);

    let file_stem = cli
        .input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");

    let extension = cli
        .input
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    if extension == "pdf" {
        println!("Processing PDF file: {}", cli.input.display());
        let file_data = fs::read(&cli.input)?;
        let total_pages = get_pdf_page_count(&cli.input)?;
        println!("Total pages detected: {}", total_pages);

        for page_num in 1..=total_pages {
            print!(
                "[{}/{}] Recognizing page {}... ",
                page_num, total_pages, page_num
            );

            match client
                .recognize_page(file_data.clone(), true, page_num)
                .await
            {
                Ok(text) => {
                    let output_filename = format!("{}_page_{}.txt", file_stem, page_num);
                    let output_path = cli.output_dir.join(output_filename);

                    let mut file = File::create(&output_path)?;
                    file.write_all(text.as_bytes())?;

                    println!("Saved to {}", output_path.display());
                }
                Err(e) => {
                    println!("Error: {}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
        }
    } else {
        println!("Processing Image file: {}", cli.input.display());
        let file_data = fs::read(&cli.input)?;

        let output_filename = format!("{}.txt", file_stem);
        let output_path = cli.output_dir.join(output_filename);

        print!("Recognizing... ");
        match client.recognize_page(file_data, false, 1).await {
            Ok(text) => {
                let mut file = File::create(&output_path)?;
                file.write_all(text.as_bytes())?;
                println!("Saved to {}", output_path.display());
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }

    Ok(())
}

/*
 * ============================================================
 * CLI 使用说明 (Command Line Interface Usage)
 * ============================================================
 *
 * 编译项目:
 *   cargo build --release
 *
 * 基本运行格式:
 *   cargo run -- --input <文件路径> --output-dir <目录路径>
 *
 * 1. 识别图片文件:
 *    说明: 识别指定的图片，并将结果保存为 .txt 文件。
 *    示例:
 *      cargo run -- --input ./images/photo.jpg --output-dir ./results
 *    结果: 在 ./results 目录下生成 photo.txt
 *
 * 2. 识别 PDF 文件 (多页):
 *    说明: 自动检测 PDF 总页数，并循环识别每一页。
 *    示例:
 *      cargo run -- --input ./docs/manual.pdf --output-dir ./results
 *    结果: 在 ./results 目录下生成 manual_page_1.txt, manual_page_2.txt ...
 *
 * 3. 参数详解:
 *    --input, -i:   [必填] 指定要识别的文件路径 (支持 jpg, png, pdf 等)。
 *    --output-dir, -o: [必填] 指定识别结果的存放目录。如果目录不存在会自动创建。
 *    --help, -h:    查看帮助信息。
 *
 * 注意事项:
 *    - 请确保当前目录下存在 .env 文件，并正确配置了 BAIDU_OCR_API_KEY 和 BAIDU_OCR_SECRET_KEY。
 *    - 程序不会在控制台打印识别出的文字，只会打印处理进度。
 * ============================================================
 */
