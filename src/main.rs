use reqwest::blocking::Client;
use scraper::{Html,Selector};
use regex::Regex;
use std::path::Path;
use std::fs;
use url;
use reqwest::header::HeaderMap;
use std::fs::File;
use std::thread;
use image::open;
use image::ImageFormat;
use jpeg_to_pdf::JpegToPdf;
use std::io::BufWriter;

fn get_request_headers(url: &str) -> Result<HeaderMap, url::ParseError> {
    let domain = url::Url::parse(url)?.host_str().unwrap().to_string();
    let mut headers = HeaderMap::new();
    headers.insert("Accept", "image/png,image/svg+xml,image/*;q=0.8,video/*;q=0.8,*/*;q=0.5".parse().unwrap());
    headers.insert("Accept-Encoding", "gzip, deflate, br".parse().unwrap());
    headers.insert("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_6) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/13.1.2 Safari/605.1.15".parse().unwrap());
    headers.insert("Host", domain.parse().unwrap());
    headers.insert("Accept-Language", "en-ca".parse().unwrap());
    headers.insert("Referer", "https://manganelo.com/".parse().unwrap());
    headers.insert("Connection", "keep-alive".parse().unwrap());
    Ok(headers)
}

fn download_image(name: &str,url: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let headers = get_request_headers(url)?;
    let mut response = client.get(url).headers(headers).send()?;
    if response.status().is_success() {
        let mut file = File::create(name)?;
        response.copy_to(&mut file)?;
    }
    let image_file = open(name)?;
    image_file.save_with_format(name, ImageFormat::Jpeg)?;
    Ok(())
}

fn page_links(url: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let client = Client::new();
    let res = client.get(url).send()?;
    let body = res.text()?;
    let document = Html::parse_document(&body);
    let selector = Selector::parse("div.container-chapter-reader").unwrap();
    let div = document.select(&selector).next().unwrap();
    let selector = Selector::parse("img").unwrap();
    let page_urls = div
        .select(&selector)
        .map(|img| img.value().attr("data-src").unwrap().to_string())
        .collect();
    return Ok(page_urls);
}

fn download_manga(name: &str, url:&str) -> Result<(), Box<dyn std::error::Error>> {
    let pages = page_links(url)?.clone();
    let cur_path = std::env::current_dir()?;
    let path = format!("{}/{}", std::env::current_dir()?.display(), name);
    if !Path::new(&path).exists() {
        fs::create_dir(&path)?;
    }
    std::env::set_current_dir(&path)?;

    let mut handles = vec![];

    for (i, page) in pages.iter().enumerate() {
        let page_name = format!("{}.jpg", i);
        let page_url = page.clone();
        let handle = thread::spawn(move || {
            download_image(&page_name, &page_url).unwrap_or_else(|e| {
                eprintln!("Failed to download {}: {}", page_url, e);
            });
        });
        handles.push(handle);
    }
    for handle in handles {
        handle.join().unwrap();
    }
    let outfile = fs::File::create(name.to_string() + ".pdf")?;
    let mut pdf = JpegToPdf::new();
    for i in 0..pages.len() {
        let image_path = format!("{}.jpg", i);
        match fs::read(&image_path) {
            Ok(image_data) => {
                pdf = pdf.add_image(image_data);
            }
            Err(err) => {
                eprintln!("Failed to read image {}: {}", image_path, err);
            }
        }
    }
    pdf.create_pdf(&mut BufWriter::new(outfile))?;
    let pdf_path = format!("{}/{}.pdf", std::env::current_dir()?.display(), name);
    fs::copy(pdf_path, format!("{}/{}.pdf", cur_path.display(), name))?;
    fs::remove_dir_all(&path)?;
    std::env::set_current_dir(&cur_path)?;
    Ok(())
}

fn chapter_links(url: &str) -> Result<Vec<(String, String)>, reqwest::Error> {
    let client = Client::new();
    let res = client.get(url).send()?;
    let body = res.text()?;
    let document = Html::parse_document(&body);
    let selector = Selector::parse("a.chapter-name.text-nowrap").unwrap();
    let links = document
        .select(&selector)
        .map(|a| (a.inner_html().trim().to_string(), a.value().attr("href").unwrap().to_string()))
        .collect();
    return Ok(links);
}

fn sort_chapters(chapters: Vec<(String, String)>) -> Vec<(String, String)> {
    let re = Regex::new(r"Chapter (\d+(?:\.\d+)?)").unwrap();
    let mut chapters = chapters;
    chapters.sort_by(|a, b| {
        let a_num = re.captures(&a.0).and_then(|cap| cap.get(1)).map_or(f64::INFINITY, |m| m.as_str().parse::<f64>().unwrap_or(f64::INFINITY));
        let b_num = re.captures(&b.0).and_then(|cap| cap.get(1)).map_or(f64::INFINITY, |m| m.as_str().parse::<f64>().unwrap_or(f64::INFINITY));
        a_num.partial_cmp(&b_num).unwrap()
    });
    chapters
}

fn main() -> Result<(), Box<dyn std::error::Error>>{
    println!("Enter the URL of the manga from https://ww8.manganelo.tv : ");
    let mut url = String::new();
    std::io::stdin().read_line(&mut url).unwrap();
    let url = url.trim();    
    let domain = url::Url::parse(url).unwrap().host_str().unwrap().to_string();
    let host_url = "https://".to_string() + &domain;
    let chapters = chapter_links(url).unwrap();
    let chapters: Vec<(String, String)> = chapters.into_iter().filter(|(k, _)| k.contains("Chapter")).collect();
    let chapters = sort_chapters(chapters);
    for (name, link) in chapters {
        println!("{}: {}", name, link);
        let link = host_url.clone() + &link;
        download_manga(&name, &link)?;
    }

    Ok(())
}