use async_std::{prelude::*, task};
use futures::channel::mpsc;
use futures::sink::SinkExt;

use std::time::{Duration, Instant};

use glium::glutin::{self, Event, WindowEvent};
use glium::{Display, Surface};

use imgui::*;
use imgui_glium_renderer::Renderer;
use imgui_winit_support::{HiDpiMode, WinitPlatform};

type Sender<T> = mpsc::UnboundedSender<T>;
type Receiver<T> = mpsc::UnboundedReceiver<T>;

#[derive(Debug)]
enum UiEvent {
    ToggleUiState { current_state: bool },
    ToggleUiStateFinished { new_state: bool },
}

fn main() {
    task::block_on(run_ui());
}

//background task which manages processing messages independently of the GUI
async fn broker_loop(mut inbound_events: Receiver<UiEvent>, mut outbound_events: Sender<UiEvent>) {
    while let Some(event) = inbound_events.next().await {
        match event {
            UiEvent::ToggleUiState { current_state } => {
                let sleep_duration = 2;
                println!("Toggling button state in {} seconds...", sleep_duration);
                task::sleep(Duration::from_secs(sleep_duration)).await;
                println!("Changing now!");
                outbound_events.send(UiEvent::ToggleUiStateFinished { new_state: !current_state }).await.unwrap();
            }
            _ => (),
        }
    }
}

async fn run_ui() {
    //setup two channels and a broker
    //this gives us a bi-directional channel we can use to communicate between the UI and
    //any background activity
    let (mut broker_sender, broker_receiver) = mpsc::unbounded();
    let (ui_sender, mut ui_receiver) = mpsc::unbounded();
    //setup the broker task, it is responsible for performing actions in the background without blocking the GUI
    let _broker_handle = task::spawn(broker_loop(broker_receiver, ui_sender));

    //setup our imgui drawing ccontext
    let mut imgui = Context::create();

    //setup window/imgui renderer
    let title = "async-imgui";
    let mut events_loop = glutin::EventsLoop::new();
    let context = glutin::ContextBuilder::new().with_vsync(true);
    let builder = glutin::WindowBuilder::new()
        .with_title(title.to_owned())
        .with_dimensions(glutin::dpi::LogicalSize::new(1024f64, 768f64));
    let display = Display::new(builder, context, &events_loop).expect("Failed to initialize display");
    let mut renderer = Renderer::init(&mut imgui, &display).expect("Failed to initialize renderer");

    //bind the platform events to imgui
    let gl_window = display.gl_window();
    let window = gl_window.window();
    let mut platform = WinitPlatform::init(&mut imgui);
    platform.attach_window(imgui.io_mut(), &window, HiDpiMode::Rounded);

    //window state
    let mut show_extra_label = true;

    //run state
    let mut run = true;
    let mut last_frame = Instant::now();

    while run {
        //handle platform events first
        events_loop.poll_events(|event| {
            //pass to imgui
            platform.handle_event(imgui.io_mut(), &window, &event);

            //handle close manually
            if let Event::WindowEvent { event, .. } = event {
                if let WindowEvent::CloseRequested = event {
                    run = false;
                }
            }
        });

        //poll here instead of await so we do not block the gui thread
        let status = futures::poll!(ui_receiver.next());
        match status {
            futures::task::Poll::Ready(message) => {
                let message = message.unwrap();
                println!("Some message is here: {:?}", message);
                match message {
                    UiEvent::ToggleUiStateFinished { new_state } => {
                        show_extra_label = new_state;
                    }
                    _ => (),
                }
            }
            _ => (),
        };

        //prep a new frame
        let io = imgui.io_mut();
        platform.prepare_frame(io, &window).expect("Failed to start frame");
        last_frame = io.update_delta_time(last_frame);

        //draw gui
        let ui = imgui.frame();
        Window::new(im_str!("async imgui-rs")).size([300.0, 300.0], Condition::FirstUseEver).build(&ui, || {
            ui.text_wrapped(im_str!(
                "Click the button below to toggle the text below after some time.  You should still be able to drag this window while that is happening."
            ));
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
