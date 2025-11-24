use openxr as xr;
use nalgebra::Vector3;
use crate::metrics::SensorFrame;

// tracking collector stores simple state and supports 3dof mode
pub struct TrackingCollector {
    last_position: Vector3<f32>,
    total_drift_cm: f32,
    frame_count: u64,
    // when true, only use orientation (no positional tracking)
    pub force_3dof: bool,
}

impl TrackingCollector {
    // create a new collector with default state
    pub fn new() -> Self {
        Self {
            last_position: Vector3::zeros(),
            total_drift_cm: 0.0,
            frame_count: 0,
            force_3dof: false,
        }
    }

    // enable or disable 3dof mode
    pub fn set_3dof(&mut self, on: bool) {
        self.force_3dof = on;
    }

    // collect a single sensor frame from openxr spaces
    // returns a sensor frame ready to store in metrics
    pub fn collect_frame(
        &mut self,
        stage: &xr::Space,
        hand_left: &xr::Space,
        hand_right: &xr::Space,
        time: xr::Time,
        timestamp_ms: u64,
    ) -> Result<SensorFrame, Box<dyn std::error::Error>> {

        // locate the head pose in the stage space
        let view_location = stage.locate(stage, time)?;

        let pos = view_location.pose.position;
        let ori = view_location.pose.orientation;

        // if 3dof is forced, zero out positional components
        let head_position = if self.force_3dof {
            [0.0f32, 0.0, 0.0]
        } else {
            [pos.x, pos.y, pos.z]
        };

        // velocity is not available here; set zeros for now
        let vel = [0.0f32, 0.0, 0.0];
        let ang_vel = [0.0f32, 0.0, 0.0];

        // update simple drift estimate using positional changes (if any)
        let current_pos = Vector3::new(head_position[0], head_position[1], head_position[2]);
        if self.last_position.norm() > 0.0001 {
            let drift = (current_pos - self.last_position).norm() * 100.0;
            self.total_drift_cm += drift;
        }
        self.last_position = current_pos;

        // try to read controller locations; ignore errors
        let left_pos = hand_left.locate(stage, time)
            .ok()
            .map(|loc| [loc.pose.position.x, loc.pose.position.y, loc.pose.position.z]);

        let right_pos = hand_right.locate(stage, time)
            .ok()
            .map(|loc| [loc.pose.position.x, loc.pose.position.y, loc.pose.position.z]);

        self.frame_count += 1;

        Ok(SensorFrame {
            timestamp_ms,
            head_position,
            head_orientation: [ori.x, ori.y, ori.z, ori.w],
            left_controller_pos: left_pos,
            right_controller_pos: right_pos,
            angular_velocity: ang_vel,
            linear_velocity: vel,
        })
    }

    // total drift in centimeters observed
    pub fn get_total_drift(&self) -> f32 {
        self.total_drift_cm
    }

    // number of frames collected so far
    pub fn get_frame_count(&self) -> u64 {
        self.frame_count
    }

    // print a compact live status line
    pub fn print_live_stats(&self, frame: &SensorFrame) {
        let speed = (
            frame.linear_velocity[0].powi(2) +
            frame.linear_velocity[1].powi(2) +
            frame.linear_velocity[2].powi(2)
        ).sqrt();

        println!(
            "frame {} | pos: [{:.3}, {:.3}, {:.3}] | speed: {:.3} m/s | drift: {:.2} cm",
            self.frame_count,
            frame.head_position[0],
            frame.head_position[1],
            frame.head_position[2],
            speed,
            self.total_drift_cm
        );
    }
}