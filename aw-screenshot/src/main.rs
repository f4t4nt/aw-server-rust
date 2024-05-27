use clap::crate_version;
use clap::Parser;
use screenshots::{display_info::DisplayInfo, Screen};
use image::DynamicImage;
use std::error::Error;
use std::io::Cursor;
use chrono::{Duration, Utc};
use serde_json::{Map, Value};
use aw_models::Event;

#[derive(Parser)]
#[clap(version = crate_version!(), author = "Nishant Bhakar")]
struct Opts {
    #[clap(long, short, default_value = "false")]
    verbose: bool,

    #[clap(long, short, default_value = "localhost")]
    server: String,

    #[clap(long, short, default_value = "5666")]
    port: u16,

    #[clap(long)]
    auth_token: Option<String>,

    #[clap(long)]
    device_id: Option<String>,

    #[clap(long, short, default_value = "5")]
    time_interval: u16,
}

fn get_screenshots() -> Result<Vec<(DynamicImage, String, usize)>, Box<dyn Error>> {
    let mut screenshots = Vec::new();
    for (idx, display_info) in DisplayInfo::all()?.into_iter().enumerate() {
        println!("display_info {display_info:?}");
        let screen = Screen::new(&display_info);
        let image = screen.capture()?;
        let dynamic_image = DynamicImage::ImageRgba8(image);
        let time = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
        screenshots.push((dynamic_image, time, idx));
    }
    Ok(screenshots)
}

fn save_screenshots(screenshots: Vec<(DynamicImage, String, usize)>) -> Result<(), Box<dyn Error>> {
    for (dynamic_image, time, idx) in screenshots {
        dynamic_image.save_with_format(format!("target/screenshots/{}_{}.webp", time, idx), image::ImageFormat::WebP)?;
    }
    Ok(())
}

fn screenshots_to_events(screenshots: Vec<(DynamicImage, String, usize)>) -> Result<Vec<Event>, Box<dyn Error>> {
    let mut events = Vec::new();
    for (dynamic_image, _, idx) in screenshots {
        let mut data = Map::new();
        data.insert("format".to_string(), Value::String("webp".to_string()));
        data.insert("display".to_string(), Value::Number(idx.into()));

        // Save the image to a buffer
        let mut buffer = Cursor::new(Vec::new());
        dynamic_image.write_to(&mut buffer, image::ImageFormat::WebP)?;
        let blob = buffer.into_inner();

        events.push(Event {
            id: None,
            timestamp: Utc::now(),
            duration: Duration::seconds(0),
            data,
            blob_data: Some(blob),
        });
    }

    Ok(events)
}

async fn send_events_to_server(
    events: Vec<Event>,
    server: &str,
    port: u16,
    bucket_id: &str) -> Result<(), Box<dyn Error>> {
    let url = format!("http://{}:{}/api/0/buckets/{}/events", server, port, bucket_id);
    let client = reqwest::Client::new();
    let res = client.post(&url)
        .json(&events)
        .send()
        .await?;

    println!("Response: {}", res.status());

    Ok(())
}

#[tokio::main]
async fn main() {
    let opts = Opts::parse();

    loop {
        match get_screenshots() {
            Ok(screenshots) => {
                match save_screenshots(screenshots.clone()) {
                    Ok(_) => println!("Screenshots saved successfully."),
                    Err(e) => eprintln!("Failed to save screenshots: {}", e),
                }

                match screenshots_to_events(screenshots) {
                    Ok(events) => {
                       if let Err(err) = send_events_to_server(
                            events,
                            opts.server.as_str(),
                            opts.port,
                            "aw-watcher-screenshot_Razerator").await {
                            eprintln!("Failed to send events to server: {}", err);
                        } else {
                            println!("Converted screenshots to events successfully.")
                        }
                    },
                    Err(e) => eprintln!("Failed to convert screenshots to events: {}", e),
                }
            },
            Err(e) => eprintln!("Failed to get screenshots: {}", e),
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(opts.time_interval as u64)).await;
    }
}
