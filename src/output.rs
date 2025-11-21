use crate::metrics::SessionMetrics;
use std::fs::File;
use std::io::Write;

pub struct DataExporter;

impl DataExporter {
    // json formatuan gorde
    pub fn save_json(metrics: &SessionMetrics) -> Result<String, Box<dyn std::error::Error>> {
        let filename = format!(
            "vr_tracking_{}.json",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );

        let json = serde_json::to_string_pretty(metrics)?;
        let mut file = File::create(&filename)?;
        file.write_all(json.as_bytes())?;

        println!("json gordeta: {}", filename);
        Ok(filename)
    }

    // csv formatuan gorde (python analisirakoa errazagoa)
    pub fn save_csv(metrics: &SessionMetrics) -> Result<String, Box<dyn std::error::Error>> {
        let filename = format!(
            "vr_tracking_{}.csv",
            chrono::Local::now().format("%Y%m%d_%H%M%S")
        );

        let mut file = File::create(&filename)?;
        
        // goiburua
        writeln!(file, "timestamp_ms,pos_x,pos_y,pos_z,ori_x,ori_y,ori_z,ori_w,vel_x,vel_y,vel_z,angvel_x,angvel_y,angvel_z")?;

        // frame bakoitzeko lerroa
        for frame in &metrics.frames {
            writeln!(
                file,
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
                frame.timestamp_ms,
                frame.head_position[0],
                frame.head_position[1],
                frame.head_position[2],
                frame.head_orientation[0],
                frame.head_orientation[1],
                frame.head_orientation[2],
                frame.head_orientation[3],
                frame.linear_velocity[0],
                frame.linear_velocity[1],
                frame.linear_velocity[2],
                frame.angular_velocity[0],
                frame.angular_velocity[1],
                frame.angular_velocity[2],
            )?;
        }

        println!("csv gordeta: {}", filename);
        Ok(filename)
    }

    // txosten labur bat kontsolan
    pub fn print_report(metrics: &SessionMetrics) {
        metrics.print_summary();
        
        let stats = metrics.calculate_statistics();
        
        println!("\n=== estatistikak ===");
        println!("gehienezko abiadura: {:.3} m/s", stats.max_linear_speed);
        println!("batez besteko abiadura: {:.3} m/s", stats.avg_linear_speed);
        println!("gehienezko biraketa: {:.3} rad/s", stats.max_angular_speed);
    }

    // python sortu analisirakoa
    pub fn generate_python_script(csv_filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let script = format!(r#"#!/usr/bin/env python3
# python script automatikoki sortua datuak aztertzeko

import pandas as pd
import matplotlib.pyplot as plt
import numpy as np

# csv kargatu
df = pd.read_csv("{}")

# denbora segundotan
df['time_s'] = df['timestamp_ms'] / 1000.0

# figura sortu
fig, axes = plt.subplots(2, 2, figsize=(12, 10))

# posizioa denboraren arabera
axes[0, 0].plot(df['time_s'], df['pos_x'], label='x')
axes[0, 0].plot(df['time_s'], df['pos_y'], label='y')
axes[0, 0].plot(df['time_s'], df['pos_z'], label='z')
axes[0, 0].set_xlabel('denbora (s)')
axes[0, 0].set_ylabel('posizioa (m)')
axes[0, 0].set_title('buruaren posizioa')
axes[0, 0].legend()
axes[0, 0].grid(True)

# abiadura
speed = np.sqrt(df['vel_x']**2 + df['vel_y']**2 + df['vel_z']**2)
axes[0, 1].plot(df['time_s'], speed)
axes[0, 1].set_xlabel('denbora (s)')
axes[0, 1].set_ylabel('abiadura (m/s)')
axes[0, 1].set_title('abiadura lineala')
axes[0, 1].grid(True)

# biraketa abiadura
ang_speed = np.sqrt(df['angvel_x']**2 + df['angvel_y']**2 + df['angvel_z']**2)
axes[1, 0].plot(df['time_s'], ang_speed)
axes[1, 0].set_xlabel('denbora (s)')
axes[1, 0].set_ylabel('abiadura angeluarra (rad/s)')
axes[1, 0].set_title('biraketa abiadura')
axes[1, 0].grid(True)

# 3d ibilbidea
ax = fig.add_subplot(2, 2, 4, projection='3d')
ax.plot(df['pos_x'], df['pos_y'], df['pos_z'])
ax.set_xlabel('x (m)')
ax.set_ylabel('y (m)')
ax.set_zlabel('z (m)')
ax.set_title('buruaren ibilbidea 3d-n')

plt.tight_layout()
plt.savefig('vr_analysis.png', dpi=300)
print("grafika gordeta: vr_analysis.png")
plt.show()
"#, csv_filename);

        let script_name = "analyze_tracking.py";
        let mut file = File::create(script_name)?;
        file.write_all(script.as_bytes())?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(script_name)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(script_name, perms)?;
        }

        println!("python script sortua: {}", script_name);
        println!("exekutatu: python3 {}", script_name);

        Ok(())
    }
}