use super::{render::Images, start_input};
use crate::{Config, STATE_REST};
use rustix::event::{PollFd, PollFlags, Timespec, poll};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
    },
    shm::{Shm, ShmHandler, slot::Buffer, slot::SlotPool},
};
use std::{
    error::Error,
    io,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
};
use wayland_client::{
    Connection, Proxy, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub(crate) fn run(config: Config) -> Result<()> {
    let images = Images::load(&config);
    let (width, height) = images.size();
    let connection = Connection::connect_to_env()
        .map_err(|error| io::Error::other(format!("cannot connect to Wayland: {error}")))?;
    let (globals, mut queue) = registry_queue_init(&connection)?;
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh)?;
    let layer_shell = LayerShell::bind(&globals, &qh)
        .map_err(|_| io::Error::other("the compositor does not support wlr-layer-shell"))?;
    let shm = Shm::bind(&globals, &qh)?;
    let layer = layer_shell.create_layer_surface(
        &qh,
        compositor.create_surface(&qh),
        Layer::Top,
        Some("bongo"),
        None,
    );
    let position = config
        .window
        .as_ref()
        .and_then(|window| window.position.as_deref())
        .unwrap_or("top-left");
    let (anchor, margins) = placement(position);
    layer.set_anchor(anchor);
    layer.set_margin(margins.0, margins.1, margins.2, margins.3);
    layer.set_exclusive_zone(0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_size(width, height);
    layer.commit();

    let input = Arc::new(AtomicU8::new(STATE_REST));
    let mut widget = Widget {
        registry: RegistryState::new(&globals),
        output: OutputState::new(&globals, &qh),
        pool: SlotPool::new((width * height * 4) as usize, &shm)?,
        shm,
        layer,
        images,
        input: input.clone(),
        width,
        height,
        scale: 1,
        frames: None,
        buffer: None,
        last_state: None,
        configured: false,
        exit: false,
        failure: None,
    };
    start_input(
        input,
        config
            .input
            .and_then(|input| input.device)
            .unwrap_or_else(|| "AT Translated Set 2 keyboard".to_owned()),
    );
    while !widget.exit {
        queue.dispatch_pending(&mut widget)?;
        if widget.exit {
            break;
        }
        // Idle compositors may throttle frame callbacks to one per second.
        // Poll input state every 16 ms independently so keystrokes stay responsive.
        widget.redraw();
        connection.flush()?;
        if let Some(guard) = queue.prepare_read() {
            let readable = {
                let mut fds = [PollFd::from_borrowed_fd(
                    guard.connection_fd(),
                    PollFlags::IN,
                )];
                let timeout = Timespec {
                    tv_sec: 0,
                    tv_nsec: 16_000_000,
                };
                match poll(&mut fds, Some(&timeout)) {
                    Ok(_) => !fds[0].revents().is_empty(),
                    Err(rustix::io::Errno::INTR) => false,
                    Err(error) => return Err(error.into()),
                }
            };
            if readable {
                match guard.read() {
                    Ok(_) => (),
                    Err(wayland_client::backend::WaylandError::Io(error))
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }
    match widget.failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn placement(position: &str) -> (Anchor, (i32, i32, i32, i32)) {
    match position {
        "top-right" => (Anchor::TOP | Anchor::RIGHT, (16, 16, 0, 0)),
        "bottom-left" => (Anchor::BOTTOM | Anchor::LEFT, (0, 0, 16, 16)),
        "bottom-right" => (Anchor::BOTTOM | Anchor::RIGHT, (0, 16, 16, 0)),
        _ => (Anchor::TOP | Anchor::LEFT, (16, 0, 0, 16)),
    }
}

struct Widget {
    registry: RegistryState,
    output: OutputState,
    shm: Shm,
    pool: SlotPool,
    layer: LayerSurface,
    images: Images,
    input: Arc<AtomicU8>,
    width: u32,
    height: u32,
    scale: u32,
    frames: Option<[Vec<u8>; 3]>,
    buffer: Option<Buffer>,
    last_state: Option<usize>,
    configured: bool,
    exit: bool,
    failure: Option<Box<dyn Error>>,
}

impl Widget {
    fn draw(&mut self) -> Result<()> {
        if !self.configured {
            return Ok(());
        }
        let state = usize::from(self.input.load(Ordering::Relaxed).min(crate::STATE_RIGHT));
        if self.frames.is_none() {
            self.frames = Some(
                self.images
                    .render(self.width, self.height, self.scale)
                    .ok_or_else(|| io::Error::other("cannot allocate the image buffer"))?,
            );
            self.last_state = None;
        }
        let surface = self.layer.wl_surface();
        if self.last_state != Some(state) {
            let pixels = &self.frames.as_ref().unwrap()[state];
            let width = i32::try_from(self.width * self.scale)?;
            let height = i32::try_from(self.height * self.scale)?;
            if let Some(canvas) = self.buffer.as_ref().and_then(|b| self.pool.canvas(b)) {
                canvas[..pixels.len()].copy_from_slice(pixels);
            } else {
                let (buffer, canvas) =
                    self.pool
                        .create_buffer(width, height, width * 4, wl_shm::Format::Argb8888)?;
                canvas[..pixels.len()].copy_from_slice(pixels);
                self.buffer = Some(buffer);
            }
            if surface.version() >= 3 {
                surface.set_buffer_scale(self.scale as i32);
            }
            self.buffer.as_ref().unwrap().attach_to(surface)?;
            if surface.version() >= 4 {
                surface.damage_buffer(0, 0, width, height);
            } else {
                surface.damage(0, 0, self.width as i32, self.height as i32);
            }
            self.last_state = Some(state);
            self.layer.commit();
        }
        Ok(())
    }

    fn redraw(&mut self) {
        if let Err(error) = self.draw() {
            self.failure = Some(error);
            self.exit = true;
        }
    }
}

impl CompositorHandler for Widget {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        factor: i32,
    ) {
        self.scale = if surface.version() >= 3 {
            factor.max(1) as u32
        } else {
            1
        };
        self.frames = None;
        self.buffer = None;
        self.redraw();
    }

    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }

    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {}

    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for Widget {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
    ) {
        if configure.new_size.0 != 0 {
            self.width = configure.new_size.0;
        }
        if configure.new_size.1 != 0 {
            self.height = configure.new_size.1;
        }
        self.frames = None;
        self.buffer = None;
        self.configured = true;
        self.redraw();
    }
}

impl OutputHandler for Widget {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl ShmHandler for Widget {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for Widget {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers!(OutputState);
}

delegate_compositor!(Widget);
delegate_output!(Widget);
delegate_shm!(Widget);
delegate_layer!(Widget);
delegate_registry!(Widget);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corners_anchor_only_the_requested_edges_with_sixteen_pixel_margins() {
        for (position, anchor, margins) in [
            ("top-left", Anchor::TOP | Anchor::LEFT, (16, 0, 0, 16)),
            ("top-right", Anchor::TOP | Anchor::RIGHT, (16, 16, 0, 0)),
            ("bottom-left", Anchor::BOTTOM | Anchor::LEFT, (0, 0, 16, 16)),
            (
                "bottom-right",
                Anchor::BOTTOM | Anchor::RIGHT,
                (0, 16, 16, 0),
            ),
        ] {
            assert_eq!(placement(position), (anchor, margins));
        }
        assert_eq!(placement("unknown"), placement("top-left"));
    }
}
