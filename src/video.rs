use std::sync::Mutex;

use gstreamer::{
    prelude::{Cast, GstBinExtManual},
    traits::ElementExt,
    BufferList,
};
use gstreamer_player::prelude::VideoOverlayExtManual;
use gstreamer_video::VideoInfo;
use tokio::sync::mpsc::{self, UnboundedReceiver};

pub struct Pipeline {
    pipeline: gstreamer::Pipeline,
    source: gstreamer_app::AppSrc,
    status: gstreamer::State,
    rx: UnboundedReceiver<gstreamer::Message>,
    pub video_info: VideoInfo,
}

impl Pipeline {
    pub fn new(win_id: usize, resolution: (u32, u32), fps: u16) -> anyhow::Result<Self> {
        let pipeline = gstreamer::Pipeline::new(None);
        let appsrc = gstreamer::ElementFactory::make("appsrc", None)?;
        let videoconvert = gstreamer::ElementFactory::make("videoconvert", None)?;
        let sink = gstreamer::ElementFactory::make("xvimagesink", None)?;

        pipeline.add_many(&[&appsrc, &videoconvert, &sink])?;
        gstreamer::Element::link_many(&[&appsrc, &videoconvert, &sink])?;

        let source = appsrc
            .dynamic_cast::<gstreamer_app::AppSrc>()
            .expect("Source element is expected to be an appsrc!");

        let vidoverlay = sink
            .dynamic_cast::<gstreamer_video::VideoOverlay>()
            .expect("could not cast overlay");

        unsafe { vidoverlay.set_window_handle(win_id) };

        let video_info = gstreamer_video::VideoInfo::builder(
            gstreamer_video::VideoFormat::Rgb,
            resolution.0,
            resolution.1,
        )
        .fps(gstreamer::Fraction::new(fps.into(), 1))
        .build()
        .expect("Failed to create video info");

        source.set_caps(Some(&video_info.to_caps().unwrap()));
        source.set_format(gstreamer::Format::Time);

        let bus = pipeline
            .bus()
            .expect("Pipeline without bus. Shouldn't happen!");

        let (tx, rx) = mpsc::unbounded_channel();
        let locked_tx = Mutex::new(tx);

        bus.set_sync_handler(move |_, msg| {
            locked_tx.lock().unwrap().send(msg.clone()).unwrap();
            gstreamer::BusSyncReply::Drop
        });

        Ok(Self {
            pipeline,
            source,
            status: gstreamer::State::Ready,
            rx,
            video_info,
        })
    }

    pub fn push_frames(&mut self, buffer: BufferList) -> anyhow::Result<()> {
        self.source.push_buffer_list(buffer)?;
        if self.status != gstreamer::State::Playing {
            self.pipeline.set_state(gstreamer::State::Playing)?;
            self.status = gstreamer::State::Playing;
        }

        Ok(())
    }

    pub async fn events(&mut self) -> Option<gstreamer::Message> {
        self.rx.recv().await
    }
}
