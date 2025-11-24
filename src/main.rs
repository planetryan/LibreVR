mod session;
mod tracking;
mod metrics;
mod output;
mod vr_renderer;

use std::time::{Duration, Instant};
use session::VrSession;
use tracking::TrackingCollector;
use metrics::SessionMetrics;
use output::DataExporter;
use vr_renderer::VrRenderer;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO:
    // --3dof : enable orientation-only tracking
    // --video <path> : optional video file to play to the hmd (decoder not implemented here)
    let mut enable_3dof = false;
    let mut _video_path: Option<String> = None;
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--3dof" => enable_3dof = true,
            "--video" => {
                if i + 1 < args.len() {
                    _video_path = Some(args[i+1].clone());
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }

    println!("librevr starting...");
    println!("================================\n");

    // create xr + vulkan session
    let mut vr_session = VrSession::new()?;

    // create a renderer (vulkan context is moved/cloned as needed)
    // in real code use Arc for shared ownership
    let renderer = VrRenderer::new(vr_session.vk.clone()); // adjust ownership as needed

    // tracking and metrics
    let mut tracker = TrackingCollector::new();
    tracker.set_3dof(enable_3dof);
    let mut metrics = SessionMetrics::new();
    let start_time = Instant::now();

    println!("collecting 10 seconds of tracking data...\n");

    // run the main xr loop for 10 seconds
    vr_session.run_loop(Duration::from_secs(10), |session, time| {
        let timestamp_ms = start_time.elapsed().as_millis() as u64;

        // collect tracking frame
        let frame = tracker.collect_frame(
            &session.stage,
            &session.hand_space_left,
            &session.hand_space_right,
            time,
            timestamp_ms,
        )?;

        metrics.add_frame(frame.clone());

        if tracker.get_frame_count() % 30 == 0 {
            tracker.print_live_stats(&frame);
        }

        // TODO:
        // video playback integration point:
        // - decode next frame (ffmpeg or other)
        // - for each eye acquire swapchain image index
        // - call renderer.upload_frame_to_swapchain(...)
        // note: decoder and swapchain per-eye management are not implemented in this example.

        Ok(true) // continue running
    })?;

    // finalize metrics
    let duration = start_time.elapsed().as_secs_f32();
    metrics.finalize(duration, tracker.get_total_drift());

    // print and save results
    DataExporter::print_report(&metrics);

    println!("\nsaving data...");
    let _json_file = DataExporter::save_json(&metrics)?;
    let csv_file = DataExporter::save_csv(&metrics)?;
    DataExporter::generate_python_script(&csv_file)?;

    println!("\nall done");
    println!("run `python3 analyze_tracking.py` to view graphs");

    Ok(())
}