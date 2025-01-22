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
        // Read and process JPEG segments
        loop {
            let segment = self.read_segment(input).await?;
            if segment.is_none() {
                return Ok(());
            }
            let segment = segment.unwrap();
            self.segments.push(segment.clone());

            // End of image marker
            if segment[1] == 0xD9 {
                break;
            }
        }

        // Process Huffman and quantization tables
        let mut huffman_tables = Vec::new();
        let mut quant_tables = Vec::new();

        for segment in &self.segments {
            let segment_type = segment[1];
            match segment_type {
                0xC4 => huffman_tables.push(segment.clone()), // Huffman tables
                0xDB => quant_tables.push(segment.clone()),   // Quantization tables
                _ => {}
            }
        }

        // Find Start of Frame marker
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

        // Combine all segments into final JPEG data
        let total_length: usize = self.segments.iter().map(|seg| seg.len()).sum();
        let mut jpeg_data = Vec::with_capacity(total_length);
        for segment in &self.segments {
            jpeg_data.extend_from_slice(segment);
        }

        // Send JPEG image event
        output_func(VncEvent::JpegImage(*rect, jpeg_data)).await?;

        // Cache tables for future use
        if !huffman_tables.is_empty() {
            self.cached_huffman_tables = huffman_tables;
        }
        if !quant_tables.is_empty() {
            self.cached_quant_tables = quant_tables;
        }

        // Clear segments for next image
        self.segments.clear();

        Ok(())
    }

    async fn read_segment<S: AsyncRead + Unpin>(
        &mut self,
        input: &mut S,
    ) -> Result<Option<Vec<u8>>, VncError> {
        // Read marker
        let marker = input.read_u8().await?;
        if marker != 0xFF {
            tracing::error!("Illegal JPEG marker received (byte: : {}", marker);
            return Err(VncError::InvalidImageData);
        }

        let segment_type = input.read_u8().await?;

        // Handle markers with no length field
        if (0xD0..=0xD9).contains(&segment_type) || segment_type == 0x01 {
            return Ok(Some(vec![marker, segment_type]));
        }

        // Read length field
        let length = input.read_u16().await? as usize;
        if length < 2 {
            return Err(VncError::InvalidImageData);
        }

        // Read segment data
        let mut segment = vec![marker, segment_type];
        segment.extend_from_slice(&(length as u16).to_be_bytes());

        let mut data = vec![0u8; length - 2];
        input.read_exact(&mut data).await?;

        // Handle Start of Scan segment specially
        if segment_type == 0xDA {
            let mut extra_data = Vec::new();
            loop {
                let mut buf = [0u8; 1];
                input.read_exact(&mut buf).await?;
                extra_data.push(buf[0]);

                if extra_data.len() >= 2
                    && extra_data[extra_data.len() - 2] == 0xFF
                    && extra_data[extra_data.len() - 1] != 0x00
                    && !(0xD0..=0xD7).contains(&extra_data[extra_data.len() - 1])
                {
                    extra_data.truncate(extra_data.len() - 2);
                    break;
                }
            }
            segment.extend(data);
            segment.extend(extra_data);
        } else {
            segment.extend(data);
        }

        Ok(Some(segment))
    }
}
