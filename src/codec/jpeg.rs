use crate::{PixelFormat, Rect, VncError, VncEvent};
use std::future::Future;
use tokio::io::{AsyncRead, AsyncReadExt};

pub struct Decoder {
    cached_quant_tables: Vec<Vec<u8>>,
    cached_huffman_tables: Vec<Vec<u8>>,
    segments: Vec<Vec<u8>>,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            cached_quant_tables: Vec::new(),
            cached_huffman_tables: Vec::new(),
            segments: Vec::new(),
        }
    }

    pub async fn decode<S, F, Fut>(
        &mut self,
        format: &PixelFormat,
        rect: &Rect,
        input: &mut S,
        output_func: &F,
    ) -> Result<(), VncError>
    where
        S: AsyncRead + Unpin,
        F: Fn(VncEvent) -> Fut,
        Fut: Future<Output = Result<(), VncError>>,
    {
        // Read JPEG segments
        loop {
            let segment = self.read_segment(input).await?;
            if segment.is_none() {
                return Ok(());
            }
            let segment = segment.unwrap();
            self.segments.push(segment.clone());

            if segment[1] == 0xD9 {
                break;
            }
        }

        // Process tables
        let mut huffman_tables = Vec::new();
        let mut quant_tables = Vec::new();

        for segment in &self.segments {
            match segment[1] {
                0xC4 => huffman_tables.push(segment.clone()),
                0xDB => quant_tables.push(segment.clone()),
                _ => {}
            }
        }

        // Find SOF marker
        let sof_index = self
            .segments
            .iter()
            .position(|x| x[1] == 0xC0 || x[1] == 0xC2)
            .ok_or(VncError::InvalidImageData)?;

        // Insert cached tables if needed
        if quant_tables.is_empty() {
            self.segments.splice(
                sof_index + 1..sof_index + 1,
                self.cached_quant_tables.clone(),
            );
        }
        if huffman_tables.is_empty() {
            self.segments.splice(
                sof_index + 1..sof_index + 1,
                self.cached_huffman_tables.clone(),
            );
        }

        // Combine segments into JPEG data
        let total_length: usize = self.segments.iter().map(|seg| seg.len()).sum();
        let mut jpeg_data = Vec::with_capacity(total_length);

        for segment in &self.segments {
            jpeg_data.extend_from_slice(segment);
        }

        // Convert JPEG to target pixel format
        let decoded = self.decode_jpeg_to_format(&jpeg_data, format)?;

        // Send image event
        output_func(VncEvent::RawImage(*rect, decoded)).await?;

        // Cache tables for future use
        if !huffman_tables.is_empty() {
            self.cached_huffman_tables = huffman_tables;
        }
        if !quant_tables.is_empty() {
            self.cached_quant_tables = quant_tables;
        }

        self.segments.clear();
        Ok(())
    }

    fn decode_jpeg_to_format(
        &self,
        jpeg_data: &[u8],
        format: &PixelFormat,
    ) -> Result<Vec<u8>, VncError> {
        // Create output buffer based on pixel format
        let bpp = format.bits_per_pixel / 8;
        let mut output = Vec::new();

        // Decode JPEG data
        // let decoded = jpeg_decode::Decoder::new(jpeg_data)
        //     .decode()
        //     .map_err(|_| VncError::InvalidImageData)?;
        let mut decoder = zune_jpeg::JpegDecoder::new(jpeg_data);
        // decode the file
        let pixels = decoder.decode().unwrap();

        // Convert RGB to target pixel format
        for pixel in pixels.chunks(3) {
            let r = pixel[0];
            let g = pixel[1];
            let b = pixel[2];

            let pixel_value = if format.true_color_flag > 0 {
                // True color conversion
                let r = ((r as u32 * format.red_max as u32) / 255) << format.red_shift;
                let g = ((g as u32 * format.green_max as u32) / 255) << format.green_shift;
                let b = ((b as u32 * format.blue_max as u32) / 255) << format.blue_shift;

                let value = r | g | b;
                if format.big_endian_flag > 0 {
                    value.to_be_bytes()
                } else {
                    value.to_le_bytes()
                }
            } else {
                // Indexed color (not typically used for JPEG)
                [r, g, b, 255]
            };

            output.extend_from_slice(&pixel_value[..bpp as usize]);
        }

        Ok(output)
    }

    async fn read_segment<S: AsyncRead + Unpin>(
        &mut self,
        input: &mut S,
    ) -> Result<Option<Vec<u8>>, VncError> {
        // Read marker
        let marker = input.read_u8().await?;
        if marker != 0xFF {
            return Err(VncError::InvalidImageData);
        }

        let segment_type = input.read_u8().await?;

        // Handle markers with no length
        if (0xD0..=0xD9).contains(&segment_type) || segment_type == 0x01 {
            return Ok(Some(vec![marker, segment_type]));
        }

        // Read length and data
        let length = input.read_u16().await? as usize;
        if length < 2 {
            return Err(VncError::InvalidImageData);
        }

        let mut segment = vec![marker, segment_type];
        segment.extend_from_slice(&(length as u16).to_be_bytes());

        let mut data = vec![0u8; length - 2];
        input.read_exact(&mut data).await?;
        segment.extend(data);

        Ok(Some(segment))
    }
}
