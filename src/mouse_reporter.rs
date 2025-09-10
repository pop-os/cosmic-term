use cosmic::{
    iced::mouse::{Event as MouseEvent, ScrollDelta},
    iced::{Event, keyboard::Modifiers, mouse::Button},
};

use crate::terminal::Terminal;

const SCROLL_SPEED: u32 = 3;

#[derive(Default)]
pub struct MouseReporter {
    last_movment_x: Option<u32>,
    last_movment_y: Option<u32>,
    accumulated_scroll_x: f32,
    accumulated_scroll_y: f32,
    button: Option<Button>,
}

impl MouseReporter {
    fn button_number(button: Button) -> Option<u8> {
        match button {
            Button::Left => Some(0),
            Button::Middle => Some(1),
            Button::Right => Some(2),
            _ => None,
        }
    }

    //Implemented according to
    //https://invisible-island.net/xterm/ctlseqs/ctlseqs.html#h2-Mouse-Tracking
    pub fn normal_mouse_code(
        &mut self,
        event: Event,
        modifiers: &Modifiers,
        is_utf8: bool,
        x: u32,
        y: u32,
    ) -> Option<Vec<u8>> {
        //Buttons are handle slightly different between normal and sgr
        //for normal/utf8 the button release is always reported as button 3
        let mut button = (match event {
            Event::Mouse(MouseEvent::ButtonPressed(b)) => {
                self.button = Some(b);
                Self::button_number(b)
            }
            Event::Mouse(MouseEvent::ButtonReleased(_b)) => {
                self.button = None;
                Some(3)
            }
            Event::Mouse(MouseEvent::CursorMoved { .. }) => {
                //Button pressed is reported as button 32 + 0,1,2 and event code M
                //And only reported if a button is previously pressed
                if (self.last_movment_x, self.last_movment_y) == (Some(x), Some(y)) {
                    return None;
                } else {
                    self.last_movment_x = Some(x);
                    self.last_movment_y = Some(y);
                }
                //It seems that we should add 32 to signal movement even for normal mode
                //On button-motion events, xterm adds 32 to the event code (the third
                //character, Cb).
                //For example, motion into cell x,y with button 1 down is reported as
                //CSI M @ CxCy ( @  = 32 + 0 (button 1) + 32 (motion indicator) ).
                self.button.and_then(Self::button_number).map(|b| b + 32)
            }
            _ => None,
        })?;

        if modifiers.shift() {
            button += 4;
        }
        if modifiers.alt() {
            button += 8;
        }
        if modifiers.control() {
            button += 16;
        }

        //Normal mode have a max of 223 (255 - 32), while utf8 extend this to 2015
        let max_point: usize = if is_utf8 { 2015 } else { 223 };
        if x as usize >= max_point || y as usize >= max_point {
            return None;
        }

        let utf8_encode_and_append = |mut pos: u32, dest: &mut Vec<u8>| {
            pos += 1 + 32;
            let mut utf8 = [0; 2]; //This is large enough since we have a max of 2015
            dest.extend_from_slice(
                (char::from_u32(pos).unwrap()) //This unwrap and encode_utf8 is safe due to our
                    //specific range, pos will max be 2047
                    .encode_utf8(&mut utf8)
                    .as_bytes(),
            );
        };

        //SPEC: Likewise, Cb will be UTF-8 encoded, to reduce confusion with wheel mouse events.
        //Always, or only when the the cooardinates is used? No other terminal seems to do this a
        //all? Doing what they are doing for now.
        let mut buf: Vec<u8> = vec![b'\x1b', b'[', b'M', 32 + button];
        //Should we remove 32+button from previous line, and use this instead? Or only on >= 95
        //utf8_encode_and_append(32 + button, &mut buf);

        //For utf8 spec say: For positions less than 95, the resulting output is identical under both modes.
        //But also: Under normal mouse mode, positions outside (160,94) result in byte pairs which can be interpreted as a single UTF-8
        if is_utf8 && x >= 95 {
            utf8_encode_and_append(x, &mut buf);
        } else {
            //SPEC: For positions less than 95, the resulting output is identical under both modes.
            buf.push(32 + 1 + x as u8);
        }

        if is_utf8 && y >= 95 {
            utf8_encode_and_append(y, &mut buf);
        } else {
            //SPEC For positions less than 95, the resulting output is identical under both modes.
            buf.push(32 + 1 + y as u8);
        }
        Some(buf)
    }

