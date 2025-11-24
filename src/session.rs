use openxr as xr;
use std::time::{Duration, Instant};
use ash::{vk, Entry as AshEntry, Instance as AshInstance};
use ash::version::{EntryV1_0, InstanceV1_0};
use std::ffi::CStr;

pub struct VulkanContext {
    pub entry: AshEntry,
    pub instance: AshInstance,
    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue_family_index: u32,
    pub queue: vk::Queue,
}

pub struct SwapchainInfo {
    pub handle: xr::Swapchain<xr::Vulkan>,
    pub images_count: usize,
    // other per-swapchain bookkeeping
}

pub struct VrSession {
    pub xr_instance: xr::Instance,
    pub system: xr::SystemId,
    pub session: xr::Session<xr::Vulkan>,
    pub frame_wait: xr::FrameWaiter,
    pub frame_stream: xr::FrameStream<xr::Vulkan>,
    pub stage: xr::Space,
    pub action_set: xr::ActionSet,
    pub hand_space_left: xr::Space,
    pub hand_space_right: xr::Space,

    // our Vulkan pieces (kept so we can operate on swapchain images)
    pub vk: VulkanContext,
    // swapchain handles per view
    pub swapchains: Vec<SwapchainInfo>,
}

impl VrSession {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        println!("xr sistema hasieratzen...");

        let entry = unsafe { xr::Entry::load()? };
        // enumerate runtime extensions so we can enable them if needed
        let available_exts = entry.enumerate_extensions()?;
        println!("OpenXR runtime reports {} extensions", available_exts.len());
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

        // Create a Vulkan context that is compatible with OpenXR runtime
        let vk_ctx = Self::create_vulkan_for_openxr(&xr_instance)?;

        // Create the OpenXR session: pass Vulkan handles
        let (session, frame_wait, frame_stream) = unsafe {
            xr_instance.create_session::<xr::Vulkan>(
                system,
                &xr::vulkan::SessionCreateInfo {
                    instance: vk_ctx.instance.handle().as_raw() as _,
                    physical_device: vk_ctx.physical_device.as_raw() as _,
                    device: vk_ctx.device.handle().as_raw() as _,
                    queue_family_index: vk_ctx.queue_family_index,
                    queue_index: 0,
                },
            )?
        };

        // create reference spaces and actions as before
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

        // create swapchains for each view
        let mut swapchains = Vec::new();
        let formats = unsafe { session.enumerate_swapchain_formats::<xr::Vulkan>()? };
        // pick a format; prefer VK_FORMAT_R8G8B8A8_UNORM
        let preferred_vk_format = vk::Format::R8G8B8A8_UNORM.as_raw() as i64; // map to i64 for XR
        let selected_format = formats.iter()
            .find(|f| **f == preferred_vk_format)
            .cloned()
            .unwrap_or(formats[0]);

        for view in &view_config_views {
            let (width, height) = (view.recommended_image_rect_width, view.recommended_image_rect_height);
            let swapchain = session.create_swapchain(&xr::SwapchainCreateInfo {
                usage: xr::SwapchainUsage::COLOR_ATTACHMENT | xr::SwapchainUsage::SAMPLED,
                format: selected_format,
                sample_count: view.recommended_swapchain_sample_count,
                width,
                height,
                face_count: 1,
                array_size: 1,
                mip_count: 1,
            })?;
            let images = swapchain.enumerate_images::<xr::Vulkan>()?;
            swapchains.push(SwapchainInfo {
                handle: swapchain,
                images_count: images.len(),
            });
        }

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
            vk: vk_ctx,
            swapchains,
        })
    }

    // Create a Vulkan instance/device suitable for OpenXR.
    // This is simplified; in production you must choose physical device and queue family carefully.
    fn create_vulkan_for_openxr(xr_instance: &xr::Instance) -> Result<VulkanContext, Box<dyn std::error::Error>> {
        // Create ash entry and instance
        let entry = unsafe { AshEntry::new()? };
        let app_name = CStr::from_bytes_with_nul(b"librevr\0").unwrap();
        let engine_name = CStr::from_bytes_with_nul(b"librevr_engine\0").unwrap();

        let app_info = vk::ApplicationInfo::builder()
            .application_name(app_name)
            .engine_name(engine_name)
            .api_version(vk::make_version(1, 1, 0));

        let create_info = vk::InstanceCreateInfo::builder().application_info(&app_info);
        let instance = unsafe { entry.create_instance(&create_info, None)? };

        // pick first physical device for simplicity
        let phys_devs = unsafe { instance.enumerate_physical_devices()? };
        let physical_device = phys_devs
            .get(0)
            .ok_or("No Vulkan physical devices available")?
            .to_owned();

        // find a queue family that supports graphics
        let queue_family_index = unsafe {
            instance.get_physical_device_queue_family_properties(physical_device)
                .iter()
                .enumerate()
                .find_map(|(i, q)| {
                    if q.queue_flags.contains(vk::QueueFlags::GRAPHICS) {
                        Some(i as u32)
                    } else {
                        None
                    }
                })
                .ok_or("No graphics queue family")?
        };

        let priority = [1.0f32];
        let queue_info = vk::DeviceQueueCreateInfo::builder()
            .queue_family_index(queue_family_index)
            .queue_priorities(&priority);

        let device_create_info = vk::DeviceCreateInfo::builder().queue_create_infos(std::slice::from_ref(&queue_info));
        let device = unsafe { instance.create_device(physical_device, &device_create_info, None)? };
        let queue = unsafe { device.get_device_queue(queue_family_index, 0) };

        Ok(VulkanContext {
            entry,
            instance,
            physical_device,
            device,
            queue_family_index,
            queue,
        })
    }

    // run_loop and other methods remain basically the same, but rendering must be performed:
    // - acquire swapchain image
    // - copy/upload the decoded video frame to the swapchain image
    // - release swapchain image
    // The renderer will live in a separate module (see src/vr_renderer.rs).
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