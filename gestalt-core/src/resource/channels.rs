use crate::{message::MpscChannel, resource::retrieval::ResourceFetch};

global_channel!(MpscChannel, RESOURCE_FETCH, ResourceFetch, 65536);