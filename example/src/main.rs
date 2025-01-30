use anyhow::{Context, Result};
use minifb::{Key, Scale, Window, WindowOptions};
use tokio::{self, net::TcpStream};
use tracing::{event, Level};
use vnc::{PixelFormat, Rect, VncConnector, VncEvent, X11Event};

struct CanvasUtils {
    window: Window,
    video: Vec<u32>,
    width: u32,
    height: u32,
}

impl CanvasUtils {
    fn options(&self) -> WindowOptions {
        // Configure window options to show controls
        let options = WindowOptions {
            resize: true,      // Enable resizing (required for maximize button)
            borderless: false, // Show window decorations
            scale: Scale::FitScreen,
            ..WindowOptions::default()
        };
        options
    }
    fn new() -> Result<Self> {
        let options = WindowOptions {
            resize: true,      // Enable resizing (required for maximize button)
            borderless: false, // Show window decorations
            scale: Scale::FitScreen,
            ..WindowOptions::default()
        };
        Ok(Self {
            window: Window::new(
                "mstsc-rs Remote Desktop in Rust",
                800_usize,
                600_usize,
                options,
            )
            .with_context(|| "Unable to create window".to_string())?,
            video: vec![],
            width: 800,
            height: 600,
        })
    }

    fn init(&mut self, width: u32, height: u32) -> Result<()> {
        let mut window = Window::new(
            "mstsc-rs Remote Desktop in Rust",
            width as usize,
            height as usize,
            self.options(),
        )
        .with_context(|| "Unable to create window")?;
        window.set_target_fps(60);
        self.window = window;
        self.width = width;
        self.height = height;
        self.video.resize(height as usize * width as usize, 0);
        Ok(())
    }

    fn draw(&mut self, rect: Rect, data: Vec<u8>) -> Result<()> {
        // tracing::info!("width: {} height: {}", rect.width, rect.height);

        let bytes_per_pixel = 4;
        let grouped_pix: Vec<_> = data.chunks_exact(bytes_per_pixel).collect();
        let converted_data = grouped_pix
            .iter()
            .map(|x| u32::from_le_bytes(x[0..bytes_per_pixel].try_into().unwrap()) & 0x00_ff_ff_ff)
            .collect::<Vec<_>>();

        for y in 0..rect.height as usize {
            let start = (rect.y as usize + y) * self.width as usize + rect.x as usize;
            let converted_slice =
                &converted_data[y * rect.width as usize..(y + 1) * rect.width as usize];

            self.video[start..start + rect.width as usize].copy_from_slice(&converted_slice);
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.window
            .update_with_buffer(&self.video, self.width as usize, self.height as usize)
            .with_context(|| "Unable to update screen buffer")?;
        Ok(())
    }

    fn copy(&mut self, dst: Rect, src: Rect) -> Result<()> {
        tracing::info!("Copy");
        let mut tmp = vec![0; src.width as usize * src.height as usize];
        let mut tmp_idx = 0;
        for y in 0..src.height as usize {
            let mut s_idx = (src.y as usize + y) * self.width as usize + src.x as usize;
            for _ in 0..src.width {
                tmp[tmp_idx] = self.video[s_idx];
                tmp_idx += 1;
                s_idx += 1;
            }
        }
        tmp_idx = 0;
        for y in 0..src.height as usize {
            let mut d_idx = (dst.y as usize + y) * self.width as usize + dst.x as usize;
            for _ in 0..src.width {
                self.video[d_idx] = tmp[tmp_idx];
                tmp_idx += 1;
                d_idx += 1;
            }
        }
        Ok(())
    }

    fn close(&self) {}

    fn test(&mut self) {
        self.init(1920, 1080);
    }

    fn hande_vnc_event(&mut self, event: VncEvent) -> Result<()> {
        match event {
            VncEvent::SetResolution(screen) => {
                tracing::info!("Resize {:?}", screen);
                self.init(screen.width as u32, screen.height as u32)?
            }
            VncEvent::RawImage(rect, data) => {
                self.draw(rect, data)?;
            }
            VncEvent::Bell => {
                tracing::warn!("Bell event got, but ignore it");
            }
            VncEvent::SetPixelFormat(_) => unreachable!(),
            VncEvent::Copy(dst, src) => {
                self.copy(dst, src)?;
            }
            VncEvent::JpegImage(_rect, _data) => {
                tracing::warn!("Jpeg event got, but ignore it");
            }
            VncEvent::SetCursor(rect, data) => {
                if rect.width != 0 {
                    self.draw(rect, data)?;
                }
            }
            VncEvent::Text(string) => {
                tracing::info!("Got clipboard message {}", string);
            }
            _ => tracing::error!("{:?}", event),
        }
        Ok(())
    }
}

struct MouseUtil {
    pub mask: u8,
    pub x: u16,
    pub y: u16,
}

impl MouseUtil {
    fn new() -> Self {
        Self {
            mask: 0,
            x: 0,
            y: 0,
        }
    }

