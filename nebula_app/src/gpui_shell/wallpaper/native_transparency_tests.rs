//! Opt-in real HWND/DWM acceptance: owned stripe backdrop, no shell or user data.
use super::*;
use gpui::ParentElement as _;
use gpui::{AppContext as _, AsyncApp, Context, Render, WindowBounds, WindowHandle, WindowOptions};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;
use windows_sys::Win32::Foundation::{HWND, POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, GetDC, GetPixel, ReleaseDC};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClientRect, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

struct Surface {
    backdrop: bool,
}
impl Render for Surface {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.backdrop {
            gpui::canvas(
                |_, _, _| (),
                |bounds, _, window, _| {
                    let stripe = px(16.0 / window.scale_factor());
                    for index in 0..(bounds.size.width / stripe).ceil() as usize {
                        let color = if index % 2 == 0 { gpui::white() } else { gpui::black() };
                        window.paint_quad(fill(
                            Bounds::new(
                                bounds.origin + point(stripe * index as f32, px(0.0)),
                                size(stripe, bounds.size.height),
                            ),
                            color,
                        ));
                    }
                },
            )
            .size_full()
            .into_any_element()
        } else {
            let mut background = gpui::rgb(0x181818);
            background.a = window_opacity(cx);
            div()
                .size_full()
                .relative()
                .bg(background)
                .child(
                    div()
                        .absolute()
                        .left(px(64.0))
                        .top(px(64.0))
                        .size(px(32.0))
                        .bg(gpui::rgb(0xff0000)),
                )
                .into_any_element()
        }
    }
}

fn open(cx: &mut App, backdrop: bool) -> WindowHandle<Surface> {
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(120.0), px(120.0)),
                size(px(640.0), px(360.0)),
            ))),
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        |_, cx| cx.new(|_| Surface { backdrop }),
    )
    .unwrap()
}

fn hwnd(window: &Window) -> HWND {
    let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window).unwrap().as_raw()
    else {
        panic!("Windows HWND required")
    };
    handle.hwnd.get() as HWND
}

fn client_rect(handle: HWND) -> Result<(POINT, RECT), String> {
    let mut origin = POINT { x: 0, y: 0 };
    let mut bounds: RECT = unsafe { std::mem::zeroed() };
    if unsafe { GetClientRect(handle, &mut bounds) } == 0
        || unsafe { ClientToScreen(handle, &mut origin) } == 0
    {
        return Err("owned native client rectangle unavailable".into());
    }
    Ok((origin, bounds))
}

fn accent_state(handle: HWND) -> Option<u32> {
    test_native_accent_state(handle as isize)
}

fn pixel(dc: windows_sys::Win32::Graphics::Gdi::HDC, x: i32, y: i32) -> Result<[u8; 3], String> {
    let color = unsafe { GetPixel(dc, x, y) };
    if color == u32::MAX {
        return Err("interactive desktop pixels unavailable".into());
    }
    Ok([color as u8, (color >> 8) as u8, (color >> 16) as u8])
}

fn sample(backdrop: HWND, front: HWND, scale: f32, label: &str) -> Result<i32, String> {
    let (back_origin, _) = client_rect(backdrop)?;
    let (origin, bounds) = client_rect(front)?;
    let x = back_origin.x + ((origin.x + bounds.right / 2 - back_origin.x) / 32) * 32;
    let y = origin.y + bounds.bottom / 2;
    let dc = unsafe { GetDC(std::ptr::null_mut()) };
    if dc.is_null() { return Err("interactive desktop DC unavailable".into()); }
    let result = (|| {
        let white = pixel(dc, x + 8, y)?;
        let black = pixel(dc, x + 24, y)?;
        let opaque = pixel(dc, origin.x + (80.0 * scale) as i32, origin.y + (80.0 * scale) as i32)?;
        let mut image = image::RgbImage::new(32, 8);
        for py in 0..8 { for px in 0..32 {
            image.put_pixel(px, py, image::Rgb(pixel(dc, x + px as i32, y + py as i32)?));
        }}
        if let Some(directory) = std::env::var_os("PEBREL_OPACITY_QA_OUTPUT") {
            let directory = std::path::PathBuf::from(directory);
            std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
            image.save(directory.join(format!("{label}.png"))).map_err(|error| error.to_string())?;
        }
        if opaque[0] <= 235 || opaque[1] >= 20 || opaque[2] >= 20 {
            return Err(format!("opacity dimmed opaque content: {opaque:?}"));
        }
        let contrast = white.iter().map(|v| i32::from(*v)).sum::<i32>() / 3
            - black.iter().map(|v| i32::from(*v)).sum::<i32>() / 3;
        crate::gpui_shell::try_write_stderr(format_args!(
            "native opacity ROI {label}: successful_setter_state={:?}, white={white:?}, black={black:?}, contrast={contrast}, Windows build={}\n",
            accent_state(front), windows_build_number()
        ));
        Ok(contrast)
    })();
    unsafe { ReleaseDC(std::ptr::null_mut(), dc) };
    result
}

