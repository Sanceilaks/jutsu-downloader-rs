use futures::StreamExt;
use kuchiki::traits::TendrilSink;
use std::io::{stdin, BufRead, Read, Write};
use std::ops::Index;
use std::{cmp::min, io::stdout};
use tokio::io::AsyncWriteExt;

use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;

fn pause() {
    let mut stdout = stdout();
    stdout.write(b"Press Enter to continue...").unwrap();
    stdout.flush().unwrap();
    stdin().read(&mut [0]).unwrap();
}

pub async fn download_file(client: &Client, url: &str, path: &str) -> Result<(), String> {
    // Reqwest setup
    let res = client
        .get(url)
        .send()
        .await
        .or(Err(format!("Failed to GET from '{}'", &url)))?;
    let total_size = res
        .content_length()
        .ok_or(format!("Failed to get content length from '{}'", &url))?;

    // Indicatif setup
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .unwrap()
        .progress_chars("#>-"));
    pb.set_message(format!("Downloading {}", url));

    // download chunks
    let mut file = tokio::fs::File::create(path)
        .await
        .or(Err(format!("Failed to create file '{}'", path)))?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.or(Err(format!("Error while downloading file")))?;
        file.write_all(&chunk)
            .await
            .or(Err(format!("Error while writing to file")))?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        pb.set_position(new);
    }

    pb.finish_with_message(format!("Downloaded {} to {}", url, path));
    return Ok(());
}

struct Video {
    url: String,
    resolution: String,
}

#[tokio::main]
async fn main() {
    let mut url = String::new();
    stdin().lock().read_line(&mut url).unwrap();

    let webclient = reqwest::Client::builder().
        user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/99.0.4844.51 Safari/537.36")
        .http2_keep_alive_while_idle(true)
        .http2_keep_alive_interval(std::time::Duration::from_secs(60))
        .tcp_keepalive(std::time::Duration::from_secs(60))
        .build().unwrap();

    println!("Establishing connection...");
    let jutsu_connection = webclient.get(&url).send().await.unwrap();

    println!("Downloading webpage...");
    let content = jutsu_connection.text().await.unwrap();

    let html = kuchiki::parse_html().one(content.to_owned());

    let sources: Vec<Video> = html
        .select("#my-player > source")
        .unwrap()
        .map(|e| Video {
            url: e.attributes.borrow().get("src").unwrap().to_owned(),
            resolution: e.attributes.borrow().get("res").unwrap().to_owned(),
        })
        .collect();
    
    println!("Select resolution:");

    for (i, x) in sources.iter().enumerate() {
        println!("{}: {}", i + 1, x.resolution);
    }

    let mut selection = String::new();
    stdin().lock().read_line(&mut selection).unwrap();
    let selection: usize = selection.trim().parse().unwrap();

    let source = &sources[selection - 1];

    println!("Downloading video...");
    download_file(&webclient, &source.url, "output.mp4")
        .await
        .unwrap();
}
