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
        self.read_all_segments(input).await?;
        self.insert_cached_tables();

        let jpeg_data = self.combine_segments();
        let decoded = self.decode_jpeg_to_format(&jpeg_data, format)?;

        output_func(VncEvent::RawImage(*rect, decoded)).await?;
        self.cache_tables();
        self.segments.clear();

        Ok(())
    }

    async fn read_all_segments<S: AsyncRead + Unpin>(
        &mut self,
        input: &mut S,
    ) -> Result<(), VncError> {
        loop {
            match self.read_segment(input).await? {
                Some(segment) => {
                    if segment[1] == 0xD9 {
                        self.segments.push(segment);
                        break;
                    }
                    self.segments.push(segment);
                }
                None => break,
            }
        }
        Ok(())
    }

    fn insert_cached_tables(&mut self) {
        if !self.contains_marker(0xC0) && !self.contains_marker(0xC2) {
            return; // No SOF marker, invalid JPEG
        }

        let sof_index = self
            .segments
            .iter()
            .position(|seg| seg[1] == 0xC0 || seg[1] == 0xC2)
            .unwrap();

        if !self.contains_marker(0xDB) {
            self.segments.splice(
                sof_index + 1..sof_index + 1,
                self.cached_quant_tables.clone(),
            );
        }
        if !self.contains_marker(0xC4) {
            self.segments.splice(
                sof_index + 1..sof_index + 1,
                self.cached_huffman_tables.clone(),
            );
        }
    }

    fn contains_marker(&self, marker: u8) -> bool {
        self.segments.iter().any(|seg| seg[1] == marker)
    }

    fn combine_segments(&self) -> Vec<u8> {
        let total_length: usize = self.segments.iter().map(Vec::len).sum();
        let mut jpeg_data = Vec::with_capacity(total_length);
        for segment in &self.segments {
            jpeg_data.extend_from_slice(segment);
        }
        jpeg_data
    }

    fn decode_jpeg_to_format(
        &self,
        jpeg_data: &[u8],
        format: &PixelFormat,
    ) -> Result<Vec<u8>, VncError> {
        let mut decoder = zune_jpeg::JpegDecoder::new(jpeg_data);
        let pixels = decoder.decode().map_err(|_| VncError::InvalidImageData)?;

        let bpp = format.bits_per_pixel / 8;
        let mut output = Vec::with_capacity(pixels.len() / 3 * bpp as usize);

        for chunk in pixels.chunks_exact(3) {
            let (r, g, b) = (chunk[0], chunk[1], chunk[2]);
            let pixel_value = if format.true_color_flag > 0 {
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
        let marker = input.read_u8().await?;
        if marker != 0xFF {
            return Err(VncError::InvalidImageData);
        }

        let segment_type = input.read_u8().await?;
        if (0xD0..=0xD9).contains(&segment_type) || segment_type == 0x01 {
            return Ok(Some(vec![marker, segment_type]));
        }

        let length = input.read_u16().await? as usize;
        if length < 2 {
            return Err(VncError::InvalidImageData);
        }

        let mut segment = Vec::with_capacity(length + 2);
        segment.extend_from_slice(&[marker, segment_type]);
        segment.extend_from_slice(&(length as u16).to_be_bytes());

        let mut data = vec![0u8; length - 2];
        input.read_exact(&mut data).await?;
        segment.extend(data);

        Ok(Some(segment))
    }

    fn cache_tables(&mut self) {
        self.cached_quant_tables = self
            .segments
            .iter()
            .filter(|seg| seg[1] == 0xDB)
            .cloned()
            .collect();

        self.cached_huffman_tables = self
            .segments
            .iter()
            .filter(|seg| seg[1] == 0xC4)
            .cloned()
            .collect();
    }
}
