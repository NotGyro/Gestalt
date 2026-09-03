use std::sync::Arc;

use gestalt_proc_macros::ChannelSet;
use image::DynamicImage;

use crate::{message::MpscChannel, resource::retrieval::ResourceFetch, ChannelDomain, DomainMultiChannel, MpscReceiver, StaticChannelAtom};
use super::{image::InternalImage, ResourceInfo, ResourceLocation, ResourceRetrievalError};

// ==== Retrieval: ====

#[derive(Clone, Debug)]
pub struct FetchResponse<T> {
    pub id: ResourceLocation, 
    pub resource: Result<Arc<T>, ResourceRetrievalError>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceHandlerId {
    Text,
    Image,
    Audio,
    Mesh,
    Video,
    Script,
}
impl ChannelDomain for ResourceHandlerId {}

static_channel_atom!(ImageRequests, MpscChannel<ResourceFetch<DynamicImage>>, ResourceFetch<DynamicImage>, 4096);
//static_channel_atom!(AudioRequests, MpscChannel<ResourceFetch>, ResourceFetch, 4096);
//static_channel_atom!(MeshRequests, MpscChannel<ResourceFetch>, ResourceFetch, 4096);
//static_channel_atom!(ScriptRequests, MpscChannel<ResourceFetch>, ResourceFetch, 4096);
//static_channel_atom!(TextRequests, MpscChannel<ResourceFetch>, ResourceFetch, 4096);

#[derive(ChannelSet, Clone)]
pub struct ResourceSysChannels {
	#[channel(ImageRequests)]
    pub image_requests: <ImageRequests as StaticChannelAtom>::Channel,
	//#[channel(AudioRequests)]
    //pub audio_requests: <AudioRequests as StaticChannelAtom>::Channel,
	//#[channel(MeshRequests)]
    //pub mesh_requests: <MeshRequests as StaticChannelAtom>::Channel,
	//#[channel(ScriptRequests)]
    //pub script_requests: <ScriptRequests as StaticChannelAtom>::Channel,
	//#[channel(TextRequests)]
    //pub text_requests: <TextRequests as StaticChannelAtom>::Channel,
}

#[derive(ChannelSet)]
pub struct RetrieverChannels {
	#[take_receiver(ImageRequests)]
    pub image_requests: MpscReceiver<ResourceFetch<DynamicImage>>,
	//#[take_receiver(AudioRequests)]
    //pub audio_requests: MpscReceiver<ResourceFetch>,
	//#[take_receiver(MeshRequests)]
    //pub mesh_requests: MpscReceiver<ResourceFetch>,
	//#[take_receiver(ScriptRequests)]
    //pub script_requests: MpscReceiver<ResourceFetch>,
	//#[take_receiver(TextRequests)]
    //pub text_requests: MpscReceiver<ResourceFetch>,
}
