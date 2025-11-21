# lighthouse tracking system - technical reference

### to whomever reads:
#### thank you for taking the time for reading this document, it is somewhat extensive and I wrote it in like a few days whenever I could during my free time, I want this to be something me and other people can refer to for SteamVR/HTC IR lighthouse tracking system
#### most of this comes from reverse engineering work by the community, libsurvive, lighthouse-fpga, and random forum posts, I just put it here so I would not have to go digging for it again if I need information

---

## lighthouse v1

lighthouse v1 uses rotating IR lasers plus sync flashes. base station emits omnidirectional LED pulse (sync), then two laser sweeps (horizontal and vertical) paint across the room. photodiodes on your headset measure time between sync and sweep hit = angle. two base stations + trig = 6dof position.

runs at 120hz effective rate per tracked object.

**base station components:**
- led array for sync pulses
- two rotating drums with ir lasers and cylindrical lenses
- horizontal sweep rotor (spins around vertical axis)
- vertical sweep rotor (spins around horizontal axis)
- photodiode for multi-station sync

### sync pulse timing

sync pulses happen every 8,333 microseconds (120hz). pulse width encodes which base station, which axis, and a data bit for ootx:

```
pulse width ranges (microseconds):

59-72     station a, horizontal, ootx bit 0
73-86     station a, horizontal, ootx bit 1  
87-100    station a, vertical, ootx bit 0
101-114   station a, vertical, ootx bit 1
115-128   station b, horizontal, ootx bit 0
129-139   station b, horizontal, ootx bit 1
```

decoding is more straightforward - subtract 59 and divide by 13 to get bucket index 0-5. anything outside this range is either a skip pulse or noise.

**important note:** sync led pulses have slow decay due to capacitance. the comparator circuit might see the pulse as 10-20us longer than the actual led on-time. calibrate for this or you'll decode wrong buckets.

### sweep timing to angle conversion

after sync pulse, laser sweep arrives somewhere between 1,222 and 6,777 microseconds later. this directly maps to angle:

```
angle (radians) = (delta_time_us - 4000) * (pi / 8333)

where:
  delta_time_us = sweep_time - sync_time
  4000 = center offset (0 degrees)
  8333 = full cycle period (180 degree rotation)

example:
  sync at 1000us, sweep at 5000us
  delta = 4000us
  angle = (4000 - 4000) * (pi/8333) = 0 radians (straight ahead)
  
  sync at 1000us, sweep at 3000us  
  delta = 2000us
  angle = (2000 - 4000) * (pi/8333) = -0.754 rad = -43 degrees
```

working range is roughly -60 to +60 degrees (pi/3 radians).

### frame structure

complete frame takes 8.333ms:

```
t=0ms:     sync pulse (62-139us duration)
t=0.06ms:  horizontal sweep starts (if not skip)
t=4.16ms:  second sync pulse
t=4.22ms:  vertical sweep starts (if not skip)
t=8.33ms:  repeat
```

with two base stations, they use tdma (time division multiple access):
- frame 0: station a horizontal
- frame 1: station a vertical
- frame 2: station b horizontal  
- frame 3: station b vertical
- repeat

this is why v1 is limited to 2 base stations max adding more would need more time slots and drop the frame rate.

### ootx data stream

that data bit in the sync pulse? it's a serial bitstream at 120 bits/sec. accumulate 264 bits (33 bytes) for a complete packet. takes about 2.2 seconds to receive one full packet.

**packet structure:**

```
offset | field                | size  | description
-------|---------------------|-------|---------------------------
0-1    | protocol version    | u16   | currently 0x0006
2-3    | firmware version    | u16   | base station firmware
4-7    | id                  | u32   | unique base station id
8-25   | fcal calibration    | 18b   | factory calibration data
26-28  | accelerometer cal   | 3b    | accel calibration  
29     | mode                | u8    | a/b/c station mode
30-31  | fault flags         | u16   | error status
32     | crc                 | u8    | packet checksum
```

**fcal data is critical** - contains correction factors for angle calculation. each axis (horizontal and vertical) stores:
- phase: angle offset in radians
- tilt: tilt correction factor
- curve: lens curvature correction
- gibphase: gibbous distortion phase
- gibmag: gibbous distortion magnitude

without fcal you get 1-3 degree errors. with fcal you get submillimeter accuracy. always decode ootx before trying serious tracking.

---

## lighthouse v2

### changes

v2 ditched omnidirectional sync pulses entirely. instead, **timing data is encoded directly into the laser sweep beam** using high-speed modulation. way faster

