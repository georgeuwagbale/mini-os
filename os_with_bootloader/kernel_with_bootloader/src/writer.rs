//writer.rs
mod constants;

use core::{
    fmt::{self, Arguments, Write},
    ptr,
};

use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use constants::font_constants;
use constants::font_constants::{BACKUP_CHAR, CHAR_RASTER_HEIGHT, FONT_WEIGHT};
use lazy_static::lazy_static;
use noto_sans_mono_bitmap::{get_raster, RasterizedChar};
use spin::Mutex;

/// Additional vertical space between lines
const LINE_SPACING: usize = 2;

/// Additional horizontal space between characters.
const LETTER_SPACING: usize = 0;

// Tab spacing (horizontal spacing)
const TAB_SPACING: usize = 30;

/// Padding from the border. Prevent that font is too close to border.
const BORDER_PADDING: usize = 5;

/// Returns the raster of the given char or the raster of [`font_constants::BACKUP_CHAR`].
pub fn get_char_raster(c: char) -> RasterizedChar {
    fn get(c: char) -> Option<RasterizedChar> {
        get_raster(c, FONT_WEIGHT, CHAR_RASTER_HEIGHT)
    }
    get(c).unwrap_or_else(|| get(BACKUP_CHAR).expect("Should get raster of backup char."))
}

/// Allows logging text to a pixel-based framebuffer.
pub struct FrameBufferWriter {
    framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
}

lazy_static! {
    pub static ref FRAME_BUFFER_WRITER: Mutex<Option<FrameBufferWriter>> = Mutex::new(None);
}

impl FrameBufferWriter {
    /// Creates a new logger that uses the given framebuffer.
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) {
        // Initialize the frame_buffer_writer if it's None
        let frame_buffer_writer = FrameBufferWriter::new_(framebuffer, info);
        if FRAME_BUFFER_WRITER.lock().is_none() {
            *FRAME_BUFFER_WRITER.lock() = Some(frame_buffer_writer);
        }
    }

    /// Creates a new logger that uses the given framebuffer.
    pub fn new_(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut logger = Self {
            framebuffer,
            info,
            x_pos: 0,
            y_pos: 0,
        };

        logger.clear();
        logger
    }

    fn newline(&mut self) {
        self.y_pos += font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        self.carriage_return()
    }

    fn carriage_return(&mut self) {
        self.x_pos = BORDER_PADDING;
    }

    fn width(&self) -> usize {
        self.info.width
    }

    fn height(&self) -> usize {
        self.info.height
    }

    fn tab(&mut self) {
        self.x_pos += TAB_SPACING;
    }

    pub fn set_y_pos(&mut self, y: usize) {
        self.y_pos = y;
    }

    pub fn set_x_pos(&mut self, x: usize) {
        self.x_pos = x;
    }

    pub fn cursor_left(&mut self) {
        if self.x_pos > BORDER_PADDING {
            // Move the cursor back by one character width
            self.x_pos -= font_constants::CHAR_RASTER_WIDTH;
        } else {
            if self.y_pos
                >= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING + BORDER_PADDING
            {
                self.x_pos = self.width() - (font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING);
                self.y_pos -= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
            } else {
                // Already at the top-left position, can't go back further
                return;
            }
        }
    }

    pub fn cursor_right(&mut self) {
        if self.x_pos + font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING < self.width() {
            // Move the cursor forward by one character width
            self.x_pos += font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING;
        } else {
            // Reached the end of the line, move to the next line if possible
            self.newline();
        }
    }

    pub fn cursor_up(&mut self) {
        if self.y_pos >= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING + BORDER_PADDING {
            // Move the cursor up by one line height
            self.y_pos -= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        } else {
            // Already at the top position, can't go up further
            return;
        }
    }

    pub fn cursor_down(&mut self) {
        if self.y_pos + font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING < self.height() {
            // Move the cursor down by one line height
            self.y_pos += font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        } else {
            // Reached the bottom of the screen, can't go down further
            return;
        }
    }

    pub fn backspace(&mut self) {
        if self.x_pos > BORDER_PADDING {
            // Move the cursor back by one character width
            self.x_pos -= font_constants::CHAR_RASTER_WIDTH;
        } else {
            if self.y_pos
                >= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING + BORDER_PADDING
            {
                self.x_pos = self.width() - (font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING);
                self.y_pos -= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
            } else {
                // Already at the top-left position, can't go back further
                return;
            }
        }
        // Clear the pixel data of the last character
        for y in 0..font_constants::CHAR_RASTER_HEIGHT.val() {
            for x in 0..font_constants::CHAR_RASTER_WIDTH {
                self.write_pixel(self.x_pos + x, self.y_pos + y, 0);
            }
        }
    }

    /// Erases all text on the screen. Resets `self.x_pos` and `self.y_pos`.
    pub fn clear(&mut self) {
        self.x_pos = BORDER_PADDING;
        self.y_pos = BORDER_PADDING;
        self.framebuffer.fill(0);
    }

    /// Writes a single char to the framebuffer. Takes care of special control characters, such as
    /// newlines and carriage returns.
    pub fn write_char(&mut self, c: char) {
        match c {
            '\t' => self.tab(),
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                let new_xpos = self.x_pos + font_constants::CHAR_RASTER_WIDTH;
                if new_xpos >= self.width() {
                    self.newline();
                }
                let new_ypos =
                    self.y_pos + font_constants::CHAR_RASTER_HEIGHT.val() + BORDER_PADDING;
                if new_ypos >= self.height() {
                    self.clear();
                }
                self.write_rendered_char(get_char_raster(c));
            }
        }
    }

    /// Prints a rendered char into the framebuffer.
    /// Updates `self.x_pos`.
    pub fn write_rendered_char(&mut self, rendered_char: RasterizedChar) {
        for (y, row) in rendered_char.raster().iter().enumerate() {
            for (x, byte) in row.iter().enumerate() {
                self.write_pixel(self.x_pos + x, self.y_pos + y, *byte);
            }
        }
        self.x_pos += rendered_char.width() + LETTER_SPACING;
    }
    

    fn write_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let pixel_offset = (y * self.info.stride) + x; // Added bracket
        let color = match self.info.pixel_format {
            PixelFormat::Rgb => [intensity, intensity, intensity / 2, 0],
            PixelFormat::Bgr => [intensity / 2, intensity, intensity, 0],
            PixelFormat::U8 => [if intensity > 200 { 0xf } else { 0 }, 0, 0, 0],
            other => {
                // set a supported (but invalid) pixel format before panicking to avoid a double
                // panic; it might not be readable though
                self.info.pixel_format = PixelFormat::Rgb;
                panic!("pixel format {:?} not supported in logger", other)
            }
        };
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;
        self.framebuffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
        let _ = unsafe { ptr::read_volatile(&self.framebuffer[byte_offset]) };
    }
}

unsafe impl Send for FrameBufferWriter {}
unsafe impl Sync for FrameBufferWriter {}

impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::writer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: Arguments) {
    // Use the global frame_buffer_writer
    use x86_64::instructions::interrupts;

    // To avoid deadlock, I disabled interrupts as long as the Mutex is locked
    interrupts::without_interrupts(|| {
        if let Some(frame_buffer_writer) = FRAME_BUFFER_WRITER.lock().as_mut() {
            frame_buffer_writer.write_fmt(args).unwrap();
        }
    });
    // The `without_interrupts` function takes a closure and executes it in an interrupt-free environment.
}
