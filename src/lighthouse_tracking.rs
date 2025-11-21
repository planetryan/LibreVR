use std::time::{Duration, Instant};
use nalgebra::{Vector3, Point3};

extern "C" {
    // fotodiodo pulse bat prozesatu
    // itzulera: 0=ezezaguna, 1=sync, 2=sweep
    fn lighthouse_process_pulse(
        pulse_start_us: u64,
        pulse_duration_us: u64,
        sensor_id: u64,
    ) -> u64;
    
    // uneko angeluak lortu (pointer [f32; 2]-ra: [horizontal, vertical])
    fn lighthouse_decode_angles() -> *const f32;
    
    // egoera garbitu
    fn lighthouse_reset_state();
}

// lighthouse base station bat (bi behar dira posizio 3d-rako)
#[derive(Debug, Clone)]
pub struct BaseStation {
    pub id: u8,
    pub position: Point3<f32>,
    pub orientation: nalgebra::UnitQuaternion<f32>,
}

impl BaseStation {
    pub fn new(id: u8, position: Point3<f32>) -> Self {
        Self {
            id,
            position,
            orientation: nalgebra::UnitQuaternion::identity(),
        }
    }
}

// lighthouse tracker nagusia
pub struct LighthouseTracker {
    base_stations: Vec<BaseStation>,
    sensors: Vec<SensorState>,
    start_time: Instant,
    frame_count: u64,
}

// sentsore bakoitzeko egoera
#[derive(Debug, Clone)]
struct SensorState {
    sensor_id: u8,
    last_angles: [f32; 2],  // [horizontal, vertical] radianak
    position: Option<Point3<f32>>,
}

impl LighthouseTracker {
    pub fn new(num_sensors: usize) -> Self {
        unsafe {
            lighthouse_reset_state();
        }
        
        let sensors = (0..num_sensors)
            .map(|i| SensorState {
                sensor_id: i as u8,
                last_angles: [0.0, 0.0],
                position: None,
            })
            .collect();
        
        Self {
            base_stations: Vec::new(),
            sensors,
            start_time: Instant::now(),
            frame_count: 0,
        }
    }
    
    // base station bat gehitu
    pub fn add_base_station(&mut self, station: BaseStation) {
        println!(
            "base station {} gehituta: pos=[{:.2}, {:.2}, {:.2}]",
            station.id,
            station.position.x,
            station.position.y,
            station.position.z
        );
        self.base_stations.push(station);
    }
    
    // fotodiodo pulse bat prozesatu
    // photodiode hardware-tik deituko litzateke (interrupt edo polling)
    pub fn process_photodiode_pulse(
        &mut self,
        sensor_id: u8,
        pulse_start: Duration,
        pulse_duration: Duration,
    ) -> PulseType {
        let pulse_start_us = pulse_start.as_micros() as u64;
        let pulse_duration_us = pulse_duration.as_micros() as u64;
        
        let result = unsafe {
            lighthouse_process_pulse(
                pulse_start_us,
                pulse_duration_us,
                sensor_id as u64,
            )
        };
        
        match result {
            1 => {
                // sync pulse - ez dago ezer gehiago egitekorik oraingoz
                PulseType::Sync
            }
            2 => {
                // sweep pulse - angeluak eguneratu
                self.update_sensor_angles(sensor_id);
                PulseType::Sweep
            }
            _ => PulseType::Unknown,
        }
    }
    
    // sentsore baten angeluak eguneratu asm-tik irakurrita
    fn update_sensor_angles(&mut self, sensor_id: u8) {
        let angles_ptr = unsafe { lighthouse_decode_angles() };
        let angles = unsafe { std::slice::from_raw_parts(angles_ptr, 2) };
        
        if let Some(sensor) = self.sensors.get_mut(sensor_id as usize) {
            sensor.last_angles = [angles[0], angles[1]];
            
            // bi base station badaude, 3d posizioa kalkulatu
            if self.base_stations.len() >= 2 {
                if let Some(pos) = self.triangulate_position(sensor) {
                    sensor.position = Some(pos);
                }
            }
        }
    }
    
