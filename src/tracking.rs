use openxr as xr;
use nalgebra::Vector3;
use crate::metrics::SensorFrame;

pub struct TrackingCollector {
    last_position: Vector3<f32>,
    total_drift_cm: f32,
    frame_count: u64,
}

impl TrackingCollector {
    pub fn new() -> Self {
        Self {
            last_position: Vector3::zeros(),
            total_drift_cm: 0.0,
            frame_count: 0,
        }
    }

    pub fn collect_frame(
        &mut self,
        stage: &xr::Space,
        hand_left: &xr::Space,
        hand_right: &xr::Space,
        time: xr::Time,
        timestamp_ms: u64,
    ) -> Result<SensorFrame, Box<dyn std::error::Error>> {
        
        let view_location = stage.locate(stage, time)?;
        
        let pos = view_location.pose.position;
        let ori = view_location.pose.orientation;
        
        // velocity not available in this openxr version, set to zero
        let vel = [0.0f32, 0.0, 0.0];
        let ang_vel = [0.0f32, 0.0, 0.0];

        let current_pos = Vector3::new(pos.x, pos.y, pos.z);
        if self.last_position.norm() > 0.0001 {
            let drift = (current_pos - self.last_position).norm() * 100.0;
            self.total_drift_cm += drift;
        }
        self.last_position = current_pos;

        let left_pos = hand_left.locate(stage, time)
            .ok()
            .map(|loc| [loc.pose.position.x, loc.pose.position.y, loc.pose.position.z]);
        
        let right_pos = hand_right.locate(stage, time)
            .ok()
            .map(|loc| [loc.pose.position.x, loc.pose.position.y, loc.pose.position.z]);

        self.frame_count += 1;

        Ok(SensorFrame {
            timestamp_ms,
            head_position: [pos.x, pos.y, pos.z],
            head_orientation: [ori.x, ori.y, ori.z, ori.w],
            left_controller_pos: left_pos,
            right_controller_pos: right_pos,
            angular_velocity: ang_vel,
            linear_velocity: vel,
        })
    }

    pub fn get_total_drift(&self) -> f32 {
        self.total_drift_cm
    }

    pub fn get_frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn print_live_stats(&self, frame: &SensorFrame) {
        let speed = (
            frame.linear_velocity[0].powi(2) + 
            frame.linear_velocity[1].powi(2) + 
            frame.linear_velocity[2].powi(2)
        ).sqrt();

        println!(
            "frame {} | pos: [{:.3}, {:.3}, {:.3}] | abiadura: {:.3} m/s | drift: {:.2} cm",
            self.frame_count,
            frame.head_position[0],
            frame.head_position[1],
            frame.head_position[2],
            speed,
            self.total_drift_cm
        );
    }
}