use alacritty_terminal::{
    config::{Config, PtyConfig},
    event::{Event, EventListener, Notify, WindowSize},
    event_loop::{EventLoop, Notifier},
    grid::Dimensions,
    index::{Column, Line, Point},
    sync::FairMutex,
    tty, Term,
};
use std::sync::{mpsc, Arc};

struct TermSize {
    rows: usize,
    cols: usize,
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.rows
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

#[derive(Clone)]
struct EventProxy(mpsc::Sender<Event>);

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        let _ = self.0.send(event);
    }
}

fn main() {
    let config = Config::default();
    let dimensions = TermSize { rows: 24, cols: 80 };
    let (event_tx, event_rx) = mpsc::channel();
    let event_proxy = EventProxy(event_tx);
    let term = Term::new(&config, &dimensions, event_proxy.clone());
    let term = Arc::new(FairMutex::new(term));

    let pty_config = PtyConfig::default();
    let window_size = WindowSize {
        num_lines: dimensions.rows as u16,
        num_cols: dimensions.cols as u16,
        cell_width: 8,   /*TODO*/
        cell_height: 16, /*TODO*/
    };
    let window_id = 0;
    let pty = tty::new(&pty_config, window_size, window_id).unwrap();

    let event_loop = EventLoop::new(term.clone(), event_proxy, pty, pty_config.hold, false);
    let notifier = Notifier(event_loop.channel());
    let join_handle = event_loop.spawn();

    for i in 0..dimensions.rows {
        notifier.notify(format!("echo {}\r", i).into_bytes());
    }
    notifier.notify(&b"exit 0\r"[..]);

    loop {
        let event = match event_rx.recv() {
            Ok(ok) => ok,
            Err(err) => {
                eprintln!("failed to recv event: {}", err);
                break;
            }
        };
        println!("{:?}", event);
        match event {
            Event::PtyWrite(text) => notifier.notify(text.into_bytes()),
            Event::Exit => break,
            _ => {}
        }

        let mut last_point = Point::new(Line(0), Column(0));
        let mut text = String::new();
        for indexed in term.lock().grid().display_iter() {
            if indexed.point.line != last_point.line {
                println!("{:2}: {:?}", last_point.line.0, text);
                text.clear();
            }
            //TODO: use indexed.point.column?
            text.push(indexed.cell.c);
            last_point = indexed.point;
        }
        println!("{:2}: {:?}", last_point.line.0, text);
    }

    join_handle.join().unwrap();
}