    fn changed(&mut self, window: &Window) -> bool {
        let mut x = 0;
        let mut y = 0;

        window.get_mouse_pos(minifb::MouseMode::Clamp).map(|mouse| {
            // tracing::info!("Mouse position: x {} y {}", mouse.0 as u16, mouse.1 as u16);
            x = mouse.0 as u16;
            y = mouse.1 as u16;
        });

        // canvas.window.get_scroll_wheel().map(|scroll| {
        //     tracing::info!("scrolling - x {} y {}", scroll.0, scroll.1);
        // });

        let left_down = window.get_mouse_down(minifb::MouseButton::Left);
        // tracing::info!("is left down? {}", left_down);

        let right_down = window.get_mouse_down(minifb::MouseButton::Right);
        // tracing::info!("is right down? {}", right_down);

        let middle_down = window.get_mouse_down(minifb::MouseButton::Middle);
        // tracing::info!("is middle down? {}", middle_down);

        let mut mask: u8 = 0;

        if left_down {
            mask |= 1;
        }

        if middle_down {
            mask |= 1 << 1
        }

        if right_down {
            mask |= 1 << 2
        }

        if (self.x, self.y, self.mask) != (x, y, mask) {
            (self.x, self.y, self.mask) = (x, y, mask);
            true
        } else {
            false
        }
    }
}

fn convert_key_to_u32(key: minifb::Key) -> u32 {
    key as u32
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create tracing subscriber
    #[cfg(debug_assertions)]
    let subscriber = tracing_subscriber::fmt()
        .pretty()
        .with_max_level(Level::ERROR)
        .finish();

    #[cfg(not(debug_assertions))]
    let subscriber = tracing_subscriber::fmt()
        .pretty()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let tcp = TcpStream::connect("localhost:5901").await?;
    let vnc = VncConnector::new(tcp)
        .set_auth_method(async move { Ok("none".to_string()) })
        // .add_encoding(vnc::VncEncoding::Tight)
        // .add_encoding(vnc::VncEncoding::Zrle)
        // .add_encoding(vnc::VncEncoding::CopyRect)
        // .add_encoding(vnc::VncEncoding::Raw)
        .add_encoding(vnc::VncEncoding::Jpeg)
        .allow_shared(true)
        .set_pixel_format(PixelFormat::bgra())
        .build()?
        .try_start()
        .await?
        .finish()?;

    let mut canvas = CanvasUtils::new()?;
    let mut mouse = MouseUtil::new();
    // canvas.test();
    let mut now = std::time::Instant::now();
    let mut pressed_keys = Vec::<u32>::new();

    loop {
        let mut events = Vec::<X11Event>::new();

        match vnc.poll_event().await {
            Ok(Some(e)) => {
                let _ = canvas.hande_vnc_event(e);
            }
            Ok(None) => (),
            Err(e) => {
                tracing::error!("{}", e.to_string());
                break;
            }
        }

        canvas
            .window
            .get_keys_pressed(minifb::KeyRepeat::No)
            .iter()
            .for_each(|key| {
                let converted_key = convert_key_to_u32(*key);
                // tracing::info!("Pressed {}", pressed_keys.contains(&converted_key));
                // tracing::info!("pressed_keys {:?}", pressed_keys);
                // tracing::info!("converted_keys {}", converted_key);
                if !pressed_keys.contains(&converted_key) {
                    // tracing::info!("Pushing Key pressed: {:?}", key);
                    pressed_keys.push(converted_key);
                    let event = X11Event::KeyEvent((converted_key, true).into());
                    events.push(event);
                    tracing::info!("Events pressed: {:?}", events);
                   
                }
                // tracing::info!("Key pressed: {:?}", pressed_keys);
                // tracing::info!("Events pressed: {:?}", events);
            });

        canvas.window.get_keys_released().iter().for_each(|key| {
            let converted_key = convert_key_to_u32(*key);
            // tracing::info!("Released {}", pressed_keys.contains(&converted_key));

            if pressed_keys.contains(&converted_key) {
                // tracing::info!("Removing Key released: {:?}", key);
                pressed_keys.retain(|&x| x != converted_key);
                events.push(X11Event::KeyEvent((convert_key_to_u32(*key), false).into()));
                tracing::info!("Events released: {:?}", events);

            }
            // tracing::info!("Key released: {:?}", pressed_keys);
            // tracing::info!("Events released: {:?}", events);
        });

        if mouse.changed(&canvas.window) {
            let _ = vnc
                .input(X11Event::PointerEvent(
                    (mouse.x, mouse.y, mouse.mask).into(),
                ))
                .await;
        }

        if now.elapsed().as_millis() > 16 {
            // Add code for receiver of input events

            // tracing::info!("Sending events");
            // let event = events.pop().unwrap_or(X11Event::Refresh);
            for event in events.iter() {
                let _ = vnc.input(event.clone()).await;
            }
            let _ = canvas.flush();
            let _ = vnc.input(X11Event::Refresh).await;
            now = std::time::Instant::now();
        }
    }
    canvas.close();
    let _ = vnc.close().await;
    Ok(())
}
