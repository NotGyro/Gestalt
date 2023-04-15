use crate::common::{FastHashMap, new_fast_hash_map};
use crate::resource::image::{
	ImageProvider, InternalImage, ID_MISSING_TEXTURE, ID_PENDING_TEXTURE, RetrieveImageError,
};
use crate::resource::ResourceId;
use glam::Vec2;
use image::{GenericImage, ImageError, RgbaImage};
use log::error;
use std::collections::HashMap;

use crate::client::render::{generate_missing_texture_image, generate_pending_texture_image};

use super::generate_error_texture_image;

const INDEX_MISSING_TEXTURE: usize = 0;
const INDEX_PENDING_TEXTURE: usize = 1;

#[derive(thiserror::Error, Debug)]
pub enum ArrayTextureError {
	#[error("Tried to add texture {0} into a texture array, but the array is at the provided maximum number of textures which is {1}")]
	AddOverMax(String, usize),
	#[error("Tried to build an ArrayTextureLayout with {0} cells into an ArrayTexture with {1} max cells.")]
	BuildOverMax(usize, usize),
	#[error("Tried to add texture {0} into a texture array, the image size is {1:?}. The expected image size was {2:?}")]
	WrongImageSize(String, (u32, u32), (u32, u32)),
}

#[derive(Clone, Debug)]
pub struct ArrayTextureSwapRemove {
	pub removed_slot: u32,
	pub swap_source_slot: u32,
	pub removed_resource: ResourceId, 
	pub swapped_in_resource: ResourceId,
}

#[derive(Clone, Debug)]
pub enum ArrayTextureChange {
	SwapRemove(Box<ArrayTextureSwapRemove>),
	EndRemove {
		slot: u32,
		removed_resource: ResourceId,
	},
	Added {
		slot: u32,
		added_resource: ResourceId,
	}
} 

pub struct ArrayTextureLayout {
	/// Array of texture atlas tiles.
	textures: Vec<ResourceId>,
	/// Pixel width and height
	pub texture_size: (u32, u32),
	/// Resource ID to index in planned_textures up there.
	reverse_index: FastHashMap<ResourceId, usize>,
	/// Max total number of textures in this texture array.
	max_planned_textures: u32,
	/// Changes made since last rebuild 
	changes: Vec<ArrayTextureChange>,
	/// How many times has this texture array changed?
	revision: u64,
}

impl ArrayTextureLayout {
	/// Generates a layout for a texture atlas for tile_size*tile_size texture
	pub fn new(
        texture_size: (u32, u32),
		max_planned_textures: Option<u32>,
	) -> Self {
		let mut textures = Vec::default();
		let mut reverse_index = new_fast_hash_map();

		// Make sure we've got the missing texture and any other builtins.
		textures.insert(INDEX_MISSING_TEXTURE, ID_MISSING_TEXTURE);
		textures.insert(INDEX_PENDING_TEXTURE, ID_PENDING_TEXTURE);
		for (i, elem) in (&textures).into_iter().enumerate() {
			reverse_index.insert(*elem, i);
		}

		let max_planned_textures = match max_planned_textures {
			Some(val) => {
				if val < 2 {
					2
				} else {
					val
				}
			}
			None => u32::MAX,
		};

		Self {
			textures,
			texture_size,
			reverse_index,
			max_planned_textures,
			changes: Vec::new(),
			revision: 0,
		}
	}

	pub fn get_index_for_texture(&self, resource: &ResourceId) -> Option<usize> {
		self.reverse_index.get(resource).copied()
	}

