use core::ptr::NonNull;

use alloc::{vec, boxed::Box};
use embedded_graphics::{prelude::{DrawTarget, OriginDimensions, Size}, pixelcolor::RgbColor, Pixel};
use tinybmp::{RawBmp, Bpp, ColorTable};
use uefi::{proto::console::gop::{BltOp, BltPixel, GraphicsOutput, BltRegion}, prelude::BootServices};

fn ensure_supported_bmp<'a>(splash: &RawBmp) -> uefi::Result<ColorTable<'a>> {
    Ok(())
}

struct GOPWrapper(GraphicsOutput);

impl OriginDimensions for GOPWrapper {
    fn size(&self) -> Size {
        let sz = self.0.current_mode_info().resolution();
        return (sz.0 as u32, sz.1 as u32).into()
    }
}

impl DrawTarget for GOPWrapper {
    type Color = RgbColor;
    type Error = uefi::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>> {
        let (width, height) = self.size().into();
        for Pixel(coord, color) in pixels.into_iter() {
            // https://github.com/rust-lang/rust/issues/37854
            if let Ok((x, y)) = coord.try_into() {
                if x >= 0 && x < width && y >= 0 && y < height {
                    let index: u32 = x + y * height;
                    unsafe { self.0.frame_buffer().write_byte(index as usize, color); }
                }
            }
        }
        Ok(())
    }

    fn fill_solid(&mut self, area: &embedded_graphics::primitives::Rectangle, color: Self::Color) -> Result<(), Self::Error> {
        
    }

    fn fill_contiguous<I>(&mut self, area: &embedded_graphics::primitives::Rectangle, colors: I) -> Result<(), Self::Error>
        where
            I: IntoIterator<Item = Self::Color>, {
        
    }
}

fn graphics_splash(boot_services: &BootServices, contents: &[u8]) -> uefi::Result {
    let mut gop = boot_services.open_protocol_exclusive::<GraphicsOutput>(
        boot_services.get_handle_for_protocol::<GraphicsOutput>()?
    )?;
    let current_mode = gop.current_mode_info();

    let splash_bmp = tinybmp::RawBmp::from_slice(contents)
        .map_err(|_err| uefi::Status::LOAD_ERROR)?;

    let splash_header = splash_bmp.header();
    let splash_color_table = ensure_supported_bmp(&splash_bmp)?;

    // Center it if possible.
    let cur_resolution = current_mode.resolution();
    let splash_size: (usize, usize) = (splash_header.image_size.width.try_into().unwrap(), splash_header.image_size.height.try_into().unwrap());
    // FIXME: properly error handle usize < u32... :)
    let x_pos = (cur_resolution.0 - core::cmp::min(splash_size.0, cur_resolution.0)) / 2;
    let y_pos = (cur_resolution.1 - core::cmp::min(splash_size.1, cur_resolution.1)) / 2;

    let background = BltOp::VideoFill {
            color: BltPixel::new(0x0, 0x0, 0x0),
            dest: (0, 0),
            dims: current_mode.resolution()
    };

    // Blit the background
    gop.blt(background)?;

    // Read the current contents to do alpha blending
    let mut current_contents = vec![BltPixel::new(0, 0, 0); splash_size.0 * splash_size.1].into_boxed_slice();

    gop.blt(BltOp::VideoToBltBuffer {
        buffer: &mut (*current_contents),
        src: (x_pos, y_pos),
        dest: BltRegion::Full,
        dims: splash_size
    })?;

    // Transform the current contents into the buffer to blt containing the bmp
    // SAFETY: `current_contents` is big enough to hold the BMP contents.
    unsafe { write_bmp_to_contents(splash_bmp, &splash_color_table, &mut (*current_contents))?; }

    // Blit the BMP
    gop.blt(BltOp::BufferToVideo {
        buffer: &(*current_contents),
        src: BltRegion::Full,
        dest: (x_pos, y_pos),
        dims: splash_size
    })
}
