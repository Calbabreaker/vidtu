use ffmpeg_next::{self as ffmpeg};

pub struct VideoDecoder {
    decoder: ffmpeg::decoder::Video,
    input: ffmpeg::format::context::Input,
    video_stream_index: usize,
    scaler: Option<ffmpeg::software::scaling::Context>,
}

impl VideoDecoder {
    pub fn from_file(path: impl AsRef<std::path::Path>) -> anyhow::Result<Self> {
        let input = ffmpeg::format::input(&path)?;
        let video_stream = input
            .streams()
            .best(ffmpeg_next::media::Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let video_stream_index = video_stream.index();

        let decoder = ffmpeg::codec::Context::from_parameters(video_stream.parameters())?
            .decoder()
            .video()?;

        Ok(Self {
            scaler: None,
            decoder,
            input,
            video_stream_index,
        })
    }

    pub fn set_output_size(&mut self, width: u32, height: u32) -> anyhow::Result<()> {
        self.scaler = Some(ffmpeg_next::software::scaling::Context::get(
            self.decoder.format(),
            self.decoder.width(),
            self.decoder.height(),
            self.decoder.format(),
            width,
            height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        )?);
        Ok(())
    }

    pub fn next_frame(&mut self) -> anyhow::Result<ffmpeg_next::util::frame::Video> {
        let mut frame = self.next_raw_frame()?;
        if let Some(scaler) = self.scaler.as_mut() {
            let mut scaled_frame = ffmpeg::util::frame::Video::empty();
            scaler.run(&frame, &mut scaled_frame)?;
            scaled_frame.set_pts(frame.pts());
            frame = scaled_frame
        }
        Ok(frame)
    }

    fn next_raw_frame(&mut self) -> anyhow::Result<ffmpeg::util::frame::Video> {
        let mut frame = ffmpeg::util::frame::Video::empty();
        while let Some((stream, packet)) = self.input.packets().next() {
            if stream.index() == self.video_stream_index {
                self.decoder.send_packet(&packet)?;
                if self.decoder.receive_frame(&mut frame).is_ok() {
                    return Ok(frame);
                }
            }
        }

        self.decoder.send_eof()?;
        self.decoder.receive_frame(&mut frame)?;
        Ok(frame)
    }

    pub fn frame_timestamp(&self, frame: &ffmpeg_next::util::frame::Video) -> std::time::Duration {
        let stream = self.input.stream(self.video_stream_index).unwrap();
        let time_base: f64 = stream.time_base().into();
        std::time::Duration::from_secs_f64(frame.pts().unwrap_or(0) as f64 * time_base)
    }

    pub fn total_duration(&self) -> std::time::Duration {
        std::time::Duration::from_secs_f64(
            self.input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64,
        )
    }
}
