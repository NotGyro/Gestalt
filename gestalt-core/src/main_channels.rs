use gestalt_proc_macros::ChannelSet;

use crate::net::net_channels::EngineNetChannels;

use crate::ChannelCapacityConf;

#[derive(ChannelSet)]
pub struct MainChannelSet {
	pub net_channels: EngineNetChannels,
}

impl MainChannelSet {
    pub fn new(conf: &ChannelCapacityConf) -> Self {
        Self {
            net_channels: EngineNetChannels::new(conf)
        }
    }
}