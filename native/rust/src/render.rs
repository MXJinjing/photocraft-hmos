//! NativeWindow ownership stays outside the upstream safe-Rust workspace.
use raw_window_handle::{
    DisplayHandle, HasDisplayHandle, HasWindowHandle, OhosNdkWindowHandle, WindowHandle,
};
use std::{ffi::c_void, ptr::NonNull, sync::Arc};

unsafe extern "C" {
    fn OH_NativeWindow_NativeObjectUnreference(object: *mut c_void) -> i32;
    fn pc_set_window_color_space(window: *mut c_void, p3: bool) -> i32;
}
/// The XComponent callback takes a native reference before sending this to the worker.
pub struct Window(pub usize);
impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: every Window owns one reference acquired by OnSurfaceCreated; it is
        // dropped only after all surfaces borrowing it have been destroyed.
        unsafe {
            OH_NativeWindow_NativeObjectUnreference(self.0 as *mut c_void);
        }
    }
}
impl HasDisplayHandle for Window {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, raw_window_handle::HandleError> {
        Ok(DisplayHandle::ohos())
    }
}
impl HasWindowHandle for Window {
    fn window_handle(&self) -> Result<WindowHandle<'_>, raw_window_handle::HandleError> {
        let ptr = NonNull::new(self.0 as *mut c_void)
            .ok_or(raw_window_handle::HandleError::Unavailable)?;
        // SAFETY: the borrowed handle cannot outlive the owned native reference.
        Ok(unsafe { WindowHandle::borrow_raw(OhosNdkWindowHandle::new(ptr).into()) })
    }
}
struct GpuRender {
    pub surface: Option<wgpu::Surface<'static>>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub renderer: Arc<egui::mutex::RwLock<egui_wgpu::Renderer>>,
    pending_free: Vec<egui::TextureId>,
    pub config: wgpu::SurfaceConfiguration,
    pub adapter: wgpu::Adapter,
    pub instance: wgpu::Instance,
    pub window: Option<std::sync::Arc<Window>>,
    pub color: crate::display_color::DisplayColor,
    health: photocraft_ui_egui::gpu_canvas::DeviceHealth,
}
impl GpuRender {
    pub fn new(window: Arc<Window>, w: u32, h: u32, request_p3: bool) -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::GL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        // Stage marks help diagnose emulator APP_INPUT_BLOCK vs GPU stalls: these
        // block_on calls run on the render worker, not the ArkTS input thread.
        crate::bridge::notify(0, "stage", "native GPU requesting adapter", &[]);
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        crate::bridge::notify(
            0,
            "stage",
            &format!(
                "native GPU adapter ready ({:?}/{:?})",
                adapter.get_info().backend,
                adapter.get_info().device_type
            ),
            &[],
        );
        crate::bridge::notify(0, "stage", "native GPU requesting device", &[]);
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("PhotoCraft For HarmonyOS"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        crate::bridge::notify(0, "stage", "native GPU device ready", &[]);
        let mut config = surface
            .get_default_config(&adapter, w.max(1), h.max(1))
            .ok_or("no native surface configuration")?;
        let caps = surface.get_capabilities(&adapter);
        config.format = caps
            .formats
            .iter()
            .copied()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Bgra8Unorm
                )
            })
            .unwrap_or(config.format);
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 1;
        surface.configure(&device, &config);
        let p3 = request_p3 && declare_color(&window, true).is_ok();
        if !p3 {
            declare_color(&window, false)?;
            if request_p3 {
                crate::bridge::notify(0, "stage", "Native P3 rejected; using tagged sRGB", &[]);
            }
        }
        let color = crate::display_color::DisplayColor::new(p3)?;
        crate::bridge::notify(
            0,
            "stage",
            if p3 {
                "Display P3 output (system managed)"
            } else {
                "sRGB output (system managed)"
            },
            &[],
        );
        let renderer = Arc::new(egui::mutex::RwLock::new(egui_wgpu::Renderer::new(
            &device,
            config.format,
            Default::default(),
        )));
        let health = photocraft_ui_egui::gpu_canvas::DeviceHealth::watch(&device);
        Ok(Self {
            surface: Some(surface),
            device,
            queue,
            renderer,
            pending_free: Vec::new(),
            config,
            adapter,
            instance,
            window: Some(window),
            color,
            health,
        })
    }
    pub fn egui_render_state(&self) -> egui_wgpu::RenderState {
        egui_wgpu::RenderState {
            adapter: self.adapter.clone(),
            available_adapters: vec![self.adapter.clone()],
            instance: self.instance.clone(),
            device: self.device.clone(),
            queue: self.queue.clone(),
            target_format: self.config.format,
            renderer: self.renderer.clone(),
            surface_config: egui_wgpu::SurfaceConfig {
                present_mode: self.config.present_mode,
                desired_maximum_frame_latency: Some(self.config.desired_maximum_frame_latency),
            },
        }
    }
    pub fn resize(&mut self, w: u32, h: u32) -> Result<(), String> {
        if w == 0 || h == 0 {
            return Ok(());
        }
        self.config.width = w;
        self.config.height = h;
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
            if let Some(window) = &self.window {
                declare_color(window, self.color.p3)?;
            }
        }
        Ok(())
    }
    pub fn detach(&mut self) {
        // 先释放借用窗口的 surface，再释放原生窗口引用；GPU device 保留供下次绑定使用。
        self.surface = None;
        self.window = None;
    }
    pub fn bind(&mut self, window: Arc<Window>, w: u32, h: u32) -> Result<(), String> {
        let surface = self
            .instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        self.surface = Some(surface);
        self.window = Some(window);
        self.resize(w.max(1), h.max(1))
    }
    pub fn reset_surface(&mut self) -> Result<(), String> {
        let window = self.window.clone().ok_or("native window unavailable")?;
        self.surface = Some(
            self.instance
                .create_surface(window)
                .map_err(|e| e.to_string())?,
        );
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
            if let Some(window) = &self.window {
                declare_color(window, self.color.p3)?;
            }
        }
        Ok(())
    }
    pub fn paint(
        &mut self,
        ctx: &egui::Context,
        mut output: egui::FullOutput,
    ) -> Result<bool, String> {
        // The host already mirrored these images. Consume egui's delta container
        // before driver calls so unwinding cannot trigger its debug Drop assertion.
        let mut texture_updates = std::mem::take(&mut output.textures_delta.set);
        self.pending_free.extend(output.textures_delta.free.drain());
        output.textures_delta.clear();
        if let Some(fault) = self.health.fault() {
            return Err(fault.to_string());
        }
        // Keep the shared renderer alive independently of `self`: a lost surface can be
        // rebound while this guard owns the callback resources.
        let shared_renderer = self.renderer.clone();
        let mut renderer = shared_renderer.write();
        for (id, deltas) in &mut texture_updates {
            for delta in deltas {
                self.color.texture(ctx, *id, &mut delta.image);
                renderer.update_texture(&self.device, &self.queue, *id, delta);
            }
        }
        let Some(surface) = &self.surface else {
            return Ok(false);
        };
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated => {
                if let Some(surface) = &self.surface {
                    surface.configure(&self.device, &self.config);
                    if let Some(window) = &self.window {
                        declare_color(window, self.color.p3)?;
                    }
                }
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.reset_surface()?;
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("native surface validation failed".into());
            }
        };
        let mut jobs = ctx.tessellate(output.shapes, output.pixels_per_point);
        if self.color.p3 {
            for job in &mut jobs {
                if let egui::epaint::Primitive::Mesh(mesh) = &mut job.primitive {
                    for vertex in &mut mesh.vertices {
                        vertex.color = self.color.ui_color(vertex.color);
                    }
                }
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: output.pixels_per_point,
        };
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let mut commands =
            renderer.update_buffers(&self.device, &self.queue, &mut encoder, &jobs, &screen);
        let view = frame.texture.create_view(&Default::default());
        {
            let attachments = [Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.15,
                        g: 0.15,
                        b: 0.15,
                        a: 1.,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })];
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("PhotoCraft UI"),
                color_attachments: &attachments,
                ..Default::default()
            });
            renderer.render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        commands.push(encoder.finish());
        self.queue.submit(commands);
        self.queue.present(frame);
        for id in self.pending_free.drain(..) {
            renderer.free_texture(&id);
        }
        if let Some(fault) = self.health.fault() {
            return Err(fault.to_string());
        }
        Ok(true)
    }
}

