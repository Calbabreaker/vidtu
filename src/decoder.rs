use ffmpeg_next as ffmpeg;

pub struct DecoderCommon {
    input: ffmpeg::format::context::Input,
    stream_index: usize,
}

impl DecoderCommon {
    fn new(
        path: impl AsRef<std::path::Path>,
        media_type: ffmpeg::media::Type,
    ) -> anyhow::Result<Self> {
        let input = ffmpeg::format::input(&path)?;
        let stream = input
            .streams()
            .best(media_type)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        Ok(Self {
            stream_index: stream.index(),
            input,
        })
    }

    fn decoder(&self) -> anyhow::Result<ffmpeg::decoder::Decoder> {
        let stream = self.input.stream(self.stream_index).unwrap();
        Ok(ffmpeg::codec::Context::from_parameters(stream.parameters())?.decoder())
    }

    fn next_raw_frame(
        &mut self,
        frame_out: &mut ffmpeg::Frame,
        decoder: &mut ffmpeg::decoder::Opened,
    ) -> anyhow::Result<()> {
        while let Some((stream, packet)) = self.input.packets().next() {
            if stream.index() == self.stream_index {
                decoder.send_packet(&packet)?;
                if decoder.receive_frame(frame_out).is_ok() {
                    return Ok(());
                }
            }
        }

        decoder.send_eof()?;
        decoder.receive_frame(frame_out)?;
        Ok(())
    }

    pub fn total_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(
            self.input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64,
        )
    }

    pub fn timestamp(&self, pts: Option<i64>) -> std::time::Duration {
        let stream = self.input.stream(self.stream_index).unwrap();
        let time_base: f64 = stream.time_base().into();
        std::time::Duration::from_secs_f64(pts.unwrap_or_default() as f64 * time_base)
    }
}

pub struct VideoDecoder {
    decoder: ffmpeg::decoder::Video,
    scaler: Option<ffmpeg::software::scaling::Context>,
    pub common: DecoderCommon,
}

impl VideoDecoder {
    pub fn from_file(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let common = DecoderCommon::new(path, ffmpeg::media::Type::Video)?;
        Ok(Self {
            scaler: None,
            decoder: common.decoder()?.video()?,
            common,
        })
    }

    pub fn set_output_size(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.scaler = Some(ffmpeg::software::scaling::Context::get(
            self.decoder.format(),
            self.decoder.width(),
            self.decoder.height(),
            ffmpeg::format::Pixel::RGB24,
            width,
            height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        )?);
        Ok(())
    }

    pub fn next_frame(&mut self) -> anyhow::Result<ffmpeg::frame::Video> {
        let mut frame = ffmpeg::frame::Video::empty();
        self.common.next_raw_frame(&mut frame, &mut self.decoder)?;
        if let Some(scaler) = self.scaler.as_mut() {
            let mut scaled_frame = ffmpeg::frame::Video::empty();
            scaler.run(&frame, &mut scaled_frame)?;
            scaled_frame.set_pts(frame.pts());
            frame = scaled_frame
        }
        Ok(frame)
    }
}

pub struct AudioDecoder {
    decoder: ffmpeg::decoder::Audio,
    resampler: Option<ffmpeg::software::resampling::Context>,
    pub common: DecoderCommon,
}

impl AudioDecoder {
    pub fn from_file(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let common = DecoderCommon::new(path, ffmpeg::media::Type::Audio)?;
        let mut decoder = common.decoder()?.audio()?;
        if decoder.channel_layout().is_empty() {
            decoder.set_channel_layout(ffmpeg::ChannelLayout::default(decoder.channels() as i32));
        }
        Ok(Self {
            decoder,
            resampler: None,
            common,
        })
    }

    pub fn set_output_format(
        &mut self,
        format: ffmpeg::util::format::Sample,
        num_channels: u32,
        rate: u32,
    ) -> anyhow::Result<()> {
        self.resampler = Some(ffmpeg::software::resampling::Context::get(
            self.decoder.format(),
            self.decoder.channel_layout(),
            self.decoder.rate(),
            format,
            ffmpeg::ChannelLayout::default(num_channels as i32),
            rate,
        )?);
        Ok(())
    }

    pub fn next_frame(&mut self) -> anyhow::Result<ffmpeg::frame::Audio> {
        let mut frame = ffmpeg::frame::Audio::empty();
        self.common.next_raw_frame(&mut frame, &mut self.decoder)?;
        let mut resampled = ffmpeg::frame::Audio::empty();
        let resampler = self.resampler.as_mut().unwrap();
        resampler.run(&frame, &mut resampled)?;
        Ok(resampled)
    }
}
