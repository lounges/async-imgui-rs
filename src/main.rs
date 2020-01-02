use async_std::{prelude::*, task};
use std::time::Duration;

use glium::glutin::{self, Event, WindowEvent};
use glium::{Display, Surface};
use imgui::*;
use imgui_glium_renderer::Renderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};
use std::time::Instant;

use futures::channel::mpsc;
use futures::sink::SinkExt;
type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

#[derive(Debug)]
enum UiEvent {
    ToggleUiState { current_state: bool },
}

#[derive(Debug)]
enum UiEventCompleted {
    ConsoleMessageFinished { new_state: bool },
}

fn main() {
    task::block_on(run_ui());
}

async fn broker_loop(mut inbound_events: Receiver<UiEvent>, mut outbound_events: Sender<UiEventCompleted>) {
    while let Some(event) = inbound_events.next().await {
        match event {
            UiEvent::ToggleUiState { current_state } => {
                let sleep_duration = 2;
                println!("Toggling button state in {} seconds...", sleep_duration);
                task::sleep(Duration::from_secs(sleep_duration)).await;
                println!("Changing now!");
                outbound_events.send(UiEventCompleted::ConsoleMessageFinished { new_state: !current_state }).await.unwrap();
            }
        }
    }
}

async fn run_ui() {
    let (mut broker_sender, broker_receiver) = mpsc::unbounded();
    let (ui_sender, mut ui_receiver) = mpsc::unbounded();
    let _broker_handle = task::spawn(broker_loop(broker_receiver, ui_sender));

    let mut imgui = Context::create();
    imgui.set_ini_filename(None);

    let title = "async-imgui";
    let mut events_loop = glutin::EventsLoop::new();
    let context = glutin::ContextBuilder::new().with_vsync(true);
    let builder = glutin::WindowBuilder::new()
        .with_title(title.to_owned())
        .with_dimensions(glutin::dpi::LogicalSize::new(1024f64, 768f64));
    let display = Display::new(builder, context, &events_loop).expect("Failed to initialize display");

    let mut platform = WinitPlatform::init(&mut imgui);
    let gl_window = display.gl_window();
    let window = gl_window.window();
    platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Rounded);

    let mut renderer = Renderer::init(&mut imgui, &display).expect("Failed to initialize renderer");
    let mut last_frame = Instant::now();
    let mut run = true;
    let mut show_extra_label = true;

    while run {
        events_loop.poll_events(|event| {
            platform.handle_event(imgui.io_mut(), &window, &event);

            if let Event::WindowEvent { event, .. } = event {
                if let WindowEvent::CloseRequested = event {
                    run = false;
                }
            }
        });

        let status = futures::poll!(ui_receiver.next());
        match status {
            futures::task::Poll::Ready(message) => {
                let message = message.unwrap();
                println!("Some message is here: {:?}", message);
                match message {
                    UiEventCompleted::ConsoleMessageFinished { new_state } => {
                        show_extra_label = new_state;
                    }
                }
            }
            _ => (),
        };

        //handle io
        let io = imgui.io_mut();
        platform.prepare_frame(io, &window).expect("Failed to start frame");
        last_frame = io.update_delta_time(last_frame);

        //draw gui
        let ui = imgui.frame();
        Window::new(im_str!("async imgui-rs")).size([400.0, 300.0], Condition::FirstUseEver).build(&ui, || {
            ui.text_wrapped(im_str!("Click the button below to toggle the text below after some time."));
            if show_extra_label {
                ui.text_wrapped(im_str!("This line is extra!"));
            }
            if ui.button(im_str!("Toggle"), [75.0, 23.0]) {
                task::block_on(broker_sender.send(UiEvent::ToggleUiState { current_state: show_extra_label })).unwrap();
            }
        });

        //render
        let mut target = display.draw();
        target.clear_color_srgb(0.1, 0.1, 0.1, 1.0);
        platform.prepare_render(&ui, &window);
        let draw_data = ui.render();
        renderer.render(&mut target, draw_data).expect("Rendering failed");
        target.finish().expect("Failed to swap buffers");
    }
}
