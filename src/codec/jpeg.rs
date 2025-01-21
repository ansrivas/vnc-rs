use crate::{PixelFormat, Rect, VncError, VncEvent};
use std::future::Future;
use tokio::io::{AsyncRead, AsyncReadExt};

use std::error::Error;
use std::io::{Read, Write};

use super::uninit_vec;

pub struct Decoder {}

impl Decoder {
    pub fn new() -> Self {
        Self {}
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
        // +----------------------------+--------------+-------------+
        // | No. of bytes               | Type [Value] | Description |
        // +----------------------------+--------------+-------------+
        // | width*height*bytesPerPixel | PIXEL array  | pixels      |
        // +----------------------------+--------------+-------------+
        let bpp = format.bits_per_pixel / 8;
        let buffer_size = bpp as usize * rect.height as usize * rect.width as usize;
        let mut pixels = uninit_vec(buffer_size);
        input.read_exact(&mut pixels).await?;
        output_func(VncEvent::JpegImage(*rect, pixels)).await?;
        Ok(())
    }
}

pub struct JPEGDecoder {
    cached_quant_tables: Vec<Vec<u8>>,
    cached_huffman_tables: Vec<Vec<u8>>,
    segments: Vec<Vec<u8>>,
}

impl JPEGDecoder {
    pub fn new() -> Self {
        JPEGDecoder {
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
        // sock: &mut R,
        // display: &mut W,
    ) -> Result<(), VncError> {
        // A rect of JPEG encodings is simply a JPEG file
        loop {
            let segment = self.read_segment(sock)?;
            if segment.is_none() {
                return Ok(false);
            }
            let segment = segment.unwrap();
            self.segments.push(segment.clone());

            // End of image?
            if segment[1] == 0xD9 {
                break;
            }
        }

        let mut huffman_tables = Vec::new();
        let mut quant_tables = Vec::new();
        for segment in &self.segments {
            let segment_type = segment[1];
            if segment_type == 0xC4 {
                // Huffman tables
                huffman_tables.push(segment.clone());
            } else if segment_type == 0xDB {
                // Quantization tables
                quant_tables.push(segment.clone());
            }
        }

        let sof_index = self
            .segments
            .iter()
            .position(|x| x[1] == 0xC0 || x[1] == 0xC2);
        if sof_index.is_none() {
            return Err("Illegal JPEG image without SOF".into());
        }
        let sof_index = sof_index.unwrap();

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

        let total_length: usize = self.segments.iter().map(|seg| seg.len()).sum();
        let mut data = Vec::with_capacity(total_length);
        for segment in &self.segments {
            data.extend(segment);
        }

        display.write_all(&data)?;

        if !huffman_tables.is_empty() {
            self.cached_huffman_tables = huffman_tables;
        }
        if !quant_tables.is_empty() {
            self.cached_quant_tables = quant_tables;
        }

        self.segments.clear();

        Ok(true)
    }

    fn read_segment<R: Read>(&mut self, sock: &mut R) -> Result<Option<Vec<u8>>, Box<dyn Error>> {
        let mut marker_buf = [0u8; 2];
        if sock.read_exact(&mut marker_buf).is_err() {
            return Ok(None);
        }

        let marker = marker_buf[0];
        if marker != 0xFF {
            return Err(format!("Illegal JPEG marker received (byte: {})", marker).into());
        }

        let segment_type = marker_buf[1];
        if (0xD0..=0xD9).contains(&segment_type) || segment_type == 0x01 {
            // No length after marker
            return Ok(Some(vec![marker, segment_type]));
        }

        let mut length_buf = [0u8; 2];
        sock.read_exact(&mut length_buf)?;
        let length = u16::from_be_bytes(length_buf);
        if length < 2 {
            return Err(format!("Illegal JPEG length received (length: {})", length).into());
        }

        let mut segment_data = vec![0u8; (length - 2) as usize];
        sock.read_exact(&mut segment_data)?;

        let mut segment = vec![marker, segment_type];
        segment.extend_from_slice(&length_buf);
        segment.extend(segment_data);

        Ok(Some(segment))
    }
}
