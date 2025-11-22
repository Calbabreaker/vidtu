use ffmpeg_next as ffmpeg;

pub struct DecoderCommon {
    input: ffmpeg::format::context::Input,
    stream_index: usize,
    pub real_timestamp: std::time::Duration,
}

impl DecoderCommon {
    fn new(
        path: impl AsRef<std::path::Path>,
        media_type: ffmpeg::media::Type,
    ) -> anyhow::Result<Self> {
        ffmpeg::log::set_level(ffmpeg::log::Level::Quiet);
        let input = ffmpeg::format::input(&path)?;
        let stream = input
            .streams()
            .best(media_type)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        Ok(Self {
            stream_index: stream.index(),
            input,
            real_timestamp: std::time::Duration::ZERO,
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
        let timestamp_pts = (self.real_timestamp.as_secs_f64() / self.timebase()) as i64;
        while let Some((stream, packet)) = self.input.packets().next() {
            if stream.index() == self.stream_index {
                decoder.send_packet(&packet)?;
                if decoder.receive_frame(frame_out).is_ok()
                    && frame_out.pts().unwrap_or_default() > timestamp_pts
                {
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

    fn timebase(&self) -> f64 {
        let stream = self.input.stream(self.stream_index).unwrap();
        stream.time_base().into()
    }

    pub fn timestamp(&self, pts: Option<i64>) -> std::time::Duration {
        std::time::Duration::from_secs_f64(pts.unwrap_or_default() as f64 * self.timebase())
    }

    pub fn seek(&mut self, timestamp: std::time::Duration) -> anyhow::Result<()> {
        self.real_timestamp = timestamp;
        unsafe {
            // ffmpeg::format::Input::seek doesn't take a stream index so we gotta call the ffi
            // manually
            match ffmpeg::ffi::avformat_seek_file(
                self.input.as_mut_ptr(),
                self.stream_index as i32,
                i64::MIN,
                (timestamp.as_secs_f64() / self.timebase()) as i64,
                i64::MAX,
                ffmpeg::ffi::AVSEEK_FLAG_BACKWARD,
            ) {
                s if s >= 0 => Ok(()),
                e => Err(ffmpeg::Error::from(e))?,
            }
        }
    }

    pub fn frame_rate(&self) -> u32 {
        f64::from(
            self.input
                .stream(self.stream_index)
                .unwrap()
                .avg_frame_rate()
                .reduce(),
        ) as u32
    }
}

pub struct VideoDecoder {
    pub decoder: ffmpeg::decoder::Video,
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

    /// Get the next video frame while discarding all packets that are before common.real_timestamp
    pub fn next_frame(&mut self) -> anyhow::Result<ffmpeg::frame::Video> {
        let mut frame = ffmpeg::frame::Video::empty();
        self.common.next_raw_frame(&mut frame, &mut self.decoder)?;
        let scaler = self.scaler.as_mut().unwrap();
        let mut scaled_frame = ffmpeg::frame::Video::empty();
        scaler.run(&frame, &mut scaled_frame)?;
        scaled_frame.set_pts(frame.pts());
        Ok(scaled_frame)
    }
}

pub struct AudioDecoder {
    pub decoder: ffmpeg::decoder::Audio,
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
        resampled.set_pts(frame.pts());
        Ok(resampled)
    }
}
