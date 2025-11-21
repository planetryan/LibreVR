mod session;
mod tracking;
mod metrics;
mod output;

use std::time::{Duration, Instant};
use session::VrSession;
use tracking::TrackingCollector;
use metrics::SessionMetrics;
use output::DataExporter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("vive pro 2 ikerketa framework");
    println!("================================\n");

    // saioa sortu
    let mut vr_session = VrSession::new()?;
    
    // tracking eta metrikak hasieratu
    let mut tracker = TrackingCollector::new();
    let mut metrics = SessionMetrics::new();
    let start_time = Instant::now();

    println!("10 segunduko datuak biltzen...\n");

    // loop nagusia
    vr_session.run_loop(Duration::from_secs(10), |session, time| {
        let timestamp_ms = start_time.elapsed().as_millis() as u64;
        
        // sentsoreen datuak bildu
        let frame = tracker.collect_frame(
            &session.stage,
            &session.hand_space_left,
            &session.hand_space_right,
            time,
            timestamp_ms,
        )?;

        metrics.add_frame(frame.clone());

        // 30 frame bakoitzean erakutsi
        if tracker.get_frame_count() % 30 == 0 {
            tracker.print_live_stats(&frame);
        }

        Ok(true)  // jarraitu
    })?;

    // metrikak bukatu
    let duration = start_time.elapsed().as_secs_f32();
    metrics.finalize(duration, tracker.get_total_drift());

    // emaitzak erakutsi
    DataExporter::print_report(&metrics);

    // gorde
    println!("\ndatuak gordetzen...");
    let _json_file = DataExporter::save_json(&metrics)?; // TODO:: parsoak
    let csv_file = DataExporter::save_csv(&metrics)?;
    DataExporter::generate_python_script(&csv_file)?;

    println!("\nguztia prest");
    println!("python script-a exekutatu grafikak ikusteko: python3 analyze_tracking.py");

    Ok(())
}