/// Own the native reference independently of GPU lifetime. A panic during GPU
/// creation, drawing or teardown cannot invalidate the host's bookkeeping.
pub struct Render {
    pub width: u32,
    pub height: u32,
    window: Option<Arc<Window>>,
    gpu: Option<GpuRender>,
    pub recovery: crate::recovery::Recovery,
    cpu_color: crate::display_color::DisplayColor,
    request_p3: bool,
    #[cfg(feature = "host-recovery-test")]
    injected_fault: Option<String>,
}
impl Render {
    pub fn new(request_p3: bool) -> Result<Self, String> {
        Ok(Self {
            width: 0,
            height: 0,
            window: None,
            gpu: None,
            recovery: Default::default(),
            cpu_color: crate::display_color::DisplayColor::new(false)?,
            request_p3,
            #[cfg(feature = "host-recovery-test")]
            injected_fault: None,
        })
    }
    pub fn attached(&self) -> bool {
        self.window.is_some()
    }
    pub fn color(&self) -> &crate::display_color::DisplayColor {
        self.gpu.as_ref().map_or(&self.cpu_color, |gpu| &gpu.color)
    }
    pub fn fault(&self) -> Option<String> {
        #[cfg(feature = "host-recovery-test")]
        if let Some(fault) = &self.injected_fault {
            return Some(fault.clone());
        }
        self.gpu
            .as_ref()
            .and_then(|gpu| gpu.health.fault())
            .map(|fault| fault.to_string())
    }
    #[cfg(feature = "host-recovery-test")]
    pub fn inject_fault(&mut self) {
        if let Some(gpu) = &self.gpu {
            gpu.health.mark(photocraft_ui_egui::gpu_canvas::Fault::Lost(
                "injected native device loss".into(),
            ));
        } else {
            self.injected_fault = Some("injected native device loss (CPU producer)".into());
        }
    }
    pub fn egui_render_state(&self) -> Option<egui_wgpu::RenderState> {
        self.gpu.as_ref().map(GpuRender::egui_render_state)
    }
    pub fn observe_canvas(&mut self, app: &photocraft_ui_egui::PhotocraftApp) {
        // GpuCanvas installs the device's single callback pair. Keep that same
        // flag in the host even after upstream removes the canvas on a fault.
        if let Some(health) = app.gpu_health()
            && let Some(gpu) = self.gpu.as_mut()
        {
            gpu.health = health;
        }
    }
    pub fn bind(&mut self, window: Window, w: u32, h: u32) -> Result<(), String> {
        if self.recovery.halted {
            return Ok(());
        }
        self.window = Some(Arc::new(window));
        self.width = w;
        self.height = h;
        let window = self.window.clone().ok_or("native window unavailable")?;
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.bind(window, w, h)?;
        } else {
            self.gpu = Some(GpuRender::new(window, w, h, self.request_p3)?);
        }
        Ok(())
    }
    pub fn resize(&mut self, w: u32, h: u32) -> Result<(), String> {
        self.width = w;
        self.height = h;
        if self.recovery.halted {
            return Ok(());
        }
        if w == 0 || h == 0 {
            return Ok(());
        }
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.resize(w, h)
        } else {
            Ok(())
        }
    }
    pub fn detach(&mut self) {
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.detach();
        }
        self.window = None;
    }
    pub fn halt(&mut self) {
        if let Some(gpu) = self.gpu.take() {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(gpu)));
        }
        self.recovery.failed();
    }
    pub fn paint(
        &mut self,
        ctx: &egui::Context,
        output: egui::FullOutput,
    ) -> Result<bool, String> {
        if self.recovery.halted || !self.attached() || self.width == 0 || self.height == 0 {
            return Ok(false);
        }
        let gpu = self.gpu.as_mut().ok_or("native GPU renderer unavailable")?;
        gpu.paint(ctx, output)
    }
}

fn declare_color(window: &Window, p3: bool) -> Result<(), String> {
    // SAFETY: Window owns a live reference; all declarations and EGL operations run
    // serially on the render worker. The C++ boundary uses the SDK enum definitions.
    let result = unsafe { pc_set_window_color_space(window.0 as *mut c_void, p3) };
    if result == 0 {
        Ok(())
    } else {
        Err(format!("native output color space rejected: {result}"))
    }
}
