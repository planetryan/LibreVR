use openxr as xr;
use std::time::{Duration, Instant};
use ash::vk::Handle;

pub struct VrSession {
    pub xr_instance: xr::Instance,
    pub system: xr::SystemId, // egutia
    pub session: xr::Session<xr::Vulkan>, //egutia
    pub frame_wait: xr::FrameWaiter,
    pub frame_stream: xr::FrameStream<xr::Vulkan>,
    pub stage: xr::Space,
    pub action_set: xr::ActionSet, // egutia
    pub hand_space_left: xr::Space,
    pub hand_space_right: xr::Space,
}

impl VrSession {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        println!("xr sistema hasieratzen...");

        let entry = unsafe { xr::Entry::load()? };
        let _extensions = entry.enumerate_extensions()?; // TODO:: egutia !!!

        let mut enabled = xr::ExtensionSet::default();
        enabled.khr_vulkan_enable = true;

        let xr_instance = entry.create_instance(
            &xr::ApplicationInfo {
                application_name: "vive pro 2 driver",
                application_version: 1,
                engine_name: "ikerketa framework",
                engine_version: 1,
                api_version: xr::Version::new(1, 0, 0),
            },
            &enabled,
            &[],
        )?;

        let instance_props = xr_instance.properties()?;
        println!(
            "runtime: {} {}.{}.{}",
            instance_props.runtime_name,
            instance_props.runtime_version.major(),
            instance_props.runtime_version.minor(),
            instance_props.runtime_version.patch(),
        );

        let system = xr_instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;

        let view_config_views = xr_instance.enumerate_view_configuration_views(
            system,
            xr::ViewConfigurationType::PRIMARY_STEREO,
        )?;

        println!("pantaila config:");
        for (i, view) in view_config_views.iter().enumerate() {
            println!(
                "  begi {}: {}x{}",
                i,
                view.recommended_image_rect_width,
                view.recommended_image_rect_height
            );
        }

        let vk_instance = Self::create_vulkan()?;

        let (session, frame_wait, frame_stream) = unsafe {
            xr_instance.create_session::<xr::Vulkan>(
                system,
                &xr::vulkan::SessionCreateInfo {
                    instance: vk_instance.handle().as_raw() as _,
                    physical_device: std::ptr::null_mut(),
                    device: std::ptr::null_mut(),
                    queue_family_index: 0,
                    queue_index: 0,
                },
            )?
        };

        let stage = session.create_reference_space(
            xr::ReferenceSpaceType::STAGE,
            xr::Posef::IDENTITY,
        )?;

        let action_set = xr_instance.create_action_set("input", "input", 0)?;

        let hand_pose = action_set.create_action::<xr::Posef>(
            "hand_pose",
            "hand pose",
            &[],
        )?;

        session.attach_action_sets(&[&action_set])?;

        let hand_space_left = hand_pose.create_space(
            session.clone(),
            xr::Path::NULL,
            xr::Posef::IDENTITY,
        )?;

        let hand_space_right = hand_pose.create_space(
            session.clone(),
            xr::Path::NULL,
            xr::Posef::IDENTITY,
        )?;

        println!("xr saioa prest\n");

        Ok(Self {
            xr_instance,
            system,
            session,
            frame_wait,
            frame_stream,
            stage,
            action_set,
            hand_space_left,
            hand_space_right,
        })
    }

    fn create_vulkan() -> Result<ash::Instance, Box<dyn std::error::Error>> {
        let entry = unsafe { ash::Entry::load()? };
        let app_info = ash::vk::ApplicationInfo::default()
            .application_name(std::ffi::CStr::from_bytes_with_nul(b"vr\0").unwrap())
            .api_version(ash::vk::make_api_version(0, 1, 1, 0));

        let create_info = ash::vk::InstanceCreateInfo::default().application_info(&app_info);

        let instance = unsafe { entry.create_instance(&create_info, None)? };
        Ok(instance)
    }

    pub fn run_loop<F>(
        &mut self,
        duration: Duration,
        mut callback: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnMut(&mut Self, xr::Time) -> Result<bool, Box<dyn std::error::Error>>,
    {
        let mut event_storage = xr::EventDataBuffer::new();
        let end_time = Instant::now() + duration;

        loop {
            if Instant::now() > end_time {
                break;
            }

            while let Some(event) = self.xr_instance.poll_event(&mut event_storage)? {
                if let xr::Event::SessionStateChanged(e) = event {
                    if e.state() == xr::SessionState::EXITING
                        || e.state() == xr::SessionState::LOSS_PENDING
                    {
                        return Ok(());
                    }
                }
            }

            let frame_state = self.frame_wait.wait()?;
            self.frame_stream.begin()?;

            if !frame_state.should_render {
                self.frame_stream.end(
                    frame_state.predicted_display_time,
                    xr::EnvironmentBlendMode::OPAQUE,
                    &[],
                )?;
                continue;
            }

            let should_continue = callback(self, frame_state.predicted_display_time)?;

            if !should_continue {
                self.frame_stream.end(
                    frame_state.predicted_display_time,
                    xr::EnvironmentBlendMode::OPAQUE,
                    &[],
                )?;
                break;
            }

            self.frame_stream.end(
                frame_state.predicted_display_time,
                xr::EnvironmentBlendMode::OPAQUE,
                &[],
            )?;
        }

        Ok(())
    }
}