    pub fn sgr_mouse_code(
        &mut self,
        event: Event,
        modifiers: &Modifiers,
        x: u32,
        y: u32,
    ) -> Option<Vec<u8>> {
        let (button_no, event_code) = (match event {
            Event::Mouse(MouseEvent::ButtonPressed(button)) => {
                //Button pressed is reported as button 0,1,2 and event code M
                self.button = Some(button);
                Some((Self::button_number(button), "M"))
            }
            Event::Mouse(MouseEvent::ButtonReleased(button)) => {
                //Button pressed is reported as button 0,1,2 and event code m
                self.button = None;
                Some((Self::button_number(button), "m"))
            }
            Event::Mouse(MouseEvent::CursorMoved { .. }) => {
                //Button pressed is reported as button 32 + 0,1,2 and event code M
                //And only reported if a button is previously pressed
                if (self.last_movment_x, self.last_movment_y) == (Some(x), Some(y)) {
                    return None;
                } else {
                    self.last_movment_x = Some(x);
                    self.last_movment_y = Some(y);
                }
                self.button
                    .map(|button| (Self::button_number(button).map(|b| b + 32), "M"))
            }
            _ => None,
        })?;

        if let Some(mut button_no) = button_no {
            if modifiers.shift() {
                button_no += 4;
            }
            if modifiers.alt() {
                button_no += 8;
            }
            if modifiers.control() {
                button_no += 16;
            }
            let term_code = format!("\x1b[<{};{};{}{}", button_no, x + 1, y + 1, event_code);
            Some(term_code.as_bytes().to_vec())
        } else {
            None
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn sgr_mouse_wheel_scroll(
        &mut self,
        term_cell_width: f32,
        term_cell_height: f32,
        delta: ScrollDelta,
        modifiers: &Modifiers,
        x: u32,
        y: u32,
    ) -> impl Iterator<Item = Vec<u8>> {
        let (lines_x, lines_y) = match delta {
            ScrollDelta::Lines { x, y } => (x as i32, y as i32),
            ScrollDelta::Pixels { x, y } => {
                //Accumulate change
                self.accumulated_scroll_x += x / term_cell_width;
                self.accumulated_scroll_y += y / term_cell_height;

                //Resolve lines crossed
                let lines_x = self.accumulated_scroll_x as i32;
                let lines_y = self.accumulated_scroll_y as i32;

                //Subtract accounted lines from accumulators
                self.accumulated_scroll_x -= lines_x as f32;
                self.accumulated_scroll_y -= lines_y as f32;

                (lines_x, lines_y)
            }
        };

        //Resolve modifier flags
        let mut modifier_flags = 0;

        if modifiers.shift() {
            modifier_flags += 4;
        }
        if modifiers.alt() {
            modifier_flags += 8;
        }
        if modifiers.control() {
            modifier_flags += 16;
        };

        //Resolve base inputs
        let button_no_y = match lines_y.cmp(&0) {
            std::cmp::Ordering::Less => 65,    //Wheel Down
            std::cmp::Ordering::Greater => 64, //Wheel Up
            std::cmp::Ordering::Equal => 0,    //Unused
        };

        let button_no_x = match lines_x.cmp(&0) {
            std::cmp::Ordering::Less => 66,    //Wheel Left
            std::cmp::Ordering::Greater => 67, //Wheel Right
            std::cmp::Ordering::Equal => 0,    //Unused
        };

        //Generate term codes
        let x_iter = std::iter::repeat(button_no_x).take(lines_x.unsigned_abs() as _);
        let y_iter = std::iter::repeat(button_no_y).take(lines_y.unsigned_abs() as _);

        x_iter
            .chain(y_iter)
            .map(move |button_no| button_no + modifier_flags)
            .map(move |button_no| {
                let term_code = format!("\x1b[<{};{};{}M", button_no, x + 1, y + 1);
                term_code.as_bytes().to_vec()
            })
    }

    //Emulate mouse wheel scroll with up/down arrows. Using mouse spec uses
    //scroll-back and scroll-forw actions, which moves whole windows like page up/page down.
    pub fn report_mouse_wheel_as_arrows(
        terminal: &Terminal,
        term_cell_width: f32,
        term_cell_height: f32,
        delta: ScrollDelta,
    ) {
        let (_delta_x, delta_y) = match delta {
            ScrollDelta::Lines { x, y } => (x, y),
            ScrollDelta::Pixels { x, y } => (x / term_cell_width, y / term_cell_height),
        };
        //Send delta_y * SCROLL_SPEED number of Up/Down arrows
        for _ in 0..(delta_y.abs() as u32 * SCROLL_SPEED) {
            if delta_y > 0.0 {
                terminal.input_no_scroll(b"\x1B[A".as_slice())
            } else if delta_y < 0.0 {
                terminal.input_no_scroll(b"\x1B[B".as_slice())
            }
        }
    }
}
