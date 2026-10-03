//! Opt-in real HWND/DWM acceptance: owned stripe backdrop, no shell or user data.
use super::*;
use gpui::{AppContext as _, AsyncApp, Context, Render, WindowBounds, WindowHandle, WindowOptions};
use gpui::ParentElement as _;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, GetDC, GetPixel, ReleaseDC};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
};
use winit::raw_window_handle::{HasWindowHandle as _, RawWindowHandle};

struct Surface { backdrop: bool }
impl Render for Surface {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.backdrop {
            gpui::canvas(|_, _, _| (), |bounds, _, window, _| {
                let stripe = px(16.0 / window.scale_factor());
                for index in 0..(bounds.size.width / stripe).ceil() as usize {
                    let color = if index % 2 == 0 { gpui::white() } else { gpui::black() };
                    window.paint_quad(fill(Bounds::new(
                        bounds.origin + point(stripe * index as f32, px(0.0)),
                        size(stripe, bounds.size.height),
                    ), color));
                }
            }).size_full().into_any_element()
        } else {
            let mut background = gpui::rgb(0x181818);
            background.a = window_opacity(cx);
            div().size_full().relative().bg(background)
                .child(div().absolute().left(px(64.0)).top(px(64.0)).size(px(32.0)).bg(gpui::rgb(0xff0000)))
                .into_any_element()
        }
    }
}

fn open(cx: &mut App, backdrop: bool) -> WindowHandle<Surface> {
    cx.open_window(WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(point(px(120.0), px(120.0)), size(px(640.0), px(360.0))))),
        window_background: WindowBackgroundAppearance::Transparent,
        ..Default::default()
    }, |_, cx| cx.new(|_| Surface { backdrop })).unwrap()
}

fn hwnd(window: &Window) -> HWND {
    let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window).unwrap().as_raw() else { panic!("Windows HWND required") };
    handle.hwnd.get() as HWND
}

fn client_rect(handle: HWND) -> (POINT, RECT) {
    let mut origin = POINT { x: 0, y: 0 };
    let mut bounds: RECT = unsafe { std::mem::zeroed() };
    assert_ne!(unsafe { GetClientRect(handle, &mut bounds) }, 0);
    assert_ne!(unsafe { ClientToScreen(handle, &mut origin) }, 0);
    (origin, bounds)
}

fn accent_state(handle: HWND) -> u32 {
    type Getter = unsafe extern "system" fn(HWND, *mut WindowCompositionAttributeData) -> i32;
    let getter: Getter = unsafe {
        let module = GetModuleHandleA(c"user32.dll".as_ptr() as *const u8);
        let address = GetProcAddress(module, c"GetWindowCompositionAttribute".as_ptr() as *const u8).expect("native WCA readback must be available");
        std::mem::transmute(address)
    };
    let mut policy = AccentPolicy { state: 0, flags: 0, gradient_color: 0, animation_id: 0 };
    let mut data = WindowCompositionAttributeData { attribute: 19, data: &mut policy as *mut _ as *mut core::ffi::c_void, size: std::mem::size_of::<AccentPolicy>() };
    assert_ne!(unsafe { getter(handle, &mut data) }, 0, "native accent readback failed");
    policy.state
}

fn pixel(dc: windows_sys::Win32::Graphics::Gdi::HDC, x: i32, y: i32) -> [u8; 3] {
    let color = unsafe { GetPixel(dc, x, y) };
    assert_ne!(color, u32::MAX, "interactive desktop pixels unavailable");
    [color as u8, (color >> 8) as u8, (color >> 16) as u8]
}

fn sample(backdrop: HWND, front: HWND, scale: f32, label: &str) -> i32 {
    let (back_origin, _) = client_rect(backdrop);
    let (origin, bounds) = client_rect(front);
    let x = back_origin.x + ((origin.x + bounds.right / 2 - back_origin.x) / 32) * 32;
    let y = origin.y + bounds.bottom / 2;
    let dc = unsafe { GetDC(std::ptr::null_mut()) };
    assert!(!dc.is_null(), "interactive desktop DC unavailable");
    let white = pixel(dc, x + 8, y);
    let black = pixel(dc, x + 24, y);
    let opaque = pixel(dc, origin.x + (80.0 * scale) as i32, origin.y + (80.0 * scale) as i32);
    let mut image = image::RgbImage::new(32, 8);
    for py in 0..8 { for px in 0..32 {
        image.put_pixel(px, py, image::Rgb(pixel(dc, x + px as i32, y + py as i32)));
    }}
    unsafe { ReleaseDC(std::ptr::null_mut(), dc) };
    if let Some(directory) = std::env::var_os("PEBREL_OPACITY_QA_OUTPUT") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        image.save(directory.join(format!("{label}.png"))).unwrap();
    }
    assert!(opaque[0] > 235 && opaque[1] < 20 && opaque[2] < 20, "opacity must not dim opaque content: {opaque:?}");
    let contrast = white.iter().map(|v| i32::from(*v)).sum::<i32>() / 3
        - black.iter().map(|v| i32::from(*v)).sum::<i32>() / 3;
    eprintln!("native opacity ROI {label}: accent={}, white={white:?}, black={black:?}, contrast={contrast}, Windows build={}", accent_state(front), windows_build_number());
    contrast
}