**improvements over v1:**
- single rotor instead of two = cheaper, quieter, more reliable
- no sync flash = simpler photodiode circuits
- supports 4+ base stations (16 channels max)
- faster rotation with better coverage

**downside:** decoding is way harder. needs fpga-level processing for real-time biphase mark code demodulation at 6mhz.

### rotor geometry

v2 base station has one rotor with two laser slits at +45 and -45 degrees:

```
        top view
        
     [+45 slit]  <- azimuth sweep
         /
        /
    ---*---  rotation axis
        \
         \
     [-45 slit]  <- elevation sweep
```

as rotor spins, first slit sweeps azimuth, then ~10ms later second slit sweeps elevation. both beams are modulated with timing data.

### beam modulation - biphase mark code

laser is modulated at 6mhz using biphase mark code (bmc), also called fm1 encoding.

**bmc rules:**
- always transition at bit boundary
- for bit 0: no transition in middle
- for bit 1: transition in middle

```
bit stream:  1    1    0    1    0    0
bmc output:  _-_-_-_____-____-_____
             ^   ^   ^   ^   ^   ^  (boundary transitions)
               ^   ^     ^          (mid-bit transitions for 1s)
```

each bit is 166 nanoseconds (1/6mhz). bmc is self-clocking so no separate clock recovery needed.

### lfsr pseudorandom sequences

the 6mhz bmc signal comes from a 17-bit linear feedback shift register (lfsr) that generates pseudo-random bit patterns.

**lfsr properties:**
- 17-bit register
- polynomial determines sequence (32 different polynomials used)
- period: 131,071 bits = 21.85ms at 6mhz
- each base station channel uses 2 polynomials (one for each ootx bit state)

**example polynomials (from bitcraze lighthouse-fpga):**
```
channel 1:  0x0001d258, 0x00017e04
channel 2:  0x0001ff6b, 0x00013f67
channel 3:  0x0001b9ee, 0x000198d1
...
channel 16: 0x00013750, 0x0001cb8d
```

base station channel = (poly_index / 2) + 1  
ootx bit = poly_index & 0x01

### sweep decoding process

when photodiode sees a sweep:

1. **envelope detection** - capture pulse start/end (typically 10-30 bits)
2. **bmc demodulation** - decode bmc to extract 17-bit lfsr data
3. **lfsr matching** - brute force search through 32 polynomials to find which generated this sequence
4. **offset calculation** - lfsr state tells you how far into the rotor cycle you are
5. **angle extraction** - convert offset to angle using rotor geometry

```
offset_time = lfsr_state / 6000000  // convert to seconds
angle = (offset_time / rotor_period) * 2*pi

rotor_period is ~20ms (varies by channel)
angle range: -60 to +60 degrees
```

### why v2 decoding needs fpga

6mhz modulation means 166ns per bit. typical microcontroller interrupt latency is 1-10 microseconds. you'd miss most of the signal. fpga can do parallel processing at 24-48mhz clock to capture and decode in real-time.

**fpga pipeline (conceptual):**
```
stage 1: envelope detector @ 24mhz
  input: photodiode signal
  output: pulse start, pulse end, duration

stage 2: bmc decoder @ 48mhz
  input: pulse envelope
  output: 17-bit lfsr data word

stage 3: lfsr matcher @ 24mhz  
  input: 17-bit word
  output: polynomial index (0-31) or invalid

stage 4: angle calculator
  input: polynomial index, timestamp
  output: sweep angle
```

bitcraze implemented this in spinalhdl for their lighthouse deck. check their repo for full verilog.

---

## vive / vive pro 2 usb architecture

### hardware topology

vive headsets use a link box to break out pc connections:

```
[pc] --usb3.0--> [link box] --proprietary cable--> [headset]
  |--displayport-->    |
  |--12v power---------|

link box functions:
- usb 3.0 hub
- displayport passthrough (with dsc for pro 2)
- power distribution
- sync signal conditioning
```

vive pro 2 needs displayport 1.4 with display stream compression (dsc) to hit 5k resolution at 120hz. raw bandwidth would be ~35gbps but dsc compresses to ~11gbps.

### usb device enumeration

when vive connects, multiple usb devices appear:

```
vendor:product ids:

original vive:
  28de:2000  - main hmd (tracking + sensors)
  28de:2012  - audio device
  28de:2102  - front camera

vive pro 2:
  0bb4:xxxx  - htc corporation vid
  (pid varies by generation)
```

### hid report structure

vive uses usb hid class for sensor communication. key report ids:

**report 0x20: imu data**
```c
struct imu_report {
    uint8_t report_id;         // 0x20
    uint16_t timestamp_hi;     // upper 16 bits
    uint8_t sequence;          // increments each sample
    
    struct {
        int16_t accel_x;       // +/-4g range
        int16_t accel_y;
        int16_t accel_z;
        int16_t gyro_x;        // +/-2000 dps range
        int16_t gyro_y;
        int16_t gyro_z;
        uint32_t timestamp_lo; // lower 32 bits at 48mhz
    } samples[3];              // 3 samples per report
};

// imu rate: 1000hz (1 sample per ms)
// usb report rate: ~333hz (3 samples per report)

// conversions:
accel_g = (raw / 32768.0) * 4.0
gyro_dps = (raw / 32768.0) * 2000.0
```

**report 0x21: photodiode light events**
```c
struct light_report {
    uint8_t report_id;         // 0x21
    uint16_t timestamp_hi;     // upper 16 bits
    uint8_t payload_size;      // num light events
    uint8_t flags;             // event type flags
    
    struct {
        uint8_t sensor_id;     // bits 7-3: sensor num (0-31)
                               // bits 2-0: reserved
        uint8_t timestamp_mid; 
        uint16_t timestamp_lo; // together = 24-bit timestamp
        uint8_t duration;      // pulse duration
    } events[];                // variable length
};

// timestamps are 48mhz clock ticks
// convert to microseconds: ticks / 48
```

this is the critical one for lighthouse tracking. each photodiode pulse generates an event with precise timing.

**report 0x24: button state**
```c
struct button_report {
    uint8_t report_id;         // 0x24
    uint8_t data_size;
    uint8_t buttons;           // button bitmap
    uint8_t trigger;           // analog 0-255
    int16_t trackpad_x;        // -32768 to 32767
    int16_t trackpad_y;
    uint32_t reserved;
};
```

### photodiode sensor array

vive headset has 32 photodiodes placed around the housing for 360-degree coverage. each is individually numbered and has a known position vector relative to headset origin.

**sensor circuit (simplified):**
```
[photodiode] -> [transimpedance amp] -> [high-pass filter] -> [comparator] -> [fpga]
     |               |                       |                     |
  ir photon      current->voltage        ac coupling          digital edge
  to current     gain ~100k ohm          cutoff ~20khz        with hysteresis
```

some design challenges to consider:
- balance sensitivity vs saturation in transimpedance stage
- high-pass removes dc (ambient light rejection)
- comparator needs hysteresis to avoid chatter
- typical response time: 10-80 microseconds from photon to digital edge

---

## sensor fusion pipeline

lighthouse gives absolute position at 120hz. imu gives orientation at 1000hz. combine them:

```
photodiode angles (120hz) -> position solver -> kalman filter -> 6dof pose
                                                      ^
      imu samples (1000hz) -> orientation int --------+

fusion algorithm:
1. imu provides prediction step (1000hz)
2. lighthouse provides correction step (120hz)  
3. kalman filter weights predictions based on uncertainty
4. handles occlusion by falling back to imu dead-reckoning
```

without sensor fusion you get 120hz jitter. with fusion you get smooth 1000hz pose updates.

---

## position solving algorithms

given multiple photodiode angles from multiple base stations, solve for 6dof pose. this is a perspective-n-point (pnp) problem.

**common algorithms:**

**epnp (efficient perspective-n-point):**
- express 3d sensor positions as weighted sum of 4 control points
- solve for control point positions
- fast, works with 4+ sensors

**levenberg-marquardt (mpfit):**
- iterative non-linear least squares
- minimize reprojection error
- slower but more accurate

**sba (sparse bundle adjustment):**
- graph optimization over multiple frames
- best accuracy but computationally expensive
- mainly used offline for calibration

libsurvive implements all three. epnp for real-time, sba for calibration.

---

## calibration data

steamvr stores base station calibration in `lighthouse/lighthousedb.json`:

```json
{
  "lighthouse0": {
    "id": "0x12345678",
    "mode": "A",
    "position": [1.234, 2.345, 0.567],
    "rotation": [0.707, 0.0, 0.707, 0.0],
    "fcal": {
      "0": {
        "phase": 0.001234,
        "tilt": 0.000456,
        "curve": 0.000789,
        "gibphase": 1.234,
        "gibmag": 0.001
      },
      "1": { ... }
    }
  }
}
```

room setup wizard determines base station positions/orientations and stores them here. without this, tracking is relative only.

---

## timing accuracy requirements

at 60 degree fov, 1 microsecond timing error = ~0.02 degrees angular error.