	pub fn get_or_make_index_for_texture(
		&mut self,
		resource: &ResourceId,
	) -> Result<u32, ArrayTextureError> {
		match self.get_index_for_texture(resource) {
			Some(idx) => Ok(idx as u32),
			None => {
				self.revision += 1;

				let idx = self.textures.len() as u32;
				if idx >= self.max_planned_textures {
					return Err(ArrayTextureError::AddOverMax(resource_debug!(resource), self.max_planned_textures as usize));
				}
				//Insert this into the tiles.
				self.textures.push(*resource);
				//Make sure we can look it up the other way, too.
				self.reverse_index.insert(*resource, idx as usize);
                
				self.changes.push(
					ArrayTextureChange::Added { slot: idx, added_resource: resource.clone() }
				);
				Ok(idx)
			}
		}
	}
	pub fn get_revision(&self) -> u64 {
		self.revision
	}
	pub fn get_max_textures(&self) -> u32 {
		self.max_planned_textures
	}
	pub fn get_texture_count(&self) -> usize {
		self.textures.len()
	}
	pub fn get_missing_texture_idx(&self) -> u32 { 
		INDEX_MISSING_TEXTURE
	}
	pub fn get_pending_texture_idx(&self) -> u32 { 
		INDEX_PENDING_TEXTURE
	}
	pub fn unload(&mut self, resource: &ResourceId) {
		if let Some(idx) = self.reverse_index.get(resource) { 
			let idx = *idx as u32;
			let last_elem = (self.textures.len() - 1) as u32;
			if idx == last_elem {
				// Its at the end, we can just delete this 
				let resource = self.textures.remove(idx as usize);
				self.changes.push(
					ArrayTextureChange::EndRemove { 
						slot: idx,
						removed_resource: resource
					}
				);
			} else {
				// Somewhere in the middle - swap-remove needed
				let removed_resource = self.textures.swap_remove(idx as usize);
				self.changes.push(
					ArrayTextureChange::SwapRemove(Box::new(
						ArrayTextureSwapRemove {
							removed_slot: idx,
							swap_source_slot: last_elem,
							removed_resource,
							swapped_in_resource: self.textures.get(idx as usize)
								.expect("Swap_remove did not place an element at the old index!")
								.clone()
						}
					))
				);
			}
		}
	}
	pub fn finish_changes(&mut self) -> Vec<ArrayTextureChange> { 
		let mut swapper = Vec::default();
		std::mem::swap(&mut self.changes, &mut swapper); 
		swapper
	}
}

pub struct ArrayTexture { 
	layout: ArrayTextureLayout,
	last_rebuilt_revision: u64,
	/// How many cells to add each time we run out of cells and have to rebuild.
	max_cells: u32,
	current_cell_capacity: u32,
	array_texture: wgpu::Texture,
	error_image: RgbaImage, 
	missing_image: RgbaImage,
	pending_image: RgbaImage,
}