async fn settle(cx: &AsyncApp) {
    cx.background_executor().timer(Duration::from_millis(250)).await;
}

async fn exercise(backdrop: WindowHandle<Surface>, front: WindowHandle<Surface>, cx: &AsyncApp) -> Result<(), String> {
    settle(cx).await;
    for (opacity, label) in [(0.55, "none-55"), (1.0, "none-100"), (0.55, "none-55-restored")] {
        cx.update(|cx| {
            cx.global_mut::<VisualEffects>().opacity = opacity;
            apply_window_effects(cx);
            backdrop.update(cx, |_, window, _| unsafe { SetWindowPos(hwnd(window), HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE); }).unwrap();
            front.update(cx, |_, window, _| {
                unsafe { SetWindowPos(hwnd(window), HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE); }
                window.refresh();
            }).unwrap();
        });
        settle(cx).await;
        let (contrast, accent) = cx.update(|cx| {
            let back = backdrop.update(cx, |_, window, _| hwnd(window)).unwrap();
            front.update(cx, |_, window, _| (sample(back, hwnd(window), window.scale_factor(), label), accent_state(hwnd(window)))).unwrap()
        });
        if accent != 2 { return Err(format!("Transparent native accent was overwritten: {accent}")); }
        if (opacity < 1.0 && contrast < 60) || (opacity == 1.0 && contrast.abs() > 3) {
            return Err(format!("desktop transparency mismatch for {label}: {contrast}"));
        }
    }
    cx.update(|cx| {
        front.update(cx, |_, window, _| {
            window.set_background_appearance(WindowBackgroundAppearance::Opaque);
            apply_windows_accent_policy(window, BlurModeName::None, WindowBackgroundAppearance::Opaque);
            window.refresh();
        }).unwrap();
    });
    settle(cx).await;
    let (contrast, accent) = cx.update(|cx| {
        let back = backdrop.update(cx, |_, window, _| hwnd(window)).unwrap();
        front.update(cx, |_, window, _| (sample(back, hwnd(window), window.scale_factor(), "opaque-appearance"), accent_state(hwnd(window)))).unwrap()
    });
    if accent != 0 || contrast.abs() > 3 { return Err("Opaque appearance became transparent".into()); }
    cx.update(|cx| {
        front.update(cx, |_, window, _| {
            window.set_background_appearance(WindowBackgroundAppearance::Transparent);
            // Exercise the transparent fallback without pretending this runner is old Windows.
            apply_windows_accent_policy(window, BlurModeName::Mica, WindowBackgroundAppearance::Transparent);
            window.refresh();
        }).unwrap();
    });
    settle(cx).await;
    let (contrast, accent) = cx.update(|cx| {
        let back = backdrop.update(cx, |_, window, _| hwnd(window)).unwrap();
        front.update(cx, |_, window, _| (sample(back, hwnd(window), window.scale_factor(), "mica-transparent-fallback"), accent_state(hwnd(window)))).unwrap()
    });
    if accent != 2 || contrast < 60 { return Err("Transparent Mica fallback is opaque".into()); }
    for mode in [BlurModeName::Acrylic, BlurModeName::Mica] {
        cx.update(|cx| {
            cx.global_mut::<VisualEffects>().blur = mode;
            apply_window_effects(cx);
        });
        settle(cx).await;
        cx.update(|cx| {
            cx.global_mut::<VisualEffects>().blur = BlurModeName::None;
            apply_window_effects(cx);
        });
        settle(cx).await;
        let contrast = cx.update(|cx| {
            let back = backdrop.update(cx, |_, window, _| hwnd(window)).unwrap();
            front.update(cx, |_, window, _| sample(back, hwnd(window), window.scale_factor(), "material-to-none")).unwrap()
        });
        if contrast < 60 { return Err(format!("material to None did not restore sharp transparency: {mode:?}, {contrast}")); }
    }
    Ok(())
}

#[test]
#[ignore = "opens real HWNDs and reads composed desktop pixels; run only in an interactive Windows QA job"]
fn native_windows_none_opacity_preserves_transparent_composition() {
    let result = Rc::new(RefCell::new(None));
    let shared = result.clone();
    let timed_out = Rc::new(Cell::new(false));
    let watchdog = timed_out.clone();
    let guard = crate::platform::acrylic::RunGuard::default();
    gpui_platform::application().run(move |cx| {
        crate::platform::acrylic::init(cx);
        test_install_visual_effects(cx, 0.55, BlurModeName::None);
        let backdrop = open(cx, true);
        let front = open(cx, false);
        apply_window_effects(cx);
        cx.spawn(async move |cx| {
            *shared.borrow_mut() = Some(exercise(backdrop, front, cx).await);
            cx.update(|cx| cx.quit());
        }).detach();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(20)).await;
            watchdog.set(true);
            cx.update(|cx| cx.quit());
        }).detach();
    });
    drop(guard);
    assert!(!timed_out.get(), "native QA timed out");
    assert_eq!(result.borrow_mut().take(), Some(Ok(())));
}
