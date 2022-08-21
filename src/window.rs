use tracing::info;
use x11rb::{
    connection::Connection,
    protocol::xproto::{
        AtomEnum, BackingStore, Circulate, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask,
        PropMode, WindowClass,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

pub struct XConnection {
    connection: RustConnection,
    screen_num: usize,
}

impl XConnection {
    pub fn new() -> Self {
        let (connection, screen_num) = x11rb::connect(None).unwrap();
        Self {
            connection,
            screen_num,
        }
    }

    pub fn create_window(&self) -> anyhow::Result<usize> {
        info!("creating window");

        let screen = &self.connection.setup().roots[self.screen_num];
        let win_id = self.connection.generate_id().unwrap();
        let gc_id = self.connection.generate_id().unwrap();

        let win_aux = CreateWindowAux::new()
            .event_mask(EventMask::EXPOSURE | EventMask::STRUCTURE_NOTIFY)
            .backing_store(BackingStore::ALWAYS)
            .save_under(Some(false.into()));

        let gc_aux = CreateGCAux::new().foreground(screen.black_pixel);

        let (width, height) = (screen.width_in_pixels, screen.height_in_pixels);

        self.connection.create_window(
            screen.root_depth,
            win_id,
            screen.root,
            0,
            0,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &win_aux,
        )?;

        self.connection
            .circulate_window(Circulate::LOWER_HIGHEST, win_id)?;

        let xa = self
            .connection
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE")?
            .reply()?
            .atom;

        let prop = self
            .connection
            .intern_atom(false, b"_NET_WM_WINDOW_TYPE_DESKTOP")?
            .reply()?
            .atom;

        self.connection.change_property32(
            PropMode::REPLACE,
            win_id,
            xa,
            AtomEnum::ATOM,
            &[prop],
        )?;

        self.connection.create_gc(gc_id, win_id, &gc_aux)?;
        self.connection.map_window(win_id)?;
        self.connection.flush()?;
        self.connection.sync()?;

        info!(window_id = win_id.to_string(), "created window");

        Ok(win_id.try_into()?)
    }
}
