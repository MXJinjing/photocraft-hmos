//! NativeWindow ownership stays outside the upstream safe-Rust workspace.
use raw_window_handle::{
    DisplayHandle, HasDisplayHandle, HasWindowHandle, OhosNdkWindowHandle, WindowHandle,
};
use std::{collections::HashMap, ffi::c_void, ptr::NonNull, sync::Arc};

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
/// One native sub-window (floating document window): its own surface on the shared device.
struct Child {
    surface: Option<wgpu::Surface<'static>>,
    window: Arc<Window>,
    config: wgpu::SurfaceConfiguration,
}
/// What presenting one frame to a surface did.
enum Drawn {
    Presented,
    /// The surface needs `configure` again.
    Outdated,
    /// The surface must be recreated from its native window.
    Lost,
    Skipped,
}
struct GpuRender {
    children: HashMap<u32, Child>,
    failed: Vec<u32>,
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
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("PhotoCraft For HarmonyOS"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .map_err(|e| e.to_string())?;
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
            children: HashMap::new(),
            failed: Vec::new(),
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
    /// Present one egui frame to `surface`. Associated function so callers can borrow the
    /// surface's owner (root or child) and the device/queue/colour state at the same time.
    #[allow(clippy::too_many_arguments)]
    fn draw(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color: &mut crate::display_color::DisplayColor,
        renderer: &mut egui_wgpu::Renderer,
        ctx: &egui::Context,
        surface: &wgpu::Surface<'static>,
        config: &wgpu::SurfaceConfiguration,
        shapes: Vec<egui::epaint::ClippedShape>,
        pixels_per_point: f32,
    ) -> Result<Drawn, String> {
        let frame = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated => return Ok(Drawn::Outdated),
            wgpu::CurrentSurfaceTexture::Lost => return Ok(Drawn::Lost),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(Drawn::Skipped);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("native surface validation failed".into());
            }
        };
        let mut jobs = ctx.tessellate(shapes, pixels_per_point);
        if color.p3 {
            for job in &mut jobs {
                if let egui::epaint::Primitive::Mesh(mesh) = &mut job.primitive {
                    for vertex in &mut mesh.vertices {
                        vertex.color = color.ui_color(vertex.color);
                    }
                }
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [config.width, config.height],
            pixels_per_point,
        };
        let mut encoder = device.create_command_encoder(&Default::default());
        let mut commands = renderer.update_buffers(device, queue, &mut encoder, &jobs, &screen);
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
        // Submit per surface: egui_wgpu reuses its uniform/vertex buffers for every target.
        queue.submit(commands);
        queue.present(frame);
        Ok(Drawn::Presented)
    }

    pub fn attach_child(
        &mut self,
        key: u32,
        window: Arc<Window>,
        w: u32,
        h: u32,
    ) -> Result<(), String> {
        let surface = self
            .instance
            .create_surface(window.clone())
            .map_err(|e| e.to_string())?;
        let caps = surface.get_capabilities(&self.adapter);
        if !caps.formats.contains(&self.config.format) {
            return Err("sub-window surface format differs from the main window".into());
        }
        // Same format/present mode as the root: one egui_wgpu renderer serves every surface.
        let mut config = self.config.clone();
        config.width = w.max(1);
        config.height = h.max(1);
        surface.configure(&self.device, &config);
        declare_color(&window, self.color.p3)?;
        self.children.insert(
            key,
            Child {
                surface: Some(surface),
                window,
                config,
            },
        );
        Ok(())
    }

    pub fn resize_child(&mut self, key: u32, w: u32, h: u32) -> Result<(), String> {
        if w == 0 || h == 0 {
            return Ok(());
        }
        if let Some(child) = self.children.get_mut(&key) {
            child.config.width = w;
            child.config.height = h;
            if let Some(surface) = &child.surface {
                surface.configure(&self.device, &child.config);
                declare_color(&child.window, self.color.p3)?;
            }
        }
        Ok(())
    }

