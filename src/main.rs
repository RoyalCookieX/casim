mod simulation;

use simulation::Simulation;
use std::{rc::Rc, time};
use winit::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{Event, MouseButton, MouseScrollDelta, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::Window,
};

#[derive(Clone, Copy, Debug)]
enum UpdateMode {
    Tick { next: time::Instant },
    Step { requested: bool },
}

impl UpdateMode {
    fn new_tick() -> Self {
        Self::Tick {
            next: time::Instant::now() + time::Duration::from_secs_f32(1.0 / FRAMES_PER_SECOND),
        }
    }

    fn new_step() -> Self {
        Self::Step { requested: false }
    }
}

const WINDOW_SIZE: PhysicalSize<u32> = PhysicalSize::new(900, 900);
const FRAMES_PER_SECOND: f32 = 144.0;

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().expect("new event loop");
    let window = Rc::new(Window::new(&event_loop).expect("new window"));
    window.set_resizable(false);
    window.set_title("Casim");
    let _ = window.request_inner_size(WINDOW_SIZE);
    if let Some(monitor) = event_loop.primary_monitor() {
        let monitor_size = monitor.size();
        let window_size = window.outer_size();
        window.set_outer_position(PhysicalPosition::new(
            (monitor_size.width - window_size.width) / 2,
            (monitor_size.height - window_size.height) / 2,
        ));
    }
    let mut simulation = Simulation::new(window.clone());
    let mut exit = false;
    let mut window_focused = false;
    let mut polling = false;
    let mut update_mode = UpdateMode::new_tick();
    let mut cursor_enabled = false;
    let mut cursor_radius = 1;
    let mut cursor_position = [0, 0];
    let mut cursor_cell_id = simulation::CellId::Sand;
    let mut cursor_erase = false;
    event_loop
        .run(|event, event_loop| match event {
            Event::NewEvents(start_cause) => match start_cause {
                StartCause::Init => event_loop.set_control_flow(ControlFlow::Poll),
                StartCause::Poll => polling = true,
                _ => polling = false,
            },
            Event::WindowEvent { window_id, event } if window_id == window.id() => match event {
                WindowEvent::CloseRequested => {
                    exit = true;
                }
                WindowEvent::Focused(focused) => {
                    window_focused = focused;
                }
                WindowEvent::Resized(_) => {
                    simulation.reconfigure();
                }
                WindowEvent::CursorLeft { .. } => {
                    cursor_enabled = false;
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let window_size = window.inner_size().cast::<f64>();
                    let simulation_size = Simulation::SIZE.map(|value| value as f64);
                    cursor_position = [
                        ((position.x / window_size.width) * simulation_size[0]) as u32,
                        (((window_size.height - position.y) / window_size.height)
                            * simulation_size[1]) as u32,
                    ];
                }
                WindowEvent::MouseInput { state, button, .. } => match button {
                    MouseButton::Left | MouseButton::Right => {
                        cursor_enabled = state.is_pressed();
                        cursor_erase = button == MouseButton::Right;
                    }
                    _ => {}
                },
                WindowEvent::MouseWheel { delta, .. } => {
                    cursor_radius = match delta {
                        MouseScrollDelta::LineDelta(_, y) if y > 0.0 => cursor_radius + 1,
                        MouseScrollDelta::LineDelta(_, y) if y < 0.0 => cursor_radius - 1,
                        _ => return,
                    }
                    .clamp(1, 20);
                }
                WindowEvent::KeyboardInput { event, .. } if event.state.is_pressed() => {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::ShiftLeft) => {
                            update_mode = match update_mode {
                                UpdateMode::Tick { .. } => UpdateMode::new_step(),
                                UpdateMode::Step { .. } => UpdateMode::new_tick(),
                            }
                        }
                        PhysicalKey::Code(KeyCode::Space) => {
                            if let UpdateMode::Step { requested } = &mut update_mode {
                                *requested = true;
                            };
                        }
                        PhysicalKey::Code(KeyCode::Digit1) => {
                            cursor_cell_id = simulation::CellId::Rock;
                        }
                        PhysicalKey::Code(KeyCode::Digit2) => {
                            cursor_cell_id = simulation::CellId::Sand;
                        }
                        PhysicalKey::Code(KeyCode::Digit3) => {
                            cursor_cell_id = simulation::CellId::Water;
                        }
                        _ => {}
                    }
                }
                WindowEvent::RedrawRequested => {
                    simulation.redraw();
                }
                _ => {}
            },
            Event::AboutToWait => {
                if exit {
                    event_loop.exit();
                }
                if !polling || !window_focused {
                    return;
                }
                let cell_id = if cursor_erase {
                    simulation::CellId::Void
                } else {
                    cursor_cell_id
                };
                simulation.set_cursor(cursor_enabled, cursor_radius, cursor_position, cell_id);
                window.request_redraw();
                match &mut update_mode {
                    UpdateMode::Tick { next } => {
                        let now = time::Instant::now();
                        if now < *next {
                            return;
                        }
                        *next = now + time::Duration::from_secs_f32(1.0 / FRAMES_PER_SECOND);
                    }
                    UpdateMode::Step { requested } => {
                        if !*requested {
                            return;
                        }
                        *requested = false;
                    }
                }
                simulation.step();
            }
            _ => {}
        })
        .expect("run event loop");
}
