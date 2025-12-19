use cosmic::cctk::{
    wayland_client::{
        Connection, Dispatch, QueueHandle, delegate_noop,
        globals::{GlobalListContents, registry_queue_init},
        protocol::{wl_registry, wl_surface::WlSurface},
    },
    wayland_protocols::xdg::system_bell::v1::client::xdg_system_bell_v1::XdgSystemBellV1,
};

#[derive(Clone, Debug)]
pub struct Bell {
    bell: XdgSystemBellV1,
    surface: WlSurface,
    conn: Connection,
}

impl Bell {
    pub fn ring(&self) {
        self.bell.ring(Some(&self.surface));
        let _ = self.conn.flush();
    }
}

struct State {}

pub fn bind_bell(conn: &Connection, surface: &WlSurface) -> Option<Bell> {
    // XXX unwrap
    let (globals, event_queue) = registry_queue_init::<State>(&conn).unwrap();
    let qh = event_queue.handle();
    match globals.bind(&qh, 1..=1, ()) {
        Ok(bell) => Some(Bell {
            bell,
            surface: surface.clone(),
            conn: conn.clone(),
        }),
        Err(_) => None,
    }
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
    fn event(
        _: &mut State,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<State>,
    ) {
        // Ignore globals added after initialization
    }
}

delegate_noop!(State: XdgSystemBellV1);
