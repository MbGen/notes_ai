use crate::{AppWindow, NotePointer};
use slint::{ComponentHandle, winit_030::{winit::event::{WindowEvent, ElementState, MouseButton}, EventResult, WinitWindowAccessor}};
use std::time::{Duration, Instant};

// Observe gestures without consuming events: TextInput keeps native selection,
// keyboard shortcuts, focus and context-menu behavior.
pub fn install(ui: &AppWindow) {
    let weak = ui.as_weak();
    let mut position = (0.0_f64, 0.0_f64);
    let mut down: Option<((f64, f64), Instant)> = None;
    let mut dragged = false;
    ui.window().on_winit_window_event(move |window, event| {
        match event {
            WindowEvent::CursorMoved { position: p, .. } => {
                position = (p.x, p.y);
                if let Some((start, _)) = down {
                    let distance = (position.0-start.0).hypot(position.1-start.1);
                    dragged |= distance > 4.0 * window.scale_factor() as f64;
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                down = Some((position, Instant::now())); dragged = false;
            }
            WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                let now = Instant::now();
                let short_click = down.take().is_some_and(|(_, time)| now.duration_since(time) < Duration::from_millis(500)) && !dragged;
                if let Some(ui) = weak.upgrade() {
                    let pointer = ui.global::<NotePointer>();
                    pointer.set_copy_click(short_click);
                    pointer.set_released(pointer.get_released().wrapping_add(1));
                }
            }
            WindowEvent::Focused(false) | WindowEvent::CursorLeft { .. } => { down = None; dragged = false; }
            _ => {}
        }
        EventResult::Propagate
    });
}