async fn settle(cx: &AsyncApp) {
    cx.background_executor().timer(Duration::from_millis(250)).await;
}

async fn exercise(
    backdrop: WindowHandle<Surface>,
    front: WindowHandle<Surface>,
    cx: &AsyncApp,
) -> Result<(), String> {
    settle(cx).await;
    let mut policy_mismatch = None;
    for (opacity, label) in [(0.55, "none-55"), (1.0, "none-100"), (0.55, "none-55-restored")] {
        cx.update(|cx| {
            cx.global_mut::<VisualEffects>().opacity = opacity;
            apply_window_effects(cx);
            backdrop
                .update(cx, |_, window, _| unsafe {
                    SetWindowPos(
                        hwnd(window),
                        HWND_TOPMOST,
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    );
                })
                .unwrap();
            front
                .update(cx, |_, window, _| {
                    unsafe {
                        SetWindowPos(
                            hwnd(window),
                            HWND_TOPMOST,
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        );
                    }
                    window.refresh();
                })
                .unwrap();
        });
        settle(cx).await;
        let (contrast, accent) = cx.update(|cx| {
            let back = backdrop.update(cx, |_, window, _| hwnd(window)).unwrap();
            front
                .update(cx, |_, window, _| {
                    (
                        sample(back, hwnd(window), window.scale_factor(), label),
                        accent_state(hwnd(window)),
                    )
                })
                .unwrap()
        });
        let contrast = contrast?;
        if accent != Some(2) {
            policy_mismatch = Some(format!("Transparent native accent was overwritten: {accent:?}"));
        }
        if (opacity < 1.0 && contrast < 60) || (opacity == 1.0 && contrast.abs() > 3) {
            return Err(format!("desktop transparency mismatch for {label}: {contrast}"));
        }
    }
    cx.update(|cx| {
        front
            .update(cx, |_, window, _| {
                window.set_background_appearance(WindowBackgroundAppearance::Opaque);
                apply_windows_accent_policy(
                    window,
                    BlurModeName::None,
                    WindowBackgroundAppearance::Opaque,
                );
                window.refresh();
            })
            .unwrap();
    });
    settle(cx).await;
    let (contrast, accent) = cx.update(|cx| {
        let back = backdrop.update(cx, |_, window, _| hwnd(window)).unwrap();
        front
            .update(cx, |_, window, _| {
                (
                    sample(back, hwnd(window), window.scale_factor(), "opaque-appearance"),
                    accent_state(hwnd(window)),
                )
            })
            .unwrap()
    });
    let contrast = contrast?;
    if accent != Some(0) || contrast.abs() > 3 {
        return Err("Opaque appearance became transparent".into());
    }
    cx.update(|cx| {
        front
            .update(cx, |_, window, _| {
                window.set_background_appearance(WindowBackgroundAppearance::Transparent);
                // Exercise the transparent fallback without pretending this runner is old Windows.
                apply_windows_accent_policy(
                    window,
                    BlurModeName::Mica,
                    WindowBackgroundAppearance::Transparent,
                );
                window.refresh();
            })
            .unwrap();
    });
    settle(cx).await;
    let (contrast, accent) = cx.update(|cx| {
        let back = backdrop.update(cx, |_, window, _| hwnd(window)).unwrap();
        front
            .update(cx, |_, window, _| {
                (
                    sample(back, hwnd(window), window.scale_factor(), "mica-transparent-fallback"),
                    accent_state(hwnd(window)),
                )
            })
            .unwrap()
    });
    let contrast = contrast?;
    if accent != Some(2) {
        policy_mismatch = Some(format!("Transparent Mica fallback accent was overwritten: {accent:?}"));
    }
    if contrast < 60 {
        return Err("Transparent Mica fallback is opaque".into());
    }
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
            front
                .update(cx, |_, window, _| {
                    sample(back, hwnd(window), window.scale_factor(), "material-to-none")
                })
                .unwrap()
        });
        let contrast = contrast?;
        if contrast < 60 {
            return Err(format!(
                "material to None did not restore sharp transparency: {mode:?}, {contrast}"
            ));
        }
    }
    match policy_mismatch {
        Some(error) => Err(error),
        None => Ok(()),
    }
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
        })
        .detach();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(Duration::from_secs(20)).await;
            watchdog.set(true);
            cx.update(|cx| cx.quit());
        })
        .detach();
    });
    drop(guard);
    assert!(!timed_out.get(), "native QA timed out");
    assert_eq!(result.borrow_mut().take(), Some(Ok(())));
}