    pub fn paint(
        &mut self,
        ctx: &egui::Context,
        mut output: egui::FullOutput,
        frames: Vec<crate::viewports::ChildFrame>,
    ) -> Result<bool, String> {
        // The host already mirrored these images. Consume egui's delta container
        // before driver calls so unwinding cannot trigger its debug Drop assertion.
        // `textures_delta` already merges start-up, sub-window and root deltas in pass order.
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
        if self.surface.is_none() {
            return Ok(false);
        }
        // Sub-windows first: a failure there drops only that window, never the root.
        for frame in frames {
            let Some(child) = self.children.get_mut(&frame.key) else {
                continue;
            };
            let Some(surface) = &child.surface else {
                continue;
            };
            let drawn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Self::draw(
                    &self.device,
                    &self.queue,
                    &mut self.color,
                    &mut renderer,
                    ctx,
                    surface,
                    &child.config,
                    frame.shapes,
                    frame.pixels_per_point,
                )
            }))
            .unwrap_or_else(|_| Err("sub-window drawing panicked".into()));
            match drawn {
                Ok(Drawn::Outdated) => {
                    surface.configure(&self.device, &child.config);
                }
                Ok(Drawn::Lost) => {
                    match self.instance.create_surface(child.window.clone()) {
                        Ok(surface) => {
                            surface.configure(&self.device, &child.config);
                            child.surface = Some(surface);
                        }
                        Err(_) => {
                            self.children.remove(&frame.key);
                            self.failed.push(frame.key);
                        }
                    }
                }
                Ok(Drawn::Presented | Drawn::Skipped) => {}
                Err(_) => {
                    self.children.remove(&frame.key);
                    self.failed.push(frame.key);
                }
            }
        }
        let Some(surface) = &self.surface else {
            return Ok(false);
        };
        let drawn = Self::draw(
            &self.device,
            &self.queue,
            &mut self.color,
            &mut renderer,
            ctx,
            surface,
            &self.config,
            output.shapes,
            output.pixels_per_point,
        )?;
        match drawn {
            Drawn::Presented => {}
            Drawn::Outdated => {
                if let Some(surface) = &self.surface {
                    surface.configure(&self.device, &self.config);
                    if let Some(window) = &self.window {
                        declare_color(window, self.color.p3)?;
                    }
                }
                return Ok(false);
            }
            Drawn::Lost => {
                self.reset_surface()?;
                return Ok(false);
            }
            Drawn::Skipped => return Ok(false),
        }
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
        frames: Vec<crate::viewports::ChildFrame>,
    ) -> Result<bool, String> {
        if self.recovery.halted || !self.attached() || self.width == 0 || self.height == 0 {
            return Ok(false);
        }
        let gpu = self.gpu.as_mut().ok_or("native GPU renderer unavailable")?;
        gpu.paint(ctx, output, frames)
    }
    /// Give a native sub-window's NativeWindow a surface on the shared device.
    pub fn attach_child(&mut self, key: u32, window: Window, w: u32, h: u32) -> Result<(), String> {
        let gpu = self.gpu.as_mut().ok_or("native GPU renderer unavailable")?;
        gpu.attach_child(key, Arc::new(window), w, h)
    }
    pub fn resize_child(&mut self, key: u32, w: u32, h: u32) -> Result<(), String> {
        match self.gpu.as_mut() {
            Some(gpu) => gpu.resize_child(key, w, h),
            None => Ok(()),
        }
    }
    /// Drop the surface (and native window reference) but keep the window registered upstream.
    pub fn detach_child(&mut self, key: u32) {
        self.remove_child(key);
    }
    pub fn remove_child(&mut self, key: u32) {
        if let Some(gpu) = self.gpu.as_mut() {
            // Surface first, then the NativeWindow reference it borrows.
            if let Some(mut child) = gpu.children.remove(&key) {
                child.surface = None;
            }
        }
    }
    /// Sub-windows whose surface failed while drawing; the caller asks the app to close them.
    pub fn take_failed_children(&mut self) -> Vec<u32> {
        self.gpu
            .as_mut()
            .map(|gpu| std::mem::take(&mut gpu.failed))
            .unwrap_or_default()
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
