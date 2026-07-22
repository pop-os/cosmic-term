use cosmic::Application;
use cosmic::iced::{
    self, Subscription,
    futures::SinkExt,
    platform_specific::runtime::wayland::layer_surface::SctkLayerSurfaceSettings,
    platform_specific::shell::commands::layer_surface::{
        Anchor, KeyboardInteractivity, Layer, destroy_layer_surface, get_layer_surface,
    },
    stream, window,
};
use cosmic::style;
use cosmic::widget;
use std::any::TypeId;

use crate::App;
use crate::Message;

/// Scan /proc for an existing cosmic-term process with --drop-down flag.
pub fn find_existing_dropdown_pid() -> Option<u32> {
    let current_pid = std::process::id();
    let proc_dir = match std::fs::read_dir("/proc") {
        Ok(d) => d,
        Err(_) => return None,
    };
    for entry in proc_dir.flatten() {
        if let Some(pid) = entry
            .file_name()
            .to_string_lossy()
            .parse::<u32>()
            .ok()
            .filter(|&p| p != current_pid)
            .and_then(|pid| {
                std::fs::read(entry.path().join("cmdline"))
                    .ok()
                    .filter(|cmdline| {
                        let args: Vec<&[u8]> = cmdline.split(|b| *b == 0).collect();
                        args.iter().any(|a| {
                            a.ends_with(b"cosmic-term")
                                || a.ends_with(b"cosmic-term\n")
                                || a == b"cosmic-term"
                        }) && args.iter().any(|a| *a == b"--drop-down")
                    })
                    .map(|_| pid)
            })
        {
            return Some(pid);
        }
    }
    None
}

impl App {
    pub fn create_dropdown_layer_surface(
        id: window::Id,
        height_px: u32,
    ) -> cosmic::app::Task<Message> {
        get_layer_surface(SctkLayerSurfaceSettings {
            id,
            keyboard_interactivity: KeyboardInteractivity::OnDemand,
            anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
            namespace: "cosmic-term-dropdown".into(),
            layer: Layer::Overlay,
            size: Some((None, Some(height_px))),
            ..Default::default()
        })
    }

    /// Returns a valid parent window ID for popups.
    pub fn popup_parent_id(&self) -> Option<window::Id> {
        self.dropdown_surface.or_else(|| self.core.main_window_id())
    }

    /// Destroy and recreate the current dropdown surface (e.g. after a config change).
    /// Uses `height_override` if provided, otherwise uses `self.config.dropdown_height`.
    pub fn recreate_dropdown_surface(
        &self,
        height_override: Option<u32>,
    ) -> cosmic::app::Task<Message> {
        if let Some(id) = self.dropdown_surface {
            let height = height_override.unwrap_or(self.config.dropdown_height);
            cosmic::app::Task::batch([
                destroy_layer_surface(id),
                Self::create_dropdown_layer_surface(id, height),
            ])
        } else {
            cosmic::app::Task::none()
        }
    }

    /// Subscription for SIGUSR1 signal handling in drop-down mode.
    pub fn subscribe_dropdown_signals(&self) -> Subscription<Message> {
        struct DropDownSignalSubscription;

        Subscription::run_with(TypeId::of::<DropDownSignalSubscription>(), |_| {
            stream::channel(
                1,
                |mut output: iced::futures::channel::mpsc::Sender<Message>| async move {
                    let mut signal_stream = match tokio::signal::unix::signal(
                        tokio::signal::unix::SignalKind::user_defined1(),
                    ) {
                        Ok(stream) => stream,
                        Err(e) => {
                            log::error!("Failed to create signal stream: {}", e);
                            return;
                        }
                    };

                    loop {
                        signal_stream.recv().await;
                        let _ = output.send(Message::ToggleDropDown).await;
                    }
                },
            )
        })
    }

    /// In drop-down mode, `view_main()` is bypassed. Wrap the terminal view
    /// with the header bar and context drawer ourselves.
    pub fn wrap_dropdown_view<'a>(
        &'a self,
        view: cosmic::Element<'a, Message>,
    ) -> cosmic::Element<'a, Message> {
        let mut view = view;

        if self.core.window.show_context
            && let Some(context) = self.context_drawer()
        {
            view = widget::context_drawer(
                context.title,
                context.actions,
                context.header,
                context.footer,
                context.on_close,
                view,
                context.content,
                320.0,
            )
            .into();
        }

        if self.config.show_headerbar_dropdown {
            let mut header = widget::header_bar();
            for el in self.header_start() {
                header = header.start(el);
            }
            for el in self.header_end() {
                header = header.end(el);
            }

            // Wrap header in themed container — layer surface has no window chrome.
            let header = widget::container(header)
                .class(style::Container::WindowBackground)
                .width(iced::Length::Fill);

            view = widget::column::with_children(vec![header.into(), view]).into();
        }

        view
    }
}
