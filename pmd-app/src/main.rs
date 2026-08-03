// SPDX-License-Identifier: AGPL-3.0-only

#![forbid(unsafe_code)]

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    let mut screenshot: Option<String> = None;
    let mut song: Option<String> = None;
    let mut at_seconds = 3.0_f32;
    let mut muted = [false; pmd_core::CHANNEL_COUNT];
    let mut list: Vec<String> = Vec::new();
    let mut loop_target: Option<u32> = None;
    let mut message = String::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--screenshot" => screenshot = Some(args.next().unwrap_or_default()),
            "--song" => song = args.next(),
            "--at" => {
                at_seconds = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(at_seconds)
            }
            "--mute" => {
                if let Some(channel) = args.next().and_then(|value| value.parse::<usize>().ok()) {
                    if let Some(slot) = muted.get_mut(channel) {
                        *slot = true;
                    }
                }
            }
            "--list" => {
                if let Some(paths) = args.next() {
                    list.extend(paths.split(',').map(str::to_owned));
                }
            }
            "--loops" => loop_target = args.next().and_then(|value| value.parse().ok()),
            "--message" => message = args.next().unwrap_or_default(),
            _ => {}
        }
    }

    if let Some(path) = screenshot {
        let path = if path.is_empty() {
            String::from("shot.png")
        } else {
            path
        };
        let result = match song {
            Some(song) => pmd_app::capture::write_song(&pmd_app::capture::SongCapture {
                song: std::path::Path::new(&song),
                output: &path,
                at_seconds,
                muted,
                list,
                list_cursor: 0,
                loop_target,
                message,
            }),
            None => pmd_app::capture::write_screenshot(&path),
        };
        match result {
            Ok(()) => {
                println!("wrote {path} at 640x400");
                return Ok(());
            }
            Err(error) => {
                eprintln!("screenshot failed: {error}");
                std::process::exit(1);
            }
        }
    }

    let scale = std::env::var("PMDMINI_SCALE")
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .filter(|s| *s >= 1 && *s <= 6)
        .unwrap_or(2);
    let (width, height) = pmd_app::fb::present_size(scale);

    let requested = std::env::var("PMDMINI_WINDOW")
        .ok()
        .and_then(|value| {
            let (w, h) = value.split_once(['x', 'X'])?;
            Some([w.trim().parse::<f32>().ok()?, h.trim().parse::<f32>().ok()?])
        })
        .unwrap_or([width as f32, height as f32]);

    let (min_width, min_height) = pmd_app::fb::present_size(1);

    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(requested)
            .with_min_inner_size([min_width as f32 / 2.0, min_height as f32 / 2.0])
            .with_resizable(true)
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "pmdmini",
        native_options,
        Box::new(|cc| Ok(Box::new(pmd_app::PmdApp::new(cc)))),
    )
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{prelude::wasm_bindgen, JsCast};

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
fn start() {
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();
    wasm_bindgen_futures::spawn_local(async {
        let window = web_sys::window().expect("browser window is available");
        let document = window.document().expect("browser document is available");
        let canvas = document
            .get_element_by_id("pmdmini_canvas")
            .expect("index.html contains #pmdmini_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#pmdmini_canvas is a canvas");

        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| Ok(Box::new(pmd_app::PmdApp::new(cc)))),
            )
            .await
            .expect("eframe web runner starts");
    });
}
