use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
// TODO: GET BSD AND LINUX KERNEL APIs WORKING !

#[cfg(target_os = "linux")]
use std::os::unix::io::AsRawFd;

#[cfg(target_os = "linux")]
use nix::libc;

// gailuaren informazioa
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub vendor_id: u16,
    pub product_id: u16,
    pub device_path: String,
}

// kernel mailako api
pub struct KernelApi {
    device_file: Option<File>,
    device_info: DeviceInfo,
}

impl KernelApi {
    // gailua ireki
    pub fn open(device_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(device_path)?;

        println!("gailua irekita: {}", device_path);

        Ok(Self {
            device_file: Some(file),
            device_info: DeviceInfo {
                vendor_id: 0x0BB4,  // htc
                product_id: 0x0000,  // vive pro 2 (aldatu behar)
                device_path: device_path.to_string(),
            },
        })
    }

    // sentsoreen datuak irakurri kernel-etik
    pub fn read_sensors(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        if let Some(ref mut file) = self.device_file {
            let mut buffer = vec![0u8; 64];  // ohiko imu frame tamaina
            file.read_exact(&mut buffer)?;
            Ok(buffer)
        } else {
            Err("gailua ez dago irekita".into())
        }
    }

    // komando bat bidali gailura
    pub fn send_command(&mut self, command: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(ref mut file) = self.device_file {
            file.write_all(command)?;
            Ok(())
        } else {
            Err("gailua ez dago irekita".into())
        }
    }

    #[cfg(target_os = "linux")]
    // ioctl (input/output control) deiak kernel kontrolatzaileari
    pub fn ioctl(&self, request: u64, data: &mut [u8]) -> Result<i32, Box<dyn std::error::Error>> {
        if let Some(ref file) = self.device_file {
            let fd = file.as_raw_fd();
            let result = unsafe {
                libc::ioctl(fd, request as libc::c_ulong, data.as_mut_ptr())
            };
            
            if result < 0 {
                Err(format!("ioctl huts egin du: {}", result).into())
            } else {
                Ok(result)
            }
        } else {
            Err("gailua ez dago irekita".into())
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn ioctl(&self, _request: u64, _data: &mut [u8]) -> Result<i32, Box<dyn std::error::Error>> {
        Err("ioctl linux-en bakarrik".into())
    }

    // gailuaren informazioa lortu
    pub fn get_device_info(&self) -> &DeviceInfo {
        &self.device_info
    }

    // gailua itxi
    pub fn close(&mut self) {
        self.device_file = None;
        println!("gailua itxita");
    }
}

impl Drop for KernelApi {
    fn drop(&mut self) {
        self.close();
    }
}

// usb bidezko komunikazioa (libusb erabiliz)
// oharra: normalean ez da behar openxr erabiltzen denean
pub mod usb {
    use std::time::Duration;

    pub struct UsbDevice {
        context: Option<()>,  // libusb context hemen joango litzateke
    }

    impl UsbDevice {
        pub fn find_vive_devices() -> Result<Vec<String>, Box<dyn std::error::Error>> {
            // hau normalean libusb-rekin egingo litzateke
            // baina openxr erabiltzean ez da beharrezkoa
            println!("usb gailuak bilatzen...");
            
            // adibide baterako:
            // lsusb | grep -i htc exekutatuko litzateke
            
            Ok(vec![])
        }

        pub fn open(vendor_id: u16, product_id: u16) -> Result<Self, Box<dyn std::error::Error>> {
            println!("usb gailua irekitzen: {:04x}:{:04x}", vendor_id, product_id);
            
            Ok(Self {
                context: None,
            })
        }

        pub fn read_bulk(&mut self, endpoint: u8, size: usize, timeout: Duration) 
            -> Result<Vec<u8>, Box<dyn std::error::Error>> 
        {
            // bulk transferentzia irakurri
            let _timeout_ms = timeout.as_millis();
            println!("bulk irakurtzen endpoint {}", endpoint);
            Ok(vec![0; size])
        }

        pub fn write_bulk(&mut self, endpoint: u8, data: &[u8], timeout: Duration) 
            -> Result<usize, Box<dyn std::error::Error>> 
        {
            let _timeout_ms = timeout.as_millis();
            println!("bulk idazten endpoint {}: {} byte", endpoint, data.len());
            Ok(data.len())
        }

        pub fn control_transfer(
            &mut self,
            request_type: u8,
            request: u8,
            value: u16,
            index: u16,
            data: &mut [u8],
            timeout: Duration,
        ) -> Result<usize, Box<dyn std::error::Error>> {
            let _timeout_ms = timeout.as_millis();
            println!("control transfer: req={:#04x} val={:#06x}", request, value);
            Ok(0)
        }
    }
}

// helper funtzioak
pub mod helpers {
    // byte array hex formatuan inprimatu
    pub fn print_hex(label: &str, data: &[u8]) {
        print!("{}: ", label);
        for byte in data {
            print!("{:02x} ", byte);
        }
        println!();
    }

    // quaternion euler angeluetara bihurtu (debugging-erako)
    pub fn quaternion_to_euler(q: [f32; 4]) -> [f32; 3] {
        let [x, y, z, w] = q;
        
        // roll (x-axis)
        let sinr_cosp = 2.0 * (w * x + y * z);
        let cosr_cosp = 1.0 - 2.0 * (x * x + y * y);
        let roll = sinr_cosp.atan2(cosr_cosp);

        // pitch (y-axis)
        let sinp = 2.0 * (w * y - z * x);
        let pitch = if sinp.abs() >= 1.0 {
            std::f32::consts::PI / 2.0 * sinp.signum()
        } else {
            sinp.asin()
        };

        // yaw (z-axis)
        let siny_cosp = 2.0 * (w * z + x * y);
        let cosy_cosp = 1.0 - 2.0 * (y * y + z * z);
        let yaw = siny_cosp.atan2(cosy_cosp);

        [roll, pitch, yaw]
    }

    // euler angeluak gradutan
    pub fn to_degrees(radians: [f32; 3]) -> [f32; 3] {
        [
            radians[0].to_degrees(),
            radians[1].to_degrees(),
            radians[2].to_degrees(),
        ]
    }
}