    // sentsore baten 3d posizioa kalkulatu triangulazio bidez
    // bi base station-etik angeluak erabiliz
    fn triangulate_position(&self, sensor: &SensorState) -> Option<Point3<f32>> {
        if self.base_stations.len() < 2 {
            return None;
        }
        
        let [angle_h, angle_v] = sensor.last_angles;
        
        // base station 1-etik ray bat kalkulatu
        let bs1 = &self.base_stations[0];
        let ray1_dir = self.angle_to_ray_direction(angle_h, angle_v);
        let ray1 = Ray {
            origin: bs1.position,
            direction: bs1.orientation * ray1_dir,
        };
        
        // base station 2-tik ere berdina (praktikan, bi base station-ak
        // sweep desberdinak ikusten dira, baina sinplifikatzeko kasu honetan...)
        let bs2 = &self.base_stations[1];
        let ray2 = Ray {
            origin: bs2.position,
            direction: bs2.orientation * ray1_dir,
        };
        
        // bi ray-en intersekzio puntua kalkulatu (ray-ray closest point)
        self.ray_ray_closest_point(&ray1, &ray2)
    }
    
    // angeluak ray direction-era bihurtu
    fn angle_to_ray_direction(&self, angle_h: f32, angle_v: f32) -> Vector3<f32> {
        // horizontal angelua: x inguruan biraketa
        // vertical angelua: y inguruan biraketa
        
        let x = angle_h.sin() * angle_v.cos();
        let y = angle_v.sin();
        let z = -angle_h.cos() * angle_v.cos();
        
        Vector3::new(x, y, z).normalize()
    }
    
    // bi ray-en arteko puntu hurbilena
    fn ray_ray_closest_point(&self, ray1: &Ray, ray2: &Ray) -> Option<Point3<f32>> {
        let w0 = ray1.origin - ray2.origin;
        let a = ray1.direction.dot(&ray1.direction);
        let b = ray1.direction.dot(&ray2.direction);
        let c = ray2.direction.dot(&ray2.direction);
        let d = ray1.direction.dot(&w0);
        let e = ray2.direction.dot(&w0);
        
        let denom = a * c - b * b;
        if denom.abs() < 1e-6 {
            return None;  // paraleloak
        }
        
        let t1 = (b * e - c * d) / denom;
        let t2 = (a * e - b * d) / denom;
        
        let p1 = ray1.origin + ray1.direction * t1;
        let p2 = ray2.origin + ray2.direction * t2;
        
        // bi puntuen erdiko puntua itzuli
        Some(Point3::from((p1 + p2.coords) / 2.0))
    }
    
    // uneko tracking egoera lortu
    pub fn get_tracked_position(&self, sensor_id: u8) -> Option<Point3<f32>> {
        self.sensors
            .get(sensor_id as usize)
            .and_then(|s| s.position)
    }
    
    // debug info inprimatu
    pub fn print_status(&self) {
        println!("\n=== lighthouse tracking status ===");
        println!("base stations: {}", self.base_stations.len());
        println!("sensors: {}", self.sensors.len());
        println!("frames: {}", self.frame_count);
        
        for (i, sensor) in self.sensors.iter().enumerate() {
            if let Some(pos) = sensor.position {
                println!(
                    "  sensor {}: pos=[{:.3}, {:.3}, {:.3}] angles=[{:.3}, {:.3}]",
                    i,
                    pos.x, pos.y, pos.z,
                    sensor.last_angles[0].to_degrees(),
                    sensor.last_angles[1].to_degrees()
                );
            } else {
                println!("  sensor {}: ez dago posiziorik", i);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseType {
    Unknown,
    Sync,
    Sweep,
}

struct Ray {
    origin: Point3<f32>,
    direction: Vector3<f32>,
}