```
angular velocity = 180 deg / 8333us = 0.0216 deg/us

at 3m distance:
  1us error -> 0.0216 deg error -> ~1mm position error

for sub-millimeter accuracy need sub-microsecond timing
```

this is why everything uses 48mhz hardware counters and hardware interrupt timestamping. software timestamps would be too jittery.

---

## building custom tracked devices

minimum hardware requirements:

**for v1:**
- 4+ ir photodiodes (bpw34 or similar, 850-950nm sensitive)
- transimpedance amp + comparator per photodiode
- microcontroller with 48mhz timer and fast interrupts
- usb or wireless for data transmission

**for v2:**
- same photodiodes
- fpga for 6mhz bmc decoding (mcu can't keep up)
- more complex but supports more base stations

**software pipeline:**
```
1. interrupt on photodiode edge
2. timestamp with hardware counter
3. measure pulse duration
4. classify as sync vs sweep
5. decode sync metadata
6. calculate angles from sweep timing
7. collect angles from multiple sensors
8. solve for 6dof pose
9. fuse with imu if available
10. output tracking data
```

challenges:
- interrupt latency must be <10us for v1, <1us for v2
- clock drift between sensors (need shared time reference)
- sync pulse disambiguation during occlusion
- geometric calibration (sensor positions relative to device origin)

---

## debugging tools

**libsurvive visualizer:**
```bash
survive-cli --visualizer
```
shows live tracking visualization, base station positions, sensor hits

**usb packet capture:**
```bash
# linux
sudo modprobe usbmon
sudo cat /sys/kernel/debug/usb/usbmon/1u > capture.txt

# or use wireshark
wireshark -i usbmon1 -f "usb.addr == X"
```

**lighthouse console (v2 base stations):**
```bash
# connect to base station usb port
screen /dev/ttyACM0 115200

# available commands (undocumented):
> help
> status  
> channel N
> ootx
```

---

## common issues and fixes

**problem: tracking is jittery**
- check for reflective surfaces (mirrors, glass, shiny floors)
- ensure base stations are solidly mounted (vibration = noise)
- verify fcal calibration data is loaded
- increase kalman filter weight on imu prediction

**problem: frequent occlusion**
- add more photodiodes in occluded areas
- use sensor fusion to maintain pose during brief occlusions
- consider adding third base station (v2 only)

**problem: drift over time**
- imu bias drift (calibrate imu regularly)
- base station position shift (remount more securely)
- temperature-dependent clock drift (use temperature-compensated oscillator)

**problem: v2 decoding fails**
- fpga clock not fast enough (need 24mhz minimum, 48mhz better)
- lfsr polynomial table incomplete (need all 32 polynomials)
- bmc decoder timing off (verify against known good capture)

---

## open source projects and references

**libsurvive (most complete v1/v2 implementation):**
- github: https://github.com/cntools/libsurvive
- full tracking stack in c
- supports vive, index, tundra trackers
- includes calibration, posers, sensor fusion
- actively maintained

**lighthouse-fpga (bitcraze v2 fpga decoder):**
- github: https://github.com/bitcraze/lighthouse-fpga
- spinalhdl implementation of v2 decoding
- runs on crazyflie lighthouse deck
- includes full bmc decoder and lfsr matcher
- great reference for hardware implementation

**lighthouseRedox (nairol's early v1 reverse engineering):**
- github: https://github.com/nairol/LighthouseRedox
- original timing analysis and protocol docs
- historical but comprehensive v1 documentation
- includes sync pulse analysis and ootx decoder

**openhmd (open source hmd drivers):**
- github: https://github.com/OpenHMD/OpenHMD
- includes vive driver with lighthouse support
- lighter weight than libsurvive
- good for embedded applications

**academic papers:**

"lighthouse: a lightweight 3d optical tracking system" (valve, 2016)
- original lighthouse design paper
- covers v1 timing and architecture
- doesn't cover v2 (wasn't released yet)

"reverse engineering the lighthouse tracking system" (kevin lynagh, 2016)
- blog post series on v1 reverse engineering
- circuit analysis of photodiode frontend
- sync pulse timing measurements
- https://kevinlynagh.com/notes/lighthouse/

**useful forum threads:**

reddit r/vive technical discussions
- lots of early reverse engineering discussions
- people posting usb captures and timing analysis

bitcraze forums
- detailed v2 decoding discussions
- fpga implementation details
- lfsr polynomial discovery process

**related tools:**

wireshark usb dissectors for vive hid reports
steamvr driver developer docs (limited, mostly nda'd)
openvr/openxr api specifications