impl ArrayTexture {
	fn resize_buffer(&mut self, 
		device: &mut wgpu::Device) { 
		
		let array_size = wgpu::Extent3d { 
			width: self.layout.texture_size.0,
			height: self.layout.texture_size.1,
			depth_or_array_layers: self.current_cell_capacity
		};
		// Create the buffer on the GPU.
		self.array_texture = device.create_texture(
			&wgpu::TextureDescriptor {
				size: array_size,
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::Rgba8UnormSrgb,
				usage: wgpu::TextureUsages::TEXTURE_BINDING 
						| wgpu::TextureUsages::COPY_DST
						| wgpu::TextureUsages::COPY_SRC,
				label: Some("diffuse_texture"),
				view_formats: &[],
			}
		);
	}
	pub fn full_rebuild<TextureSource: ImageProvider>(&mut self, 
			device: &mut wgpu::Device,
			queue: &mut wgpu::Queue,
			texture_source: &mut TextureSource)
				-> Result<(), ArrayTextureError> { 
		let texture_size = self.layout.texture_size;
	
		let layout_cells = self.layout.textures.len() as u32; 
		if layout_cells > self.max_cells { 
			return Err(
				ArrayTextureError::BuildOverMax(layout_cells as usize, 
					self.max_cells as usize)
			);
		}
		
		// Expand the underlying buffer if the expected cell count changed. 
		let requested_cells = layout_cells.next_power_of_two();
		let requested_cells = requested_cells.min(self.max_cells);
		if requested_cells > self.current_cell_capacity { 
			self.current_cell_capacity = requested_cells;
			self.resize_buffer(device);
		}

		let texture_size_layer = wgpu::Extent3d {
            width: texture_size.0,
            height: texture_size.1,
            depth_or_array_layers: 1
        };

		for (texture_index, resource_texture) in self.layout.textures.iter().enumerate() {
			let mut texture_to_use = match texture_source.load_image(resource_texture) {
				crate::resource::ResourceStatus::Pending => &self.pending_image,
				crate::resource::ResourceStatus::Errored(e) => match e { 
					RetrieveImageError::DoesNotExist(_) => &self.missing_image, 
					_ => &self.error_image,
				},
				crate::resource::ResourceStatus::Ready(image) => image,
			};
			if resource_texture == &ID_PENDING_TEXTURE {
				texture_to_use = &self.pending_image;
			} else if resource_texture == &ID_MISSING_TEXTURE {
				texture_to_use = &self.pending_image;
			}

			//Is it the wrong size?
			if !((texture_to_use.width() == texture_size.0)
				&& (texture_to_use.height() == texture_size.1))
			{
				error!(
					"Tried to add texture {} into a texture array, but the image size is ({},{}). The expected image size was ({},{})",
					resource_debug!(resource_texture),
					texture_to_use.width(), texture_to_use.height(),
					texture_size.0, texture_size.1,
				);
				//Rebind
				texture_to_use = &self.error_image;
			}
			log::debug!("Attempting to write texture {} into index {} of an array texture.", 
				resource_debug!(resource_texture), texture_index); 
			queue.write_texture(
				//Dest
				wgpu::ImageCopyTexture {
					texture: &self.array_texture,
					mip_level: 0,
					origin: wgpu::Origin3d { 
						x: 0, 
						y: 0, 
						z: texture_index as u32,
					},
					aspect: wgpu::TextureAspect::All,
				},
				//Source
				texture_to_use,
				wgpu::ImageDataLayout {
					offset: 0,
					bytes_per_row: std::num::NonZeroU32::new(4 * texture_size_layer.width),
					rows_per_image: std::num::NonZeroU32::new(texture_size_layer.height),
				},
				texture_size_layer,
			);
		}
		self.last_rebuilt_revision = self.layout.revision;
		Ok(())
	}
	pub fn new(
		layout: ArrayTextureLayout,
		max_cells: Option<u32>,
		device: &mut wgpu::Device,
	) -> Result<Self, ArrayTextureError> {
		let texture_size = layout.texture_size;
		let layout_cells = layout.textures.len() as u32;
	
		let max_cells = match max_cells {
			Some(val) => {
				if val < 2 {
					2
				} else {
					val
				}
			}
			None => u32::MAX,
		};

		if layout.textures.len() > max_cells as usize { 
			return Err(
				ArrayTextureError::BuildOverMax(layout.textures.len(), 
					max_cells as usize)
			);
		}

		let current_cell_capacity = layout_cells.max(layout_cells.next_power_of_two());
		let current_cell_capacity = current_cell_capacity.min(max_cells);

		let array_size = wgpu::Extent3d { 
			width: texture_size.0,
			height: texture_size.1,
			depth_or_array_layers: current_cell_capacity
		};
		// Create the buffer on the GPU.
		let texture_buffer = device.create_texture(
			&wgpu::TextureDescriptor {
				size: array_size,
				mip_level_count: 1,
				sample_count: 1,
				dimension: wgpu::TextureDimension::D2,
				format: wgpu::TextureFormat::Rgba8UnormSrgb,
				usage: wgpu::TextureUsages::TEXTURE_BINDING 
						| wgpu::TextureUsages::COPY_DST
						| wgpu::TextureUsages::COPY_SRC,
				label: Some("diffuse_texture"),
				view_formats: &[],
			}
		);
		
		let missing_image = generate_missing_texture_image(texture_size.0, texture_size.1);
		let error_image = generate_error_texture_image(texture_size.0, texture_size.1);
		let pending_image = generate_pending_texture_image(texture_size.0, texture_size.1);

		Ok(Self {
			last_rebuilt_revision: layout.revision,
			layout,
			max_cells,
			current_cell_capacity,
			array_texture: texture_buffer,
			missing_image,
			error_image,
			pending_image,
		})
	}
}
