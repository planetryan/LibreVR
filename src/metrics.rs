use serde::{Serialize, Deserialize};

// frame bakoitzeko sentsoreen datuak
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensorFrame {
    pub timestamp_ms: u64,
    pub head_position: [f32; 3],
    pub head_orientation: [f32; 4],  // quaternion
    pub left_controller_pos: Option<[f32; 3]>,
    pub right_controller_pos: Option<[f32; 3]>,
    pub angular_velocity: [f32; 3],
    pub linear_velocity: [f32; 3],
}

// saio osoaren metrikak
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetrics {
    pub session_id: String,
    pub start_time: String,
    pub duration_secs: f32,
    pub total_frames: usize,
    pub dropped_frames: u32,
    pub avg_fps: f32,
    pub position_drift_cm: f32,
    pub frames: Vec<SensorFrame>,
}

impl SessionMetrics {
    pub fn new() -> Self {
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            start_time: chrono::Local::now().to_rfc3339(),
            duration_secs: 0.0,
            total_frames: 0,
            dropped_frames: 0,
            avg_fps: 0.0,
            position_drift_cm: 0.0,
            frames: Vec::new(),
        }
    }

    pub fn add_frame(&mut self, frame: SensorFrame) {
        self.frames.push(frame);
        self.total_frames = self.frames.len();
    }

    pub fn finalize(&mut self, duration_secs: f32, drift_cm: f32) {
        self.duration_secs = duration_secs;
        self.position_drift_cm = drift_cm;
        self.avg_fps = self.total_frames as f32 / duration_secs;
    }

    pub fn print_summary(&self) {
        println!("\n=== ikerketa saioa amaitua ===");
        println!("saio id: {}", self.session_id);
        println!("iraupena: {:.1}s", self.duration_secs);
        println!("frame kopurua: {}", self.total_frames);
        println!("batez besteko fps: {:.1}", self.avg_fps);
        println!("posizioa drift: {:.2} cm", self.position_drift_cm);
        
        if self.total_frames > 0 {
            let first = &self.frames[0];
            let last = &self.frames[self.total_frames - 1];
            
            let total_movement = (
                (last.head_position[0] - first.head_position[0]).powi(2) +
                (last.head_position[1] - first.head_position[1]).powi(2) +
                (last.head_position[2] - first.head_position[2]).powi(2)
            ).sqrt();
            
            println!("guztizko mugimendua: {:.2} m", total_movement);
        }
    }

    pub fn calculate_statistics(&self) -> Statistics {
        if self.frames.is_empty() {
            return Statistics::default();
        }

        let mut max_speed = 0.0f32;
        let mut avg_speed = 0.0f32;
        let mut max_angular = 0.0f32;

        for frame in &self.frames {
            let speed = (
                frame.linear_velocity[0].powi(2) +
                frame.linear_velocity[1].powi(2) +
                frame.linear_velocity[2].powi(2)
            ).sqrt();

            let angular = (
                frame.angular_velocity[0].powi(2) +
                frame.angular_velocity[1].powi(2) +
                frame.angular_velocity[2].powi(2)
            ).sqrt();

            max_speed = max_speed.max(speed);
            avg_speed += speed;
            max_angular = max_angular.max(angular);
        }

        avg_speed /= self.frames.len() as f32;

        Statistics {
            max_linear_speed: max_speed,
            avg_linear_speed: avg_speed,
            max_angular_speed: max_angular,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Statistics {
    pub max_linear_speed: f32,
    pub avg_linear_speed: f32,
    pub max_angular_speed: f32,
}